// Mirrors the serde shapes the Rust backend emits.
//
// Keep this in sync with src-tauri/src/{domain,commands}/. Drift here
// shows up at runtime, not compile time, so tests against the real
// invoke surface (Phase 7 onward) are how we catch it.

export type RefreshMode = "a2" | "direct";

export interface ProfileSettings {
  refreshMode: RefreshMode;
  speed: number;
  contrast: number;
  ditherMode: number;
  whiteFilter: number;
  blackFilter: number;
  coldLight: number;
  warmLight: number;
}

// Built-in presets serialize as bare kebab-case strings; custom
// profiles serialize as a UUID string. Both reach the frontend as a
// raw string — discriminated by whether it looks like a UUID.
export type ProfileId = string;

export interface Profile {
  id: ProfileId;
  name: string;
  builtIn: boolean;
  hotkey: string | null;
  settings: ProfileSettings;
  createdAt: string; // ISO 8601
  modifiedAt: string;
}

export interface ApplyOutcome {
  profileId: ProfileId;
  framesWritten: number;
}

export interface DeviceStatus {
  connected: boolean;
}

export type AppErrorKind =
  | "not-found"
  | "read-only"
  | "invalid-input"
  | "device-not-connected"
  | "device-nak"
  | "persistence-failed"
  | "internal";

export interface AppError {
  kind: AppErrorKind;
  message: string;
}

// Hard-coded id of the default preset (matches Rust BuiltInPreset::Coding).
export const DEFAULT_PROFILE_ID: ProfileId = "coding";
