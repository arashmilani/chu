//! Pure functions that produce HID feature-report byte sequences for
//! the Mira protocol. Wire format follows the community `mira-js`
//! implementation (VID 0x0416 / PID 0x5020).
//!
//! Every frame starts with the USB report ID (0x00) followed by a
//! single opcode byte and zero or more value bytes.

/// USB report ID prefix on every feature write.
pub const USB_REPORT_ID: u8 = 0x00;

mod opcode {
    pub const REFRESH: u8 = 0x01;
    pub const SET_REFRESH_MODE: u8 = 0x02;
    pub const SET_SPEED: u8 = 0x04;
    pub const SET_CONTRAST: u8 = 0x05;
}

/// Refresh modes exposed by the device. Naming follows the spec's
/// two-option surface (`a2 | direct`) — internally `direct` maps to
/// the `gray_update` opcode value (0x02) used by `mira-js`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshMode {
    /// Fast binary refresh, good for typing latency.
    A2,
    /// Full grayscale refresh (vendor: `gray_update`).
    Direct,
}

impl RefreshMode {
    pub fn opcode_value(self) -> u8 {
        match self {
            RefreshMode::A2 => 0x03,
            RefreshMode::Direct => 0x02,
        }
    }
}

/// Encode `set_speed`. Speed is clamped to the spec range [1..=7] and
/// then inverted via `11 - n` to match the device's wire convention
/// (higher numeric on the wire = slower).
pub fn encode_set_speed(speed: u8) -> Vec<u8> {
    let clamped = speed.clamp(1, 7);
    let wire = 11 - clamped;
    vec![USB_REPORT_ID, opcode::SET_SPEED, wire]
}

/// Force a full refresh. One-shot, no parameters.
pub fn encode_refresh() -> Vec<u8> {
    vec![USB_REPORT_ID, opcode::REFRESH]
}

/// Encode `set_refresh_mode`.
pub fn encode_set_refresh_mode(mode: RefreshMode) -> Vec<u8> {
    vec![USB_REPORT_ID, opcode::SET_REFRESH_MODE, mode.opcode_value()]
}

/// Encode `set_contrast`. Value is clamped to the spec range [0..=15].
pub fn encode_set_contrast(contrast: u8) -> Vec<u8> {
    let clamped = contrast.clamp(0, 15);
    vec![USB_REPORT_ID, opcode::SET_CONTRAST, clamped]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_speed_in_range_inverts_value() {
        // Spec range is 1..=7; wire value is 11 - n, so 1 -> 10 and 7 -> 4.
        let cases = [
            (1u8, 10u8),
            (2, 9),
            (3, 8),
            (4, 7),
            (5, 6),
            (6, 5),
            (7, 4),
        ];
        for (input, wire) in cases {
            assert_eq!(
                encode_set_speed(input),
                vec![0x00, 0x04, wire],
                "speed={input}",
            );
        }
    }

    #[test]
    fn set_speed_clamps_below_range_to_one() {
        assert_eq!(encode_set_speed(0), vec![0x00, 0x04, 10]);
    }

    #[test]
    fn set_speed_clamps_above_range_to_seven() {
        assert_eq!(encode_set_speed(8), vec![0x00, 0x04, 4]);
        assert_eq!(encode_set_speed(255), vec![0x00, 0x04, 4]);
    }

    #[test]
    fn set_contrast_covers_full_range() {
        for n in 0..=15u8 {
            assert_eq!(encode_set_contrast(n), vec![0x00, 0x05, n], "contrast={n}");
        }
    }

    #[test]
    fn set_contrast_clamps_above_range_to_fifteen() {
        assert_eq!(encode_set_contrast(16), vec![0x00, 0x05, 15]);
        assert_eq!(encode_set_contrast(255), vec![0x00, 0x05, 15]);
    }
}
