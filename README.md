# Mira Boox Pro Controller

A small, fast, cross-platform desktop utility for controlling Boox Mira and
Mira Pro e-ink monitors. Tauri 2 + Rust + React.

For the long version, see:

- [docs/specs.md](docs/specs.md) — product spec, protocol notes, UX rules
- [docs/plan.md](docs/plan.md) — phased, TDD-first implementation plan

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

The TDD loop is what makes this project possible. Both halves of the
suite must be green for every commit.

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

MIT — see [LICENSE](LICENSE) once added in Phase 12.
