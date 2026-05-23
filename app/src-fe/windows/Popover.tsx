import { useEffect, useState } from "react";

import { Welcome } from "../components/Welcome";
import {
  applyProfile,
  forceRefresh,
  getDeviceStatus,
  isFirstRun,
  listProfiles,
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

  const visible = profiles.slice(0, 8);

  async function onApply(id: ProfileId) {
    try {
      await applyProfile(id);
      setActiveId(id);
    } catch {
      // Errors surface as toasts in Phase 7+ — silent for the shell.
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
      <header className="connection-bar">
        <span aria-hidden="true">{status.connected ? "●" : "○"} </span>
        <span>{status.connected ? "Connected" : "Disconnected"}</span>
      </header>

      {activeId && (
        <section aria-label="Active profile" className="active-chip">
          <strong>
            {profiles.find((p) => p.id === activeId)?.name ?? activeId}
          </strong>
        </section>
      )}

      <section aria-label="Profiles" className="profile-grid">
        {visible.map((p) => (
          <button
            key={p.id}
            type="button"
            data-active={p.id === activeId}
            onClick={() => onApply(p.id)}
          >
            {p.name}
          </button>
        ))}
      </section>

      <footer className="quick-actions">
        <button type="button" onClick={() => forceRefresh().catch(() => {})}>
          Force full refresh
        </button>
      </footer>
    </div>
  );
}
