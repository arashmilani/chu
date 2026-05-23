import { useState } from "react";

type Tab = "General" | "Hotkeys" | "Device" | "About";
const TABS: Tab[] = ["General", "Hotkeys", "Device", "About"];

// Settings shell with four tabs. Bodies are placeholders until
// Phase 9 wires real preferences in.
export function Settings() {
  const [tab, setTab] = useState<Tab>("General");

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
      </section>
    </div>
  );
}
