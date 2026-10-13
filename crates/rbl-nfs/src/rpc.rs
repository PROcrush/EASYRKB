//! SUN-RPC version 2 call and reply framing (RFC 5531).
//!
//! Datagram framing only. Over TCP, RPC prefixes each message with a record
//! mark; players talk to rekordbox over UDP, so that is all this handles, and
//! a TCP path would need the record header added rather than assumed absent.

use crate::xdr::{Reader, Writer, XdrError};

pub const RPC_VERSION: u32 = 2;

pub const MSG_CALL: u32 = 0;
pub const MSG_REPLY: u32 = 1;

pub const MSG_ACCEPTED: u32 = 0;
pub const MSG_DENIED: u32 = 1;

/// Accept status.
pub mod accept {
    pub const SUCCESS: u32 = 0;
    pub const PROG_UNAVAIL: u32 = 1;
    pub const PROG_MISMATCH: u32 = 2;
    pub const PROC_UNAVAIL: u32 = 3;
    pub const GARBAGE_ARGS: u32 = 4;
    pub const SYSTEM_ERR: u32 = 5;
}

/// Reject status.
pub mod reject {
    pub const RPC_MISMATCH: u32 = 0;
    pub const AUTH_ERROR: u32 = 1;
}

pub const AUTH_NULL: u32 = 0;
pub const AUTH_UNIX: u32 = 1;

/// The stamp a CDJ puts in its `AUTH_UNIX` credential. We do not require it —
/// authentication is meaningless on a link network — but a reply that echoes
/// what the player sent stays closest to what it expects.
pub const CDJ_AUTH_STAMP: u32 = 0x967b_8703;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RpcError {
    #[error(transparent)]
    Xdr(#[from] XdrError),
    #[error("expected an RPC call, got message type {0}")]
    NotACall(u32),
    #[error("unsupported RPC version {0}")]
    Version(u32),
}

/// A credential or verifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Auth {
    pub flavor: u32,
    pub body: Vec<u8>,
}

impl Auth {
    /// The empty credential a server sends back as its verifier.
    pub fn null() -> Self {
        Self { flavor: AUTH_NULL, body: Vec::new() }
    }

    fn read(reader: &mut Reader<'_>) -> Result<Self, XdrError> {
        let flavor = reader.u32()?;
        let body = reader.opaque()?.to_vec();
        Ok(Self { flavor, body })
    }

    fn write(&self, writer: &mut Writer) {
        writer.u32(self.flavor).opaque(&self.body);
    }
}

/// A decoded RPC call, with the procedure arguments left unread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call<'a> {
    pub xid: u32,
    pub program: u32,
    pub version: u32,
    pub procedure: u32,
    pub credential: Auth,
    pub verifier: Auth,
    /// Everything after the verifier: the procedure's own arguments.
    pub arguments: &'a [u8],
}

impl<'a> Call<'a> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, RpcError> {
        let mut reader = Reader::new(bytes);
        let xid = reader.u32()?;
        let message_type = reader.u32()?;
        if message_type != MSG_CALL {
            return Err(RpcError::NotACall(message_type));
        }
        let rpc_version = reader.u32()?;
        if rpc_version != RPC_VERSION {
            return Err(RpcError::Version(rpc_version));
        }
        let program = reader.u32()?;
        let version = reader.u32()?;
        let procedure = reader.u32()?;
        let credential = Auth::read(&mut reader)?;
        let verifier = Auth::read(&mut reader)?;
        let arguments = bytes.get(reader.position()..).unwrap_or(&[]);
        Ok(Self { xid, program, version, procedure, credential, verifier, arguments })
    }

    /// A reader positioned at the procedure's arguments.
    pub fn reader(&self) -> Reader<'a> {
        Reader::new(self.arguments)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut writer = Writer::with_capacity(64 + self.arguments.len());
        writer.u32(self.xid).u32(MSG_CALL).u32(RPC_VERSION);
        writer.u32(self.program).u32(self.version).u32(self.procedure);
        self.credential.write(&mut writer);
        self.verifier.write(&mut writer);
        let mut bytes = writer.into_bytes();
        bytes.extend_from_slice(self.arguments);
        bytes
    }
}

/// Starts an accepted reply, leaving the writer positioned for the results.
pub fn accepted(xid: u32, status: u32) -> Writer {
    let mut writer = Writer::with_capacity(64);
    writer.u32(xid).u32(MSG_REPLY).u32(MSG_ACCEPTED);
    Auth::null().write(&mut writer);
    writer.u32(status);
    writer
}

/// A complete accepted reply carrying no results.
pub fn accepted_empty(xid: u32, status: u32) -> Vec<u8> {
    accepted(xid, status).into_bytes()
}

/// A program-mismatch reply, which must name the versions we do support.
pub fn program_mismatch(xid: u32, low: u32, high: u32) -> Vec<u8> {
    let mut writer = accepted(xid, accept::PROG_MISMATCH);
    writer.u32(low).u32(high);
    writer.into_bytes()
}

/// An RPC-version-mismatch rejection.
pub fn rpc_mismatch(xid: u32, low: u32, high: u32) -> Vec<u8> {
    let mut writer = Writer::with_capacity(32);
    writer.u32(xid).u32(MSG_REPLY).u32(MSG_DENIED).u32(reject::RPC_MISMATCH);
    writer.u32(low).u32(high);
    writer.into_bytes()
}

/// A decoded reply, for the fake-CDJ client and for the replay tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply<'a> {
    pub xid: u32,
    pub reply_status: u32,
    pub accept_status: u32,
    /// Everything after the accept status: the procedure's results.
    pub results: &'a [u8],
}

impl<'a> Reply<'a> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, RpcError> {
        let mut reader = Reader::new(bytes);
        let xid = reader.u32()?;
        let message_type = reader.u32()?;
        if message_type != MSG_REPLY {
            return Err(RpcError::NotACall(message_type));
        }
        let reply_status = reader.u32()?;
        if reply_status != MSG_ACCEPTED {
            let accept_status = reader.u32()?;
            return Ok(Self {
                xid,
                reply_status,
                accept_status,
                results: bytes.get(reader.position()..).unwrap_or(&[]),
            });
        }
        // The server's verifier, which we do not check.
        Auth::read(&mut reader)?;
        let accept_status = reader.u32()?;
        Ok(Self {
            xid,
            reply_status,
            accept_status,
            results: bytes.get(reader.position()..).unwrap_or(&[]),
        })
    }

    pub fn is_success(&self) -> bool {
        self.reply_status == MSG_ACCEPTED && self.accept_status == accept::SUCCESS
    }

    pub fn reader(&self) -> Reader<'a> {
        Reader::new(self.results)
    }
}
