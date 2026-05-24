pub mod commands;
pub mod domain;
pub mod hotkeys;
pub mod mira;
pub mod tray;

use std::sync::{Arc, Mutex};

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Shared, lazily-refreshable hidapi handle. Wrapping a single
/// instance in a Mutex avoids the macOS issue where two concurrent
/// `HidApi::new()` calls (watcher + tab-load) walk IOKit at the same
/// time and hang the process.
type SharedHidApi = Arc<Mutex<hidapi::HidApi>>;

use crate::commands::{AppError, AppState};
use crate::domain::hotkeys::{
    Binding, SLOT_PROFILE_1, SLOT_PROFILE_2, SLOT_PROFILE_3, SLOT_PROFILE_4, SLOT_PROFILE_5,
    SLOT_REFRESH,
};
use crate::domain::profile::{Profile, ProfileId, ProfileSettings};
use crate::hotkeys::{binding_to_shortcut, HotkeyManager};
use crate::tray::TrayState;

const WINDOW_SETTINGS: &str = "settings";
const TRAY_ID: &str = "chu-tray";
const PROFILE_MENU_PREFIX: &str = "profile:";

// -- Tauri commands --------------------------------------------------

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
    let _ = refresh_tray(&app);
    Ok(outcome)
}

#[tauri::command]
fn duplicate_profile(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    id: ProfileId,
) -> Result<ProfileId, AppError> {
    let new_id = commands::profiles::duplicate_profile(&state, id)?;
    let _ = refresh_tray(&app);
    Ok(new_id)
}

#[tauri::command]
fn rename_profile(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    id: ProfileId,
    new_name: String,
) -> Result<(), AppError> {
    commands::profiles::rename_profile(&state, id, new_name)?;
    let _ = refresh_tray(&app);
    Ok(())
}

