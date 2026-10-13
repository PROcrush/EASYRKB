//! OS-provided adapter labels for the LINK interface picker.

#[derive(Clone, Default)]
pub struct Label {
    pub adapter: Option<String>,
    pub connection: Option<String>,
}

#[cfg(target_os = "macos")]
pub fn for_interface(name: &str) -> Label {
    use std::{collections::HashMap, process::Command, sync::OnceLock, time::{Duration, Instant}};
    type Cache = Option<(Instant, HashMap<String, Label>)>;
    static CACHE: OnceLock<parking_lot::Mutex<Cache>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| parking_lot::Mutex::new(None)).lock();
    if cache.as_ref().is_none_or(|(at, _)| at.elapsed() >= Duration::from_secs(30)) {
        let labels = Command::new("/usr/sbin/networksetup")
            .arg("-listallhardwareports")
            .env("LC_ALL", "C")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| parse_hardware_ports(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default();
        *cache = Some((Instant::now(), labels));
    }
    cache.as_ref().and_then(|(_, labels)| labels.get(name)).cloned().unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
pub fn for_interface(_name: &str) -> Label {
    // Do not infer Wi-Fi from an interface name: users can rename adapters.
    Label::default()
}

#[cfg(any(target_os = "macos", test))]
fn parse_hardware_ports(output: &str) -> std::collections::HashMap<String, Label> {
    let mut labels = std::collections::HashMap::new();
    let mut adapter = None;
    for line in output.lines() {
        if let Some(port) = line.strip_prefix("Hardware Port: ") {
            adapter = Some(port.trim().to_owned());
        } else if let Some(device) = line.strip_prefix("Device: ") {
            if let Some(port) = adapter.take() {
                let kind = if port == "Wi-Fi" || port == "AirPort" {
                    Some("wireless")
                } else if port.contains("Ethernet") || port.contains("LAN") || port.starts_with("Thunderbolt") {
                    Some("wired")
                } else {
                    None
                };
                labels.insert(device.trim().to_owned(), Label {
                    adapter: Some(port),
                    connection: kind.map(str::to_owned),
                });
            }
        }
    }
    labels
}

#[cfg(test)]
mod tests {
    #[test]
    fn labels_hardware_without_guessing_from_device_names() {
        let labels = super::parse_hardware_ports("Hardware Port: Wi-Fi\nDevice: en7\n\nHardware Port: USB 10/100/1000 LAN\nDevice: en0\n\nHardware Port: Custom\nDevice: en9\n");
        assert_eq!(labels["en7"].connection.as_deref(), Some("wireless"));
        assert_eq!(labels["en0"].connection.as_deref(), Some("wired"));
        assert_eq!(labels["en0"].adapter.as_deref(), Some("USB 10/100/1000 LAN"));
        assert_eq!(labels["en9"].connection, None);
        assert!(!labels.contains_key("en1"));
    }
}
