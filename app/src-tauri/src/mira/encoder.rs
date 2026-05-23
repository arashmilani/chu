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
    pub const SET_DITHER_MODE: u8 = 0x09;
    pub const SET_COLOR_FILTER: u8 = 0x11;
    pub const SET_COLD_LIGHT: u8 = 0x06;
    pub const SET_WARM_LIGHT: u8 = 0x07;
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

/// Encode `set_dither_mode`. Value is clamped to [0..=3].
///
/// 0 = off, 1 = Bayer, 2 = Floyd-Steinberg, 3 = custom (poorly
/// documented; surface but expect surprises).
pub fn encode_set_dither_mode(mode: u8) -> Vec<u8> {
    let clamped = mode.clamp(0, 3);
    vec![USB_REPORT_ID, opcode::SET_DITHER_MODE, clamped]
}

/// Encode the combined `set_color_filter` opcode.
///
/// The device exposes both whiten-background and deepen-blacks behind
/// a single opcode that takes both values together. We always send
/// both, even if only one changed — callers track the partner value.
///
/// Spec ranges are 0..=127 for each filter; the white value is then
/// inverted on the wire to `255 - white`, matching `mira-js`. A
/// `white_filter` of 0 (no whitening) yields wire byte `0xFF`.
pub fn encode_set_color_filter(white_filter: u8, black_filter: u8) -> Vec<u8> {
    let white = white_filter.clamp(0, 127);
    let black = black_filter.clamp(0, 127);
    vec![
        USB_REPORT_ID,
        opcode::SET_COLOR_FILTER,
        255 - white,
        black,
    ]
}

/// Encode `set_cold_light`. Value is clamped to [0..=254].
pub fn encode_set_cold_light(brightness: u8) -> Vec<u8> {
    let clamped = brightness.clamp(0, 254);
    vec![USB_REPORT_ID, opcode::SET_COLD_LIGHT, clamped]
}

/// Encode `set_warm_light`. Value is clamped to [0..=254].
pub fn encode_set_warm_light(brightness: u8) -> Vec<u8> {
    let clamped = brightness.clamp(0, 254);
    vec![USB_REPORT_ID, opcode::SET_WARM_LIGHT, clamped]
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

    #[test]
    fn set_dither_mode_covers_full_range() {
        for n in 0..=3u8 {
            assert_eq!(
                encode_set_dither_mode(n),
                vec![0x00, 0x09, n],
                "dither={n}",
            );
        }
    }

    #[test]
    fn set_dither_mode_clamps_above_range_to_three() {
        assert_eq!(encode_set_dither_mode(4), vec![0x00, 0x09, 3]);
        assert_eq!(encode_set_dither_mode(255), vec![0x00, 0x09, 3]);
    }

    #[test]
    fn set_refresh_mode_a2_maps_to_three() {
        assert_eq!(
            encode_set_refresh_mode(RefreshMode::A2),
            vec![0x00, 0x02, 0x03]
        );
    }

    #[test]
    fn set_refresh_mode_direct_maps_to_gray_update_two() {
        // Spec's "direct = full grayscale" maps to mira-js's gray_update (0x02).
        assert_eq!(
            encode_set_refresh_mode(RefreshMode::Direct),
            vec![0x00, 0x02, 0x02]
        );
    }

    #[test]
    fn set_color_filter_inverts_white_and_passes_black() {
        // white=0 (no whitening) => wire byte 0xFF; black=0 => 0x00.
        assert_eq!(
            encode_set_color_filter(0, 0),
            vec![0x00, 0x11, 0xFF, 0x00]
        );
        // white=16 => 255-16=239; black=8 => 8 (Coding preset values).
        assert_eq!(
            encode_set_color_filter(16, 8),
            vec![0x00, 0x11, 239, 8]
        );
        // Spec max 127 for each.
        assert_eq!(
            encode_set_color_filter(127, 127),
            vec![0x00, 0x11, 128, 127]
        );
    }

    #[test]
    fn set_color_filter_clamps_above_spec_range() {
        // Inputs above 127 saturate.
        assert_eq!(
            encode_set_color_filter(200, 200),
            vec![0x00, 0x11, 128, 127]
        );
    }

    #[test]
    fn set_cold_light_covers_endpoints() {
        assert_eq!(encode_set_cold_light(0), vec![0x00, 0x06, 0]);
        assert_eq!(encode_set_cold_light(127), vec![0x00, 0x06, 127]);
        assert_eq!(encode_set_cold_light(254), vec![0x00, 0x06, 254]);
    }

    #[test]
    fn set_cold_light_clamps_above_range() {
        assert_eq!(encode_set_cold_light(255), vec![0x00, 0x06, 254]);
    }

    #[test]
    fn set_warm_light_covers_endpoints() {
        assert_eq!(encode_set_warm_light(0), vec![0x00, 0x07, 0]);
        assert_eq!(encode_set_warm_light(127), vec![0x00, 0x07, 127]);
        assert_eq!(encode_set_warm_light(254), vec![0x00, 0x07, 254]);
    }

    #[test]
    fn set_warm_light_clamps_above_range() {
        assert_eq!(encode_set_warm_light(255), vec![0x00, 0x07, 254]);
    }

    #[test]
    fn refresh_is_a_two_byte_oneshot() {
        assert_eq!(encode_refresh(), vec![0x00, 0x01]);
    }
}