#[tauri::command]
fn delete_profile(
    state: State<'_, Arc<AppState>>,
    app: AppHandle,
    id: ProfileId,
) -> Result<(), AppError> {
    commands::profiles::delete_profile(&state, id)?;
    let _ = refresh_tray(&app);
    Ok(())
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
fn reset_profile_to_defaults(
    state: State<'_, Arc<AppState>>,
    id: ProfileId,
) -> Result<(), AppError> {
    commands::profiles::reset_profile_to_defaults(&state, id)
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
fn get_active_profile_id(state: State<'_, Arc<AppState>>) -> Option<ProfileId> {
    state.active_profile_id()
}

#[tauri::command]
fn set_launch_at_login(state: State<'_, Arc<AppState>>, value: bool) {
    state.set_launch_at_login(value);
}

#[tauri::command]
fn set_auto_refresh(state: State<'_, Arc<AppState>>, enabled: bool, seconds: u32) {
    state.set_auto_refresh(enabled, seconds);
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

    if let Some(manager) = manager.as_ref() {
        if let Some(prev) = manager.take(&slot) {
            let _ = global.unregister(prev);
        }
    }

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

/// Pause all OS-level hotkey registrations so the frontend hotkey
/// recorder can actually receive key chords. While suspended, the
/// in-memory hotkey config (per-slot bindings) is untouched — only
/// the OS registration is dropped. `resume_hotkeys` re-registers
/// from the current config.
#[tauri::command]
fn suspend_hotkeys(app: AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    if let Some(manager) = app.try_state::<Arc<HotkeyManager>>() {
        for slot in manager.registered_slots() {
            manager.take(&slot);
        }
    }
    let _ = refresh_tray(&app);
}

/// Re-register OS-level hotkeys from the saved per-slot bindings.
/// Skips slots already in the manager (e.g. a binding committed via
/// `set_hotkey` while suspended). Best-effort: a slot that fails to
/// register is left out — the next attempt to set it surfaces the
/// failure to the user.
#[tauri::command]
fn resume_hotkeys(state: State<'_, Arc<AppState>>, app: AppHandle) {
    let bindings = state.app_settings().hotkeys;
    let global = app.global_shortcut();
    if let Some(manager) = app.try_state::<Arc<HotkeyManager>>() {
        let already: std::collections::HashSet<String> =
            manager.registered_slots().into_iter().collect();
        for (slot, text) in &bindings {
            if already.contains(slot) {
                continue;
            }
            if let Ok(parsed) = Binding::parse(text) {
                if let Some(shortcut) = binding_to_shortcut(&parsed) {
                    if global.register(shortcut).is_ok() {
                        manager.set(slot.clone(), shortcut);
                    }
                }
            }
        }
    }
    let _ = refresh_tray(&app);
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
    let _ = register_saved_shortcuts(&app);
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

/// List currently-connected Mira devices.
///
/// `async` so Tauri runs us on the async runtime; we then jump to a
/// blocking thread for the hidapi call because enumerating HID
/// devices on macOS can take tens of ms and we don't want to tie up
/// the IPC runtime.
#[tauri::command]
async fn list_devices(
    hid: State<'_, SharedHidApi>,
) -> Result<Vec<crate::mira::discovery::DeviceInfo>, AppError> {
    let hid = hid.inner().clone();
    let devices = tauri::async_runtime::spawn_blocking(move || -> Vec<_> {
        let mut api = match hid.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Err(e) = api.refresh_devices() {
            eprintln!("[mira] list_devices: refresh failed: {e}");
        }
        crate::mira::discovery::enumerate_mira(&api)
    })
    .await
    .map_err(|e| AppError::internal(format!("list_devices join: {e}")))?;
    Ok(devices)
}

#[tauri::command]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn udev_rule_text() -> &'static str {
    "SUBSYSTEM==\"hidraw\", ATTRS{idVendor}==\"0416\", ATTRS{idProduct}==\"5020\", \
     MODE=\"0660\", GROUP=\"plugdev\", TAG+=\"uaccess\"\n"
}

#[tauri::command]
fn udev_rule_present() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/etc/udev/rules.d/70-chu.rules").exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

#[tauri::command]
fn select_device(
    state: State<'_, Arc<AppState>>,
    hid: State<'_, SharedHidApi>,
    app: AppHandle,
    serial: Option<String>,
) -> Result<(), AppError> {
    state.set_selected_device_serial(serial);
    let _ = try_attach_selected_device(&state, &hid, &app);
    Ok(())
}

#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), AppError> {
    open_settings_window(&app).map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

// -- Internal helpers ------------------------------------------------

fn open_settings_window(app: &AppHandle) -> Result<(), tauri::Error> {
    if let Some(window) = app.get_webview_window(WINDOW_SETTINGS) {
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(
        app,
        WINDOW_SETTINGS,
        WebviewUrl::App("index.html?window=settings".into()),
    )
    .title("Chu — Settings")
    .inner_size(820.0, 600.0)
    .build()?;

    // Tray app: closing Settings hides the window, it doesn't quit
    // the process. The user explicitly quits via the tray menu.
    let hide_target = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hide_target.hide();
        }
    });

    Ok(())
}

fn try_attach_selected_device(state: &Arc<AppState>, hid: &SharedHidApi, app: &AppHandle) -> bool {
    let mut api = match hid.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
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
            let _ = refresh_tray(app);
        }
        return false;
    }

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
                // Push the user's last-active preset back onto the
                // device so reconnects (and cold starts) restore the
                // look they had before — no toggle, just the right
                // default. Best-effort: errors are logged, not fatal.
                if let Some(active) = state.active_profile_id() {
                    match state.apply_profile(&active) {
                        Ok(frames) => {
                            let _ = app.emit(
                                "profile:applied",
                                commands::profiles::ApplyOutcome {
                                    profile_id: active,
                                    frames_written: frames.len(),
                                },
                            );
                        }
                        Err(e) => {
                            eprintln!("[mira] auto-apply on connect failed: {e}");
                        }
                    }
                }
                let _ = refresh_tray(app);
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

fn spawn_device_watcher(state: Arc<AppState>, hid: SharedHidApi, app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            // hidapi I/O is blocking; jump off the runtime so the
            // IPC main thread stays responsive when this fires
            // concurrently with a command (e.g. the user opening the
            // Device tab).
            let state = state.clone();
            let hid = hid.clone();
            let app = app.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                try_attach_selected_device(&state, &hid, &app);
            })
            .await;
        }
    });
}

