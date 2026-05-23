import { useEffect, useRef, useState } from "react";

import { HotkeyRecorder } from "../components/HotkeyRecorder";
import { SettingsForm } from "../components/SettingsForm";
import {
  applyProfile,
  appVersion,
  deleteProfile,
  duplicateProfile,
  getAppSettings,
  listProfiles,
  renameProfile,
  resetHotkeys,
  resetProfileToDefaults,
  setHotkey,
  setLaunchAtLogin,
  updateProfileSettings,
} from "../ipc";
import type {
  AppSettings,
  Profile,
  ProfileId,
  ProfileSettings,
} from "../ipc/types";
import { HOTKEY_SLOTS } from "../ipc/types";

type Tab = "Profiles" | "General" | "Hotkeys" | "About";
const TABS: Tab[] = ["Profiles", "General", "Hotkeys", "About"];

interface SettingsProps {
  /** Test/preview hook — skip the IPC bootstrap. */
  initialAppSettings?: AppSettings;
  initialProfiles?: Profile[];
  initialVersion?: string;
}

export function Settings({
  initialAppSettings,
  initialProfiles,
  initialVersion,
}: SettingsProps = {}) {
  const [tab, setTab] = useState<Tab>("Profiles");
  const [settings, setSettings] = useState<AppSettings | null>(
    initialAppSettings ?? null,
  );
  const [profiles, setProfiles] = useState<Profile[]>(initialProfiles ?? []);
  const [version, setVersion] = useState<string>(initialVersion ?? "");

  useEffect(() => {
    if (initialAppSettings === undefined) {
      getAppSettings()
        .then(setSettings)
        .catch(() => {});
    }
    if (initialProfiles === undefined) {
      listProfiles()
        .then(setProfiles)
        .catch(() => {});
    }
    if (initialVersion === undefined) {
      appVersion()
        .then(setVersion)
        .catch(() => {});
    }
  }, [initialAppSettings, initialProfiles, initialVersion]);

  async function refreshProfiles() {
    try {
      const ps = await listProfiles();
      setProfiles(ps);
    } catch {
      // ignore
    }
  }

  // Optimistic patch for slider edits: the IPC is fired-and-forgotten
  // because the loop fires per-input event and a refresh-from-disk
  // would remount the editor mid-drag. Without this, switching chips
  // and switching back surfaces stale settings from the last
  // refresh, even though the backend has the edit.
  function patchProfileSettings(id: ProfileId, settings: ProfileSettings) {
    setProfiles((prev) =>
      prev.map((p) => (p.id === id ? { ...p, settings } : p)),
    );
  }

  return (
    <div className="settings">
      <nav
        aria-label="Settings sections"
        role="tablist"
        className="settings__tabs"
      >
        {TABS.map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            className="btn"
            aria-selected={t === tab}
            data-active={t === tab}
            onClick={() => setTab(t)}
          >
            {t}
          </button>
        ))}
      </nav>

      <section
        role="tabpanel"
        aria-label={`${tab} settings`}
        className="settings__pane"
      >
        <h1>{tab}</h1>

        {tab === "Profiles" && (
          <ProfilesPane
            profiles={profiles}
            onChange={refreshProfiles}
            onPatchProfileSettings={patchProfileSettings}
          />
        )}
        {tab === "General" && settings && (
          <GeneralPane settings={settings} onChange={setSettings} />
        )}
        {tab === "Hotkeys" && settings && (
          <HotkeysPane settings={settings} onChange={setSettings} />
        )}
        {tab === "About" && <AboutPane version={version} />}
        {tab !== "Profiles" && tab !== "About" && !settings && <p>Loading…</p>}
      </section>
    </div>
  );
}

// -- Profiles tab ---------------------------------------------------

interface ProfilesPaneProps {
  profiles: Profile[];
  onChange: () => Promise<void> | void;
  onPatchProfileSettings: (id: ProfileId, settings: ProfileSettings) => void;
}

