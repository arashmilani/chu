# Mira Boox Pro Controller — Specification

A small, fast, cross-platform desktop utility for controlling the Boox Mira and
Mira Pro e-ink monitors. Built because the vendor-supplied software is clunky,
slow, and offers a poor user experience for what is fundamentally a thin
configuration layer over a well-understood USB HID device.

---

## 1. Goals

- One-click switching between curated presets and user-defined profiles.
- Live tuning of every exposed knob with immediate, on-device feedback.
- Configurable global hotkeys, with defaults chosen to never collide with
  popular IDEs, browsers, or OS shortcuts.
- A tray/menu-bar workflow that stays out of the way and starts in <1 second.
- Cross-platform: macOS, Windows, Linux — single codebase, single UX.
- Small binary footprint and low idle memory (this is, after all, a utility
  meant to run alongside the writing/coding work the e-ink display enables).

## 2. Non-Goals (v1)

- Per-application auto-switching (deferred; requires per-OS focused-window
  APIs and meaningful UX design).
- Profile import/export, sync, or sharing.
- Live preview / split-screen test patterns.
- Image/video playback or any in-app content.
- Firmware updates or device flashing.
- Support for non-Mira Boox devices (tablets/readers).

These are tracked in [§13 Future Work](#13-future-work) and are intentionally
excluded from the v1 surface area.

## 3. Target Devices

The app targets Boox e-ink monitors that expose the documented USB HID
interface:

| Device         | USB VID | USB PID | Notes                                     |
| -------------- | ------- | ------- | ----------------------------------------- |
| Mira (13.3")   | 0x0416  | 0x5020  | Monochrome                                |
| Mira Pro       | 0x0416  | 0x5020  | 25.3" monochrome — primary target         |
| Mira Pro Color | 0x0416  | 0x5020  | Same protocol; color filter on top of EPD |

All three devices speak the same HID protocol. Variant-specific UI affordances
(e.g. color-filter hint on the Pro Color) are detected at runtime from the USB
descriptor where possible, otherwise selected manually in Settings.

## 4. Tech Stack

- **Shell:** [Tauri 2](https://tauri.app/) — small native binaries, native
  webview, single Rust process backing one or more webview windows.
- **Backend:** Rust. USB HID via [`hidapi`](https://crates.io/crates/hidapi)
  (mature, cross-platform, no kernel driver needed on any of the three OSes).
- **Frontend:** React + TypeScript + Vite. Styling is **plain CSS with CSS
  Modules** (Vite-native, zero extra deps) over a tiny set of design tokens
  in `src/styles/tokens.css`. Components are built from **native HTML
  elements** — `<dialog>` for modals, `<input type="range">` for sliders,
  `<details>` / `<menu>` where they fit — with a handful of small in-house
  hooks (`useFocusTrap`, `usePopoverPosition`) for the gaps. No component
  library, no utility-CSS framework. The UI surface is too small to earn
  those dependencies.
- **State:** Zustand for UI state; the source of truth for device state lives
  in the Rust backend and is pushed to the frontend over Tauri events.
- **Persistence:** Serde + JSON, written to the OS-standard config directory
  (see [§10 Persistence](#10-persistence)).
- **Global hotkeys:** [`tauri-plugin-global-shortcut`](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/global-shortcut).
- **Autostart:** [`tauri-plugin-autostart`](https://github.com/tauri-apps/plugins-workspace/tree/v2/plugins/autostart)
  (off by default; opt-in from Settings).

### Why not Electron + mira-js?

Reusing [mira-js](https://github.com/ipodnerd3019/mira-js) is the shortest
path to a working app, but it locks us into Node and a ~100MB bundle for a
utility that should be invisible. The mira-js source is small and the
protocol is short; porting to Rust is a one-day job that pays back in binary
size, idle memory, and startup latency forever.

## 5. Architecture

```
┌──────────────────────────────────────────────────────────────────────┐
│                          Tauri Application                           │
│                                                                      │
│  ┌────────────────────────┐         ┌─────────────────────────────┐  │
│  │   Frontend (React)     │ Tauri   │      Backend (Rust)         │  │
│  │                        │ IPC /   │                             │  │
│  │  • Tray menu (native)  │ events  │  ┌───────────────────────┐  │  │
│  │  • Settings window     │◄───────►│  │ Command dispatcher    │  │  │
│  │    (profiles + prefs)  │         │  └──────────┬────────────┘  │  │
│  │  • Hotkey recorder UI  │         │             │               │  │
│  └────────────────────────┘         │  ┌──────────▼────────────┐  │  │
│                                     │  │ Domain                │  │  │
│                                     │  │  • Profile model      │  │  │
│                                     │  │  • Active session     │  │  │
│                                     │  │  • Hotkey manager     │  │  │
│                                     │  │  • Persistence        │  │  │
│                                     │  └──────────┬────────────┘  │  │
│                                     │             │               │  │
│                                     │  ┌──────────▼────────────┐  │  │
│                                     │  │ HID driver (mira-rs)  │  │  │
│                                     │  │  • Device discovery   │  │  │
│                                     │  │  • Packet encoder     │  │  │
│                                     │  │  • Connection mgmt    │  │  │
│                                     │  └──────────┬────────────┘  │  │
│                                     │             │ hidapi        │  │
│                                     └─────────────┼───────────────┘  │
└───────────────────────────────────────────────────┼──────────────────┘
                                                    ▼
                                            Boox Mira (USB HID)
```

Three concentric layers in the Rust backend:

1. **HID driver (`mira-rs` module):** raw protocol. Encodes/decodes USB HID
   feature reports. Knows nothing about profiles or UI. Exposes typed
   `set_speed(u8)`, `set_contrast(u8)`, `refresh()`, etc. Ported from the
   protocol as documented in `mira-js`.
2. **Domain:** profile model, hotkey bindings, active-profile state machine,
   persistence. Pure Rust, easy to unit test, no UI deps.
3. **Tauri command layer:** thin wrappers exposing domain operations to the
   frontend.

The frontend never talks to USB. The backend never renders UI. State changes
flow backend → frontend via Tauri events (`device:connected`,
`profile:applied`, `setting:changed`).

## 6. Device Settings Reference

All values below are the actual HID-level knobs the device accepts. The UI
labels them in plain language; column 1 is the internal name.

| Setting        | Range        | Default\* | UI label             | Notes                                                       |
| -------------- | ------------ | --------- | -------------------- | ----------------------------------------------------------- |
| `refresh_mode` | a2 \| direct | direct    | "Refresh style"      | a2 = fast binary (good for typing), direct = full grayscale |
| `speed`        | 1–7          | 4         | "Refresh speed"      | Higher = faster updates, more ghosting                      |
| `contrast`     | 0–15         | 8         | "Contrast"           | "Dark color enhancement" in vendor app                      |
| `dither_mode`  | 0–3          | 1         | "Dithering"          | 0=off, 1=Bayer, 2=Floyd-Steinberg, 3=custom                 |
| `white_filter` | 0–127        | 0         | "Whiten background"  | Pixels above threshold snap to white                        |
| `black_filter` | 0–127        | 0         | "Deepen blacks"      | Pixels below threshold snap to black                        |
| `cold_light`   | 0–254        | 0         | "Cool front light"   | Off by default; many users keep it off                      |
| `warm_light`   | 0–254        | 0         | "Warm front light"   | Off by default                                              |
| `refresh()`    | —            | —         | "Force full refresh" | One-shot command, not a stored setting                      |

\* "Default" here is the app's neutral starting point, not the device factory
default. Confirmed values from the device on first connect populate the
"As-found" profile (see [§7.1](#71-built-in-presets)).

### Validation rules

- Every setter clamps to its valid range before being sent to the device.
- The HID driver returns an error if the device NAKs a packet; the domain
  layer logs and surfaces a non-blocking toast.
- The driver coalesces rapid slider changes (≤16ms) into a single write to
  avoid flooding the device while the user drags.

## 7. Profiles

A **profile** is a named, complete set of all nine settings above plus
metadata (icon, hotkey assignment, created/modified timestamps). The active
profile is always exactly one; switching profiles writes every differing
setting to the device.

### 7.1 Built-in presets

Six read-only presets ship with the app. Five mirror the vendor's preset
names so existing Mira users feel at home; the sixth, **Coding**, is unique
to this app and tuned for the workflow the device is most often bought for.
The "Coding" preset will be default preset.

| Preset           | refresh_mode | speed | contrast | dither | white_filter | black_filter |
| ---------------- | ------------ | ----- | -------- | ------ | ------------ | ------------ |
| Read             | direct       | 3     | 9        | 1      | 0            | 0            |
| Text             | a2           | 5     | 11       | 0      | 12           | 6            |
| Coding [default] | a2           | 6     | 12       | 0      | 16           | 8            |
| Speed            | a2           | 7     | 8        | 0      | 0            | 0            |
| Image            | direct       | 2     | 10       | 2      | 0            | 0            |
| Video            | a2           | 7     | 7        | 0      | 0            | 0            |

**Why these values for Coding:** `a2` keeps keystroke latency low (you feel
characters land); `speed=6` is fast but holds back from Speed's max so glyphs
stay clean; `contrast=12` is the highest among the presets, because
syntax-highlighted colors collapse to grayscale on a mono panel and need
every step of separation to remain distinguishable; `dither=0` because
dithering shreds small monospace glyphs; `white_filter=16` snaps the
slightly-off-white backgrounds typical of IDEs (`#fafafa`, `#f6f8fa`) to
pure white; `black_filter=8` deepens code text without crushing the
mid-tones used for comments and selection highlights.

These values are starting points based on community recommendations and the
vendor's published presets. Their **settings are editable from the UI** —
the original spec called for read-only presets with "duplicate to edit",
but in practice users tried to tune them, hit a wall, and didn't think to
duplicate. The shipped behaviour is instead:

- Preset **settings** can be edited freely from the Editor.
- Preset **names** stay fixed so users can keep finding "Coding",
  "Read", etc. Rename is rejected for built-ins.
- Built-ins cannot be **deleted** — they're always present as a known
  starting point.
- Every preset has a **Reset to defaults** button in the Editor that
  restores the values from the table above. Custom profiles get a
  **Delete** button in the same slot instead.
- **Duplicate** is available on every profile (built-in or custom)
  and yields an editable custom copy with name "Source copy".

In addition, a sixth pseudo-profile, **"As-found"**, is generated on first
connect by reading the device's current settings. It lets the user revert to
whatever the device was set to before this app first touched it.

On device connect — cold start or hot-plug — the last-active profile
is re-applied automatically so the panel always restores to the
user's most recent setup. There is no toggle for this; an earlier
spec draft had one, but in practice "remember what I last picked"
is the only sensible default, and a setting just added clicks.

### 7.2 Custom profiles

Users can:

- Duplicate any preset or existing profile to create a new one.
- Rename, edit, reorder, and delete custom profiles.
- Assign a hotkey to each profile.

There is no hard limit on the number of custom profiles. The tray UI shows
the first 8; the rest are reachable from a "More…" submenu and from the
full editor window.

### 7.3 Profile editor UX

The editor (a separate window opened from the tray menu) shows:

- Profile name
- All nine settings as labeled sliders / segmented controls, with the
  current value and a tooltip describing what it does
- Hotkey field (click-to-record, see [§8.2](#82-hotkey-recorder))
- A "Test on device" toggle: when on, every slider change is sent live to
  the monitor; when off, changes only persist when the user clicks Save

Saving an active profile re-applies it immediately.

## 8. Global Hotkeys

### 8.1 Default bindings — conflict-aware

Default hotkeys are chosen to avoid colliding with VS Code, JetBrains IDEs,
the major browsers, Slack, Terminal/iTerm, Raycast/Alfred, and OS-level
shortcuts. The shortlist below was vetted against macOS system shortcuts,
the VS Code default keymap, JetBrains default keymap, Chrome/Safari/Firefox
defaults, and GNOME/KDE default keybindings.

| Action              | macOS            | Windows / Linux  | Notes          |
| ------------------- | ---------------- | ---------------- | -------------- |
| Switch to profile 1 | ⌃⌥1 (Ctrl+Opt+1) | Ctrl+Alt+1       |                |
| Switch to profile 2 | ⌃⌥2              | Ctrl+Alt+2       |                |
| Switch to profile 3 | ⌃⌥3              | Ctrl+Alt+3       |                |
| Switch to profile 4 | ⌃⌥4              | Ctrl+Alt+4       |                |
| Switch to profile 5 | ⌃⌥5              | Ctrl+Alt+5       |                |
| Refresh             | ⌃⌥⇧R             | Ctrl+Alt+Shift+R | See note below |

Notes on the choices:

- **`Ctrl+Alt+1..5`**: deliberately _not_ `Cmd+Shift+1..5` (which collides
  with VS Code's "Side bar visibility" and several browser tab-group
  shortcuts) and _not_ `Cmd+1..5` (browsers and IDEs use these for tab and
  editor-group switching). The `Ctrl+Alt` (Win/Linux) / `Ctrl+Opt` (macOS)
  combination is essentially unused by major IDEs, browsers, and window
  managers as a system-wide chord.
- **`Ctrl+Alt+Shift+R`** for refresh: a plain `Ctrl+Alt+R` collides with
  GNOME's default screen-recorder shortcut, so the three-modifier chord is
  used as the default. Users on macOS/Windows who don't care about the
  GNOME case can rebind to `Ctrl+Alt+R` in Settings.
- **All defaults are user-configurable**; nothing is hard-coded. The
  Settings window has a "Reset hotkeys to defaults" button.

### 8.2 Hotkey recorder

A hotkey field in the editor captures keystrokes when focused. Recording UI:

- Shows current binding with a small ⌫ to clear.
- On focus, shows "Press a key combination…".
- Accepts any combination with at least one non-modifier key and at least
  one modifier (Ctrl/Cmd/Alt/Opt/Shift).
- Validates against currently-bound hotkeys (own or system-known) and
  flashes red with a one-line explanation on conflict.
- Falls back gracefully when the OS denies registration (e.g. another app
  already owns the shortcut) — surfaces a toast with the offending hotkey
  and unbinds it.

### 8.3 Registration lifecycle

Hotkeys are registered on app startup, re-registered when changed, and
unregistered on quit. If a registration fails (OS denial), the app stays
running and shows a persistent indicator in the tray icon until resolved.

## 9. User Interface

The shipped UI is intentionally minimal: a tray icon with a native OS
context menu, and one Settings window that opens from it. The original
spec called for a custom popover + a separate profile editor window;
both were dropped because they duplicated what the OS already does
well (a native tray menu) and forced users into a second screen for
the most common action (switching profiles).

### 9.1 Tray menu (primary surface)

A single tray entry labelled with the wordmark **"Mira"** plus a
status glyph:

- Connected, hotkeys registered: **`Mira ◉`** (filled fisheye)
- Disconnected or hotkey registration failure: **`Mira ◎`** (bullseye
  with a hole)

Two states only — earlier drafts had three (`Mira —` for disconnect,
`Mira !` for hotkey failure) but in practice the bullseye glyph reads
as "needs attention" without users having to remember which suffix
means which.

Clicking the tray icon (left-click on macOS, primary click anywhere)
opens the native OS context menu directly. No popover window. The
menu contents, top to bottom:

1. **Profile items** — every profile in order, with a checkmark next
   to the currently active one. Click to apply.
2. **Refresh** — disabled when no device is connected.
3. **Settings…** — opens the only window in the app.
4. **Quit Mira.**

Because this is a native menu, OS conventions apply: keyboard
navigation, mnemonics, and accessibility integration are all handled
by the platform.

### 9.2 Settings window (the only screen)

A single resizable window opened from the tray's "Settings…" item.
Closing the window hides it; the tray icon is the persistent
surface — the user quits explicitly from there. Four tabs along
the left rail:

- **Profiles** — horizontal chip list of every profile; selecting one
  loads it into the editor below. Each chip toggles the active
  editing target. Custom profile names are inline-editable in the
  header (Enter commits, Escape reverts); built-in names stay fixed.
  Action bar: **Duplicate** (always present), and either **Reset to
  defaults** (built-ins) or **Delete** (custom). The editor is the
  full nine-setting form (refresh mode + seven sliders). Edits
  auto-save per slider event with a live "Saving…"/"Saved"
  indicator, and a leading+trailing throttle (~80 ms) pushes the
  preset to the device live while sliding so the user sees the
  effect on the panel as they drag.
- **General** — Launch at login toggle.
- **Hotkeys** — Every spec §8.1 slot with an inline recorder per row
  and a "Reset hotkeys to defaults" button at the bottom.
- **About** — version, license, source link.

There is no Devices tab. The backend picks the first connected
Mira; for users with multiple Miras, multi-device selection has
been deferred (the IPC commands are still in place for a future
surface).

### 9.3 First run

Trivial: nothing to do. The Settings window is self-explanatory and
the tray menu surfaces the common actions without onboarding. The
As-found profile is still captured automatically on first device
connection so users can revert any unintended changes; it appears in
the profile list alongside the six shipped presets.

### 9.4 E-ink-friendly UI (default, always on)

Because this app is _about_ an e-ink monitor, users will frequently view its
own UI on the Mira. Rather than building a separate "E-ink mode" and a
detection layer to switch into it, the entire UI is designed to look good on
e-ink _and_ LCD at the same time — there is exactly one rendering, and it is
biased toward e-ink readability. LCD users won't notice; e-ink users will
notice it works.

**Visual rules:**

- **Pure black on pure white.** `#000` text on `#fff` backgrounds. No
  gray-on-gray text, no `#fafafa` chrome.
- **Solid borders, no shadows.** Panels and inputs use 1–2px hard borders.
  No `box-shadow`, no blur, no translucency / vibrancy.
- **No gradients, no soft elevation.** Active and selected states use bold
  borders or solid fills, not background tints.
- **Color is never load-bearing.** Status is always communicated by both a
  text-glyph and a label (e.g. "● Connected" / "○ Disconnected"), so the UI
  is fully usable in monochrome. The glyphs in use (●, ○, ◉, ◎) are
  Unicode characters, not icon assets.
- **Large hit targets.** ≥40×40px for buttons and slider thumbs; ≥44px row
  height in lists.
- **System sans-serif** at ≥15px body / ≥17px UI.

**Motion rules:**

- Profile switches and Settings window open/close are instantaneous —
  no fades, no slides.
- No spinners, no skeleton loaders. The rare blocking operation uses a
  static "Working…" label.
- Hover styles are functional only (focus ring), not decorative
  (background fade).
- Slider drags are debounced: the on-screen value updates only when the
  user pauses (≥120ms) or releases, to avoid constant re-renders that
  wreck e-ink legibility. (This is separate from the device-write coalescing
  in [§6](#validation-rules) — one is UI redraws, the other is HID writes.)

That's the entire story. No toggle, no detection, no mode-switch. If the
defaults need tuning later, that's a v2 concern — and the right move would
be a single Settings option for "larger text", not a whole alternate theme.

## 10. Persistence

Config and profile data live as a single JSON file in the OS-standard
location:

- **macOS:** `~/Library/Application Support/MiraController/config.json`
- **Linux:** `$XDG_CONFIG_HOME/mira-controller/config.json` (default
  `~/.config/mira-controller/config.json`)
- **Windows:** `%APPDATA%\MiraController\config.json`

### Schema (sketch)

```jsonc
{
  "version": 1,
  "lastActiveProfileId": "uuid-...",
  "applyLastProfileOnConnect": true,
  "launchAtLogin": false,
  "hotkeys": {
    "profile1": "Ctrl+Alt+1",
    "profile2": "Ctrl+Alt+2",
    "...": "...",
    "refresh": "Ctrl+Alt+Shift+R",
    "openPopover": "Ctrl+Alt+Shift+M",
  },
  "profiles": [
    {
      "id": "uuid-...",
      "name": "Long-form writing",
      "hotkey": "profile3",
      "builtIn": false,
      "settings": {
        "refreshMode": "direct",
        "speed": 3,
        "contrast": 10,
        "ditherMode": 1,
        "whiteFilter": 14,
        "blackFilter": 4,
        "coldLight": 0,
        "warmLight": 0,
      },
      "createdAt": "2026-05-23T12:00:00Z",
      "modifiedAt": "2026-05-23T12:00:00Z",
    },
  ],
}
```

Writes are atomic (write-temp + rename). On schema bump, the previous file
is preserved as `config.v{old}.bak` before migration.

## 11. Multi-Device Handling

v1 supports one connected Mira at a time. The HID driver enumerates all
matching devices and uses the first one. If multiple are detected, a
notification offers to open a device picker. (Most real-world setups are
single-monitor; full multi-device UI is a v2 concern but the driver
abstraction is built to allow it.)

## 12. Build, Distribution, and Platform Concerns

### 12.1 Build

- Single Rust workspace, `cargo tauri build` for each target.
- CI: GitHub Actions matrix (macOS, Windows, Ubuntu).
- Versioning: SemVer; tag `vX.Y.Z` triggers a release build.

### 12.2 Platform notes

- **macOS:** signed and notarized DMG; the app is menu-bar-only by default
  (no Dock icon) via `LSUIElement = true`. Apple Developer ID required for
  unsigned-warning-free distribution.
- **Windows:** signed MSI/NSIS installer. WebView2 is a system component on
  Win10+ so no embedded runtime needed.
- **Linux:** AppImage as the primary artifact (no install needed). A `.deb`
  for apt-based distros is a nice-to-have. Install a udev rule on first run
  (with permission prompt) to grant the user access to the HID device:

  ```
  SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0416", ATTRS{idProduct}=="5020", \
      MODE="0660", GROUP="plugdev", TAG+="uaccess"
  ```

  If the rule is missing the app surfaces a one-click installer in the
  tray with a clear explanation.

### 12.3 License

MIT. The app is small enough that copyleft isn't necessary, and a
permissive license matches the existing ecosystem of community tooling
(mira-js, miractl).

## 13. Future Work

Tracked but out of v1 scope:

- **Per-application auto-switching.** Watch focused window via macOS
  Accessibility API / Windows `GetForegroundWindow` / X11+Wayland focus
  events; apply a per-app profile.
- **Profile import/export and shareable links.** JSON file or `mira://`
  custom URL scheme.
- **Live preview/test pattern.** A test-pattern window for tuning.
- **Auto-refresh on idle.** Force full refresh after N minutes of no
  display change to clear ghosting.
- **Light-sensor coupling** (where the host has one) to auto-tune the
  cold/warm front light.
- **Multi-device UI.** Per-device active profiles, named devices.
- **Telemetry-free crash reports** with opt-in submission.

## 14. Open Questions

These are good to revisit during implementation, none are blockers:

- The exact byte layout of HID feature reports should be confirmed by
  reading mira-js source and ideally captured via Wireshark on a working
  install — protocol docs are sparse and a few values' ranges differ across
  community tools.
- Whether the "As-found" profile survives across firmware updates (some
  Mira firmware versions reset to factory on update).
- Whether to expose `dither_mode = 3` (custom) at all in v1, since it
  requires extra parameters that aren't well-documented publicly.

---

## Appendix A — References

- [mira-js (community Node implementation)](https://github.com/ipodnerd3019/mira-js) — primary protocol reference
- [miractl (community CLI in Rust/Python)](https://github.com/clarkema/miractl) — secondary reference, value ranges
- [Boox Mira help docs](https://help.boox.com/hc/en-us/articles/4547000092180-Mira-Software-Linux-Setup) — official Linux setup, udev rule template
- [Tauri 2 docs](https://tauri.app/) — framework
- [hidapi crate](https://crates.io/crates/hidapi) — Rust USB HID bindings
