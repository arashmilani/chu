import { useEffect, useState } from "react";

import { HotkeyRecorder } from "../components/HotkeyRecorder";
import {
  appVersion,
  getAppSettings,
  resetHotkeys,
  setApplyLastOnConnect,
  setHotkey,
  setLaunchAtLogin,
} from "../ipc";
import type { AppSettings } from "../ipc/types";
import { HOTKEY_SLOTS } from "../ipc/types";

type Tab = "General" | "Hotkeys" | "Device" | "About";
const TABS: Tab[] = ["General", "Hotkeys", "Device", "About"];

export function Settings() {
  const [tab, setTab] = useState<Tab>("General");
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [version, setVersion] = useState<string>("");

  useEffect(() => {
    getAppSettings()
      .then(setSettings)
      .catch(() => {});
    appVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  return (
    <div className="settings">
      <nav aria-label="Settings sections" role="tablist" className="tabs">
        {TABS.map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            aria-selected={t === tab}
            data-active={t === tab}
            onClick={() => setTab(t)}
          >
            {t}
          </button>
        ))}
      </nav>

      <section role="tabpanel" aria-label={`${tab} settings`} className="pane">
        <h1>{tab}</h1>

        {!settings && <p>Loading…</p>}
        {settings && tab === "General" && (
          <GeneralPane
            settings={settings}
            onChange={(next) => setSettings(next)}
          />
        )}
        {settings && tab === "Hotkeys" && (
          <HotkeysPane
            settings={settings}
            onChange={(next) => setSettings(next)}
          />
        )}
        {settings && tab === "Device" && (
          <DevicePane
            settings={settings}
            onChange={(next) => setSettings(next)}
          />
        )}
        {tab === "About" && <AboutPane version={version} />}
      </section>
    </div>
  );
}

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
  return (
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
