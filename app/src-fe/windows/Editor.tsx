import { useEffect, useState } from "react";

import { SettingsForm } from "../components/SettingsForm";
import { listProfiles, updateProfileSettings } from "../ipc";
import type { Profile, ProfileSettings } from "../ipc/types";

interface EditorProps {
  /** Preview / test hook — skips the IPC fetch. */
  initialProfiles?: Profile[];
}

// Two-pane shell: profile list on the left, editor form on the right.
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
            <h1>{selected.name}</h1>
            {selected.builtIn && (
              <span className="editor__readonly">Built-in · read only</span>
            )}
            <SettingsForm
              key={selected.id}
              initial={selected.settings}
              disabled={selected.builtIn}
              onChange={(next: ProfileSettings) => {
                if (selected.builtIn) return;
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
