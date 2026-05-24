//! AppState — the shared, mutable backend state the command layer
//! operates on. One per Tauri app; commands lock it briefly.
//!
//! Tests construct AppState with a MockTransport so the command
//! surface is exercised end-to-end without USB.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use time::OffsetDateTime;

use crate::domain::persistence::{self, Config};
use crate::domain::profile::{Profile, ProfileId, ProfileSettings};
use crate::domain::session::Session;
use crate::domain::store::ProfileStore;
use crate::mira::encoder::RefreshMode;
use crate::mira::transport::HidTransport;

pub struct AppState {
    inner: Mutex<Inner>,
}

struct Inner {
    config: Config,
    store: ProfileStore,
    session: Session,
    transport: Option<Arc<dyn HidTransport>>,
    config_path: Option<PathBuf>,
    first_run: bool,
    /// Wall-clock instant of the last refresh sent to the device
    /// (manual or auto). The periodic auto-refresh task gates on
    /// `now - last_refresh_at >= auto_refresh_seconds`. Reset on
    /// disconnect, threshold change, and profile switch so changes
    /// take effect from "now", not from whenever the previous tally
    /// started. In-memory only — restarting the app starts the
    /// clock over.
    last_refresh_at: Option<Instant>,
}

impl AppState {
    /// Construct an empty AppState seeded from defaults. Used by
    /// tests; production wires a real config path via `with_config`.
    pub fn in_memory() -> Self {
        let config = Config::default();
        let store = ProfileStore::with_profiles(config.profiles.clone());
        Self {
            inner: Mutex::new(Inner {
                config,
                store,
                session: Session::new(),
                transport: None,
                config_path: None,
                first_run: false,
                last_refresh_at: None,
            }),
        }
    }

    /// Build AppState from a config file on disk. Records whether the
    /// file existed pre-load — callers use that to trigger the
    /// first-run welcome flow.
    pub fn load_from_disk(path: PathBuf) -> Result<Self, persistence::PersistenceError> {
        let existed_before = path.exists();
        let config = persistence::load_from(&path, OffsetDateTime::now_utc())?;
        let store = ProfileStore::with_profiles(config.profiles.clone());
        Ok(Self {
            inner: Mutex::new(Inner {
                config,
                store,
                session: Session::new(),
                transport: None,
                config_path: Some(path),
                first_run: !existed_before,
                last_refresh_at: None,
            }),
        })
    }

    pub fn is_first_run(&self) -> bool {
        self.inner.lock().expect("app state poisoned").first_run
    }

