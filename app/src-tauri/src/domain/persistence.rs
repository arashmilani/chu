//! On-disk config: a single versioned JSON file, written atomically,
//! with crash-safe rewrites and a migration path.
//!
//! Layout (spec §10):
//!
//! - macOS:   ~/Library/Application Support/com.arashmilani.chu/config.json
//! - Linux:   $XDG_CONFIG_HOME/chu/config.json
//! - Windows: %APPDATA%\arashmilani\Chu\config.json
//!
//! Atomicity: writes go to a `*.tmp` sibling and are renamed onto the
//! target path; if power dies mid-write the original file is intact.
//!
//! Migrations: bumping `version` runs a fixup pass and copies the old
//! file to `config.v{old}.bak` first so users can recover.
//!
//! Corruption recovery: a config file that fails to deserialize is
//! moved aside (`config.corrupt.{ts}.bak`) and a fresh default-config
//! is written in its place. We never lose the user's data silently
//! and we never refuse to launch.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::domain::profile::{built_in_profiles, BuiltInPreset, Profile, ProfileId};

/// Current config schema version. Bump when the JSON shape changes.
pub const CURRENT_VERSION: u32 = 4;

/// Smallest interval the user is allowed to pick. Anything below
/// this fires too often to be useful and gets in the way of typing.
pub const MIN_AUTO_REFRESH_SECONDS: u32 = 5;

/// Default `autoRefreshSeconds` when the user first turns the
/// feature on. 30 seconds is short enough to actually clear
/// accumulated ghosting during fast typing but long enough that the
/// refresh flash doesn't constantly interrupt.
pub const DEFAULT_AUTO_REFRESH_SECONDS: u32 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub last_active_profile_id: Option<ProfileId>,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default)]
    pub hotkeys: BTreeMap<String, String>,
    pub profiles: Vec<Profile>,
    /// Serial number of the Mira the user picked when multiple were
    /// detected. `None` means "use the first one we find".
    #[serde(default)]
    pub selected_device_serial: Option<String>,
    /// Auto-refresh: when on, a background task fires a full refresh
    /// every `auto_refresh_seconds` seconds while the active profile
    /// uses `a2` mode AND the user has shown input activity within
    /// that window (so we don't refresh an idle/AFK display). See
    /// spec §9.5.
    #[serde(default)]
    pub auto_refresh_enabled: bool,
    #[serde(default = "default_auto_refresh_seconds")]
    pub auto_refresh_seconds: u32,
}

fn default_auto_refresh_seconds() -> u32 {
    DEFAULT_AUTO_REFRESH_SECONDS
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            last_active_profile_id: Some(ProfileId::BuiltIn(BuiltInPreset::Coding)),
            launch_at_login: false,
            hotkeys: default_hotkeys(),
            profiles: built_in_profiles(),
            selected_device_serial: None,
            auto_refresh_enabled: false,
            auto_refresh_seconds: DEFAULT_AUTO_REFRESH_SECONDS,
        }
    }
}

/// Spec §8.1 default hotkeys, using the cross-platform string form
/// `tauri-plugin-global-shortcut` accepts. Modifier `Alt` covers both
/// macOS Option and Win/Linux Alt.
pub fn default_hotkeys() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("profile1".into(), "Ctrl+Alt+1".into());
    m.insert("profile2".into(), "Ctrl+Alt+2".into());
    m.insert("profile3".into(), "Ctrl+Alt+3".into());
    m.insert("profile4".into(), "Ctrl+Alt+4".into());
    m.insert("profile5".into(), "Ctrl+Alt+5".into());
    m.insert("refresh".into(), "Ctrl+Alt+Shift+R".into());
    m.insert("openPopover".into(), "Ctrl+Alt+Shift+M".into());
    m
}

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("could not resolve config directory for this platform")]
    NoConfigDir,
}

/// Resolve the OS-standard config file path (per spec §10).
pub fn config_path() -> Result<PathBuf, PersistenceError> {
    let dirs = directories::ProjectDirs::from("com", "arashmilani", "Chu")
        .ok_or(PersistenceError::NoConfigDir)?;
    Ok(dirs.config_dir().join("config.json"))
}

