# Mira Boox Pro Controller — Implementation Plan

This is a detailed, TDD-first build plan for the app described in
[specs.md](specs.md). The application lives entirely under `/app/` and is
built phase by phase, each phase landing on `main` only when its test suite
is green and its exit criteria are met.

## Current implementation status

| Phase | Status      | Tests           | Notes                                                                            |
| ----- | ----------- | --------------- | -------------------------------------------------------------------------------- |
| 0     | ✅ Complete | scaffold        | Tauri 2 + React 19 + TS, CI matrix, README                                       |
| 1     | ✅ Complete | 31 Rust tests   | All encoders, transport (mock + hidapi), discovery, coalescer, NAK               |
| 2     | ✅ Complete | 28 Rust tests   | Profile + 6 presets + AsFound + Session::apply + Store CRUD                      |
| 3     | ✅ Complete | 7 Rust tests    | Atomic writes, v0→v1 migration, corruption recovery                              |
| 4     | ✅ Complete | 23 Rust tests   | AppState, typed AppError, list/apply/refresh/status/CRUD commands                |
| 5     | ✅ Complete | 16 Rust tests   | Binding model + 6 defaults (openPopover retired) + OS register/rebind/reset      |
| 6     | 🟨 Revised  | (folded)        | Tokens + IPC retained; hooks/Popover/Editor removed in the tray-menu rewrite     |
| 7     | ✅ Complete | 4 Rust tests    | Tray icon with dynamic profile menu + ◉/◎ title; popover removed                 |
| 8     | ✅ Complete | 5 vitest specs  | SettingsForm with all 9 settings via native HTML inputs                          |
| 9     | ✅ Complete | 6 vitest specs  | Profiles + General + Hotkeys + Device + About panes in the only window           |
| 10    | 🟨 Revised  | (backend only)  | First-run flag retained for future use; UI flow dropped, as-found capture stays  |
| 11    | ✅ Complete | 2 vitest specs  | Discovery enumeration + picker in Device settings + multi-detected event         |
| 12    | 🟨 Partial  | —               | Per-OS bundle configs, udev installer helpers, release workflow; signing TBD     |
| 13    | ✅ Complete | (build)         | Lazy-loaded Editor/Settings chunks; a11y review notes below                      |

**Totals:** 125 Rust unit tests + 19 frontend specs, all green; `cargo
clippy --all-targets -- -D warnings`, `cargo fmt --check`, `pnpm lint`,
`pnpm typecheck`, `pnpm format:check` all pass. Production bundle is
back to a single chunk now that there's only one window — the
lazy-load split disappeared with the Editor/Popover windows it was
splitting.

### Phase 12 remaining

What's still needed beyond what's in `app/src-tauri/tauri.conf.json`
and `.github/workflows/release.yml`:

- **macOS signing/notarization** — release workflow reads the secrets
  but the certs themselves still need to be provisioned (Apple
  Developer ID for the DMG, app-specific password for notarization).
- **Windows signing** — likewise reads `WINDOWS_CERTIFICATE` /
  `WINDOWS_CERTIFICATE_PASSWORD`; needs an actual code-signing cert.
- **LSUIElement plist merge** — `src-tauri/Info.plist` carries the
  menu-bar-only flag, but Tauri 2's bundler doesn't merge it
  automatically. The release workflow needs a small post-bundle step
  (e.g. `plutil -insert LSUIElement -bool true …`) until the upstream
  schema gains a first-class field.
- **Linux udev installer UI** — the `udev_rule_text` /
  `udev_rule_present` commands are in place; the AppImage first-run
  prompt that calls them with a `pkexec`-elevated write isn't wired
  in yet.

### Phase 13 a11y review

What was verified during the polish pass; what's still rough:

- ✅ Every interactive element has a visible label (text, `aria-label`,
  or wrapping `<label>`).
- ✅ `role="dialog"`/`role="tab"`/`role="tabpanel"` used appropriately;
  `aria-selected` mirrors the active tab.
- ✅ Pure-keyboard click on tabs works; `:focus-visible` ring is
  non-decorative (2px black outset).
- ✅ Status never relies on color alone — a `●`/`○` glyph pairs with
  the "Connected"/"Disconnected" label per spec §9.5.
- ✅ Hit targets ≥40×40px for buttons and 44px row height for list
  rows, set via tokens.
