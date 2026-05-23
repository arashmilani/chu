import { useState } from "react";

interface HotkeyRecorderProps {
  value: string;
  onCommit: (next: string | null) => void;
}

const MODIFIER_KEYS = new Set([
  "Control",
  "Alt",
  "Shift",
  "Meta",
  "OS",
  "Option",
]);

// Click to focus, then press a chord. We render the captured value as
// soon as a non-modifier key lands; pressing Escape cancels.
export function HotkeyRecorder({ value, onCommit }: HotkeyRecorderProps) {
  const [recording, setRecording] = useState(false);

  function onKeyDown(e: React.KeyboardEvent<HTMLButtonElement>) {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") {
      setRecording(false);
      return;
    }
    if (MODIFIER_KEYS.has(e.key)) return;
    const mods: string[] = [];
    if (e.ctrlKey) mods.push("Ctrl");
    if (e.altKey) mods.push("Alt");
    if (e.shiftKey) mods.push("Shift");
    if (e.metaKey) mods.push("Cmd");
    if (mods.length === 0) return; // need at least one modifier
    const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
    const next = [...mods, key].join("+");
    setRecording(false);
    onCommit(next);
  }

  return (
    <span className="hotkey-recorder">
      <button
        type="button"
        aria-label="Hotkey binding"
        onClick={() => setRecording(true)}
        onKeyDown={onKeyDown}
        onBlur={() => setRecording(false)}
      >
        {recording ? "Press a key combination…" : value || "—"}
      </button>
      {value && !recording && (
        <button
          type="button"
          aria-label="Clear binding"
          onClick={() => onCommit(null)}
        >
          ⌫
        </button>
      )}
    </span>
  );
}
