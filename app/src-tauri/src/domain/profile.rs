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

impl BuiltInPreset {
    /// Per-preset settings from [spec §7.1]. Cold/warm light default
    /// to 0 across all presets — the front-light is opt-in.
    pub fn settings(self) -> ProfileSettings {
        match self {
            BuiltInPreset::Read => ProfileSettings {
                refresh_mode: RefreshMode::Direct,
                speed: 3,
                contrast: 9,
                dither_mode: 1,
                white_filter: 0,
                black_filter: 0,
                cold_light: 0,
                warm_light: 0,
            },
            BuiltInPreset::Text => ProfileSettings {
                refresh_mode: RefreshMode::A2,
                speed: 5,
                contrast: 11,
                dither_mode: 0,
                white_filter: 12,
                black_filter: 6,
                cold_light: 0,
                warm_light: 0,
            },
            BuiltInPreset::Coding => ProfileSettings {
                refresh_mode: RefreshMode::A2,
                speed: 6,
                contrast: 12,
                dither_mode: 0,
                white_filter: 16,
                black_filter: 8,
                cold_light: 0,
                warm_light: 0,
            },
            BuiltInPreset::Speed => ProfileSettings {
                refresh_mode: RefreshMode::A2,
                speed: 7,
                contrast: 8,
                dither_mode: 0,
                white_filter: 0,
                black_filter: 0,
                cold_light: 0,
                warm_light: 0,
            },
            BuiltInPreset::Image => ProfileSettings {
                refresh_mode: RefreshMode::Direct,
                speed: 2,
                contrast: 10,
                dither_mode: 2,
                white_filter: 0,
                black_filter: 0,
                cold_light: 0,
                warm_light: 0,
            },
            BuiltInPreset::Video => ProfileSettings {
                refresh_mode: RefreshMode::A2,
                speed: 7,
                contrast: 7,
                dither_mode: 0,
                white_filter: 0,
                black_filter: 0,
                cold_light: 0,
                warm_light: 0,
            },
            // As-found is overwritten at first connect from the actual
            // device snapshot; the static default is "neutral" so the
            // UI has something to render until a device shows up.
            BuiltInPreset::AsFound => ProfileSettings {
                refresh_mode: RefreshMode::Direct,
                speed: 4,
                contrast: 8,
                dither_mode: 1,
                white_filter: 0,
                black_filter: 0,
                cold_light: 0,
                warm_light: 0,
            },
        }
    }

    /// Display name shown in the UI.
    pub fn name(self) -> &'static str {
        match self {
            BuiltInPreset::Read => "Read",
            BuiltInPreset::Text => "Text",
            BuiltInPreset::Coding => "Coding",
            BuiltInPreset::Speed => "Speed",
            BuiltInPreset::Image => "Image",
            BuiltInPreset::Video => "Video",
            BuiltInPreset::AsFound => "As-found",
        }
    }

    /// The default preset on first launch, per spec §7.1.
    pub fn default_preset() -> Self {
        BuiltInPreset::Coding
    }

    /// All six user-facing presets, in spec display order.
    /// `AsFound` is intentionally excluded — it's generated on first
    /// connect, not shipped.
    pub fn all() -> [BuiltInPreset; 6] {
        [
            BuiltInPreset::Read,
            BuiltInPreset::Text,
            BuiltInPreset::Coding,
            BuiltInPreset::Speed,
            BuiltInPreset::Image,
            BuiltInPreset::Video,
        ]
    }

    /// Construct the read-only profile that ships with the app.
    /// `created_at` / `modified_at` use a stable epoch so the file
    /// round-trip is deterministic.
    pub fn into_profile(self) -> Profile {
        let epoch = OffsetDateTime::from_unix_timestamp(0).unwrap();
        Profile {
            id: ProfileId::BuiltIn(self),
            name: self.name().to_string(),
            built_in: true,
            hotkey: None,
            settings: self.settings(),
            created_at: epoch,
            modified_at: epoch,
        }
    }
}

/// All ship-with-the-app presets as `Profile` values.
pub fn built_in_profiles() -> Vec<Profile> {
    BuiltInPreset::all()
        .into_iter()
        .map(BuiltInPreset::into_profile)
        .collect()
}