- 🟨 Roving-tabindex for the Settings tab list isn't implemented; tabs
  are reachable individually but Left/Right arrow navigation between
  them follows the default `<button>` behaviour rather than the
  WAI-ARIA tablist pattern.
- 🟨 Focus is not explicitly moved into the Welcome card when it
  appears; the first focusable element receives focus only on the
  user's first Tab press.
- 🟨 No live-region announces device connect / disconnect — the tray
  title still surfaces the state, but a screen reader on the desktop
  won't see the change until the user re-opens the popover.

### Implementation-order divergences

- **Phase 7 first commits landed inside Phase 6.** The popover shell
  was the natural first component to render once tokens and IPC were
  in place. The remaining Phase 7 tasks (tray icon, system positioning,
  multi-window orchestration) landed afterwards.
- **Phase 5 OS-side registration landed alongside Phase 7.** The
  global-shortcut plugin needs the same Tauri runtime wiring as the
  tray, so they shipped together rather than as separate phases.

### Behavioural divergence: tray menu replaces the popover window

After live testing, the popover and profile-editor windows were
removed in favour of a native OS tray context menu + a single Settings
window. Reasoning:

- The popover duplicated what `NSStatusItem` / Windows tray /
  AppIndicator already do well, with worse keyboard navigation, no
  OS conventions, and an extra render cycle.
- The most-common action (switch profile) is now one click instead of
  click → open popover → click profile.
- The "Mira" tray title gained two states only: `Mira ◉` (filled
  fisheye, OK) and `Mira ◎` (bullseye with a hole, anything else).
  Two states require zero memory of which suffix means what.
- The `openPopover` hotkey was retired with the popover.

The Settings window keeps everything else: a new Profiles tab hosts
the full editor (chip selector + Duplicate/Reset/Delete + nine-setting
form, plus inline rename for custom profiles and a live save
indicator). The Devices tab was dropped — single-device users got
nothing from it, the backend already picks the first connected Mira,
and apply-last-profile-on-connect became a default rather than a
toggle. Tabs are now Profiles / General / Hotkeys / About.
First-run is silent — As-found capture still happens on initial
device connection.

### Behavioural divergence: editable built-in presets

Spec §7.1 originally called for built-ins to be read-only with
"duplicate to edit". Live testing showed that's an unforgiving model
— users try to tune the slider, hit a wall, and don't think to
duplicate. The shipped behaviour:

- Preset *settings* are editable from the UI; only names and the
  list-membership are locked.
- Every preset has a **Reset to defaults** button in the Editor that
  restores the spec §7.1 values. Custom profiles get **Delete** in
  the same slot.
- A **Duplicate** button is always present, so the original "make a
  copy" workflow is still one click away.

Backend: `AppState::update_settings` no longer rejects built-ins;
`AppState::reset_to_defaults` is new and is wired through a
`reset_profile_to_defaults` Tauri command. `ProfileStore::rename` and
`ProfileStore::delete` still reject built-ins so the recognized names
and the always-present list are guaranteed.

---

## 1. Working Method

### 1.1 TDD discipline (non-negotiable)

Every behavioral change — feature, refactor, or bug fix — follows the
red → green → commit cycle:

1. **Red.** Write the test that describes the new behavior, or the
   regression you're fixing. Run the test suite. **It must fail**, and the
   failure message must clearly point at what's missing or wrong. If a new
   test passes the first time, it isn't testing what you think it is —
   adjust the test (better assertion, stronger setup) until it fails for
   the right reason, then proceed.
2. **Green.** Write the minimum code to make the failing test pass. No
   extras, no speculative APIs, no helpers that aren't yet needed by a
   second caller.
3. **Refactor.** Clean up only what's directly in front of you. The suite
   stays green throughout.
4. **Commit.** A fully green suite is a **hard prerequisite** for every
   commit. We never push a red commit; broken tests stay in the working
   tree until they're green.

The same loop applies to bug fixes: reproduce the bug with a failing test
*first*, then fix the production code. The new test then guards against
regression forever.

### 1.2 Commit cadence

- **One commit per logical change.** A "logical change" is the smallest
  standalone unit that leaves the suite better than it was before: one
  behavior added, one refactor performed, one bug fixed.
- **Short-lived commits.** If an hour has passed without a commit, the
  batch has grown too large — break the next slice smaller.
