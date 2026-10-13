//! Binding the sockets the three RPC programs answer on.
//!
//! One blocking thread per socket, using `std::net`. There is no async runtime
//! here on purpose: a datagram in, a reply out, with no I/O in between except
//! one bounded file read, so a thread costs a stack and nothing else, while a
//! runtime would cost a dependency and a scheduler on the read path.

use std::io;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::Server;

/// The largest datagram we will read. A request is small — a `READ` asks for
/// 32 KB but carries none — so this is headroom for a long `LOOKUP` name and
/// a generous `READDIR` budget, not for data.
const DATAGRAM: usize = 65_535;

/// [STATIC] libFilSiNE `_tkfTransportOpen` requests 0x10000 bytes for both
/// `SO_SNDBUF` and `SO_RCVBUF`. The OS may clamp or account for these differently.
const SOCKET_BUFFER: usize = 0x10000;

/// How long a socket blocks before checking whether it has been asked to stop.
const POLL: Duration = Duration::from_millis(100);

/// Serves RPC on one bound socket until `stop` is set.
///
/// Returns only on an error that is not worth continuing through; a malformed
/// datagram, or one from a peer that has gone away, is counted and skipped.
pub fn serve(server: &Arc<Server>, socket: &UdpSocket, stop: &Arc<AtomicBool>) -> io::Result<()> {
    let socket_id = crate::Receiver::default();
    socket.set_read_timeout(Some(POLL))?;
    // Match `_tkfTransportOpen`: broadcast, then receive and send buffers.
    socket.set_broadcast(true)?;
    let socket_ref = socket2::SockRef::from(socket);
    socket_ref.set_recv_buffer_size(SOCKET_BUFFER)?;
    socket_ref.set_send_buffer_size(SOCKET_BUFFER)?;
    let mut buffer = vec![0_u8; DATAGRAM];
    while !stop.load(Ordering::Relaxed) {
        let (len, from) = match socket.recv_from(&mut buffer) {
            Ok(received) => received,
            Err(error) if is_timeout(&error) => continue,
            // A datagram whose peer has vanished surfaces here on some
            // platforms; it says nothing about the socket's health.
            Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {
                tracing::trace!(%error, "a peer went away; ignored");
                continue;
            }
            Err(error) => return Err(error),
        };
        tracing::trace!(%from, len, "RPC datagram received");
        let peer = match from {
            SocketAddr::V4(v4) => *v4.ip(),
            SocketAddr::V6(v6) => v6.ip().to_ipv4_mapped().unwrap_or(std::net::Ipv4Addr::UNSPECIFIED),
        };
        let Some(reply) = server.handle_on(buffer.get(..len).unwrap_or(&[]), peer, from.port(), socket_id) else {
            continue;
        };
        let started = std::time::Instant::now();
        match socket.send_to(&reply, from) {
            Ok(sent) => tracing::trace!(
                %from,
                sent,
                elapsed_us = started.elapsed().as_micros(),
                "RPC reply sent"
            ),
            Err(error) => {
                tracing::warn!(%from, %error, len = reply.len(), "could not send an RPC reply");
            }
        }
    }
    Ok(())
}

fn is_timeout(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut)
}

/// The three sockets a player needs, bound and serving.
///
/// Ports are taken as arguments rather than hard-coded so a test can bind
/// ephemeral loopback ports: the real ones are held by rekordbox whenever it
/// is running, and a test that fought it for them would be a test that only
/// passes when the app under test is closed.
#[derive(Debug)]
pub struct Bound {
    stop: Arc<AtomicBool>,
    threads: Vec<std::thread::JoinHandle<()>>,
    portmap: SocketAddr,
    mount: SocketAddr,
    nfs: SocketAddr,
    server: Arc<Server>,
}

