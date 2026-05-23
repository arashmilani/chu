import { lazy, Suspense } from "react";

import { Popover } from "./windows/Popover";
import { DevPreview } from "./dev-preview";
import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/components.css";

// Popover is eager — it's the cold-start entry per spec §9.1 and has
// to be interactive < 200ms. Editor and Settings are heavier and only
// open from the tray menu, so they lazy-load to keep the popover's
// initial chunk small.
const Editor = lazy(() =>
  import("./windows/Editor").then((m) => ({ default: m.Editor })),
);
const Settings = lazy(() =>
  import("./windows/Settings").then((m) => ({ default: m.Settings })),
);

export type WindowKind = "popover" | "editor" | "settings";

export function readWindowKind(search: string = ""): WindowKind {
  const params = new URLSearchParams(search);
  const w = params.get("window");
  if (w === "editor" || w === "settings") return w;
  return "popover";
}

function App() {
  if (
    typeof window !== "undefined" &&
    window.location.search.includes("preview=1")
  ) {
    return <DevPreview />;
  }

  const kind =
    typeof window === "undefined"
      ? "popover"
      : readWindowKind(window.location.search);

  if (kind === "editor") {
    return (
      <Suspense fallback={<p>Loading editor…</p>}>
        <Editor />
      </Suspense>
    );
  }
  if (kind === "settings") {
    return (
      <Suspense fallback={<p>Loading settings…</p>}>
        <Settings />
      </Suspense>
    );
  }
  return <Popover />;
}

export default App;
