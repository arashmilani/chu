pub mod commands;
pub mod domain;
pub mod mira;

use std::sync::Arc;

use tauri::{Manager, State};

use crate::commands::{AppError, AppState};
use crate::domain::profile::{Profile, ProfileId, ProfileSettings};

#[tauri::command]
fn list_profiles(state: State<'_, Arc<AppState>>) -> Vec<Profile> {
    commands::profiles::list_profiles(&state)
}

#[tauri::command]
fn apply_profile(
    state: State<'_, Arc<AppState>>,
    id: ProfileId,
) -> Result<commands::profiles::ApplyOutcome, AppError> {
    commands::profiles::apply_profile(&state, id)
}

#[tauri::command]
fn duplicate_profile(
    state: State<'_, Arc<AppState>>,
    id: ProfileId,
) -> Result<ProfileId, AppError> {
    commands::profiles::duplicate_profile(&state, id)
}

#[tauri::command]
fn rename_profile(
    state: State<'_, Arc<AppState>>,
    id: ProfileId,
    new_name: String,
) -> Result<(), AppError> {
    commands::profiles::rename_profile(&state, id, new_name)
}

#[tauri::command]
fn delete_profile(state: State<'_, Arc<AppState>>, id: ProfileId) -> Result<(), AppError> {
    commands::profiles::delete_profile(&state, id)
}

#[tauri::command]
fn update_profile_settings(
    state: State<'_, Arc<AppState>>,
    id: ProfileId,
    settings: ProfileSettings,
) -> Result<(), AppError> {
    commands::profiles::update_profile_settings(&state, id, settings)
}

#[tauri::command]
fn get_device_status(state: State<'_, Arc<AppState>>) -> commands::device::DeviceStatus {
    commands::device::get_device_status(&state)
}

#[tauri::command]
fn force_refresh(state: State<'_, Arc<AppState>>) -> Result<(), AppError> {
    commands::device::force_refresh(&state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = domain::persistence::config_path().expect("resolve config path");
    let state = Arc::new(AppState::load_from_disk(config_path).expect("load config"));

    tauri::Builder::default()
        .setup({
            let state = state.clone();
            move |app| {
                app.manage(state);
                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            apply_profile,
            duplicate_profile,
            rename_profile,
            delete_profile,
            update_profile_settings,
            get_device_status,
            force_refresh,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
