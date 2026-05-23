import { useEffect, useState } from "react";

import { listProfiles } from "../ipc";
import type { Profile } from "../ipc/types";

// Two-pane shell: profile list on the left, editor form on the right.
// Stays minimal here — the slider machinery lands in Phase 8.
export function Editor() {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    listProfiles()
      .then((ps) => {
        setProfiles(ps);
        if (ps.length > 0 && selectedId === null) {
          setSelectedId(ps[0].id);
        }
      })
      .catch(() => {});
  }, [selectedId]);

  const selected = profiles.find((p) => p.id === selectedId);

  return (
    <div className="editor">
      <nav aria-label="Profiles" className="rail">
        <ul>
          {profiles.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                data-active={p.id === selectedId}
                onClick={() => setSelectedId(p.id)}
              >
                {p.name}
                {p.builtIn ? " (built-in)" : ""}
              </button>
            </li>
          ))}
        </ul>
      </nav>

      <section aria-label="Profile editor" className="pane">
        {selected ? (
          <>
            <h1>{selected.name}</h1>
            <p>Editing this profile lands in Phase 8.</p>
          </>
        ) : (
          <p>Select a profile from the list.</p>
        )}
      </section>
    </div>
  );
}
