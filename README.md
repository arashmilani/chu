<p align="center">
  <img src="app/src-tauri/icons/source/icon-base.svg" alt="Chu" width="128" height="128">
</p>

# Chu

A small, fast, cross-platform desktop utility for controlling Boox Mira series
e-ink monitors.

## Motivation

The vendor's Mira app is a 216MB bundle that boots slowly, ignores the
fact that it's running on an e-ink panel, and leaves a few obvious knobs
unexposed. Chu aims to fix that:

- **Tiny.** ~10MB on disk and low idle memory — fitting for a utility
  that sits beside the writing/coding work the e-ink display enables.
- **Auto full-refresh in A2 mode.** When you're working in the fast A2
  refresh mode, Chu fires a full refresh on a configurable interval
  (off by default; opt in from Settings) to clear accumulated
  ghosting — but only while you're actively using the machine, so an
  idle panel isn't woken for nothing. The vendor app has no equivalent.
- **A UI that respects e-ink.** Ironically, the official app's interface
  is neither designed for e-ink nor responsive; Chu's is both.
- **One codebase, three platforms.** macOS, Windows, and Linux share the
  same UX and feature set — no second-class build.
- **Menu-bar first.** Sub-second startup, stays out of the way, and
  every documented device knob is one click away.

For the long version, see:

- [docs/specs.md](docs/specs.md) — product spec, protocol notes, UX rules
- [docs/plan.md](docs/plan.md) phased plan

## About the name

**Chu** (ちゅう) is the Japanese onomatopoeia for a mouse's squeak. Two
letters of sound, instantly memorable, and softly friendly — which suits a
small utility that quietly sits in your menu bar and adjusts your monitors.

## Repo layout

```
/
├── docs/                spec + implementation plan
└── app/
    ├── src-fe/          frontend (React + TypeScript + Vite)
    └── src-tauri/       backend (Rust, Tauri 2)
```

Everything ships under `app/`. The repo root only holds docs, CI config,
and this README.

## Prerequisites

- Node.js ≥ 22.12 (or ≥ 20.19); install via `nvm`
- pnpm 11+
- Rust stable toolchain (`rustup` recommended)
- Tauri 2 system deps for your OS:
  [tauri.app/start/prerequisites](https://tauri.app/start/prerequisites/)
- Linux only: `libudev-dev` (or your distro's equivalent) for HID access.

## Running locally

```sh
cd app
pnpm install
pnpm tauri dev          # full app (Rust + webview)
pnpm dev                # frontend only, served at http://localhost:1420
```

## Running the test suites

```sh
cd app

# Frontend
pnpm test               # vitest, single run
pnpm test:watch         # vitest, watch mode
pnpm typecheck          # tsc --noEmit
pnpm lint               # eslint
pnpm format:check       # prettier --check

# Backend
cd src-tauri
cargo test              # unit + integration
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Real-device tests (Mira plugged in)
cargo test -- --ignored
```

## Committing

Read [`docs/plan.md` §1](docs/plan.md) before your first commit.

CI runs the full gate (lint, typecheck, tests, clippy, fmt, tauri build)
on macOS, Windows, and Ubuntu. A red CI blocks merge to `main`.

## License

MIT
