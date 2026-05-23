import { useState } from "react";

import type { ProfileSettings, RefreshMode } from "../ipc/types";

interface SettingsFormProps {
  initial: ProfileSettings;
  disabled?: boolean;
  onChange?: (next: ProfileSettings) => void;
}

// Form covering all nine device settings with native HTML inputs.
// Native <input type="range"> per spec §4 (no slider component dep).
// Sliders update local state on each input; callers decide what to
// do with the stream — debounce, send live, or store on save.
//
// The component is keyed by profile id in the parent, so React resets
// internal state when the user switches profiles — no effect needed.
export function SettingsForm({
  initial,
  disabled = false,
  onChange,
}: SettingsFormProps) {
  const [s, setS] = useState<ProfileSettings>(initial);

  function update<K extends keyof ProfileSettings>(
    key: K,
    value: ProfileSettings[K],
  ) {
    const next = { ...s, [key]: value };
    setS(next);
    onChange?.(next);
  }

  return (
    <form className="settings-form" aria-label="Profile settings">
      <fieldset disabled={disabled}>
        <legend>Refresh style</legend>
        {(["a2", "direct"] as RefreshMode[]).map((mode) => (
          <label key={mode}>
            <input
              type="radio"
              name="refreshMode"
              value={mode}
              checked={s.refreshMode === mode}
              onChange={() => update("refreshMode", mode)}
            />
            {mode === "a2" ? "A2 (fast binary)" : "Direct (grayscale)"}
          </label>
        ))}
      </fieldset>

      <SliderRow
        label="Refresh speed"
        name="speed"
        value={s.speed}
        min={1}
        max={7}
        step={1}
        disabled={disabled}
        onChange={(v) => update("speed", v)}
      />
      <SliderRow
        label="Contrast"
        name="contrast"
        value={s.contrast}
        min={0}
        max={15}
        step={1}
        disabled={disabled}
        onChange={(v) => update("contrast", v)}
      />
      <SliderRow
        label="Dithering"
        name="ditherMode"
        value={s.ditherMode}
        min={0}
        max={3}
        step={1}
        disabled={disabled}
        onChange={(v) => update("ditherMode", v)}
      />
      <SliderRow
        label="Whiten background"
        name="whiteFilter"
        value={s.whiteFilter}
        min={0}
        max={127}
        step={1}
        disabled={disabled}
        onChange={(v) => update("whiteFilter", v)}
      />
      <SliderRow
        label="Deepen blacks"
        name="blackFilter"
        value={s.blackFilter}
        min={0}
        max={127}
        step={1}
        disabled={disabled}
        onChange={(v) => update("blackFilter", v)}
      />
      <SliderRow
        label="Cool front light"
        name="coldLight"
        value={s.coldLight}
        min={0}
        max={254}
        step={1}
        disabled={disabled}
        onChange={(v) => update("coldLight", v)}
      />
      <SliderRow
        label="Warm front light"
        name="warmLight"
        value={s.warmLight}
        min={0}
        max={254}
        step={1}
        disabled={disabled}
        onChange={(v) => update("warmLight", v)}
      />
    </form>
  );
}

interface SliderRowProps {
  label: string;
  name: string;
  value: number;
  min: number;
  max: number;
  step: number;
  disabled?: boolean;
  onChange: (next: number) => void;
}

function SliderRow({
  label,
  name,
  value,
  min,
  max,
  step,
  disabled,
  onChange,
}: SliderRowProps) {
  return (
    <label className="slider-row">
      <span>
        {label} <output>{value}</output>
      </span>
      <input
        type="range"
        name={name}
        aria-label={label}
        value={value}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.currentTarget.value))}
      />
    </label>
  );
}
