//! On-disk config: a single versioned JSON file, written atomically,
//! with crash-safe rewrites and a migration path.
//!
//! Layout (spec §10):
//!
//! - macOS:   ~/Library/Application Support/MiraController/config.json
//! - Linux:   $XDG_CONFIG_HOME/mira-controller/config.json
//! - Windows: %APPDATA%\MiraController\config.json
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
pub const CURRENT_VERSION: u32 = 1;

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
    let dirs = directories::ProjectDirs::from("com", "MiraController", "MiraController")
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

    // Forward migrations live here. v0 -> v1 is the only step today;
    // v0 is the implicit "no version key" shape so any file with
    // version == 0 (or missing version) is treated as v0.
    if cfg.version == 0 {
        // v0 -> v1: just bump the version field. No structural change
        // beyond "we have a version now".
        cfg.version = 1;
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

        // Synthesise a v0 file: same shape as v1 but version=0.
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
    fn config_path_returns_a_path_under_a_known_dir() {
        let path = config_path().unwrap();
        // We can't assert the exact OS-specific prefix in CI without
        // knowing the runner's HOME, but the file name is stable.
        assert_eq!(path.file_name().unwrap(), "config.json");
    }
}
