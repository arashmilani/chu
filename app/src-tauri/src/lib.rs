pub mod commands;
pub mod domain;
pub mod hotkeys;
pub mod mira;
pub mod tray;

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::commands::{AppError, AppState};
use crate::domain::hotkeys::{
    default_bindings, SLOT_OPEN_POPOVER, SLOT_PROFILE_1, SLOT_PROFILE_2, SLOT_PROFILE_3,
    SLOT_PROFILE_4, SLOT_PROFILE_5, SLOT_REFRESH,
};
use crate::domain::profile::{Profile, ProfileId, ProfileSettings};
use crate::hotkeys::{binding_to_shortcut, HotkeyManager};
use crate::tray::TrayState;

const WINDOW_POPOVER: &str = "popover";
const WINDOW_EDITOR: &str = "editor";
const WINDOW_SETTINGS: &str = "settings";

#[tauri::command]
fn list_profiles(state: State<'_, Arc<AppState>>) -> Vec<Profile> {
    commands::profiles::list_profiles(&state)
}

#[tauri::command]
fn apply_profile(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    id: ProfileId,
) -> Result<commands::profiles::ApplyOutcome, AppError> {
    let outcome = commands::profiles::apply_profile(&state, id.clone())?;
    let _ = app.emit("profile:applied", &outcome);
    Ok(outcome)
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

#[tauri::command]
fn open_editor(app: AppHandle) -> Result<(), AppError> {
    show_or_create(&app, WINDOW_EDITOR).map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), AppError> {
    show_or_create(&app, WINDOW_SETTINGS).map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
fn close_popover(app: AppHandle) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window(WINDOW_POPOVER) {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn show_or_create(app: &AppHandle, label: &str) -> Result<(), tauri::Error> {
    if let Some(window) = app.get_webview_window(label) {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let url = WebviewUrl::App(format!("index.html?window={label}").into());
    let builder = match label {
        WINDOW_POPOVER => WebviewWindowBuilder::new(app, label, url)
            .title("Mira")
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .inner_size(360.0, 480.0)
            .visible(false),
        WINDOW_EDITOR => WebviewWindowBuilder::new(app, label, url)
            .title("Mira — Profile editor")
            .inner_size(900.0, 600.0),
        WINDOW_SETTINGS => WebviewWindowBuilder::new(app, label, url)
            .title("Mira — Settings")
            .inner_size(640.0, 480.0)
            .resizable(false),
        _ => WebviewWindowBuilder::new(app, label, url),
    };
    builder.build()?;
    Ok(())
}

fn toggle_popover(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW_POPOVER) {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    } else {
        let _ = show_or_create(app, WINDOW_POPOVER);
        if let Some(window) = app.get_webview_window(WINDOW_POPOVER) {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_editor_item = MenuItem::with_id(
        app,
        "open_editor",
        "Open profile editor…",
        true,
        None::<&str>,
    )?;
    let open_settings_item =
        MenuItem::with_id(app, "open_settings", "Settings…", true, None::<&str>)?;
    let refresh_item = MenuItem::with_id(
        app,
        "force_refresh",
        "Force full refresh",
        true,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Mira", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open_editor_item,
            &open_settings_item,
            &refresh_item,
            &separator,
            &quit_item,
        ],
    )?;

    let initial_title = TrayState {
        connected: false,
        hotkeys_ok: true,
    }
    .title();

    TrayIconBuilder::with_id("mira-tray")
        .title(initial_title)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open_editor" => {
                let _ = show_or_create(app, WINDOW_EDITOR);
            }
            "open_settings" => {
                let _ = show_or_create(app, WINDOW_SETTINGS);
            }
            "force_refresh" => {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    let _ = state.force_refresh();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_popover(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn register_default_shortcuts(app: &AppHandle) -> bool {
    let manager = match app.try_state::<Arc<HotkeyManager>>() {
        Some(m) => m,
        None => return false,
    };
    let global = app.global_shortcut();

    let mut all_ok = true;
    for (slot, binding) in default_bindings() {
        match binding_to_shortcut(&binding) {
            Some(shortcut) => match global.register(shortcut) {
                Ok(()) => manager.set(slot, shortcut),
                Err(_) => all_ok = false,
            },
            None => all_ok = false,
        }
    }
    all_ok
}

fn dispatch_hotkey(app: &AppHandle, slot: &str) {
    let state = match app.try_state::<Arc<AppState>>() {
        Some(s) => s,
        None => return,
    };
    let profiles = state.list_profiles();
    let target_id = match slot {
        SLOT_PROFILE_1 => profiles.first().map(|p| p.id.clone()),
        SLOT_PROFILE_2 => profiles.get(1).map(|p| p.id.clone()),
        SLOT_PROFILE_3 => profiles.get(2).map(|p| p.id.clone()),
        SLOT_PROFILE_4 => profiles.get(3).map(|p| p.id.clone()),
        SLOT_PROFILE_5 => profiles.get(4).map(|p| p.id.clone()),
        SLOT_REFRESH => {
            let _ = state.force_refresh();
            return;
        }
        SLOT_OPEN_POPOVER => {
            toggle_popover(app);
            return;
        }
        _ => None,
    };
    if let Some(id) = target_id {
        let _ = state.apply_profile(&id);
        let _ = app.emit(
            "profile:applied",
            commands::profiles::ApplyOutcome {
                profile_id: id,
                frames_written: 0,
            },
        );
    }
}

fn slot_for_shortcut(
    manager: &HotkeyManager,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
) -> Option<String> {
    let snapshot = manager
        .registered_slots()
        .into_iter()
        .filter_map(|slot| {
            // We need to look up the shortcut for each slot. The manager
            // doesn't expose this directly, so duplicate the default
            // mapping here — fine for v1 since rebinds also update
            // through the same code path.
            default_bindings()
                .into_iter()
                .find(|(s, _)| *s == slot)
                .and_then(|(_, b)| binding_to_shortcut(&b))
                .map(|s| (slot, s))
        })
        .collect::<Vec<_>>();
    snapshot
        .into_iter()
        .find(|(_, s)| s == shortcut)
        .map(|(slot, _)| slot)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = domain::persistence::config_path().expect("resolve config path");
    let state = Arc::new(AppState::load_from_disk(config_path).expect("load config"));
    let hotkey_manager = Arc::new(HotkeyManager::new());

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler({
                    let manager = hotkey_manager.clone();
                    move |app, shortcut, event| {
                        if event.state() != ShortcutState::Pressed {
                            return;
                        }
                        if let Some(slot) = slot_for_shortcut(&manager, shortcut) {
                            dispatch_hotkey(app, &slot);
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .setup({
            let state = state.clone();
            let hotkey_manager = hotkey_manager.clone();
            move |app| {
                app.manage(state.clone());
                app.manage(hotkey_manager.clone());

                let handle = app.handle().clone();
                build_tray(&handle)?;
                let _ = register_default_shortcuts(&handle);
                // Bootstrap: show the popover once on launch so the
                // user sees the app actually started. After first
                // hide it lives in the tray.
                let _ = show_or_create(&handle, WINDOW_POPOVER);
                if let Some(window) = handle.get_webview_window(WINDOW_POPOVER) {
                    let _ = window.show();
                }
                Ok(())
            }
        })
        .on_window_event(|window, event| {
            // Hide popover on focus loss / close request so it
            // behaves like a real tray popover.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == WINDOW_POPOVER {
                    api.prevent_close();
                    let _ = window.hide();
                }
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
            open_editor,
            open_settings,
            close_popover,
            quit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::ExitRequested { .. } = event {
                let _ = app.global_shortcut().unregister_all();
            }
        });
}
