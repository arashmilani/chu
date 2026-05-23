import { useEffect, useState } from "react";

import { SettingsForm } from "../components/SettingsForm";
import {
  deleteProfile,
  duplicateProfile,
  listProfiles,
  resetProfileToDefaults,
  updateProfileSettings,
} from "../ipc";
import type { Profile, ProfileSettings } from "../ipc/types";

interface EditorProps {
  /** Preview / test hook — skips the IPC fetch. */
  initialProfiles?: Profile[];
}

// Two-pane shell: profile list on the left, editor form on the right.
// Built-in presets are editable (spec divergence) but their names are
// fixed and they can't be deleted; every preset has a Reset button.
export function Editor({ initialProfiles }: EditorProps = {}) {
  const [profiles, setProfiles] = useState<Profile[]>(initialProfiles ?? []);
  const [selectedId, setSelectedId] = useState<string | null>(
    initialProfiles?.[0]?.id ?? null,
  );

  useEffect(() => {
    if (initialProfiles !== undefined) return;
    listProfiles()
      .then((ps) => {
        setProfiles(ps);
        if (ps.length > 0 && selectedId === null) {
          setSelectedId(ps[0].id);
        }
      })
      .catch(() => {});
  }, [initialProfiles, selectedId]);

  const selected = profiles.find((p) => p.id === selectedId);

  async function refresh() {
    try {
      const ps = await listProfiles();
      setProfiles(ps);
    } catch {
      // No-op: backend is the source of truth; we'll catch up next refresh.
    }
  }

  async function onDuplicate() {
    if (!selected) return;
    try {
      const newId = await duplicateProfile(selected.id);
      const ps = await listProfiles();
      setProfiles(ps);
      setSelectedId(newId);
    } catch {
      // ignore
    }
  }

  async function onDelete() {
    if (!selected || selected.builtIn) return;
    try {
      await deleteProfile(selected.id);
      const ps = await listProfiles();
      setProfiles(ps);
      setSelectedId(ps[0]?.id ?? null);
    } catch {
      // ignore
    }
  }

  async function onReset() {
    if (!selected || !selected.builtIn) return;
    try {
      await resetProfileToDefaults(selected.id);
      await refresh();
    } catch {
      // ignore
    }
  }

  return (
    <div className="editor">
      <nav aria-label="Profiles" className="editor__rail">
        <ul>
          {profiles.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                className="btn"
                data-active={p.id === selectedId}
                onClick={() => setSelectedId(p.id)}
              >
                {p.name}
              </button>
            </li>
          ))}
        </ul>
      </nav>

      <section aria-label="Profile editor" className="editor__pane">
        {selected ? (
          <>
            <header className="editor__header">
              <h1>{selected.name}</h1>
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
        ) : (
          <p>Select a profile from the list.</p>
        )}
      </section>
    </div>
  );
}
