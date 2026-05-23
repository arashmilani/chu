// Typed wrappers around tauri.invoke. The frontend never imports
// @tauri-apps/api directly — only through this module. That gives us
// one place to swap to mocks during tests (see __mocks__/index.ts).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ApplyOutcome,
  AppError,
  AppSettings,
  DeviceStatus,
  Profile,
  ProfileId,
  ProfileSettings,
} from "./types";

export async function listProfiles(): Promise<Profile[]> {
  return invoke<Profile[]>("list_profiles");
}

export async function applyProfile(id: ProfileId): Promise<ApplyOutcome> {
  return invoke<ApplyOutcome>("apply_profile", { id });
}

export async function duplicateProfile(id: ProfileId): Promise<ProfileId> {
  return invoke<ProfileId>("duplicate_profile", { id });
}

export async function renameProfile(
  id: ProfileId,
  newName: string,
): Promise<void> {
  await invoke<void>("rename_profile", { id, newName });
}

export async function deleteProfile(id: ProfileId): Promise<void> {
  await invoke<void>("delete_profile", { id });
}

export async function updateProfileSettings(
  id: ProfileId,
  settings: ProfileSettings,
): Promise<void> {
  await invoke<void>("update_profile_settings", { id, settings });
}

export async function getDeviceStatus(): Promise<DeviceStatus> {
  return invoke<DeviceStatus>("get_device_status");
}

export async function forceRefresh(): Promise<void> {
  await invoke<void>("force_refresh");
}

export async function getAppSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_app_settings");
}

export async function setApplyLastOnConnect(value: boolean): Promise<void> {
  await invoke<void>("set_apply_last_on_connect", { value });
}

export async function setLaunchAtLogin(value: boolean): Promise<void> {
  await invoke<void>("set_launch_at_login", { value });
}

export async function setHotkey(
  slot: string,
  binding: string | null,
): Promise<void> {
  await invoke<void>("set_hotkey", { slot, binding });
}

export async function resetHotkeys(): Promise<void> {
  await invoke<void>("reset_hotkeys");
}

export async function appVersion(): Promise<string> {
  return invoke<string>("app_version");
}

export function onProfileApplied(
  handler: (payload: { profileId: ProfileId }) => void,
): Promise<UnlistenFn> {
  return listen<{ profileId: ProfileId }>("profile:applied", (e) =>
    handler(e.payload),
  );
}

export function onDeviceConnected(
  handler: (payload: DeviceStatus) => void,
): Promise<UnlistenFn> {
  return listen<DeviceStatus>("device:connected", (e) => handler(e.payload));
}

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value
  );
}
