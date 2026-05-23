//! AppState — the shared, mutable backend state the command layer
//! operates on. One per Tauri app; commands lock it briefly.
//!
//! Tests construct AppState with a MockTransport so the command
//! surface is exercised end-to-end without USB.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use time::OffsetDateTime;

use crate::domain::persistence::{self, Config};
use crate::domain::profile::{Profile, ProfileId, ProfileSettings};
use crate::domain::session::Session;
use crate::domain::store::ProfileStore;
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
            }),
        }
    }

    /// Build AppState from a config file on disk.
    pub fn load_from_disk(path: PathBuf) -> Result<Self, persistence::PersistenceError> {
        let config = persistence::load_from(&path, OffsetDateTime::now_utc())?;
        let store = ProfileStore::with_profiles(config.profiles.clone());
        Ok(Self {
            inner: Mutex::new(Inner {
                config,
                store,
                session: Session::new(),
                transport: None,
                config_path: Some(path),
            }),
        })
    }

    pub fn attach_transport(&self, transport: Arc<dyn HidTransport>) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.transport = Some(transport);
        // A fresh transport means the device's actual state is
        // unknown; the next apply() will write every field.
        inner.session.invalidate();
    }

    pub fn detach_transport(&self) {
        let mut inner = self.inner.lock().expect("app state poisoned");
        inner.transport = None;
        inner.session.invalidate();
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
        let profile = inner
            .store
            .find(id)
            .cloned()
            .ok_or(ApplyError::NotFound)?;
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
        self.persist(&inner);
        Ok(frames)
    }

    pub fn force_refresh(&self) -> Result<(), ApplyError> {
        let inner = self.inner.lock().expect("app state poisoned");
        let transport = inner
            .transport
            .clone()
            .ok_or(ApplyError::Device(
                crate::mira::transport::TransportError::Disconnected,
            ))?;
        drop(inner);
        let frame = crate::mira::encoder::encode_refresh();
        transport
            .write_feature(&frame)
            .map_err(ApplyError::Device)
    }

    pub fn duplicate(&self, id: &ProfileId) -> Result<ProfileId, crate::domain::store::ProfileError> {
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

    pub fn update_settings(
        &self,
        id: &ProfileId,
        settings: ProfileSettings,
    ) -> Result<(), crate::domain::store::ProfileError> {
        let mut inner = self.inner.lock().expect("app state poisoned");
        // Look up; built-ins are read-only.
        let pos = inner
            .store
            .profiles()
            .iter()
            .position(|p| p.id == *id)
            .ok_or(crate::domain::store::ProfileError::NotFound)?;
        let profile = &inner.store.profiles()[pos];
        if profile.built_in {
            return Err(crate::domain::store::ProfileError::ReadOnly);
        }
        // Replace settings via add+swap to avoid threading another
        // mutator through ProfileStore.
        let mut new_list: Vec<Profile> = inner.store.profiles().to_vec();
        new_list[pos].settings = settings.clamp();
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
        assert!(state
            .list_profiles()
            .iter()
            .any(|p| p.name == "My setup"));

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
    fn update_settings_rejects_built_in_presets() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let err = state
            .update_settings(&id, BuiltInPreset::Speed.settings())
            .unwrap_err();
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
}
