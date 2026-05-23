//! OS-side hotkey registration via `tauri-plugin-global-shortcut`.
//!
//! Pure helpers for turning a `domain::hotkeys::Binding` into the
//! `Shortcut` shape the plugin expects, plus a `HotkeyManager` that
//! tracks which slots are currently registered so rebinds can
//! unregister the old binding cleanly.

use std::collections::BTreeMap;
use std::sync::Mutex;

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

use crate::domain::hotkeys::{Binding, Modifier};

/// Translate a parsed `Binding` to the plugin's `Shortcut` type.
/// Returns `None` for keys we don't know how to translate (the plugin
/// only accepts a fixed set of physical key codes).
pub fn binding_to_shortcut(binding: &Binding) -> Option<Shortcut> {
    let mut mods = Modifiers::empty();
    for m in &binding.modifiers {
        match m {
            Modifier::Ctrl => mods |= Modifiers::CONTROL,
            Modifier::Alt => mods |= Modifiers::ALT,
            Modifier::Shift => mods |= Modifiers::SHIFT,
            Modifier::Cmd => mods |= Modifiers::META,
        }
    }
    let code = key_to_code(&binding.key)?;
    Some(Shortcut::new(Some(mods), code))
}

fn key_to_code(key: &str) -> Option<Code> {
    // Digit row 1..9, 0.
    match key {
        "0" => return Some(Code::Digit0),
        "1" => return Some(Code::Digit1),
        "2" => return Some(Code::Digit2),
        "3" => return Some(Code::Digit3),
        "4" => return Some(Code::Digit4),
        "5" => return Some(Code::Digit5),
        "6" => return Some(Code::Digit6),
        "7" => return Some(Code::Digit7),
        "8" => return Some(Code::Digit8),
        "9" => return Some(Code::Digit9),
        _ => {}
    }
    // Single ASCII letter A..Z (case-insensitive).
    if key.len() == 1 {
        let c = key.chars().next().unwrap().to_ascii_uppercase();
        if c.is_ascii_alphabetic() {
            return ascii_letter_code(c);
        }
    }
    // Common named keys — extend as needed.
    match key.to_ascii_uppercase().as_str() {
        "SPACE" => Some(Code::Space),
        "ENTER" | "RETURN" => Some(Code::Enter),
        "ESC" | "ESCAPE" => Some(Code::Escape),
        "TAB" => Some(Code::Tab),
        _ => None,
    }
}

fn ascii_letter_code(c: char) -> Option<Code> {
    match c {
        'A' => Some(Code::KeyA),
        'B' => Some(Code::KeyB),
        'C' => Some(Code::KeyC),
        'D' => Some(Code::KeyD),
        'E' => Some(Code::KeyE),
        'F' => Some(Code::KeyF),
        'G' => Some(Code::KeyG),
        'H' => Some(Code::KeyH),
        'I' => Some(Code::KeyI),
        'J' => Some(Code::KeyJ),
        'K' => Some(Code::KeyK),
        'L' => Some(Code::KeyL),
        'M' => Some(Code::KeyM),
        'N' => Some(Code::KeyN),
        'O' => Some(Code::KeyO),
        'P' => Some(Code::KeyP),
        'Q' => Some(Code::KeyQ),
        'R' => Some(Code::KeyR),
        'S' => Some(Code::KeyS),
        'T' => Some(Code::KeyT),
        'U' => Some(Code::KeyU),
        'V' => Some(Code::KeyV),
        'W' => Some(Code::KeyW),
        'X' => Some(Code::KeyX),
        'Y' => Some(Code::KeyY),
        'Z' => Some(Code::KeyZ),
        _ => None,
    }
}

/// Tracks which slot owns which currently-registered shortcut, so
/// rebinds can unregister the old chord before registering the new
/// one. Wraps a plain BTreeMap behind a Mutex for interior mutability;
/// the manager is shared as `Arc<HotkeyManager>` from `AppHandle::state`.
#[derive(Default)]
pub struct HotkeyManager {
    registered: Mutex<BTreeMap<String, Shortcut>>,
}

impl HotkeyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn registered_slots(&self) -> Vec<String> {
        self.registered
            .lock()
            .expect("hotkey manager poisoned")
            .keys()
            .cloned()
            .collect()
    }

    pub fn set(&self, slot: impl Into<String>, shortcut: Shortcut) {
        self.registered
            .lock()
            .expect("hotkey manager poisoned")
            .insert(slot.into(), shortcut);
    }

    pub fn take(&self, slot: &str) -> Option<Shortcut> {
        self.registered
            .lock()
            .expect("hotkey manager poisoned")
            .remove(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_ctrl_alt_one() {
        let b = Binding::parse("Ctrl+Alt+1").unwrap();
        let s = binding_to_shortcut(&b).unwrap();
        let expected = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Digit1);
        assert_eq!(s, expected);
    }

    #[test]
    fn translates_three_modifier_letter() {
        let b = Binding::parse("Ctrl+Alt+Shift+R").unwrap();
        let s = binding_to_shortcut(&b).unwrap();
        let expected = Shortcut::new(
            Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
            Code::KeyR,
        );
        assert_eq!(s, expected);
    }

    #[test]
    fn translates_cmd_to_meta() {
        let b = Binding::parse("Cmd+M").unwrap();
        let s = binding_to_shortcut(&b).unwrap();
        let expected = Shortcut::new(Some(Modifiers::META), Code::KeyM);
        assert_eq!(s, expected);
    }

    #[test]
    fn lowercase_letter_input_is_normalised() {
        let b = Binding::parse("Ctrl+m").unwrap();
        let s = binding_to_shortcut(&b).unwrap();
        let expected = Shortcut::new(Some(Modifiers::CONTROL), Code::KeyM);
        assert_eq!(s, expected);
    }

    #[test]
    fn returns_none_for_unknown_key() {
        let b = Binding {
            modifiers: [Modifier::Ctrl].into_iter().collect(),
            key: "F37".to_string(),
        };
        assert!(binding_to_shortcut(&b).is_none());
    }

    #[test]
    fn manager_tracks_set_and_take() {
        let m = HotkeyManager::new();
        let shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::Digit1);
        m.set("profile1", shortcut);
        assert_eq!(m.registered_slots(), vec!["profile1".to_string()]);
        let removed = m.take("profile1").unwrap();
        assert_eq!(removed, shortcut);
        assert!(m.registered_slots().is_empty());
    }
}
