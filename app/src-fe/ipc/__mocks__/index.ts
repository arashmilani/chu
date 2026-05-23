// Test double for ipc/. Components import from "ipc/" via aliases or
// relative paths; vitest replaces this module via vi.mock in tests
// that need to script IPC responses.

import type {
  ApplyOutcome,
  DeviceStatus,
  Profile,
  ProfileId,
  ProfileSettings,
} from "../types";

let _profiles: Profile[] = [];
let _status: DeviceStatus = { connected: false };

export function __setProfiles(profiles: Profile[]) {
  _profiles = profiles;
}

export function __setStatus(status: DeviceStatus) {
  _status = status;
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
