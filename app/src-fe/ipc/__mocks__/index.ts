// Test double for ipc/. Components import from "ipc/" via aliases or
// relative paths; vitest replaces this module via vi.mock in tests
// that need to script IPC responses.

import type {
  ApplyOutcome,
  AppSettings,
  DeviceStatus,
  Profile,
  ProfileId,
  ProfileSettings,
} from "../types";

let _profiles: Profile[] = [];
let _status: DeviceStatus = { connected: false };
let _settings: AppSettings = {
  applyLastProfileOnConnect: true,
  launchAtLogin: false,
  hotkeys: {
    profile1: "Ctrl+Alt+1",
    profile2: "Ctrl+Alt+2",
    profile3: "Ctrl+Alt+3",
    profile4: "Ctrl+Alt+4",
    profile5: "Ctrl+Alt+5",
    refresh: "Ctrl+Alt+Shift+R",
    openPopover: "Ctrl+Alt+Shift+M",
  },
};

export function __setProfiles(profiles: Profile[]) {
  _profiles = profiles;
}

export function __setStatus(status: DeviceStatus) {
  _status = status;
}

export function __setAppSettings(settings: AppSettings) {
  _settings = settings;
}

export async function listProfiles(): Promise<Profile[]> {
  return _profiles;
}

export async function applyProfile(id: ProfileId): Promise<ApplyOutcome> {
  return { profileId: id, framesWritten: 0 };
}

export async function duplicateProfile(id: ProfileId): Promise<ProfileId> {
  return id + "-copy";
}

export async function renameProfile(): Promise<void> {}
export async function deleteProfile(): Promise<void> {}
export async function updateProfileSettings(
  _id: ProfileId,
  _settings: ProfileSettings,
): Promise<void> {
  // no-op
  void _id;
  void _settings;
}

export async function getDeviceStatus(): Promise<DeviceStatus> {
  return _status;
}

export async function forceRefresh(): Promise<void> {}

export async function getAppSettings(): Promise<AppSettings> {
  return _settings;
}

export async function setApplyLastOnConnect(value: boolean): Promise<void> {
  _settings = { ..._settings, applyLastProfileOnConnect: value };
}

export async function setLaunchAtLogin(value: boolean): Promise<void> {
  _settings = { ..._settings, launchAtLogin: value };
}

export async function setHotkey(
  slot: string,
  binding: string | null,
): Promise<void> {
  const next = { ..._settings.hotkeys };
  if (binding === null) delete next[slot];
  else next[slot] = binding;
  _settings = { ..._settings, hotkeys: next };
}

export async function resetHotkeys(): Promise<void> {}

export async function appVersion(): Promise<string> {
  return "0.1.0-test";
}

export async function onProfileApplied() {
  return () => {};
}

export async function onDeviceConnected() {
  return () => {};
}

export function isAppError(value: unknown) {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value
  );
}
