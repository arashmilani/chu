import { Settings } from "./windows/Settings";
import "./styles/tokens.css";
import "./styles/reset.css";
import "./styles/components.css";

// One screen now: Settings. Profile switching and Force-refresh live
// on the tray context menu; clicking the tray icon opens the OS-native
// menu. The Tauri backend opens this window via the Settings… menu
// item; the `?window=settings` query param is set there for symmetry
// even though it's the only kind we route.

function App() {
  return <Settings />;
}

export default App;
