//! Tray UI helpers.
//!
//! The actual `TrayIconBuilder` call happens in `lib.rs::run` because
//! it needs the `AppHandle`. This module owns the bits that *don't*
//! need a Tauri runtime — title formatting and the small menu model
//! — so they can be unit-tested.

/// State the tray title reflects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayState {
    pub connected: bool,
    pub hotkeys_ok: bool,
}

impl TrayState {
    /// Wordmark plus a single glyph suffix:
    ///
    /// - Connected and hotkeys registered: `"Chu ◉"` (filled fisheye)
    /// - Anything else: `"Chu ◎"` (bullseye with a hole)
    ///
    /// Two states only — the spec originally called for three (em dash
    /// for disconnect, bang for hotkey failure) but the bullseye glyph
    /// already reads as "needs attention" without needing the user to
    /// remember which suffix means which.
    pub fn title(&self) -> &'static str {
        if self.connected && self.hotkeys_ok {
            "Chu ◉"
        } else {
            "Chu ◎"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_state_shows_filled_fisheye() {
        let s = TrayState {
            connected: true,
            hotkeys_ok: true,
        };
        assert_eq!(s.title(), "Chu ◉");
    }

    #[test]
    fn disconnected_shows_bullseye() {
        let s = TrayState {
            connected: false,
            hotkeys_ok: true,
        };
        assert_eq!(s.title(), "Chu ◎");
    }

    #[test]
    fn hotkey_failure_shows_bullseye() {
        let s = TrayState {
            connected: true,
            hotkeys_ok: false,
        };
        assert_eq!(s.title(), "Chu ◎");
    }

    #[test]
    fn both_bad_still_shows_bullseye() {
        let s = TrayState {
            connected: false,
            hotkeys_ok: false,
        };
        assert_eq!(s.title(), "Chu ◎");
    }
}