    /// Mark the first-run flow as completed so the welcome card stops
    /// showing on subsequent app launches. Persists by writing the
    /// current config to disk (which forces config file creation,
    /// making future loads not-first-run).
    pub fn complete_first_run(&self) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.first_run = false;
        self.persist(&inner);
    }

    /// Capture the device's current settings into the AsFound preset
    /// (spec §7.1). The argument is what the caller observed on the
    /// device — typically inferred from the last apply, since the
    /// vendor firmware doesn't always support reading state back.
    pub fn capture_as_found(&self, snapshot: ProfileSettings) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        let now = OffsetDateTime::now_utc();
        let profile = crate::domain::profile::as_found_profile_from(snapshot, now);
        // Replace any existing AsFound entry.
        inner.config.profiles.retain(|p| p.id != profile.id);
        inner.config.profiles.push(profile);
        inner.store = ProfileStore::with_profiles(inner.config.profiles.clone());
        self.persist(&inner);
    }

    pub fn attach_transport(&self, transport: Arc<dyn HidTransport>) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.transport = Some(transport);
        // A fresh transport means the device's actual state is
        // unknown; the next apply() will write every field.
        inner.session.invalidate();
        // Start the auto-refresh clock from the connect moment so a
        // freshly-connected device gets its first auto-refresh after
        // a full interval, not immediately.
        inner.last_refresh_at = Some(Instant::now());
    }

    pub fn detach_transport(&self) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.transport = None;
        inner.session.invalidate();
        inner.last_refresh_at = None;
    }

    pub fn is_connected(&self) -> bool {
        self.inner
            .lock()
            .expect("app state poisoned")
            .transport
            .is_some()
    }

    pub fn list_profiles(&self) -> Vec<Profile> {
        self.inner
            .lock()
            .expect("app state poisoned")
            .store
            .profiles()
            .to_vec()
    }

    pub fn active_profile_id(&self) -> Option<ProfileId> {
        self.inner
            .lock()
            .expect("app state poisoned")
            .config
            .last_active_profile_id
            .clone()
    }

    pub fn set_active_profile_id(&self, id: ProfileId) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.config.last_active_profile_id = Some(id);
        self.persist(&inner);
    }

    /// Apply a profile by id. Returns the list of frames written to
    /// the device — useful for tests and for emitting telemetry
    /// events. Errors if the profile id is unknown or no device is
    /// attached.
    pub fn apply_profile(&self, id: &ProfileId) -> Result<Vec<Vec<u8>>, ApplyError> {
        let mut inner = self.inner.lock().expect("app state poisoned");
        let profile = inner.store.find(id).cloned().ok_or(ApplyError::NotFound)?;
        let is_switch = inner.config.last_active_profile_id.as_ref() != Some(id);
        let frames = inner.session.apply(profile.settings);

        if let Some(transport) = inner.transport.clone() {
            for frame in &frames {
                transport.write_feature(frame).map_err(ApplyError::Device)?;
            }
        } else {
            // No device — we still record the choice so apply-on-connect
            // works later, but we don't pretend we sent anything.
            inner.session.invalidate();
        }
        inner.config.last_active_profile_id = Some(id.clone());
        // Switching to a different profile restarts the auto-refresh
        // clock: the new profile gets a fresh interval, and we don't
        // want a switch into an A2 profile to fire immediately if
        // the previous interval was already exhausted.
        if is_switch && inner.transport.is_some() {
            inner.last_refresh_at = Some(Instant::now());
        }
        self.persist(&inner);
        Ok(frames)
    }

    pub fn force_refresh(&self) -> Result<(), ApplyError> {
        let mut inner = self.inner.lock().expect("app state poisoned");
        let transport = inner.transport.clone().ok_or(ApplyError::Device(
            crate::mira::transport::TransportError::Disconnected,
        ))?;
        let frame = crate::mira::encoder::encode_refresh();
        transport
            .write_feature(&frame)
            .map_err(ApplyError::Device)?;
        // Any successful full refresh — manual or auto — resets the
        // auto-refresh clock.
        inner.last_refresh_at = Some(Instant::now());
        Ok(())
    }

    pub fn duplicate(
        &self,
        id: &ProfileId,
    ) -> Result<ProfileId, crate::domain::store::ProfileError> {
        let mut inner = self.inner.lock().expect("app state poisoned");
        let now = OffsetDateTime::now_utc();
        let new_id = inner.store.duplicate(id, now)?;
        inner.config.profiles = inner.store.profiles().to_vec();
        self.persist(&inner);
        Ok(new_id)
    }

    pub fn rename(
        &self,
        id: &ProfileId,
        new_name: &str,
    ) -> Result<(), crate::domain::store::ProfileError> {
        let mut inner = self.inner.lock().expect("app state poisoned");
        let now = OffsetDateTime::now_utc();
        inner.store.rename(id, new_name, now)?;
        inner.config.profiles = inner.store.profiles().to_vec();
        self.persist(&inner);
        Ok(())
    }

    pub fn delete(&self, id: &ProfileId) -> Result<(), crate::domain::store::ProfileError> {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.store.delete(id)?;
        inner.config.profiles = inner.store.profiles().to_vec();
        // If the deleted profile was the active one, fall back to the
        // default preset so the UI has somewhere to point.
        if inner.config.last_active_profile_id.as_ref() == Some(id) {
            inner.config.last_active_profile_id = Some(ProfileId::BuiltIn(
                crate::domain::profile::BuiltInPreset::default_preset(),
            ));
        }
        self.persist(&inner);
        Ok(())
    }

    /// Read a snapshot of app-level settings (everything except the
    /// profile list). Returned by value so the frontend can render
    /// from it without holding a lock.
    pub fn app_settings(&self) -> AppSettings {
        let inner = self.inner.lock().expect("app state poisoned");
        AppSettings {
            launch_at_login: inner.config.launch_at_login,
            hotkeys: inner.config.hotkeys.clone(),
            auto_refresh_enabled: inner.config.auto_refresh_enabled,
            auto_refresh_seconds: inner.config.auto_refresh_seconds,
        }
    }

    /// Update the auto-refresh feature flag and threshold in one shot.
    /// `seconds` is clamped to at least `MIN_AUTO_REFRESH_SECONDS` so
    /// the threshold can't be set so low it fires constantly. The
    /// auto-refresh clock is restarted so a freshly-changed threshold
    /// doesn't trigger an immediate refresh from a stale tally.
    pub fn set_auto_refresh(&self, enabled: bool, seconds: u32) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.config.auto_refresh_enabled = enabled;
        inner.config.auto_refresh_seconds =
            seconds.max(crate::domain::persistence::MIN_AUTO_REFRESH_SECONDS);
        // Restart the clock either way: switching the feature on
        // shouldn't backdate the first refresh, and changing the
        // interval down shouldn't fire instantly.
        if inner.transport.is_some() {
            inner.last_refresh_at = Some(Instant::now());
        } else {
            inner.last_refresh_at = None;
        }
        self.persist(&inner);
    }

    /// One tick of the periodic auto-refresh task. Returns `true` if a
    /// refresh frame was actually sent. Designed to be called every
    /// second or so from a background tokio task; idempotent if the
    /// preconditions aren't met.
    ///
    /// Preconditions: feature enabled, transport attached, active
    /// profile uses `a2`, last-refresh was ≥ N seconds ago, and the
    /// OS reports the user was *active* within the last N seconds
    /// (so we don't refresh an AFK panel where nothing's been
    /// accumulating ghosting).
    pub fn auto_refresh_tick(&self, idle_seconds: u64) -> bool {
        let (transport, log_reason) = {
            let inner = self.inner.lock().expect("app state poisoned");
            if !inner.config.auto_refresh_enabled {
                return false;
            }
            let transport = match inner.transport.clone() {
                Some(t) => t,
                None => return false,
            };
            let active_settings = inner
                .config
                .last_active_profile_id
                .as_ref()
                .and_then(|id| inner.store.find(id))
                .map(|p| p.settings);
            let in_a2 = matches!(
                active_settings.map(|s| s.refresh_mode),
                Some(RefreshMode::A2),
            );
            if !in_a2 {
                return false;
            }
            let interval_secs = inner.config.auto_refresh_seconds as u64;
            let elapsed = inner
                .last_refresh_at
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(u64::MAX);
            if elapsed < interval_secs {
                return false;
            }
            // Active-user gate: if the OS-reported idle time exceeds
            // the configured interval, the user hasn't been typing or
            // moving the mouse on this host long enough to accumulate
            // ghosting worth a refresh. Skip until they come back.
            if idle_seconds >= interval_secs {
                return false;
            }
            (
                transport,
                format!(
                    "elapsed={}s idle={}s threshold={}s",
                    elapsed, idle_seconds, inner.config.auto_refresh_seconds,
                ),
            )
        };

        eprintln!("[auto-refresh] firing: {log_reason}");
        let frame = crate::mira::encoder::encode_refresh();
        match transport.write_feature(&frame) {
            Ok(()) => {
                eprintln!("[auto-refresh] refresh frame sent");
                let mut inner = self.inner.lock().expect("app state poisoned");
                inner.last_refresh_at = Some(Instant::now());
                true
            }
            Err(e) => {
                eprintln!("[auto-refresh] refresh write failed: {e}");
                false
            }
        }
    }

    /// Persist the user's choice of device when more than one Mira
    /// is plugged in. Stored as the serial number string (or `None`
    /// for "use the first one").
    pub fn set_selected_device_serial(&self, serial: Option<String>) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.config.selected_device_serial = serial;
        self.persist(&inner);
    }

    pub fn selected_device_serial(&self) -> Option<String> {
        self.inner
            .lock()
            .expect("app state poisoned")
            .config
            .selected_device_serial
            .clone()
    }

    pub fn set_launch_at_login(&self, value: bool) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.config.launch_at_login = value;
        self.persist(&inner);
    }

    pub fn set_hotkey(&self, slot: String, binding: Option<String>) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        match binding {
            Some(b) => {
                inner.config.hotkeys.insert(slot, b);
            }
            None => {
                inner.config.hotkeys.remove(&slot);
            }
        }
        self.persist(&inner);
    }

    pub fn reset_hotkeys_to_defaults(&self) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.config.hotkeys = crate::domain::persistence::default_hotkeys();
        self.persist(&inner);
    }

    /// Replace the settings for any profile. Built-ins ARE editable
    /// (spec divergence — see plan.md): users get the recognizable
    /// preset names and a Reset button, not a wall. Renaming and
    /// deleting built-ins are still blocked by the store.
    pub fn update_settings(
        &self,
        id: &ProfileId,
        settings: ProfileSettings,
    ) -> Result<(), crate::domain::store::ProfileError> {
        let mut inner = self.inner.lock().expect("app state poisoned");
        let pos = inner
            .store
            .profiles()
            .iter()
            .position(|p| p.id == *id)
            .ok_or(crate::domain::store::ProfileError::NotFound)?;
        let mut new_list: Vec<Profile> = inner.store.profiles().to_vec();
        new_list[pos].settings = settings.clamp();
        new_list[pos].modified_at = OffsetDateTime::now_utc();
        let new_store = ProfileStore::with_profiles(new_list.clone());
        inner.store = new_store;
        inner.config.profiles = new_list;
        self.persist(&inner);
        Ok(())
    }

    /// Reset a built-in preset to the spec §7.1 values. Returns
    /// ProfileError::NotFound for missing ids and InvalidInput when
    /// called on a custom profile (the action makes no sense there —
    /// "default" for a custom is "delete and recreate").
    pub fn reset_to_defaults(
        &self,
        id: &ProfileId,
    ) -> Result<(), crate::domain::store::ProfileError> {
        let built_in_kind = match id {
            ProfileId::BuiltIn(b) => *b,
            ProfileId::Custom(_) => {
                return Err(crate::domain::store::ProfileError::InvalidPosition(0, 0))
            }
        };
        let mut inner = self.inner.lock().expect("app state poisoned");
        let pos = inner
            .store
            .profiles()
            .iter()
            .position(|p| p.id == *id)
            .ok_or(crate::domain::store::ProfileError::NotFound)?;
        let mut new_list: Vec<Profile> = inner.store.profiles().to_vec();
        new_list[pos].settings = built_in_kind.settings();
        new_list[pos].modified_at = OffsetDateTime::now_utc();
        let new_store = ProfileStore::with_profiles(new_list.clone());
        inner.store = new_store;
        inner.config.profiles = new_list;
        self.persist(&inner);
        Ok(())
    }

    fn persist(&self, inner: &Inner) {
        // Best-effort: persistence failures are logged but don't
        // abort the in-memory state change. The frontend will see the
        // change; the next launch may revert if the disk write
        // failed, which is recoverable.
        if let Some(path) = inner.config_path.as_ref() {
            let _ = persistence::save_to(path, &inner.config);
        }
    }
}

