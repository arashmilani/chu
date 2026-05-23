import { useEffect, useState } from "react";

import { Welcome } from "../components/Welcome";
import {
  applyProfile,
  forceRefresh,
  getDeviceStatus,
  isFirstRun,
  listProfiles,
  onDeviceConnected,
  onDeviceDisconnected,
} from "../ipc";
import type { DeviceStatus, Profile, ProfileId } from "../ipc/types";

interface PopoverProps {
  initialProfiles?: Profile[];
  initialStatus?: DeviceStatus;
  initialActiveId?: ProfileId | null;
  /** Skip the first-run check entirely. Test hook. */
  skipFirstRunCheck?: boolean;
}

// The everyday surface. Connection bar at top, active-profile chip,
// grid of first 8 profiles, quick actions. No animations, hard
// borders, pure black on white — per spec §9.5.
export function Popover({
  initialProfiles,
  initialStatus,
  initialActiveId = null,
  skipFirstRunCheck = false,
}: PopoverProps) {
  const [profiles, setProfiles] = useState<Profile[]>(initialProfiles ?? []);
  const [status, setStatus] = useState<DeviceStatus>(
    initialStatus ?? { connected: false },
  );
  const [activeId, setActiveId] = useState<ProfileId | null>(initialActiveId);
  const [showWelcome, setShowWelcome] = useState(false);

  useEffect(() => {
    if (initialProfiles === undefined) {
      listProfiles()
        .then(setProfiles)
        .catch(() => {});
    }
    if (initialStatus === undefined) {
      getDeviceStatus()
        .then(setStatus)
        .catch(() => {});
    }
    if (!skipFirstRunCheck) {
      isFirstRun()
        .then(setShowWelcome)
        .catch(() => {});
    }
  }, [initialProfiles, initialStatus, skipFirstRunCheck]);

  // Subscribe to device-state events from the backend's hotplug
  // watcher so the popover updates without the user re-opening it.
  useEffect(() => {
    if (initialStatus !== undefined) return; // Test/preview fixture.
    let unlistenConn: (() => void) | undefined;
    let unlistenDisc: (() => void) | undefined;
    onDeviceConnected((payload) => setStatus(payload))
      .then((fn) => (unlistenConn = fn))
      .catch(() => {});
    onDeviceDisconnected((payload) => setStatus(payload))
      .then((fn) => (unlistenDisc = fn))
      .catch(() => {});
    return () => {
      unlistenConn?.();
      unlistenDisc?.();
    };
  }, [initialStatus]);

  const visible = profiles.slice(0, 8);
  const active = profiles.find((p) => p.id === activeId);

  async function onApply(id: ProfileId) {
    try {
      await applyProfile(id);
      setActiveId(id);
    } catch {
      // Errors surface as toasts in a future polish pass.
    }
  }

  if (showWelcome) {
    return (
      <Welcome
        deviceConnected={status.connected}
        onDismiss={() => setShowWelcome(false)}
      />
    );
  }

  return (
    <div role="dialog" aria-label="Mira controller" className="popover">
      <header className="popover__bar">
        <span className="popover__status">
          <span className="popover__status-glyph" aria-hidden="true">
            {status.connected ? "●" : "○"}
          </span>
          {status.connected ? "Connected" : "Disconnected"}
        </span>
        <span className="t-eyebrow">Mira</span>
      </header>

      {active && (
        <section aria-label="Active profile" className="popover__active">
          <div className="popover__active-label">Active profile</div>
          <div className="popover__active-name">{active.name}</div>
        </section>
      )}

      <section aria-label="Profiles" className="popover__grid">
        {visible.map((p) => (
          <button
            key={p.id}
            type="button"
            className="btn"
            data-active={p.id === activeId}
            onClick={() => onApply(p.id)}
          >
            {p.name}
          </button>
        ))}
      </section>

      <footer className="popover__actions">
        <button
          type="button"
          className="btn"
          onClick={() => forceRefresh().catch(() => {})}
        >
          Force full refresh
        </button>
      </footer>
    </div>
  );
}