/// Capture the device's current settings as the "as-found" profile,
/// shown alongside the six shipped presets so users can revert to
/// however their Mira behaved before this app first wrote to it.
pub fn as_found_profile_from(snapshot: ProfileSettings, captured_at: OffsetDateTime) -> Profile {
    Profile {
        id: ProfileId::BuiltIn(BuiltInPreset::AsFound),
        name: BuiltInPreset::AsFound.name().to_string(),
        built_in: true,
        hotkey: None,
        settings: snapshot.clamp(),
        created_at: captured_at,
        modified_at: captured_at,
    }
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

    #[test]
    fn preset_read_matches_spec_table() {
        let s = BuiltInPreset::Read.settings();
        assert_eq!(s.refresh_mode, RefreshMode::Direct);
        assert_eq!(s.speed, 3);
        assert_eq!(s.contrast, 9);
        assert_eq!(s.dither_mode, 1);
        assert_eq!(s.white_filter, 0);
        assert_eq!(s.black_filter, 0);
    }

    #[test]
    fn preset_text_matches_spec_table() {
        let s = BuiltInPreset::Text.settings();
        assert_eq!(s.refresh_mode, RefreshMode::A2);
        assert_eq!(s.speed, 5);
        assert_eq!(s.contrast, 11);
        assert_eq!(s.dither_mode, 0);
        assert_eq!(s.white_filter, 12);
        assert_eq!(s.black_filter, 6);
    }

    #[test]
    fn preset_coding_matches_spec_table_and_is_the_default() {
        let s = BuiltInPreset::Coding.settings();
        assert_eq!(s.refresh_mode, RefreshMode::A2);
        assert_eq!(s.speed, 6);
        assert_eq!(s.contrast, 12);
        assert_eq!(s.dither_mode, 0);
        assert_eq!(s.white_filter, 16);
        assert_eq!(s.black_filter, 8);
        assert_eq!(BuiltInPreset::default_preset(), BuiltInPreset::Coding);
    }

    #[test]
    fn preset_speed_matches_spec_table() {
        let s = BuiltInPreset::Speed.settings();
        assert_eq!(s.refresh_mode, RefreshMode::A2);
        assert_eq!(s.speed, 7);
        assert_eq!(s.contrast, 8);
        assert_eq!(s.dither_mode, 0);
        assert_eq!(s.white_filter, 0);
        assert_eq!(s.black_filter, 0);
    }

    #[test]
    fn preset_image_matches_spec_table() {
        let s = BuiltInPreset::Image.settings();
        assert_eq!(s.refresh_mode, RefreshMode::Direct);
        assert_eq!(s.speed, 2);
        assert_eq!(s.contrast, 10);
        assert_eq!(s.dither_mode, 2);
        assert_eq!(s.white_filter, 0);
        assert_eq!(s.black_filter, 0);
    }

    #[test]
    fn preset_video_matches_spec_table() {
        let s = BuiltInPreset::Video.settings();
        assert_eq!(s.refresh_mode, RefreshMode::A2);
        assert_eq!(s.speed, 7);
        assert_eq!(s.contrast, 7);
        assert_eq!(s.dither_mode, 0);
        assert_eq!(s.white_filter, 0);
        assert_eq!(s.black_filter, 0);
    }

    #[test]
    fn built_in_profiles_lists_six_in_spec_order() {
        let profiles = built_in_profiles();
        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Read", "Text", "Coding", "Speed", "Image", "Video"]);
        assert!(profiles.iter().all(|p| p.built_in));
    }

    #[test]
    fn as_found_profile_captures_current_device_snapshot() {
        let snap = sample_settings();
        let captured_at = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let profile = as_found_profile_from(snap, captured_at);
        assert_eq!(profile.id, ProfileId::BuiltIn(BuiltInPreset::AsFound));
        assert_eq!(profile.name, "As-found");
        assert!(profile.built_in);
        assert_eq!(profile.settings, snap);
        assert_eq!(profile.created_at, captured_at);
        assert_eq!(profile.modified_at, captured_at);
    }

    #[test]
    fn as_found_profile_clamps_a_bad_snapshot() {
        let snap = ProfileSettings {
            refresh_mode: RefreshMode::A2,
            speed: 99, // out of range
            contrast: 12,
            dither_mode: 0,
            white_filter: 16,
            black_filter: 8,
            cold_light: 0,
            warm_light: 0,
        };
        let profile = as_found_profile_from(snap, OffsetDateTime::UNIX_EPOCH);
        assert_eq!(profile.settings.speed, 7);
    }
}