/// Periodic auto-refresh tick. Every second we ask the OS how long
/// since the last user input and let AppState decide whether to fire
/// a refresh. The threshold floor is 5 s, so a 1 s tick keeps the
/// fire-on-time error below 20%. The body is a single OS call plus
/// a brief AppState lock; negligible cost at this cadence.
fn spawn_auto_refresh_task(state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let state = state.clone();
            // user_idle::get() is a sync OS call; bounce off the
            // async runtime to keep the IPC thread free, even though
            // the call itself is cheap.
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let idle_seconds = match user_idle::UserIdle::get_time() {
                    Ok(t) => t.as_seconds(),
                    Err(e) => {
                        eprintln!("[auto-refresh] user-idle query failed: {e}");
                        // Treating "unknown idle" as "user is active"
                        // means we still fire on the schedule; failing
                        // closed would silently disable the feature.
                        0
                    }
                };
                state.auto_refresh_tick(idle_seconds);
            })
            .await;
        }
    });
}

// -- Tray ------------------------------------------------------------

fn profile_id_to_key(id: &ProfileId) -> String {
    serde_json::to_value(id)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

fn profile_id_from_key(s: &str) -> Option<ProfileId> {
    serde_json::from_value::<ProfileId>(serde_json::Value::String(s.to_string())).ok()
}

/// Build the tray menu from current state. Called at startup and
/// whenever profiles or device state change.
fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    let state_opt = app.try_state::<Arc<AppState>>();

    let (profiles, active_id, connected) = match state_opt {
        Some(state) => (
            state.list_profiles(),
            state.active_profile_id(),
            state.is_connected(),
        ),
        None => (Vec::new(), None, false),
    };

    for profile in &profiles {
        let id = format!("{PROFILE_MENU_PREFIX}{}", profile_id_to_key(&profile.id));
        let is_active = active_id.as_ref() == Some(&profile.id);
        let item = CheckMenuItem::with_id(app, &id, &profile.name, true, is_active, None::<&str>)?;
        menu.append(&item)?;
    }

    if !profiles.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }

    let refresh_item = MenuItem::with_id(app, "force_refresh", "Refresh", connected, None::<&str>)?;
    menu.append(&refresh_item)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let settings_item = MenuItem::with_id(app, "open_settings", "Settings…", true, None::<&str>)?;
    menu.append(&settings_item)?;

    let quit_item = MenuItem::with_id(app, "quit", "Quit Chu", true, None::<&str>)?;
    menu.append(&quit_item)?;

    Ok(menu)
}

fn tray_state(app: &AppHandle) -> TrayState {
    let connected = app
        .try_state::<Arc<AppState>>()
        .map(|s| s.is_connected())
        .unwrap_or(false);
    let hotkeys_ok = app
        .try_state::<Arc<HotkeyManager>>()
        .map(|m| !m.registered_slots().is_empty())
        .unwrap_or(true);
    TrayState {
        connected,
        hotkeys_ok,
    }
}

