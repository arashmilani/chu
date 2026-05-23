//! Hotkey binding model — pure parsing/validation, no OS integration.
//!
//! The actual `register()` / `unregister()` calls live in the Tauri
//! plugin layer; this module is purely about the *shape* of a binding
//! so the recorder UI and the conflict checker can be tested without
//! the OS.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    /// Cmd on macOS, Win/Meta elsewhere — Tauri normalises this.
    Cmd,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Binding {
    pub modifiers: BTreeSet<Modifier>,
    pub key: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BindingError {
    #[error("binding must have at least one modifier")]
    NoModifier,
    #[error("binding must have a non-modifier key")]
    NoKey,
    #[error("unknown component: {0}")]
    UnknownToken(String),
}

impl Binding {
    /// Parse strings like `"Ctrl+Alt+1"` or `"Ctrl+Shift+Alt+R"`.
    /// Token order is irrelevant; the parsed binding always
    /// re-serializes in canonical order (Ctrl, Alt, Shift, Cmd, key).
    pub fn parse(input: &str) -> Result<Self, BindingError> {
        let mut modifiers = BTreeSet::new();
        let mut key: Option<String> = None;

        for raw in input.split('+') {
            let token = raw.trim();
            if token.is_empty() {
                continue;
            }
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => {
                    modifiers.insert(Modifier::Ctrl);
                }
                "alt" | "option" | "opt" => {
                    modifiers.insert(Modifier::Alt);
                }
                "shift" => {
                    modifiers.insert(Modifier::Shift);
                }
                "cmd" | "command" | "meta" | "super" | "win" => {
                    modifiers.insert(Modifier::Cmd);
                }
                _ => {
                    if key.is_some() {
                        return Err(BindingError::UnknownToken(token.to_string()));
                    }
                    key = Some(token.to_string());
                }
            }
        }

        if modifiers.is_empty() {
            return Err(BindingError::NoModifier);
        }
        let key = key.ok_or(BindingError::NoKey)?;
        Ok(Binding { modifiers, key })
    }
}

impl fmt::Display for Binding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Canonical modifier order: Ctrl, Alt, Shift, Cmd.
        const ORDER: [Modifier; 4] = [
            Modifier::Ctrl,
            Modifier::Alt,
            Modifier::Shift,
            Modifier::Cmd,
        ];
        for m in ORDER {
            if self.modifiers.contains(&m) {
                write!(
                    f,
                    "{}+",
                    match m {
                        Modifier::Ctrl => "Ctrl",
                        Modifier::Alt => "Alt",
                        Modifier::Shift => "Shift",
                        Modifier::Cmd => "Cmd",
                    }
                )?;
            }
        }
        write!(f, "{}", self.key)
    }
}

/// Named slots from spec §8.1. Six bindings — the original
/// "open popover" hotkey is gone with the popover (the tray menu is
/// now the primary surface and opens on left-click).
pub const SLOT_PROFILE_1: &str = "profile1";
pub const SLOT_PROFILE_2: &str = "profile2";
pub const SLOT_PROFILE_3: &str = "profile3";
pub const SLOT_PROFILE_4: &str = "profile4";
pub const SLOT_PROFILE_5: &str = "profile5";
pub const SLOT_REFRESH: &str = "refresh";

/// Spec §8.1 defaults, returned as `(slot, binding)` pairs.
pub fn default_bindings() -> Vec<(&'static str, Binding)> {
    let parse = |s: &str| Binding::parse(s).expect("default binding parses");
    vec![
        (SLOT_PROFILE_1, parse("Ctrl+Alt+1")),
        (SLOT_PROFILE_2, parse("Ctrl+Alt+2")),
        (SLOT_PROFILE_3, parse("Ctrl+Alt+3")),
        (SLOT_PROFILE_4, parse("Ctrl+Alt+4")),
        (SLOT_PROFILE_5, parse("Ctrl+Alt+5")),
        (SLOT_REFRESH, parse("Ctrl+Alt+Shift+R")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_alt_digit() {
        let b = Binding::parse("Ctrl+Alt+1").unwrap();
        assert!(b.modifiers.contains(&Modifier::Ctrl));
        assert!(b.modifiers.contains(&Modifier::Alt));
        assert_eq!(b.key, "1");
    }

    #[test]
    fn parses_three_modifier_chord() {
        let b = Binding::parse("Ctrl+Alt+Shift+R").unwrap();
        assert_eq!(b.modifiers.len(), 3);
        assert_eq!(b.key, "R");
    }

    #[test]
    fn parsing_is_token_order_independent() {
        let a = Binding::parse("Shift+Ctrl+Alt+R").unwrap();
        let b = Binding::parse("Ctrl+Alt+Shift+R").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn parsing_normalises_modifier_aliases() {
        let opt = Binding::parse("Ctrl+Opt+1").unwrap();
        let alt = Binding::parse("Ctrl+Alt+1").unwrap();
        assert_eq!(opt, alt);
        let cmd = Binding::parse("Cmd+1").unwrap();
        let meta = Binding::parse("Meta+1").unwrap();
        assert_eq!(cmd, meta);
    }

    #[test]
    fn rejects_binding_without_modifier() {
        assert_eq!(Binding::parse("R").unwrap_err(), BindingError::NoModifier);
    }

    #[test]
    fn rejects_binding_without_key() {
        assert_eq!(
            Binding::parse("Ctrl+Alt+Shift").unwrap_err(),
            BindingError::NoKey
        );
    }

    #[test]
    fn rejects_binding_with_two_non_modifier_keys() {
        assert!(Binding::parse("Ctrl+A+B").is_err());
    }

    #[test]
    fn display_uses_canonical_order() {
        let b = Binding::parse("Shift+Cmd+Alt+Ctrl+K").unwrap();
        assert_eq!(b.to_string(), "Ctrl+Alt+Shift+Cmd+K");
    }

    #[test]
    fn defaults_match_spec_8_1() {
        let defs = default_bindings();
        // Six bindings — openPopover was retired with the popover.
        assert_eq!(defs.len(), 6);
        let by_slot: std::collections::HashMap<_, _> = defs.into_iter().collect();
        assert_eq!(by_slot[SLOT_PROFILE_1].to_string(), "Ctrl+Alt+1");
        assert_eq!(by_slot[SLOT_PROFILE_5].to_string(), "Ctrl+Alt+5");
        assert_eq!(by_slot[SLOT_REFRESH].to_string(), "Ctrl+Alt+Shift+R");
    }

    #[test]
    fn round_trip_through_string() {
        for (_, b) in default_bindings() {
            let serialized = b.to_string();
            let reparsed = Binding::parse(&serialized).unwrap();
            assert_eq!(reparsed, b);
        }
    }
}
