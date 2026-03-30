# app

`app/` contains the product-facing application layer for TraceBoost.

## Stack

- Rust 2024 for backend orchestration and command surfaces
- `traceboost-app` as the app/backend control plane crate
- Svelte 5 + Vite + Bun in `traceboost-frontend`
- Tauri 2 in `traceboost-frontend/src-tauri`
- generated contracts from `contracts/ts/seis-contracts`
- external `@geoviz/svelte` for section rendering

## Current Contents

- `traceboost-app`
  - reusable Rust backend helpers and developer CLI
- `traceboost-frontend`
  - the current UI host for ingest/open/view flows
- `traceboost-frontend/src-tauri`
  - desktop shell commands that call into `traceboost-app` and `seis-runtime`

## Implemented

- app/backend helpers for:
  - survey preflight
  - dataset import
  - dataset open/summary
  - section loading
- CLI commands that expose the same backend path for local development
- frontend UI for:
  - SEG-Y input path entry
  - runtime-store path entry
  - preflight/import/open actions
  - inline/xline section viewing
- Tauri command wiring for the same workflow

## Roadmap

1. Make the desktop shell the normal way to run the app, not just the browser dev host.
2. Add recent-dataset/session management and file-picker UX.
3. Improve progress reporting and user-facing error presentation during ingest and open.
4. After the import/view path is solid, add validation and refinement workflows.

## Boundary Rule

This area should depend on `runtime/`, not reach around it directly into lower layers unless there is a deliberate and documented exception.
