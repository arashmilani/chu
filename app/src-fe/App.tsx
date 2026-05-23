import { Popover } from "./windows/Popover";
import { Editor } from "./windows/Editor";
import { Settings } from "./windows/Settings";
import { DevPreview } from "./dev-preview";
import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/components.css";

// Single SPA, three windows. Tauri spawns each WebviewWindow with a
// distinct `?window=` query param; the backend code in lib.rs is the
// only place that decides which window opens with which label.
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

  if (kind === "editor") return <Editor />;
  if (kind === "settings") return <Settings />;
  return <Popover />;
}

export default App;