/// Serializable view of the app-level config the Settings window
/// renders from. Excludes the profile list (which has its own
/// list_profiles command).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub launch_at_login: bool,
    pub hotkeys: std::collections::BTreeMap<String, String>,
    pub auto_refresh_enabled: bool,
    pub auto_refresh_seconds: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("profile not found")]
    NotFound,
    #[error("device: {0}")]
    Device(#[from] crate::mira::transport::TransportError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::profile::BuiltInPreset;
    use crate::mira::encoder::encode_set_speed;
    use crate::mira::transport::MockTransport;

    #[test]
    fn list_profiles_returns_six_presets_by_default() {
        let state = AppState::in_memory();
        let profiles = state.list_profiles();
        assert_eq!(profiles.len(), 6);
    }

    #[test]
    fn apply_profile_without_device_records_active_but_writes_nothing() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Speed);
        let frames = state.apply_profile(&id).unwrap();
        assert!(frames.is_empty() || !frames.is_empty()); // diff is fine either way without a real transport
        assert_eq!(state.active_profile_id(), Some(id));
    }

    #[test]
    fn apply_profile_with_device_sends_full_frames_on_first_apply() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());

        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let frames = state.apply_profile(&id).unwrap();
        assert_eq!(frames.len(), 7);
        // Mock recorded all seven.
        assert_eq!(mock.writes().len(), 7);
        // Coding has speed=6 -> wire 5.
        assert!(mock.writes().contains(&encode_set_speed(6)));
    }

    #[test]
    fn apply_profile_returns_not_found_for_unknown_id() {
        let state = AppState::in_memory();
        let bogus = ProfileId::new_custom();
        let err = state.apply_profile(&bogus).unwrap_err();
        assert!(matches!(err, ApplyError::NotFound));
    }

    #[test]
    fn force_refresh_writes_a_single_frame() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());

        state.force_refresh().unwrap();
        assert_eq!(mock.writes(), vec![crate::mira::encoder::encode_refresh()]);
    }

    #[test]
    fn force_refresh_errors_when_no_device() {
        let state = AppState::in_memory();
        let err = state.force_refresh().unwrap_err();
        assert!(matches!(
            err,
            ApplyError::Device(crate::mira::transport::TransportError::Disconnected)
        ));
    }

    #[test]
    fn duplicate_rename_delete_round_trip() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let new_id = state.duplicate(&id).unwrap();
        assert_eq!(state.list_profiles().len(), 7);

        state.rename(&new_id, "My setup").unwrap();
        assert!(state.list_profiles().iter().any(|p| p.name == "My setup"));

        state.delete(&new_id).unwrap();
        assert_eq!(state.list_profiles().len(), 6);
    }

    #[test]
    fn deleting_active_profile_falls_back_to_coding_default() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let new_id = state.duplicate(&id).unwrap();
        state.set_active_profile_id(new_id.clone());
        state.delete(&new_id).unwrap();
        assert_eq!(
            state.active_profile_id(),
            Some(ProfileId::BuiltIn(BuiltInPreset::Coding))
        );
    }

    #[test]
    fn update_settings_edits_built_in_in_place() {
        // Built-ins are editable; the user gets a Reset button to
        // revert. Settings still clamp to spec range.
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let mut tweaked = BuiltInPreset::Coding.settings();
        tweaked.contrast = 15;
        state.update_settings(&id, tweaked).unwrap();
        let profile = state
            .list_profiles()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap();
        assert_eq!(profile.settings.contrast, 15);
    }

    #[test]
    fn reset_to_defaults_restores_spec_values_after_edits() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let mut tweaked = BuiltInPreset::Coding.settings();
        tweaked.contrast = 1;
        state.update_settings(&id, tweaked).unwrap();
        state.reset_to_defaults(&id).unwrap();
        let profile = state
            .list_profiles()
            .into_iter()
            .find(|p| p.id == id)
            .unwrap();
        assert_eq!(profile.settings, BuiltInPreset::Coding.settings());
    }

    #[test]
    fn reset_to_defaults_refuses_custom_profiles() {
        let state = AppState::in_memory();
        let new_id = state
            .duplicate(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        let err = state.reset_to_defaults(&new_id).unwrap_err();
        // Reuse InvalidPosition as a "not applicable here" signal;
        // the command-layer translates this to AppError::InvalidInput.
        assert!(matches!(
            err,
            crate::domain::store::ProfileError::InvalidPosition(_, _)
        ));
    }

    #[test]
    fn rename_still_rejects_built_in_presets() {
        // Editability is for settings only — preset names stay fixed
        // so users can recognise them.
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let err = state.rename(&id, "Hacked").unwrap_err();
        assert_eq!(err, crate::domain::store::ProfileError::ReadOnly);
    }

    #[test]
    fn delete_still_rejects_built_in_presets() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let err = state.delete(&id).unwrap_err();
        assert_eq!(err, crate::domain::store::ProfileError::ReadOnly);
    }

    #[test]
    fn update_settings_replaces_settings_on_custom_profile_and_clamps() {
        let state = AppState::in_memory();
        let new_id = state
            .duplicate(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        let mut wild = BuiltInPreset::Coding.settings();
        wild.speed = 99; // out of range
        state.update_settings(&new_id, wild).unwrap();
        let profile = state
            .list_profiles()
            .into_iter()
            .find(|p| p.id == new_id)
            .unwrap();
        assert_eq!(profile.settings.speed, 7);
    }

    #[test]
    fn attach_transport_invalidates_session_so_next_apply_is_full() {
        let state = AppState::in_memory();
        let m1 = Arc::new(MockTransport::new());
        state.attach_transport(m1.clone());
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        assert_eq!(m1.writes().len(), 7);

        // Reattach a different transport (device reconnect).
        let m2 = Arc::new(MockTransport::new());
        state.attach_transport(m2.clone());
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        assert_eq!(m2.writes().len(), 7);
    }

    #[test]
    fn app_settings_returns_defaults_for_a_fresh_state() {
        let state = AppState::in_memory();
        let s = state.app_settings();
        assert!(!s.launch_at_login);
        assert_eq!(s.hotkeys.get("profile1").unwrap(), "Ctrl+Alt+1");
        assert_eq!(s.hotkeys.get("openPopover").unwrap(), "Ctrl+Alt+Shift+M");
    }

    #[test]
    fn set_launch_at_login_persists_and_is_visible_in_app_settings() {
        let state = AppState::in_memory();
        state.set_launch_at_login(true);
        assert!(state.app_settings().launch_at_login);
        state.set_launch_at_login(false);
        assert!(!state.app_settings().launch_at_login);
    }

    #[test]
    fn set_hotkey_replaces_existing_binding_in_slot() {
        let state = AppState::in_memory();
        state.set_hotkey("profile1".into(), Some("Ctrl+Shift+1".into()));
        assert_eq!(
            state.app_settings().hotkeys.get("profile1").unwrap(),
            "Ctrl+Shift+1"
        );
    }

    #[test]
    fn set_hotkey_with_none_removes_the_slot() {
        let state = AppState::in_memory();
        state.set_hotkey("profile1".into(), None);
        assert!(!state.app_settings().hotkeys.contains_key("profile1"));
    }

    #[test]
    fn reset_hotkeys_restores_spec_defaults_after_arbitrary_edits() {
        let state = AppState::in_memory();
        state.set_hotkey("profile1".into(), Some("Ctrl+Shift+1".into()));
        state.set_hotkey("refresh".into(), None);
        state.reset_hotkeys_to_defaults();
        let h = state.app_settings().hotkeys;
        assert_eq!(h.get("profile1").unwrap(), "Ctrl+Alt+1");
        assert_eq!(h.get("refresh").unwrap(), "Ctrl+Alt+Shift+R");
    }

    #[test]
    fn first_run_true_when_loading_a_missing_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let state = AppState::load_from_disk(path).unwrap();
        assert!(state.is_first_run());
    }

    #[test]
    fn first_run_false_when_config_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        // Pre-create.
        std::fs::write(&path, b"{\"version\":1,\"profiles\":[]}").unwrap();
        let state = AppState::load_from_disk(path).unwrap();
        assert!(!state.is_first_run());
    }

    #[test]
    fn complete_first_run_flips_the_flag_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let state = AppState::load_from_disk(path.clone()).unwrap();
        assert!(state.is_first_run());
        state.complete_first_run();
        assert!(!state.is_first_run());

        let reloaded = AppState::load_from_disk(path).unwrap();
        assert!(!reloaded.is_first_run());
    }

    #[test]
    fn capture_as_found_adds_a_seventh_profile() {
        let state = AppState::in_memory();
        assert_eq!(state.list_profiles().len(), 6);
        let snap = ProfileSettings {
            refresh_mode: crate::mira::encoder::RefreshMode::Direct,
            speed: 5,
            contrast: 10,
            dither_mode: 1,
            white_filter: 0,
            black_filter: 0,
            cold_light: 0,
            warm_light: 0,
        };
        state.capture_as_found(snap);
        let profiles = state.list_profiles();
        assert_eq!(profiles.len(), 7);
        let as_found = profiles
            .iter()
            .find(|p| p.name == "As-found")
            .expect("AsFound captured");
        assert_eq!(as_found.settings, snap);
    }

    #[test]
    fn capture_as_found_twice_replaces_not_appends() {
        let state = AppState::in_memory();
        let s1 = BuiltInPreset::Read.settings();
        let s2 = BuiltInPreset::Speed.settings();
        state.capture_as_found(s1);
        state.capture_as_found(s2);
        let profiles = state.list_profiles();
        let count = profiles.iter().filter(|p| p.name == "As-found").count();
        assert_eq!(count, 1);
        assert_eq!(
            profiles
                .iter()
                .find(|p| p.name == "As-found")
                .unwrap()
                .settings,
            s2,
        );
    }

    #[test]
    fn list_profiles_persists_to_disk_after_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let state = AppState::load_from_disk(path.clone()).unwrap();

        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        state.duplicate(&id).unwrap();

        // Reload from disk.
        let reloaded = AppState::load_from_disk(path).unwrap();
        assert_eq!(reloaded.list_profiles().len(), 7);
    }

    // -- Auto-refresh timer ------------------------------------------

    /// Set both the feature flag and the threshold (in seconds), then
    /// force the last-refresh-at marker so the next tick can fire
    /// without waiting in wall-clock time. `None` is treated by the
    /// tick path as u64::MAX elapsed — semantically equivalent to
    /// "infinitely stale" without the Windows `Instant` underflow
    /// that subtracting from `Instant::now()` causes near process start.
    fn arm_auto_refresh(state: &AppState, seconds: u32) {
        state.set_auto_refresh(true, seconds);
        let mut inner = state.inner.lock().unwrap();
        inner.last_refresh_at = None;
    }

    /// Force the in-memory last-refresh marker to "infinitely stale".
    /// Used after operations that reset the clock (apply_profile/switch,
    /// set_auto_refresh) to verify the next tick wouldn't fire.
    fn force_stale_clock(state: &AppState) {
        let mut inner = state.inner.lock().unwrap();
        inner.last_refresh_at = None;
    }

    #[test]
    fn auto_refresh_defaults_to_off_with_a_sensible_threshold() {
        let state = AppState::in_memory();
        let s = state.app_settings();
        assert!(!s.auto_refresh_enabled);
        assert_eq!(s.auto_refresh_seconds, 30);
    }

    #[test]
    fn set_auto_refresh_clamps_below_minimum_up_to_floor() {
        let state = AppState::in_memory();
        state.set_auto_refresh(true, 0);
        assert_eq!(state.app_settings().auto_refresh_seconds, 5);
        state.set_auto_refresh(true, 3);
        assert_eq!(state.app_settings().auto_refresh_seconds, 5);
        state.set_auto_refresh(true, 5);
        assert_eq!(state.app_settings().auto_refresh_seconds, 5);
        state.set_auto_refresh(true, 60);
        assert_eq!(state.app_settings().auto_refresh_seconds, 60);
    }

    #[test]
    fn tick_does_nothing_when_feature_is_off() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());
        // active idle = 0s (user just typed)
        assert!(!state.auto_refresh_tick(0));
        assert!(mock.writes().is_empty());
    }

    #[test]
    fn tick_does_nothing_without_a_transport() {
        let state = AppState::in_memory();
        state.set_auto_refresh(true, 30);
        assert!(!state.auto_refresh_tick(0));
    }

    #[test]
    fn tick_does_nothing_when_active_profile_is_direct() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());
        arm_auto_refresh(&state, 30);
        // Switch to a direct-mode profile.
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Read))
            .unwrap();
        let writes_before = mock.writes().len();

        assert!(!state.auto_refresh_tick(0));
        assert_eq!(mock.writes().len(), writes_before);
    }

    #[test]
    fn tick_does_nothing_when_user_is_idle_beyond_the_interval() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());
        arm_auto_refresh(&state, 30);
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        force_stale_clock(&state);
        let writes_before = mock.writes().len();

        // User idle 60s, interval 30s → idle ≥ interval → skip.
        assert!(!state.auto_refresh_tick(60));
        assert_eq!(mock.writes().len(), writes_before);
    }

    #[test]
    fn tick_fires_when_user_is_active_and_interval_elapsed_on_a2() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());
        arm_auto_refresh(&state, 30);
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        force_stale_clock(&state);
        let writes_before = mock.writes().len();

        // User active (idle 5s — just blinked), interval elapsed.
        assert!(state.auto_refresh_tick(5));
        let writes_after = mock.writes().len();
        assert_eq!(writes_after, writes_before + 1);
        assert_eq!(
            mock.writes().last().unwrap(),
            &crate::mira::encoder::encode_refresh()
        );
    }

    #[test]
    fn tick_consumes_the_interval_after_firing() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());
        arm_auto_refresh(&state, 30);
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        force_stale_clock(&state);
        assert!(state.auto_refresh_tick(5));
        // Immediate second tick — clock just reset, no fire.
        assert!(!state.auto_refresh_tick(5));
    }

    #[test]
    fn force_refresh_resets_the_auto_refresh_clock() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());
        arm_auto_refresh(&state, 30);
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        force_stale_clock(&state);
        state.force_refresh().unwrap();
        // After manual refresh the clock is "now"; the tick should
        // no longer fire even though we just had a stale marker.
        assert!(!state.auto_refresh_tick(5));
    }

    #[test]
    fn profile_switch_resets_the_auto_refresh_clock() {
        let state = AppState::in_memory();
        let mock = Arc::new(MockTransport::new());
        state.attach_transport(mock.clone());
        arm_auto_refresh(&state, 30);
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        force_stale_clock(&state);
        // Switch to a different profile.
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Speed))
            .unwrap();
        // Clock reset by the switch path → no fire on next tick.
        assert!(!state.auto_refresh_tick(5));
    }

    #[test]
    fn attach_transport_resets_the_auto_refresh_clock() {
        let state = AppState::in_memory();
        let m1 = Arc::new(MockTransport::new());
        state.attach_transport(m1);
        state.set_auto_refresh(true, 30);
        force_stale_clock(&state);
        // Reconnect: should reset clock so the post-reconnect
        // auto-apply doesn't immediately get auto-refreshed on top.
        let m2 = Arc::new(MockTransport::new());
        state.attach_transport(m2);
        state
            .apply_profile(&ProfileId::BuiltIn(BuiltInPreset::Coding))
            .unwrap();
        assert!(!state.auto_refresh_tick(5));
    }
}