function ProfilesPane({
  profiles,
  onChange,
  onPatchProfileSettings,
}: ProfilesPaneProps) {
  const [selectedId, setSelectedId] = useState<string | null>(
    profiles[0]?.id ?? null,
  );

  // Derive a valid selection: if the user's pick is gone (deletion,
  // first load), show the first profile. We don't write back into
  // state here — the next user click does that. Keeps the effect
  // purely a "user picked" channel.
  const activeId =
    selectedId && profiles.some((p) => p.id === selectedId)
      ? selectedId
      : (profiles[0]?.id ?? null);
  const selected = profiles.find((p) => p.id === activeId);

  async function onDuplicate() {
    if (!selected) return;
    try {
      const newId = await duplicateProfile(selected.id);
      await onChange();
      setSelectedId(newId);
    } catch {
      // ignore
    }
  }

  async function onDelete() {
    if (!selected || selected.builtIn) return;
    try {
      await deleteProfile(selected.id);
      await onChange();
    } catch {
      // ignore
    }
  }

  async function onReset() {
    if (!selected || !selected.builtIn) return;
    try {
      await resetProfileToDefaults(selected.id);
      await onChange();
    } catch {
      // ignore
    }
  }

  if (profiles.length === 0) {
    return <p>Loading profiles…</p>;
  }

  return (
    <>
      <nav aria-label="Profile picker" className="profile-chips">
        {profiles.map((p) => (
          <button
            key={p.id}
            type="button"
            className="btn"
            data-active={p.id === activeId}
            onClick={() => setSelectedId(p.id)}
          >
            {p.name}
          </button>
        ))}
      </nav>

      {selected && (
        // Keyed by id + modifiedAt so the editor (and its inline name
        // input + save indicator) remounts cleanly whenever the backend
        // hands us a new snapshot — Reset to defaults, rename commit,
        // duplicate. Slider edits patch the parent without touching
        // modifiedAt, so the key stays stable across drags.
        <ProfileEditor
          key={`${selected.id}::${selected.modifiedAt}`}
          profile={selected}
          onAfterMutation={onChange}
          onPatchSettings={onPatchProfileSettings}
          onDuplicate={onDuplicate}
          onDelete={onDelete}
          onReset={onReset}
        />
      )}
    </>
  );
}

interface ProfileEditorProps {
  profile: Profile;
  onAfterMutation: () => Promise<void> | void;
  onPatchSettings: (id: ProfileId, settings: ProfileSettings) => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onReset: () => void;
}

function ProfileEditor({
  profile,
  onAfterMutation,
  onPatchSettings,
  onDuplicate,
  onDelete,
  onReset,
}: ProfileEditorProps) {
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">(
    "idle",
  );

  // Live-apply throttle: every slider tick fires updateProfileSettings
  // (cheap) but pushing HID frames at full slider rate would flood the
  // device. Leading edge fires immediately for snappy feedback;
  // trailing edge guarantees the final value lands when the drag
  // stops. 80 ms ≈ 12 fps, well within what e-ink can usefully redraw.
  const APPLY_INTERVAL_MS = 80;
  const lastApplyAtRef = useRef(0);
  const applyTimerRef = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (applyTimerRef.current !== null) {
        window.clearTimeout(applyTimerRef.current);
      }
    },
    [],
  );

  function scheduleLiveApply(id: string) {
    const now = Date.now();
    const elapsed = now - lastApplyAtRef.current;
    if (elapsed >= APPLY_INTERVAL_MS) {
      lastApplyAtRef.current = now;
      if (applyTimerRef.current !== null) {
        window.clearTimeout(applyTimerRef.current);
        applyTimerRef.current = null;
      }
      applyProfile(id).catch(() => {});
      return;
    }
    if (applyTimerRef.current !== null) return;
    applyTimerRef.current = window.setTimeout(() => {
      applyTimerRef.current = null;
      lastApplyAtRef.current = Date.now();
      applyProfile(id).catch(() => {});
    }, APPLY_INTERVAL_MS - elapsed);
  }

  return (
    <>
      <header className="editor__header">
        {profile.builtIn ? (
          <h2>{profile.name}</h2>
        ) : (
          <ProfileNameEditor
            value={profile.name}
            onCommit={async (next) => {
              try {
                await renameProfile(profile.id, next);
                await onAfterMutation();
              } catch {
                // ignore — UI will revert to the persisted name on
                // the next refresh
              }
            }}
          />
        )}
        <div className="editor__header-actions">
          <button type="button" className="btn" onClick={onDuplicate}>
            Duplicate
          </button>
          {profile.builtIn ? (
            <button type="button" className="btn" onClick={onReset}>
              Reset to defaults
            </button>
          ) : (
            <button type="button" className="btn" onClick={onDelete}>
              Delete
            </button>
          )}
        </div>
      </header>
      <p className="editor__save-status" role="status" aria-live="polite">
        {saveState === "saving"
          ? "Saving…"
          : saveState === "saved"
            ? "Saved"
            : "Changes save as you make them."}
      </p>
      <SettingsForm
        initial={profile.settings}
        onChange={(next: ProfileSettings) => {
          // Update the parent's profiles array right away so chip
          // switches don't surface stale settings; the IPC round-trip
          // is fire-and-forget for the slider drag.
          onPatchSettings(profile.id, next);
          setSaveState("saving");
          updateProfileSettings(profile.id, next)
            .then(() => setSaveState("saved"))
            .catch(() => setSaveState("idle"));
          // Push to the device so the user sees the effect on the
          // panel as they drag, not after they let go.
          scheduleLiveApply(profile.id);
        }}
      />
    </>
  );
}

