//! Selected-interface mode, rather than a guess from an interface or peer name.
//!
//! [OBS] macOS rekordbox `PSvLinkSysMgrNetworkAccess::getLinkIF`
//! (`rb_named.c`, 6145–6258) checks the current `SystemConfiguration` set's
//! IEEE80211 services for the selected MAC. A match is wireless; otherwise a
//! successful enumeration is wired. [UNKNOWN] Other platforms' vendor detector.
//! Missing or invalid data before a match yields unknown; a known match returns
//! immediately, without inspecting later services, as in the vendor valid path.

use rbl_prolink::ConnectionMode;

pub(crate) fn selected(mac: [u8; 6]) -> ConnectionMode {
    #[cfg(target_os = "macos")]
    {
        macos::selected(mac).unwrap_or(ConnectionMode::Unknown)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = mac;
        ConnectionMode::Unknown
    }
}

#[cfg(any(target_os = "macos", test))]
fn classify(
    selected: [u8; 6],
    services: impl IntoIterator<Item = Result<Option<[u8; 6]>, ()>>,
) -> ConnectionMode {
    for service in services {
        match service {
            Ok(Some(mac)) if mac == selected => return ConnectionMode::Wireless,
            Ok(_) => (),
            Err(()) => return ConnectionMode::Unknown,
        }
    }
    ConnectionMode::Wired
}

#[cfg(any(target_os = "macos", test))]
fn parse_mac(address: &str) -> Option<[u8; 6]> {
    let bytes = address.as_bytes();
    if bytes.len() != 17 {
        return None;
    }
    let mut mac = [0; 6];
    for (index, octet) in mac.iter_mut().enumerate() {
        let offset = index * 3;
        if index < 5 && bytes[offset + 2] != b':' {
            return None;
        }
        *octet = (hex_digit(bytes[offset])? << 4) | hex_digit(bytes[offset + 1])?;
    }
    Some(mac)
}

