//! Tray UI helpers.
//!
//! The actual `TrayIconBuilder` call happens in `lib.rs::run` because
//! it needs the `AppHandle`. This module owns the bits that *don't*
//! need a Tauri runtime — title formatting and the small menu model
//! — so they can be unit-tested.

/// State the tray title reflects, per spec §9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayState {
    pub connected: bool,
    pub hotkeys_ok: bool,
}

impl TrayState {
    /// Wordmark plus a single-character suffix:
    ///
    /// - Connected and hotkeys registered: `"Mira"`
    /// - Disconnected: `"Mira —"`
    /// - Hotkey registration failure: `"Mira !"`
    ///
    /// If both are bad, hotkey-failure wins because users can fix
    /// hotkeys from Settings while the device situation often resolves
    /// itself on cable reseat.
    pub fn title(&self) -> &'static str {
        match (self.connected, self.hotkeys_ok) {
            (_, false) => "Mira !",
            (false, true) => "Mira —",
            (true, true) => "Mira",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_shows_bare_wordmark() {
        let s = TrayState {
            connected: true,
            hotkeys_ok: true,
        };
        assert_eq!(s.title(), "Mira");
    }

    #[test]
    fn disconnected_appends_em_dash() {
        let s = TrayState {
            connected: false,
            hotkeys_ok: true,
        };
        assert_eq!(s.title(), "Mira —");
    }

    #[test]
    fn hotkey_failure_appends_bang() {
        let s = TrayState {
            connected: true,
            hotkeys_ok: false,
        };
        assert_eq!(s.title(), "Mira !");
    }

    #[test]
    fn hotkey_failure_takes_priority_over_disconnect() {
        let s = TrayState {
            connected: false,
            hotkeys_ok: false,
        };
        assert_eq!(s.title(), "Mira !");
    }
}