interface ProfileNameEditorProps {
  value: string;
  onCommit: (next: string) => void | Promise<void>;
}

// Inline-editable profile name for custom profiles. Looks like the
// h2 it replaces — same type, same baseline — so the header layout
// stays put whether the user is editing or not. The parent owns the
// reset-on-update lifecycle via the editor's key prop, so this
// component just manages local draft text.
function ProfileNameEditor({ value, onCommit }: ProfileNameEditorProps) {
  const [draft, setDraft] = useState(value);

  function commit() {
    const trimmed = draft.trim();
    if (!trimmed || trimmed === value) {
      setDraft(value);
      return;
    }
    void onCommit(trimmed);
  }

  return (
    <input
      type="text"
      className="profile-name-input"
      aria-label="Profile name"
      value={draft}
      onChange={(e) => setDraft(e.currentTarget.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          e.currentTarget.blur();
        } else if (e.key === "Escape") {
          e.preventDefault();
          setDraft(value);
          e.currentTarget.blur();
        }
      }}
    />
  );
}

// -- Other panes ----------------------------------------------------

interface PaneProps {
  settings: AppSettings;
  onChange: (next: AppSettings) => void;
}

function GeneralPane({ settings, onChange }: PaneProps) {
  return (
    <label className="toggle-row">
      <input
        type="checkbox"
        checked={settings.launchAtLogin}
        onChange={(e) => {
          const value = e.currentTarget.checked;
          setLaunchAtLogin(value).catch(() => {});
          onChange({ ...settings, launchAtLogin: value });
        }}
      />
      Launch at login
    </label>
  );
}

function HotkeysPane({ settings, onChange }: PaneProps) {
  return (
    <>
      <ul className="hotkey-list">
        {HOTKEY_SLOTS.map(({ slot, label }) => (
          <li key={slot}>
            <span>{label}</span>
            <HotkeyRecorder
              value={settings.hotkeys[slot] ?? ""}
              onCommit={(next) => {
                setHotkey(slot, next).catch(() => {});
                const hotkeys = { ...settings.hotkeys };
                if (next === null) delete hotkeys[slot];
                else hotkeys[slot] = next;
                onChange({ ...settings, hotkeys });
              }}
            />
          </li>
        ))}
      </ul>
      <button
        type="button"
        className="btn"
        onClick={() => {
          resetHotkeys()
            .then(() => getAppSettings())
            .then((s) => onChange(s))
            .catch(() => {});
        }}
      >
        Reset hotkeys to defaults
      </button>
    </>
  );
}

function AboutPane({ version }: { version: string }) {
  return (
    <dl className="about">
      <dt>Version</dt>
      <dd>{version || "—"}</dd>
      <dt>License</dt>
      <dd>MIT</dd>
      <dt>Source</dt>
      <dd>
        <a
          href="https://github.com/arashmilani/mira-boox-pro-controller"
          target="_blank"
          rel="noreferrer"
        >
          github.com/arashmilani/mira-boox-pro-controller
        </a>
      </dd>
    </dl>
  );
}