- **Format: Conventional Commits.**
  - `feat(domain): add profile validation`
  - `fix(hid): clamp speed before encoding`
  - `test(mira): cover dither-mode 0..=3 encoding`
  - `refactor(commands): extract apply_profile error handling`
  - `chore(ci): cache cargo registry`
- Per the repo's global preference, commits are **single-author** — no
  `Co-Authored-By` trailer.
- The phases below name a likely first commit per task. Real commits will
  often subdivide; that's encouraged.

### 1.3 Branch strategy

- **Trunk-based.** `main` is always shippable.
- Phase work happens on a short-lived branch named `phase-{N}-{slug}`
  (e.g. `phase-1-hid-driver`) and merges back via PR once the phase's exit
  criteria are met.
- A PR can ship before the whole phase is done — small, focused PRs beat
  one mega-PR per phase. Each PR must be independently green.

### 1.4 Definition of done (per commit)

- New tests written first; whole suite green.
- Rust: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`
  all pass.
- TS: `pnpm test`, `pnpm lint`, `pnpm typecheck` all pass.
- CI matrix (macOS, Windows, Ubuntu) is green on the PR.
- Commit message follows §1.2.

---

## 2. Project Structure

Everything ships under `/app/`. Repo root stays clean:

```
/
├── docs/
│   ├── specs.md
│   └── plan.md          ← this file
└── app/
    ├── src-tauri/                  Rust backend (Tauri 2)
    │   ├── src/
    │   │   ├── main.rs
    │   │   ├── lib.rs
    │   │   ├── mira/               HID driver (mira-rs module)
    │   │   │   ├── mod.rs
    │   │   │   ├── transport.rs    HidTransport trait + real impl
    │   │   │   ├── encoder.rs      Packet encoding per setting
    │   │   │   └── discovery.rs    USB enumeration
    │   │   ├── domain/
    │   │   │   ├── mod.rs
    │   │   │   ├── profile.rs      Profile model + presets
    │   │   │   ├── session.rs      Active-profile state machine
    │   │   │   ├── hotkeys.rs      Binding model + recorder
    │   │   │   └── persistence.rs  Atomic JSON read/write + migrations
    │   │   └── commands/
    │   │       ├── mod.rs
    │   │       ├── device.rs       Device status, refresh
    │   │       ├── profiles.rs     CRUD + apply
    │   │       └── settings.rs     App-level settings
    │   ├── tests/                  Integration tests
    │   │   ├── hid_roundtrip.rs    Real device (skipped by default)
    │   │   ├── persistence.rs
    │   │   └── command_layer.rs
    │   ├── Cargo.toml
    │   └── tauri.conf.json
    ├── src-fe/                     Frontend (React + TS + Vite)
    │   ├── main.tsx
    │   ├── App.tsx
    │   ├── windows/
    │   │   ├── Popover.tsx
    │   │   ├── Editor.tsx
    │   │   └── Settings.tsx
    │   ├── components/             Dumb, presentation-only
    │   ├── hooks/                  useFocusTrap, usePopoverPosition, etc.
    │   ├── store/                  Zustand stores
    │   ├── ipc/                    Thin wrappers around Tauri invoke/listen
    │   ├── styles/
    │   │   ├── tokens.css          Design tokens (colors, spacing, type)
    │   │   └── reset.css
    │   └── tests/                  Frontend tests (vitest + RTL)
    ├── index.html
    ├── package.json
    ├── tsconfig.json
    ├── vite.config.ts
    └── vitest.config.ts
```

---

## 3. Testing Strategy

### 3.1 Rust

- **Unit tests** colocated with modules using `#[cfg(test)] mod tests`.
- **Integration tests** under `app/src-tauri/tests/` — these exercise the
  command layer end-to-end with a mock transport.
- **HID mock.** A `trait HidTransport` is defined in `mira::transport`
  with two impls: the real `hidapi`-backed transport and a `MockTransport`
  that records writes and lets tests script replies. This is the single
  most important seam in the codebase — it's the reason we can do TDD on
  the driver at all.
- **Real-device tests** live in `tests/hid_roundtrip.rs` and are gated
  behind `#[ignore]`; they only run with `cargo test -- --ignored` and a
  real Mira plugged in.
