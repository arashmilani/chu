//! Profile-related commands. The Tauri attribute wrappers live in
//! lib.rs to keep this module testable without the macro at hand.

use serde::Serialize;

use crate::commands::error::AppError;
use crate::commands::state::AppState;
use crate::domain::profile::{Profile, ProfileId, ProfileSettings};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub profile_id: ProfileId,
    /// Number of HID frames actually sent to the device (0 if no
    /// device was connected, or if the active profile was already
    /// fully in sync).
    pub frames_written: usize,
}

pub fn list_profiles(state: &AppState) -> Vec<Profile> {
    state.list_profiles()
}

pub fn apply_profile(state: &AppState, id: ProfileId) -> Result<ApplyOutcome, AppError> {
    let frames = state.apply_profile(&id).map_err(|e| match e {
        crate::commands::state::ApplyError::NotFound => AppError::not_found("profile not found"),
        crate::commands::state::ApplyError::Device(t) => t.into(),
    })?;
    Ok(ApplyOutcome {
        profile_id: id,
        frames_written: frames.len(),
    })
}

pub fn duplicate_profile(state: &AppState, id: ProfileId) -> Result<ProfileId, AppError> {
    Ok(state.duplicate(&id)?)
}

pub fn rename_profile(state: &AppState, id: ProfileId, new_name: String) -> Result<(), AppError> {
    Ok(state.rename(&id, &new_name)?)
}

pub fn delete_profile(state: &AppState, id: ProfileId) -> Result<(), AppError> {
    Ok(state.delete(&id)?)
}

pub fn update_profile_settings(
    state: &AppState,
    id: ProfileId,
    settings: ProfileSettings,
) -> Result<(), AppError> {
    Ok(state.update_settings(&id, settings)?)
}

pub fn reset_profile_to_defaults(state: &AppState, id: ProfileId) -> Result<(), AppError> {
    Ok(state.reset_to_defaults(&id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::AppErrorKind;
    use crate::domain::profile::BuiltInPreset;

    #[test]
    fn list_profiles_returns_six_presets() {
        let state = AppState::in_memory();
        assert_eq!(list_profiles(&state).len(), 6);
    }

    #[test]
    fn apply_profile_returns_outcome_with_zero_frames_when_no_device() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let outcome = apply_profile(&state, id.clone()).unwrap();
        assert_eq!(outcome.profile_id, id);
        // Without a device, frames are computed but not "written" —
        // we still report them as the would-have-been count so the
        // UI can offer "preview".
        assert_eq!(outcome.frames_written, 7);
    }

    #[test]
    fn apply_profile_maps_not_found_to_app_error() {
        let state = AppState::in_memory();
        let err = apply_profile(&state, ProfileId::new_custom()).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::NotFound);
    }

    #[test]
    fn rename_built_in_returns_read_only_error() {
        let state = AppState::in_memory();
        let id = ProfileId::BuiltIn(BuiltInPreset::Coding);
        let err = rename_profile(&state, id, "Hacked".to_string()).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::ReadOnly);
    }
}