#[cfg(any(target_os = "macos", test))]
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod macos {
    use std::{ffi::CStr, ptr};

    use core_foundation::{
        array::CFArray,
        base::{CFEqual, CFType, TCFType},
        string::{kCFStringEncodingASCII, CFString, CFStringCreateWithCString, CFStringGetCString},
    };
    use system_configuration_sys::{
        network_configuration::{
            kSCNetworkInterfaceTypeIEEE80211, SCNetworkInterfaceGetHardwareAddressString,
            SCNetworkInterfaceGetInterfaceType, SCNetworkServiceGetInterface,
            SCNetworkSetCopyCurrent, SCNetworkSetCopyServices,
        },
        preferences::SCPreferencesCreate,
    };

    use super::{classify, parse_mac, ConnectionMode};

    pub(super) fn selected(mac: [u8; 6]) -> Result<ConnectionMode, ()> {
        // SAFETY: Every nullable Create/Copy/Get result is checked before use.
        // Create/Copy objects use CF RAII; borrowed services/interfaces/strings
        // are used only while their owning preferences/set/array remain alive.
        // These APIs only read configuration; no preferences are committed.
        unsafe {
            let name = CFStringCreateWithCString(
                ptr::null(),
                c"rbxport LINK interface mode".as_ptr(),
                kCFStringEncodingASCII,
            );
            if name.is_null() {
                return Err(());
            }
            let name = CFString::wrap_under_create_rule(name);
            let preferences =
                SCPreferencesCreate(ptr::null(), name.as_concrete_TypeRef(), ptr::null());
            if preferences.is_null() {
                return Err(());
            }
            let preferences = CFType::wrap_under_create_rule(preferences);
            let set = SCNetworkSetCopyCurrent(preferences.as_CFTypeRef());
            if set.is_null() {
                return Err(());
            }
            let set = CFType::wrap_under_create_rule(set);
            let services = SCNetworkSetCopyServices(set.as_CFTypeRef());
            if services.is_null() {
                return Err(());
            }
            let services: CFArray = CFArray::wrap_under_create_rule(services);
            let wireless_type = kSCNetworkInterfaceTypeIEEE80211;
            if wireless_type.is_null() {
                return Err(());
            }
            // Read lazily so a proved match returns before later service data.
            Ok(classify(
                mac,
                services.iter().map(|service| {
                    let service = *service;
                    if service.is_null() {
                        return Err(());
                    }
                    let interface = SCNetworkServiceGetInterface(service);
                    if interface.is_null() {
                        return Err(());
                    }
                    let interface_type = SCNetworkInterfaceGetInterfaceType(interface);
                    if interface_type.is_null() {
                        return Err(());
                    }
                    if CFEqual(interface_type.cast(), wireless_type.cast()) == 0 {
                        return Ok(None);
                    }
                    let address = SCNetworkInterfaceGetHardwareAddressString(interface);
                    if address.is_null() {
                        return Err(());
                    }
                    // The vendor reads the 17-character colon-separated MAC into
                    // an 18-byte ASCII buffer. Conversion failure is unknown, not
                    // an invented all-zero address that might match loopback.
                    let mut buffer = [0; 18];
                    if CFStringGetCString(address, buffer.as_mut_ptr(), 18, kCFStringEncodingASCII)
                        == 0
                    {
                        return Err(());
                    }
                    let address = CStr::from_ptr(buffer.as_ptr()).to_str().map_err(|_| ())?;
                    Ok(Some(parse_mac(address).ok_or(())?))
                }),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, parse_mac, ConnectionMode};

    const SELECTED: [u8; 6] = [0x10, 0xab, 0x02, 0xcd, 0x34, 0xef];

    // A pure SystemConfiguration service fixture: a non-IEEE80211 service's
    // hardware address is irrelevant, even when it matches the selected MAC.
    fn fixture_mode(services: Result<&[(bool, Option<&str>)], ()>) -> ConnectionMode {
        match services {
            Ok(services) => classify(
                SELECTED,
                services.iter().map(|(wireless, address)| {
                    if *wireless {
                        address.and_then(parse_mac).map(Some).ok_or(())
                    } else {
                        Ok(None)
                    }
                }),
            ),
            Err(()) => ConnectionMode::Unknown,
        }
    }

    #[test]
    fn matching_ieee80211_service_is_wireless() {
        assert_eq!(
            fixture_mode(Ok(&[
                (false, Some("10:ab:02:cd:34:ef")),
                (true, Some("10:AB:02:cD:34:ef")),
            ])),
            ConnectionMode::Wireless
        );
    }

    #[test]
    fn successful_nonmatching_enumeration_is_wired() {
        for services in [
            &[][..],
            &[(false, Some("10:ab:02:cd:34:ef"))][..],
            &[(false, None), (true, Some("00:11:22:33:44:55"))][..],
        ] {
            assert_eq!(fixture_mode(Ok(services)), ConnectionMode::Wired);
        }
    }

    #[test]
    fn unavailable_or_invalid_wireless_address_is_unknown() {
        assert_eq!(fixture_mode(Err(())), ConnectionMode::Unknown);
        assert_eq!(fixture_mode(Ok(&[(true, None)])), ConnectionMode::Unknown);
        assert_eq!(
            fixture_mode(Ok(&[(true, Some("invalid"))])),
            ConnectionMode::Unknown
        );
        assert_eq!(classify([0; 6], [Err(())]), ConnectionMode::Unknown);
    }

    #[test]
    fn matching_service_returns_before_later_invalid_data() {
        assert_eq!(
            fixture_mode(Ok(&[
                (true, Some("10:ab:02:cd:34:ef")),
                (true, Some("invalid")),
                (true, None),
            ])),
            ConnectionMode::Wireless
        );
        assert_eq!(
            fixture_mode(Ok(&[
                (true, Some("invalid")),
                (true, Some("10:ab:02:cd:34:ef")),
            ])),
            ConnectionMode::Unknown
        );
    }

    #[test]
    fn mac_parser_requires_six_hex_octets_and_colons() {
        assert_eq!(parse_mac("10:AB:02:cD:34:ef"), Some(SELECTED));
        assert_eq!(parse_mac("00:00:00:00:00:00"), Some([0; 6]));
        for invalid in [
            "",
            "10:ab:02:cd:34",
            "10:ab:02:cd:34:ef:00",
            "10-ab-02-cd-34-ef",
            "10:ab:02:cd:34:eg",
            "1:ab:02:cd:34:ef",
            " 10:ab:02:cd:34:ef",
            "10:ab:02:cd:34:ef ",
            "é0:ab:02:cd:34:ef",
        ] {
            assert_eq!(parse_mac(invalid), None, "{invalid}");
        }
    }
}
