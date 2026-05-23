import { useEffect, useState } from "react";

import { HotkeyRecorder } from "../components/HotkeyRecorder";
import { SettingsForm } from "../components/SettingsForm";
import {
  appVersion,
  deleteProfile,
  duplicateProfile,
  getAppSettings,
  listDevices,
  listProfiles,
  resetHotkeys,
  resetProfileToDefaults,
  selectDevice,
  setApplyLastOnConnect,
  setHotkey,
  setLaunchAtLogin,
  updateProfileSettings,
} from "../ipc";
import type {
  AppSettings,
  DeviceInfo,
  Profile,
  ProfileSettings,
} from "../ipc/types";
import { HOTKEY_SLOTS } from "../ipc/types";

type Tab = "Profiles" | "General" | "Hotkeys" | "Device" | "About";
const TABS: Tab[] = ["Profiles", "General", "Hotkeys", "Device", "About"];

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
          <ProfilesPane profiles={profiles} onChange={refreshProfiles} />
        )}
        {tab === "General" && settings && (
          <GeneralPane settings={settings} onChange={setSettings} />
        )}
        {tab === "Hotkeys" && settings && (
          <HotkeysPane settings={settings} onChange={setSettings} />
        )}
        {tab === "Device" && settings && (
          <DevicePane settings={settings} onChange={setSettings} />
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
}

function ProfilesPane({ profiles, onChange }: ProfilesPaneProps) {
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
        <>
          <header className="editor__header">
            <h2>{selected.name}</h2>
            <div className="editor__header-actions">
              <button type="button" className="btn" onClick={onDuplicate}>
                Duplicate
              </button>
              {selected.builtIn ? (
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
          <SettingsForm
            key={selected.id}
            initial={selected.settings}
            onChange={(next: ProfileSettings) => {
              updateProfileSettings(selected.id, next).catch(() => {});
            }}
          />
        </>
      )}
    </>
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

function DevicePane({ settings, onChange }: PaneProps) {
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [selected, setSelected] = useState<string>("");

  useEffect(() => {
    listDevices()
      .then(setDevices)
      .catch(() => {});
  }, []);

  return (
    <>
      <label className="toggle-row">
        <input
          type="checkbox"
          checked={settings.applyLastProfileOnConnect}
          onChange={(e) => {
            const value = e.currentTarget.checked;
            setApplyLastOnConnect(value).catch(() => {});
            onChange({ ...settings, applyLastProfileOnConnect: value });
          }}
        />
        Re-apply last profile when the device reconnects
      </label>

      <section aria-label="Connected devices">
        <h2>Connected devices</h2>
        {devices.length === 0 ? (
          <p>No Mira devices found.</p>
        ) : (
          <ul>
            {devices.map((d) => (
              <li key={d.serialNumber ?? `${d.vendorId}:${d.productId}`}>
                <label>
                  <input
                    type="radio"
                    name="device"
                    value={d.serialNumber ?? ""}
                    checked={selected === (d.serialNumber ?? "")}
                    onChange={() => {
                      const serial = d.serialNumber ?? "";
                      setSelected(serial);
                      selectDevice(d.serialNumber).catch(() => {});
                    }}
                  />
                  {d.productString ?? "Mira"}
                  {d.serialNumber ? ` (${d.serialNumber})` : ""}
                </label>
              </li>
            ))}
          </ul>
        )}
      </section>
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
