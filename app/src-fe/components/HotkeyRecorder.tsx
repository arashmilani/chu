import { useEffect, useRef, useState } from "react";

import { resumeHotkeys, suspendHotkeys } from "../ipc";

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

// Click to arm, then press a chord. The render shows the captured
// value as soon as a non-modifier key lands; pressing Escape cancels.
//
// Two macOS-flavoured pitfalls drove the current shape:
//   1. Clicking a <button> on macOS does NOT focus it, so a
//      button-level onKeyDown never fires for the chord. We listen on
//      `window` instead, so capture is focus-independent.
//   2. While any global shortcut is registered with the OS, the
//      plugin swallows that chord before the webview ever sees the
//      keydown. We suspend all global hotkeys for the duration of
//      the recording session, then resume on commit / cancel /
//      unmount. The 30 s safety timeout ensures a stray "armed but
//      walked away" session can't leave the OS hotkeys suspended.
export function HotkeyRecorder({ value, onCommit }: HotkeyRecorderProps) {
  const [recording, setRecording] = useState(false);
  const containerRef = useRef<HTMLSpanElement | null>(null);
  // Latest onCommit identity, so the effect below doesn't need to
  // re-attach on every parent render.
  const onCommitRef = useRef(onCommit);
  useEffect(() => {
    onCommitRef.current = onCommit;
  }, [onCommit]);

  useEffect(() => {
    if (!recording) return;

    suspendHotkeys().catch(() => {});

    function handleKeyDown(e: KeyboardEvent) {
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
      onCommitRef.current(next);
    }

    function handleMouseDown(e: MouseEvent) {
      if (
        containerRef.current &&
        containerRef.current.contains(e.target as Node)
      ) {
        return;
      }
      setRecording(false);
    }

    const safetyTimer = window.setTimeout(() => setRecording(false), 30000);

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("mousedown", handleMouseDown);
    return () => {
      window.clearTimeout(safetyTimer);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("mousedown", handleMouseDown);
      resumeHotkeys().catch(() => {});
    };
  }, [recording]);

  return (
    <span className="hotkey-recorder" ref={containerRef}>
      <button
        type="button"
        aria-label="Hotkey binding"
        onClick={() => setRecording(true)}
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