- **Tauri command tests** use `tauri::test::mock_app()` so we exercise
  command handlers without spawning a webview.

### 3.2 Frontend

- **Vitest** as the test runner (Vite-native, fast).
- **@testing-library/react** for component tests.
- **jsdom** environment. We pin `jsdom@^26` for now — the 27/29 line
  pulled in an ESM-only `@exodus/bytes` transitive dep that
  `html-encoding-sniffer` still requires synchronously, breaking
  Vitest worker boot on Node 22.11. Revisit once the toolchain
  catches up.
- IPC is faked in tests via a small `ipc/__mocks__/` shim so components
  never know whether they're talking to a real backend.
- **Lint stack:** flat ESLint config with `@eslint/js`,
  `typescript-eslint`, `eslint-plugin-react-hooks`, and
  `eslint-plugin-react-refresh`. `eslint-plugin-react` is intentionally
  *not* included: its 7.37.x line is incompatible with ESLint 10's
  context API, and React 19's JSX transform already removes the rules
  it would have provided. The Vite team's official template made the
  same call.

### 3.3 CI gate

- GitHub Actions matrix: `macos-latest`, `windows-latest`, `ubuntu-latest`.
- Every push runs: `cargo test`, `cargo clippy -D warnings`, `cargo fmt
  --check`, `pnpm test --run`, `pnpm lint`, `pnpm typecheck`, and a
  `tauri build` smoke build (no codesign).
- Required status checks on `main` mean a red CI blocks merge.

---

## 4. Phase 0 — Project Skeleton & CI

**Goal.** A repo where TDD is possible: a Tauri 2 scaffold under `/app`,
test runners wired, CI green on an empty smoke test.

**Exit criteria.**
- `cargo test`, `pnpm test`, `cargo tauri dev` all run locally.
- CI matrix green on all three OSes.
- A `README.md` at the repo root explains how to run the test suites.

### Tasks

1. **chore(scaffold): tauri 2 project under /app**
   Initialize `app/` via `pnpm create tauri-app` with the React+TS+Vite
   template. Strip the demo content; commit only the working skeleton.