/// Read+parse the file at `path`, falling back to defaults (and
/// preserving the corrupt file as a sidecar) on parse failure.
pub fn load_from(path: &Path, now: OffsetDateTime) -> Result<Config, PersistenceError> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(e) => return Err(e.into()),
    };

    match serde_json::from_slice::<Config>(&bytes) {
        Ok(cfg) => migrate(cfg, path),
        Err(_) => {
            // Move the bad file aside, write defaults.
            let sidecar = corrupt_sidecar_path(path, now);
            fs::rename(path, &sidecar)?;
            let defaults = Config::default();
            save_to(path, &defaults)?;
            Ok(defaults)
        }
    }
}

/// Write `config` atomically: serialise to a sibling `*.tmp` file,
/// fsync, then rename into place.
pub fn save_to(path: &Path, config: &Config) -> Result<(), PersistenceError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(config)?;
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&body)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Run upgrade passes between known versions, writing a backup of the
/// previous file along the way.
fn migrate(mut cfg: Config, path: &Path) -> Result<Config, PersistenceError> {
    if cfg.version == CURRENT_VERSION {
        return Ok(cfg);
    }

    let original_version = cfg.version;

    // Place the backup once, then run forward steps.
    let backup = path.with_file_name(format!("config.v{original_version}.bak"));
    fs::copy(path, &backup)?;

    // Forward migrations live here. Each step bumps `version` by one
    // and runs any data fixups. `#[serde(default)]` already supplies
    // sane values for fields added in newer versions, and serde
    // silently drops fields removed in newer versions on the next
    // save, so most steps just bump the number.
    if cfg.version == 0 {
        // v0 -> v1: introduced a version field. No structural change.
        cfg.version = 1;
    }
    if cfg.version == 1 {
        // v1 -> v2: added a (since-removed) auto-refresh-by-HID-write
        // counter. The field is gone in v3; serde drops it on save.
        cfg.version = 2;
    }
    if cfg.version == 2 {
        // v2 -> v3: replaced auto-refresh-by-HID-writes with
        // auto-refresh-by-minutes (active-user-gated). Both fields
        // are gone in v4 — serde drops them on save.
        cfg.version = 3;
    }
    if cfg.version == 3 {
        // v3 -> v4: minutes -> seconds. We deliberately don't map
        // `autoRefreshMinutes * 60` over: 15 minutes (the old
        // default) isn't a sensible auto-pick on the new 5+ seconds
        // floor. Users coming from v3 land on the v4 default (30s).
        cfg.version = 4;
    }

    // Write the migrated config back atomically.
    save_to(path, &cfg)?;
    Ok(cfg)
}

