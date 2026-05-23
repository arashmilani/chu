import { useState } from "react";

import { completeFirstRun, setLaunchAtLogin } from "../ipc";

interface WelcomeProps {
  deviceConnected: boolean;
  onDismiss: () => void;
}

// Spec §9.4 first-run flow. Three steps; the last commits the
// preferences and tells the backend the user has completed it so it
// doesn't show again.
export function Welcome({ deviceConnected, onDismiss }: WelcomeProps) {
  const [step, setStep] = useState<0 | 1 | 2>(0);
  const [launchAtLogin, setLaunch] = useState(false);

  function finish() {
    completeFirstRun().catch(() => {});
    setLaunchAtLogin(launchAtLogin).catch(() => {});
    onDismiss();
  }

  return (
    <section
      role="dialog"
      aria-label="Welcome to Mira Controller"
      className="welcome"
    >
      {step === 0 && (
        <>
          <h2>Welcome to Mira Controller</h2>
          <p>
            One-click switching between curated presets and your own profiles
            for the Boox Mira and Mira Pro.
          </p>
          <button type="button" onClick={() => setStep(1)}>
            Continue
          </button>
        </>
      )}
      {step === 1 && (
        <>
          <h2>
            {deviceConnected ? "Device detected" : "Waiting for a device"}
          </h2>
          <p>
            {deviceConnected
              ? "We've captured your current device settings as the “As-found” profile so you can revert at any time."
              : "Plug in your Mira via USB. The app will detect it automatically; you can keep using it as a tray utility in the meantime."}
          </p>
          <button type="button" onClick={() => setStep(2)}>
            Continue
          </button>
        </>
      )}
      {step === 2 && (
        <>
          <h2>Launch at login</h2>
          <p>
            Mira Controller lives in your menu bar / system tray. Enable
            launch-at-login so it's always ready when you plug in.
          </p>
          <label className="toggle-row">
            <input
              type="checkbox"
              checked={launchAtLogin}
              onChange={(e) => setLaunch(e.currentTarget.checked)}
            />
            Launch at login
          </label>
          <button type="button" onClick={finish}>
            Get started
          </button>
        </>
      )}
    </section>
  );
}
