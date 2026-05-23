//! Device discovery: filter USB HID enumerations down to the
//! Boox Mira / Mira Pro / Mira Pro Color family.
//!
//! All three variants share the same VID/PID, so discovery is a pure
//! filter — no variant-specific branching here. Variant detection
//! happens later (Settings override, or descriptor sniff).

/// Mira USB vendor ID (Onyx International).
pub const MIRA_VID: u16 = 0x0416;
/// Mira USB product ID — shared across Mira, Mira Pro, Mira Pro Color.
pub const MIRA_PID: u16 = 0x5020;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial_number: Option<String>,
    pub product_string: Option<String>,
}

/// Return only the entries that match the Mira VID/PID pair.
pub fn filter_mira_devices(devices: &[DeviceInfo]) -> Vec<DeviceInfo> {
    devices
        .iter()
        .filter(|d| d.vendor_id == MIRA_VID && d.product_id == MIRA_PID)
        .cloned()
        .collect()
}

/// Enumerate connected Mira devices via the real `hidapi` backend.
pub fn enumerate_mira(api: &hidapi::HidApi) -> Vec<DeviceInfo> {
    api.device_list()
        .filter(|d| d.vendor_id() == MIRA_VID && d.product_id() == MIRA_PID)
        .map(|d| DeviceInfo {
            vendor_id: d.vendor_id(),
            product_id: d.product_id(),
            serial_number: d.serial_number().map(str::to_string),
            product_string: d.product_string().map(str::to_string),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(vid: u16, pid: u16, serial: &str) -> DeviceInfo {
        DeviceInfo {
            vendor_id: vid,
            product_id: pid,
            serial_number: Some(serial.to_string()),
            product_string: Some("Mira".to_string()),
        }
    }

    #[test]
    fn filter_keeps_only_mira_vid_pid() {
        let listing = vec![
            dev(0x046d, 0xc52b, "logi-mouse"),
            dev(MIRA_VID, MIRA_PID, "mira-1"),
            dev(MIRA_VID, 0x9999, "wrong-pid"),
            dev(0x0416, MIRA_PID, "mira-2"),
        ];
        let found = filter_mira_devices(&listing);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].serial_number.as_deref(), Some("mira-1"));
        assert_eq!(found[1].serial_number.as_deref(), Some("mira-2"));
    }

    #[test]
    fn filter_returns_empty_when_no_mira_present() {
        let listing = vec![dev(0x046d, 0xc52b, "logi-mouse")];
        assert!(filter_mira_devices(&listing).is_empty());
    }

    #[test]
    fn filter_is_stable_when_listing_is_empty() {
        assert!(filter_mira_devices(&[]).is_empty());
    }
}
