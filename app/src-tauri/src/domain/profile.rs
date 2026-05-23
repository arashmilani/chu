//! Profile model + built-in presets. Pure data; no I/O.
//!
//! A profile is a named, complete snapshot of every device knob
//! exposed in [spec §6]. Presets ship read-only; users duplicate them
//! to make their own.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::mira::encoder::RefreshMode;

/// Names the six built-in presets plus the special "as-found" snapshot
/// captured from the device on first connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BuiltInPreset {
    Read,
    Text,
    Coding,
    Speed,
    Image,
    Video,
    AsFound,
}

/// Unique handle for a profile. Built-in presets keep stable names so
/// the config file survives across releases; custom profiles get a
/// fresh UUID per creation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProfileId {
    BuiltIn(BuiltInPreset),
    Custom(Uuid),
}

impl ProfileId {
    pub fn new_custom() -> Self {
        ProfileId::Custom(Uuid::new_v4())
    }
}

/// The nine device settings as the user/UI sees them — pre-encoder,
/// in spec ranges. The encoder layer is responsible for any wire
/// transforms (inversion, opcode mapping).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettings {
    pub refresh_mode: RefreshMode,
    pub speed: u8,
    pub contrast: u8,
    pub dither_mode: u8,
    pub white_filter: u8,
    pub black_filter: u8,
    pub cold_light: u8,
    pub warm_light: u8,
}

impl ProfileSettings {
    /// Clamp every field to the spec range. Idempotent; safe to call
    /// repeatedly. Field-by-field instead of "validate-or-error" so
    /// the UI can scrub a malformed import without rejecting it.
    pub fn clamp(mut self) -> Self {
        self.speed = self.speed.clamp(1, 7);
        self.contrast = self.contrast.clamp(0, 15);
        self.dither_mode = self.dither_mode.clamp(0, 3);
        self.white_filter = self.white_filter.clamp(0, 127);
        self.black_filter = self.black_filter.clamp(0, 127);
        self.cold_light = self.cold_light.clamp(0, 254);
        self.warm_light = self.warm_light.clamp(0, 254);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub built_in: bool,
    #[serde(default)]
    pub hotkey: Option<String>,
    pub settings: ProfileSettings,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub modified_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_settings() -> ProfileSettings {
        ProfileSettings {
            refresh_mode: RefreshMode::A2,
            speed: 6,
            contrast: 12,
            dither_mode: 0,
            white_filter: 16,
            black_filter: 8,
            cold_light: 0,
            warm_light: 0,
        }
    }

    #[test]
    fn profile_round_trips_through_json() {
        let original = Profile {
            id: ProfileId::Custom(Uuid::nil()),
            name: "Long-form writing".to_string(),
            built_in: false,
            hotkey: Some("Ctrl+Alt+3".to_string()),
            settings: sample_settings(),
            created_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
            modified_at: OffsetDateTime::from_unix_timestamp(1_700_000_500).unwrap(),
        };

        let json = serde_json::to_string(&original).unwrap();
        let restored: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(original, restored);
    }

    #[test]
    fn profile_id_built_in_serializes_as_bare_string() {
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"coding\"");
    }

    #[test]
    fn profile_id_built_in_kebab_case_for_compound_names() {
        let id = ProfileId::BuiltIn(BuiltInPreset::AsFound);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"as-found\"");
    }

    #[test]
    fn profile_id_custom_serializes_as_uuid_string() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = ProfileId::Custom(uuid);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"550e8400-e29b-41d4-a716-446655440000\"");
    }

    #[test]
    fn clamp_brings_every_field_into_spec_range() {
        let wild = ProfileSettings {
            refresh_mode: RefreshMode::Direct,
            speed: 99,
            contrast: 200,
            dither_mode: 50,
            white_filter: 200,
            black_filter: 200,
            cold_light: 255,
            warm_light: 255,
        };
        let clamped = wild.clamp();
        assert_eq!(clamped.speed, 7);
        assert_eq!(clamped.contrast, 15);
        assert_eq!(clamped.dither_mode, 3);
        assert_eq!(clamped.white_filter, 127);
        assert_eq!(clamped.black_filter, 127);
        assert_eq!(clamped.cold_light, 254);
        assert_eq!(clamped.warm_light, 254);
    }

    #[test]
    fn clamp_does_not_raise_below_minimum_for_speed() {
        let s = ProfileSettings {
            speed: 0,
            ..sample_settings()
        };
        assert_eq!(s.clamp().speed, 1);
    }

    #[test]
    fn clamp_is_idempotent() {
        let clean = sample_settings();
        assert_eq!(clean.clamp(), clean);
    }
}