2. **chore(scaffold): rename frontend dir to src-fe/**
   The Tauri template puts the frontend in `src/`, which collides
   visually with `src-tauri/`. Rename it to `src-fe/` and update
   `vite.config.ts`, `tsconfig.json`, and `tauri.conf.json`
   (`build.frontendDist` / `build.devUrl` / any path aliases) to match.
   Verify `pnpm dev` and `pnpm build` still work before committing.
3. **chore(scaffold): pnpm workspace, vitest, eslint, prettier**
   Add `vitest`, `@testing-library/react`, `jsdom`. Write one trivial
   smoke test that imports `App.tsx` and renders without crashing. Watch
   it pass; commit.
4. **chore(scaffold): rust test infra**
   Add a single `mira::transport` module with a smoke test asserting
   `MockTransport::new().writes().is_empty()` is true. Red → green →
   commit.
5. **chore(ci): github actions matrix**
   `.github/workflows/ci.yml` running the full gate (cargo test, clippy,
   fmt, pnpm test, lint, typecheck, tauri build) on all three OSes.
6. **docs(readme): how to run, how to test, how to commit**
   Minimal contributor guide that points at this plan and the spec.

---

## 5. Phase 1 — HID Driver (`mira-rs`)

**Goal.** A fully tested Rust module that encodes every setting to the
correct HID packet and writes it through a pluggable transport. No
networking, no UI, no profiles. Just bytes.

**Exit criteria.**
- Every setting from [spec §6](specs.md#6-device-settings-reference) has
  an encoder with full range coverage in tests.
- `MockTransport` records writes; tests assert exact byte sequences.
- One ignored, real-device test exists per setting; running them against
  a plugged-in Mira passes locally.

### Tasks (each is a red → green → commit cycle)

1. **test+feat(mira): HidTransport trait with Mock and Real impls**
   Trait with `write_feature(report: &[u8]) -> Result<()>` and
   `read_feature(...)`. `MockTransport` exposes `writes()` for assertions.
2. **test+feat(mira): encode set_speed in [1..=7]**
   Table-driven test covering the full range; also tests clamping for
   out-of-range inputs.
3. **test+feat(mira): encode set_contrast in [0..=15]**
4. **test+feat(mira): encode set_dither_mode in [0..=3]**
5. **test+feat(mira): encode set_refresh_mode (a2 | direct)**
6. **test+feat(mira): encode set_color_filter (white + black, combined)**
   The device exposes whiten-background and deepen-blacks as parameters
   on a single opcode (0x11), so they're a single encoder taking both
   values. White is inverted on the wire (`255 - white`); both inputs
   clamp to spec range `[0..=127]`. Callers tracking only one of the
   pair must keep the partner value around to resend it. *(Plan
   divergence: originally specified as two separate `set_white_filter`
   and `set_black_filter` encoders; consolidated to match the wire
   protocol from `mira-js`.)*
7. **test+feat(mira): encode set_cold_light in [0..=254]**
8. **test+feat(mira): encode set_warm_light in [0..=254]**
9. **test+feat(mira): encode refresh() one-shot command**
10. **test+feat(mira): discovery enumerates VID 0x0416 / PID 0x5020**
    With a faked HID enumerator; real-device test ignored.
11. **test+feat(mira): write coalescer with 16ms window**
    Rapid `set_speed(3); set_speed(4); set_speed(5)` produces exactly one
    write of value 5.
12. **test+feat(mira): NAK is surfaced as a typed error**

### Protocol notes (confirmed against `mira-js`)

- **Refresh mode mapping.** The spec exposes two modes (`a2 | direct`)
  but the device firmware understands three (`direct_update=0x01`,
  `gray_update=0x02`, `a2=0x03`). Spec's "direct = full grayscale"
  corresponds to wire value `0x02` (`gray_update`), not `0x01`. The
  third mode (`direct_update`, black/white fast) isn't exposed in v1.
- **Speed inversion.** `set_speed(n)` sends `11 - n` on the wire — so
  spec value 1 (slowest) becomes wire byte `10`, and spec value 7
  (fastest) becomes wire byte `4`. This matches `mira-js` and is what
  the device actually expects.
- **White filter inversion.** `set_color_filter` sends `255 - white`
  on the wire; black is pass-through. A spec `white_filter` of 0 (no
  whitening) emits wire byte `0xFF`.
- **Range narrowing.** `mira-js` accepts `0..=254` for white/black
  filters, but the spec narrows to `0..=127` because higher values
  saturate the panel to all-white / all-black with no useful step
  granularity. Our encoders enforce the narrower spec range.

---

## 6. Phase 2 — Domain: Profiles, Presets, Session

**Goal.** Pure Rust domain layer. No I/O, no HID, no UI. Just the model
and rules.

**Exit criteria.**
- All six built-in presets exist with the exact values from
  [spec §7.1](specs.md#71-built-in-presets).
- `Session::apply(profile)` produces the correct sequence of writes to
  the mock transport.
- "As-found" profile is generated correctly from a snapshot of current
  device settings.

### Tasks

1. **test+feat(domain): Profile struct + serde round-trip**
2. **test+feat(domain): built-in preset Read**
3. **test+feat(domain): built-in preset Text**
4. **test+feat(domain): built-in preset Coding**
5. **test+feat(domain): built-in preset Speed**
6. **test+feat(domain): built-in preset Image**
7. **test+feat(domain): built-in preset Video**
8. **test+feat(domain): As-found profile generation from snapshot**
9. **test+feat(domain): profile validation clamps every field to spec range**
10. **test+feat(domain): Session::apply writes only diffs vs current state**
    Switching between two profiles that share `cold_light=0` does not
    re-send `cold_light`.
11. **test+feat(domain): duplicate / rename / reorder / delete custom profiles**
12. **test+feat(domain): built-in presets are read-only (delete/rename rejected)**

---

## 7. Phase 3 — Persistence

**Goal.** Atomic, versioned config file at the OS-standard path. Survives
crashes mid-write.

**Exit criteria.**
- Config writes are atomic (verified by a test that kills mid-write).
- Schema migrations preserve previous file as `config.v{old}.bak`.
- Corrupt config falls back to defaults and logs.

### Tasks

1. **test+feat(persistence): write atomic via temp+rename**
2. **test+feat(persistence): config path per OS via `directories` crate**
3. **test+feat(persistence): round-trip a full Config struct**
4. **test+feat(persistence): bump-and-backup migration from v0 → v1**
   Pretend there was a v0; assert the .bak file is created.
5. **test+feat(persistence): corrupt file falls back to defaults**

---

## 8. Phase 4 — Tauri Command Layer

**Goal.** A typed IPC surface that the frontend can call. Each command is
thin — it delegates to the domain layer and translates errors.

**Exit criteria.**
- Every command is integration-tested via `tauri::test::mock_app()`.
- Errors are typed and serialize to a discriminated union the frontend
  can pattern-match on.

### Tasks

1. **test+feat(commands): list_profiles returns presets + customs**
2. **test+feat(commands): apply_profile(id) → events emitted**
   Assert `profile:applied` event with the right payload.
3. **test+feat(commands): force_refresh triggers HID refresh once**
4. **test+feat(commands): get_device_status reports connected/disconnected**
5. **test+feat(commands): create/update/delete custom profile**
6. **test+feat(commands): error type serializes as `{ kind, message }`**

---

## 9. Phase 5 — Global Hotkeys

**Goal.** OS-level hotkey registration with the conflict-aware defaults
from [spec §8.1](specs.md#81-default-bindings--conflict-aware).

**Exit criteria.**
- Defaults register on launch and unregister on quit.
- Rebinding works at runtime without restart.
- Registration failure surfaces a non-blocking warning that the UI
  exposes.

### Tasks

1. **test+feat(hotkeys): binding model parses "Ctrl+Alt+1"**
2. **test+feat(hotkeys): defaults match spec §8.1 exactly**
3. **test+feat(hotkeys): rebind unregisters old and registers new**
4. **test+feat(hotkeys): registration failure produces typed Warning**
5. **test+feat(hotkeys): "Reset to defaults" command**

---

## 10. Phase 6 — Frontend Skeleton & Styling

**Goal.** A small, fast, e-ink-friendly frontend with zero icon assets and
no component library.

**Exit criteria.**
- `tokens.css` matches [spec §9.5](specs.md#95-e-ink-friendly-ui-default-always-on).
- Three windows boot: Popover, Editor, Settings (empty shells, but
  routable and styled).
- A component test for each window asserts the visual rules — no shadows,
  no gradients, pure-black-on-white.

### Tasks

1. **test+feat(ui): tokens.css defines #000/#fff/borders/spacing/type**
2. **test+feat(ui): Popover shell renders with token styles**
3. **test+feat(ui): Editor shell renders with left rail + right pane**
4. **test+feat(ui): Settings shell renders the four tab labels (General, Hotkeys, Device, About)**
5. **test+feat(ui): useFocusTrap hook traps focus inside a container**
6. **test+feat(ui): usePopoverPosition hook anchors to a coordinate**
7. **test+feat(ipc): typed wrappers over tauri.invoke and listen**

---

## 11. Phase 7 — Tray Popover (primary UI surface)

**Goal.** The everyday one-click workflow.

**Exit criteria.**
- Popover opens in < 200ms cold (measured manually + asserted via a Rust
  startup timing test).
- Profile switching from the popover applies via the command layer and
  reflects the new active profile.

### Tasks

1. **test+feat(popover): connection bar renders device status**
2. **test+feat(popover): active profile chip shows current profile name**
3. **test+feat(popover): profile grid lists first 8 profiles**
4. **test+feat(popover): "More…" submenu lists overflow profiles**
5. **test+feat(popover): clicking a profile invokes apply_profile**
6. **test+feat(popover): "Force full refresh" invokes force_refresh**
7. **test+feat(popover): "Quit" closes the app**
8. **test+feat(tray): tray entry shows wordmark "Mira" with state suffix**

---

## 12. Phase 8 — Profile Editor

**Goal.** Edit any custom profile with live, debounced previews.

**Exit criteria.**
- All nine settings editable via native `<input type="range">` with the
  step ticks specified in the spec.
- "Test on device" toggle gates live writes.
- Hotkey recorder validates and flags conflicts.

### Tasks

1. **test+feat(editor): name field renames the profile on blur**
2. **test+feat(editor): slider for each of the 9 settings, with range from spec §6**
3. **test+feat(editor): segmented control for refresh_mode (a2 | direct)**
4. **test+feat(editor): "Test on device" toggle, off by default**
5. **test+feat(editor): with toggle on, slider drag invokes set_* on device**
6. **test+feat(editor): with toggle off, drags update local state only**
7. **test+feat(editor): UI debounce ≥120ms verified by fake timers**
8. **test+feat(editor): hotkey recorder captures combo and validates**
9. **test+feat(editor): preset profiles are read-only (sliders disabled)**
10. **test+feat(editor): duplicate-preset action creates an editable copy**

---

## 13. Phase 9 — Settings Window

**Goal.** App-level preferences from [spec §9.3](specs.md#93-settings-window).

**Exit criteria.**
- General, Hotkeys, Device, About panes all functional.
- Launch-at-login toggle uses `tauri-plugin-autostart` and round-trips
  across restarts.

### Tasks

1. **test+feat(settings/general): launch-at-login toggle round-trips**
2. **test+feat(settings/hotkeys): list of bindings with recorder per row**
3. **test+feat(settings/hotkeys): "Reset to defaults" button restores spec defaults**
4. **test+feat(settings/device): variant override dropdown persists**
5. **test+feat(settings/device): "apply last profile on connect" toggle**
6. **test+feat(settings/about): version, license, links render**

---

## 14. Phase 10 — First-Run Experience

**Goal.** The flow from [spec §9.4](specs.md#94-first-run-experience).

**Exit criteria.**
- First launch shows the welcome card; subsequent launches don't.
- "As-found" profile is captured when a device is detected on first run.

### Tasks

1. **test+feat(first-run): welcome card shown when config absent**
2. **test+feat(first-run): "As-found" profile captured from device snapshot**
3. **test+feat(first-run): preset tour previews each of the six presets**
4. **test+feat(first-run): launch-at-login offered, default off**
5. **test+feat(first-run): "waiting for device" state when none connected**

---

## 15. Phase 11 — Multi-Device Surface (minimal)

**Goal.** Handle the rare case of multiple Miras plugged in without making
v1 multi-device-aware throughout.

**Exit criteria.**
- One Mira: works as today.
- Multiple Miras: notification offers a picker; first device used by
  default.

### Tasks

1. **test+feat(device): discovery returns all matching devices**
2. **test+feat(device): with >1 device, picker notification is emitted**
3. **test+feat(device): selected device persists across restarts**

---

## 16. Phase 12 — Build, Packaging, Distribution

**Goal.** Shippable artifacts per platform per [spec §12.2](specs.md#122-platform-notes).

**Exit criteria.**
- Tagged release produces signed installers for all three OSes via CI.
- Linux AppImage offers to install the udev rule from
  [spec §12.2](specs.md#122-platform-notes) on first run.

### Tasks

1. **chore(build): tauri release config per OS**
2. **chore(ci): release workflow triggered by `v*` tag**
3. **chore(macos): code-signing + notarization via secrets**
4. **chore(windows): MSI + NSIS, signing via secrets**
5. **test+feat(linux): udev rule installer prompts and writes the rule**
6. **chore(docs): install + troubleshooting per OS in README**

---

## 17. Phase 13 — Performance & Polish

**Goal.** Hit the budgets from the spec.

**Exit criteria.**
- Popover open-to-interactive < 200ms cold (measured on a Mac mini M1).
- Idle RAM < 80 MB.
- App start-to-tray < 1s.

### Tasks

1. **test+chore(perf): startup timing harness with budget assertions**
2. **chore(perf): lazy-load Editor and Settings windows**
3. **chore(ui): audit every component against the e-ink visual rules**
4. **chore(a11y): keyboard navigation works for every interactive element**

---

## 18. Out-of-Scope Reminders

These belong to v2 per [spec §13](specs.md#13-future-work) and must not
leak into v1 phases:

- Per-application auto-switching.
- Profile import/export and shareable links.
- Live preview / test patterns.
- Multi-device UI beyond the minimal picker in Phase 11.
- Light-sensor coupling.
- Telemetry.

If a v1 task starts pulling toward one of these, stop and re-scope.

---

## 19. Quick Reference — TDD Loop Checklist

For every change, big or small:

- [ ] Wrote a test that names the new behavior or reproduces the bug.
- [ ] Ran the test. It failed for the right reason.
- [ ] Wrote the minimum code to make it pass.
- [ ] Whole suite still green.
- [ ] Linter and formatter pass.
- [ ] Committed with a Conventional Commits message.
- [ ] One logical change per commit.
- [ ] No `Co-Authored-By` trailer.
