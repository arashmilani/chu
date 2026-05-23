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
    default_bindings, Binding, SLOT_OPEN_POPOVER, SLOT_PROFILE_1, SLOT_PROFILE_2, SLOT_PROFILE_3,
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
fn get_app_settings(state: State<'_, Arc<AppState>>) -> commands::state::AppSettings {
    state.app_settings()
}

#[tauri::command]
fn set_apply_last_on_connect(state: State<'_, Arc<AppState>>, value: bool) {
    state.set_apply_last_on_connect(value);
}

#[tauri::command]
fn set_launch_at_login(state: State<'_, Arc<AppState>>, value: bool) {
    state.set_launch_at_login(value);
}

#[tauri::command]
fn set_hotkey(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    slot: String,
    binding: Option<String>,
) -> Result<(), AppError> {
    let manager = app.try_state::<Arc<HotkeyManager>>();
    let global = app.global_shortcut();

    // Unregister the previous chord (if any) for this slot.
    if let Some(manager) = manager.as_ref() {
        if let Some(prev) = manager.take(&slot) {
            let _ = global.unregister(prev);
        }
    }

    // Register the new chord (if provided + parseable).
    if let Some(text) = binding.as_deref() {
        let parsed = Binding::parse(text)
            .map_err(|e| AppError::invalid_input(format!("hotkey parse: {e}")))?;
        if let Some(shortcut) = binding_to_shortcut(&parsed) {
            global
                .register(shortcut)
                .map_err(|e| AppError::invalid_input(format!("hotkey register: {e}")))?;
            if let Some(manager) = manager.as_ref() {
                manager.set(&slot, shortcut);
            }
        }
    }

    state.set_hotkey(slot, binding);
    Ok(())
}

#[tauri::command]
fn reset_hotkeys(state: State<'_, Arc<AppState>>, app: AppHandle) {
    let global = app.global_shortcut();
    let _ = global.unregister_all();
    state.reset_hotkeys_to_defaults();
    if let Some(manager) = app.try_state::<Arc<HotkeyManager>>() {
        for slot in manager.registered_slots() {
            manager.take(&slot);
        }
    }
    let _ = register_default_shortcuts(&app);
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn is_first_run(state: State<'_, Arc<AppState>>) -> bool {
    state.is_first_run()
}

#[tauri::command]
fn complete_first_run(state: State<'_, Arc<AppState>>) {
    state.complete_first_run();
}

#[tauri::command]
fn capture_as_found(state: State<'_, Arc<AppState>>, snapshot: ProfileSettings) {
    state.capture_as_found(snapshot);
}

#[tauri::command]
fn list_devices() -> Vec<crate::mira::discovery::DeviceInfo> {
    match hidapi::HidApi::new() {
        Ok(api) => crate::mira::discovery::enumerate_mira(&api),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn udev_rule_text() -> &'static str {
    // The exact rule from spec §12.2.
    "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"0416\", ATTRS{idProduct}==\"5020\", \
     MODE=\"0660\", GROUP=\"plugdev\", TAG+=\"uaccess\"\n"
}

#[tauri::command]
fn udev_rule_present() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/etc/udev/rules.d/70-mira.rules").exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

#[tauri::command]
fn select_device(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    serial: Option<String>,
) -> Result<(), AppError> {
    state.set_selected_device_serial(serial);
    // Re-attach the matching transport, if any.
    let _ = try_attach_selected_device(&state, &app);
    Ok(())
}

fn try_attach_selected_device(state: &Arc<AppState>, app: &AppHandle) -> bool {
    let mut api = match hidapi::HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[mira] hidapi init failed: {e}");
            return false;
        }
    };
    // hidapi caches the enumeration on construction; without an
    // explicit refresh, we won't see devices that were plugged in
    // after the process started.
    if let Err(e) = api.refresh_devices() {
        eprintln!("[mira] hidapi refresh_devices failed: {e}");
    }
    let devices = crate::mira::discovery::enumerate_mira(&api);
    if devices.is_empty() {
        let was_connected = state.is_connected();
        state.detach_transport();
        if was_connected {
            let _ = app.emit(
                "device:disconnected",
                commands::device::DeviceStatus { connected: false },
            );
            eprintln!("[mira] no Mira devices found on bus (was connected, now disconnected)");
        }
        return false;
    }

    // Honour the user's selection if it matches a present device.
    let selected_serial = state.selected_device_serial();
    let target = if let Some(serial) = selected_serial.as_deref() {
        devices
            .iter()
            .find(|d| d.serial_number.as_deref() == Some(serial))
            .or_else(|| devices.first())
            .cloned()
    } else {
        devices.first().cloned()
    };

    if let Some(picked) = target {
        // If we're already connected, don't churn — just confirm.
        if state.is_connected() {
            return true;
        }
        match crate::mira::transport::HidApiTransport::open(
            &api,
            picked.vendor_id,
            picked.product_id,
        ) {
            Ok(transport) => {
                state.attach_transport(Arc::new(transport));
                let _ = app.emit(
                    "device:connected",
                    commands::device::DeviceStatus { connected: true },
                );
                eprintln!(
                    "[mira] attached Mira VID=0x{:04x} PID=0x{:04x} serial={:?}",
                    picked.vendor_id, picked.product_id, picked.serial_number
                );
                if devices.len() > 1 {
                    let _ = app.emit("device:multi-detected", &devices);
                }
                true
            }
            Err(e) => {
                eprintln!(
                    "[mira] failed to open Mira VID=0x{:04x} PID=0x{:04x}: {e}",
                    picked.vendor_id, picked.product_id
                );
                false
            }
        }
    } else {
        false
    }
}

fn spawn_device_watcher(state: Arc<AppState>, app: AppHandle) {
    // Periodic re-enumeration so hotplug works. 2s is short enough to
    // feel responsive and long enough not to thrash hidapi (which on
    // macOS re-walks IOKit on every call).
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let _ = try_attach_selected_device(&state, &app);
        }
    });
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
                let _ = try_attach_selected_device(&state, &handle);
                spawn_device_watcher(state.clone(), handle.clone());
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
            get_app_settings,
            set_apply_last_on_connect,
            set_launch_at_login,
            set_hotkey,
            reset_hotkeys,
            app_version,
            is_first_run,
            complete_first_run,
            capture_as_found,
            list_devices,
            select_device,
            udev_rule_text,
            udev_rule_present,
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