fn corrupt_sidecar_path(path: &Path, now: OffsetDateTime) -> PathBuf {
    let unix = now.unix_timestamp();
    path.with_file_name(format!(
        "{}.corrupt.{unix}.bak",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("config")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_use_current_version_and_ship_six_presets() {
        let cfg = Config::default();
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.profiles.len(), 6);
        assert!(!cfg.launch_at_login);
        assert_eq!(cfg.hotkeys.get("profile1").unwrap(), "Ctrl+Alt+1");
        // Auto-refresh ships off; the threshold is the documented
        // starting default.
        assert!(!cfg.auto_refresh_enabled);
        assert_eq!(cfg.auto_refresh_seconds, DEFAULT_AUTO_REFRESH_SECONDS);
    }

    #[test]
    fn save_then_load_round_trips_every_field() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        let original = Config {
            launch_at_login: true,
            ..Config::default()
        };
        save_to(&path, &original).unwrap();

        let restored = load_from(&path, OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn missing_file_returns_defaults_without_creating_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("absent.json");
        let cfg = load_from(&path, OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(cfg, Config::default());
        assert!(!path.exists());
    }

    #[test]
    fn save_is_atomic_via_tmp_rename() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        save_to(&path, &Config::default()).unwrap();

        // The tmp sibling should not survive a successful save.
        let tmp = path.with_extension("json.tmp");
        assert!(path.exists());
        assert!(!tmp.exists());
    }

    #[test]
    fn corrupt_file_is_moved_aside_and_defaults_are_written() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        fs::write(&path, b"not json at all { ").unwrap();

        let now = OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
        let cfg = load_from(&path, now).unwrap();
        assert_eq!(cfg, Config::default());

        // Sidecar exists, and is the original bad bytes.
        let sidecar = path.with_file_name("config.json.corrupt.1700000000.bak");
        assert!(sidecar.exists());
        assert_eq!(fs::read(&sidecar).unwrap(), b"not json at all { ");
        // The path now holds the freshly written defaults.
        let restored = load_from(&path, now).unwrap();
        assert_eq!(restored, Config::default());
    }

    #[test]
    fn v0_file_is_migrated_to_current_and_old_is_backed_up() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");

        // Synthesise a v0 file: same shape but version=0.
        let v0 = Config {
            version: 0,
            ..Config::default()
        };
        save_to(&path, &v0).unwrap();

        let migrated = load_from(&path, OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(migrated.version, CURRENT_VERSION);

        let backup = path.with_file_name("config.v0.bak");
        assert!(backup.exists());
        // The backup keeps the old version field for forensics.
        let raw_backup: Config = serde_json::from_slice(&fs::read(&backup).unwrap()).unwrap();
        assert_eq!(raw_backup.version, 0);
    }

    #[test]
    fn v2_file_with_dead_auto_refresh_hid_writes_field_migrates_forward() {
        // A real v2 file: had the abandoned `autoRefreshHidWrites`
        // counter. The v2 -> v3 -> v4 chain drops it; serde
        // backfills `autoRefreshSeconds` with the v4 default.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let v2_json = r#"{
            "version": 2,
            "lastActiveProfileId": "coding",
            "launchAtLogin": false,
            "hotkeys": {"profile1": "Ctrl+Alt+1"},
            "profiles": [],
            "selectedDeviceSerial": null,
            "autoRefreshEnabled": true,
            "autoRefreshHidWrites": 50
        }"#;
        fs::write(&path, v2_json).unwrap();

        let migrated = load_from(&path, OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(migrated.version, CURRENT_VERSION);
        // The user's "enabled" preference is preserved.
        assert!(migrated.auto_refresh_enabled);
        // v4 default takes over for the now-renamed field.
        assert_eq!(migrated.auto_refresh_seconds, DEFAULT_AUTO_REFRESH_SECONDS);

        let backup = path.with_file_name("config.v2.bak");
        assert!(backup.exists());

        // Saved file no longer carries the dead fields.
        let on_disk: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(on_disk.get("autoRefreshHidWrites").is_none());
        assert!(on_disk.get("autoRefreshMinutes").is_none());
        assert_eq!(
            on_disk.get("autoRefreshSeconds").and_then(|v| v.as_u64()),
            Some(DEFAULT_AUTO_REFRESH_SECONDS as u64),
        );
    }

    #[test]
    fn v3_file_with_dead_auto_refresh_minutes_field_migrates_to_v4() {
        // v3 had `autoRefreshMinutes`. We deliberately don't map
        // minutes -> seconds (15 min ≠ 15 sec); v4 reset to default.
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.json");
        let v3_json = r#"{
            "version": 3,
            "lastActiveProfileId": "coding",
            "launchAtLogin": false,
            "hotkeys": {"profile1": "Ctrl+Alt+1"},
            "profiles": [],
            "selectedDeviceSerial": null,
            "autoRefreshEnabled": true,
            "autoRefreshMinutes": 15
        }"#;
        fs::write(&path, v3_json).unwrap();

        let migrated = load_from(&path, OffsetDateTime::UNIX_EPOCH).unwrap();
        assert_eq!(migrated.version, CURRENT_VERSION);
        assert!(migrated.auto_refresh_enabled);
        assert_eq!(migrated.auto_refresh_seconds, DEFAULT_AUTO_REFRESH_SECONDS);

        let backup = path.with_file_name("config.v3.bak");
        assert!(backup.exists());
    }

    #[test]
    fn config_path_returns_a_path_under_a_known_dir() {
        let path = config_path().unwrap();
        // We can't assert the exact OS-specific prefix in CI without
        // knowing the runner's HOME, but the file name is stable.
        assert_eq!(path.file_name().unwrap(), "config.json");
    }
}
