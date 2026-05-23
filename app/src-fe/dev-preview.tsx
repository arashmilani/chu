// Visual-preview entry: renders all three windows with sample data so
// they can be inspected without a Tauri backend. Not shipped with the
// app — used during development to verify the visual design.
//
// Mount at `?preview=1` via the App router (or by visiting
// /dev-preview.html directly).

import { Popover } from "./windows/Popover";
import { Editor } from "./windows/Editor";
import { Settings } from "./windows/Settings";
import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/components.css";

const sampleProfiles = [
  preset("read", "Read", { speed: 3, contrast: 9, ditherMode: 1 }),
  preset("text", "Text", {
    speed: 5,
    contrast: 11,
    whiteFilter: 12,
    blackFilter: 6,
  }),
  preset("coding", "Coding", {
    speed: 6,
    contrast: 12,
    whiteFilter: 16,
    blackFilter: 8,
  }),
  preset("speed", "Speed", { speed: 7, contrast: 8 }),
  preset("image", "Image", {
    speed: 2,
    contrast: 10,
    ditherMode: 2,
    refreshMode: "direct",
  }),
  preset("video", "Video", { speed: 7, contrast: 7 }),
];

function preset(
  id: string,
  name: string,
  overrides: Partial<{
    speed: number;
    contrast: number;
    ditherMode: number;
    whiteFilter: number;
    blackFilter: number;
    refreshMode: "a2" | "direct";
  }> = {},
) {
  return {
    id,
    name,
    builtIn: true,
    hotkey: null,
    settings: {
      refreshMode: overrides.refreshMode ?? ("a2" as const),
      speed: overrides.speed ?? 4,
      contrast: overrides.contrast ?? 8,
      ditherMode: overrides.ditherMode ?? 0,
      whiteFilter: overrides.whiteFilter ?? 0,
      blackFilter: overrides.blackFilter ?? 0,
      coldLight: 0,
      warmLight: 0,
    },
    createdAt: "1970-01-01T00:00:00Z",
    modifiedAt: "1970-01-01T00:00:00Z",
  };
}

export function DevPreview() {
  const params = new URLSearchParams(
    typeof window === "undefined" ? "" : window.location.search,
  );
  const which = params.get("which") ?? "popover";

  if (which === "editor") return <Editor initialProfiles={sampleProfiles} />;
  if (which === "settings") return <Settings />;
  return (
    <Popover
      initialProfiles={sampleProfiles}
      initialStatus={{ connected: true }}
      initialActiveId={"coding"}
      skipFirstRunCheck
    />
  );
}