fn refresh_tray(app: &AppHandle) -> tauri::Result<()> {
    let tray = match app.tray_by_id(TRAY_ID) {
        Some(t) => t,
        None => return Ok(()),
    };
    let menu = build_tray_menu(app)?;
    tray.set_menu(Some(menu))?;
    tray.set_title(Some(tray_state(app).title()))?;
    Ok(())
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_tray_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .title(tray_state(app).title())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if let Some(key) = id.strip_prefix(PROFILE_MENU_PREFIX) {
                if let Some(profile_id) = profile_id_from_key(key) {
                    if let Some(state) = app.try_state::<Arc<AppState>>() {
                        match state.apply_profile(&profile_id) {
                            Ok(frames) => {
                                let _ = app.emit(
                                    "profile:applied",
                                    commands::profiles::ApplyOutcome {
                                        profile_id,
                                        frames_written: frames.len(),
                                    },
                                );
                            }
                            Err(e) => {
                                eprintln!("[mira] apply from tray failed: {e}");
                            }
                        }
                        let _ = refresh_tray(app);
                    }
                }
                return;
            }
            match id {
                "open_settings" => {
                    let _ = open_settings_window(app);
                }
                "force_refresh" => {
                    if let Some(state) = app.try_state::<Arc<AppState>>() {
                        let _ = state.force_refresh();
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .build(app)?;
    Ok(())
}

// -- Hotkeys ---------------------------------------------------------

/// Register the user's saved per-slot bindings with the OS. Used at
/// startup and after `reset_hotkeys` (which resets state to defaults
/// first, so this picks them up too). Iterating saved state — not
/// `default_bindings()` — is what makes custom bindings actually fire
/// after the app restarts: registration there used to use the spec
/// defaults, so a user-rebound chord persisted to disk and was shown
/// in Settings on restart, but the OS hotkey table still pointed at
/// the default chord and the custom one was inert until the user
/// re-saved it.
fn register_saved_shortcuts(app: &AppHandle) -> bool {
    let state = match app.try_state::<Arc<AppState>>() {
        Some(s) => s,
        None => return false,
    };
    let manager = match app.try_state::<Arc<HotkeyManager>>() {
        Some(m) => m,
        None => return false,
    };
    let global = app.global_shortcut();
    let bindings = state.app_settings().hotkeys;

    // Only slots `dispatch_hotkey` knows how to route. Anything else
    // (e.g. the retired `openPopover` slot still in legacy config
    // files) would register globally without doing anything, just
    // shadowing the chord for other apps.
    const KNOWN: &[&str] = &[
        SLOT_PROFILE_1,
        SLOT_PROFILE_2,
        SLOT_PROFILE_3,
        SLOT_PROFILE_4,
        SLOT_PROFILE_5,
        SLOT_REFRESH,
    ];

    let mut all_ok = true;
    for slot in KNOWN {
        let Some(text) = bindings.get(*slot) else {
            continue;
        };
        let parsed = match Binding::parse(text) {
            Ok(b) => b,
            Err(_) => {
                all_ok = false;
                continue;
            }
        };
        let shortcut = match binding_to_shortcut(&parsed) {
            Some(s) => s,
            None => {
                all_ok = false;
                continue;
            }
        };
        match global.register(shortcut) {
            Ok(()) => manager.set(*slot, shortcut),
            Err(_) => all_ok = false,
        }
    }
    let _ = refresh_tray(app);
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
        let _ = refresh_tray(app);
    }
}

// -- Entry point -----------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_path = domain::persistence::config_path().expect("resolve config path");
    let state = Arc::new(AppState::load_from_disk(config_path).expect("load config"));
    let hotkey_manager = Arc::new(HotkeyManager::new());
    let hid: SharedHidApi = Arc::new(Mutex::new(hidapi::HidApi::new().expect("hidapi init")));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler({
                    let manager = hotkey_manager.clone();
                    move |app, shortcut, event| {
                        if event.state() != ShortcutState::Pressed {
                            return;
                        }
                        if let Some(slot) = manager.slot_for_shortcut(shortcut) {
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
            let hid = hid.clone();
            move |app| {
                // Pin the macOS activation policy to Accessory so opening
                // a window (e.g. Settings) doesn't promote us back to a
                // regular app and add us to the Dock. LSUIElement only
                // sets the *initial* policy; AppKit can flip it the first
                // time a window activates.
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);

                app.manage(state.clone());
                app.manage(hotkey_manager.clone());
                app.manage(hid.clone());

                let handle = app.handle().clone();
                build_tray(&handle)?;
                let _ = register_saved_shortcuts(&handle);
                let _ = try_attach_selected_device(&state, &hid, &handle);
                spawn_device_watcher(state.clone(), hid.clone(), handle.clone());
                spawn_auto_refresh_task(state.clone());
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
            reset_profile_to_defaults,
            get_device_status,
            force_refresh,
            get_app_settings,
            get_active_profile_id,
            set_launch_at_login,
            set_auto_refresh,
            set_hotkey,
            reset_hotkeys,
            suspend_hotkeys,
            resume_hotkeys,
            app_version,
            is_first_run,
            complete_first_run,
            capture_as_found,
            list_devices,
            select_device,
            udev_rule_text,
            udev_rule_present,
            open_settings,
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