impl Bound {
    /// Advertises portmap, mount and NFS on `address`, and starts serving.
    /// [STATIC] libFilSiNE binds its sockets to `INADDR_ANY`.
    /// [LOCAL] On macOS, pin those sockets to the advertised interface so
    /// the default Wi-Fi route cannot override the user's adapter selection.
    /// Other platforms bind the advertised address.
    ///
    /// A port of 0 asks the OS for a free one, which is what the tests use.
    pub fn start(
        exports: crate::Exports,
        address: std::net::IpAddr,
        portmap_port: u16,
        mount_port: u16,
        nfs_port: u16,
        export_host: Option<String>,
        up: Option<Arc<std::sync::atomic::AtomicU8>>,
    ) -> io::Result<Self> {
        let portmap_socket = bind_rpc(address, portmap_port)?;
        let mount_socket = bind_rpc(address, mount_port)?;
        let nfs_socket = bind_rpc(address, nfs_port)?;

        let (portmap, mount, nfs) = (
            SocketAddr::new(address, portmap_socket.local_addr()?.port()),
            SocketAddr::new(address, mount_socket.local_addr()?.port()),
            SocketAddr::new(address, nfs_socket.local_addr()?.port()),
        );
        tracing::debug!(%portmap, %mount, %nfs, "file server bound");

        // Portmap must report the ports actually bound, which with ephemeral
        // ports are not known until now.
        let mut server = Server::new(exports, nfs.port(), mount.port());
        if let Some(host) = export_host {
            server = server.with_export_host(host);
        }
        if let Some(up) = up {
            server = server.with_gate(up);
        }
        let server = Arc::new(server);
        let stop = Arc::new(AtomicBool::new(false));

        let mut threads = Vec::with_capacity(3);
        for (name, socket) in [
            ("portmap", portmap_socket),
            ("mount", mount_socket),
            ("nfs", nfs_socket),
        ] {
            let server = Arc::clone(&server);
            let stop = Arc::clone(&stop);
            threads.push(std::thread::spawn(move || {
                if let Err(error) = serve(&server, &socket, &stop) {
                    tracing::error!(program = name, %error, "RPC socket stopped; players cannot read files");
                }
            }));
        }

        Ok(Self { stop, threads, portmap, mount, nfs, server })
    }

    /// Whether `host` has an export mounted.
    pub fn is_mounted(&self, host: std::net::Ipv4Addr) -> bool {
        self.server.is_mounted(host)
    }

    /// The hosts with an export mounted.
    pub fn mounted_hosts(&self) -> Vec<std::net::Ipv4Addr> {
        self.server.mounted_hosts()
    }

    pub const fn portmap_address(&self) -> SocketAddr {
        self.portmap
    }

    pub const fn mount_address(&self) -> SocketAddr {
        self.mount
    }

    pub const fn nfs_address(&self) -> SocketAddr {
        self.nfs
    }

    /// Stops the threads and waits for them.
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            // A thread that panicked has already logged; there is nothing to
            // recover here, and the caller is shutting down either way.
            drop(thread.join());
        }
        tracing::debug!("file server stopped");
    }
}

impl Drop for Bound {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Keep RPC replies on the adapter whose address LINK advertises.
fn bind_rpc(address: IpAddr, port: u16) -> io::Result<UdpSocket> {
    #[cfg(target_vendor = "apple")]
    {
        let bind_address = match address {
            IpAddr::V4(_) => IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(_) => IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED),
        };
        let socket = UdpSocket::bind(SocketAddr::new(bind_address, port))?;
        if !address.is_unspecified() {
            let index = if_addrs::get_if_addrs()?
                .into_iter()
                .find(|interface| interface.ip() == address)
                .and_then(|interface| interface.index)
                .and_then(std::num::NonZeroU32::new)
                .ok_or_else(|| io::Error::new(io::ErrorKind::AddrNotAvailable,
                    format!("no network interface has address {address}")))?;
            let socket_ref = socket2::SockRef::from(&socket);
            match address {
                IpAddr::V4(_) => socket_ref.bind_device_by_index_v4(Some(index))?,
                IpAddr::V6(_) => socket_ref.bind_device_by_index_v6(Some(index))?,
            }
        }
        Ok(socket)
    }
    #[cfg(not(target_vendor = "apple"))]
    UdpSocket::bind(SocketAddr::new(address, port))
}
