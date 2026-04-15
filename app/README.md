# app

`app/` contains the application layer for TraceBoost.

This is the layer where the platform becomes a user-facing workflow:

- Ophiolite provides the canonical core beneath it
- Ophiolite Charts provides the embedded visualization SDK beneath it
- TraceBoost owns the workflow shell that users actually operate

## Stack

- Rust 2024 for backend orchestration and command surfaces
- `traceboost-app` as the app/backend control plane crate
- Svelte 5 + Vite + Bun in `traceboost-frontend`
- Tauri 2 in `traceboost-frontend/src-tauri`
- generated contracts from `contracts/ts/seis-contracts`
- external `@ophiolite/charts` for section rendering
- sibling `@ophiolite/contracts` for generated gather and well-panel DTOs consumed through Ophiolite Charts

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
    - includes resolved stacking/layout metadata for unsupported prestack detection
  - dataset import
  - dataset open/summary
  - section loading
- CLI commands that expose the same backend path for local development
- frontend UI for:
  - SEG-Y input path entry
  - remembered dataset registry with active selection
  - runtime-store path entry
  - preflight/import/open actions
  - inline/xline section viewing
  - restore of the last active dataset/section on desktop relaunch
- Tauri command wiring for the same workflow

## Roadmap

1. Make the desktop shell the normal way to run the app, not just the browser dev host.
2. Expand recent-dataset/session management into richer multi-dataset workspace workflows.
3. Improve progress reporting and user-facing error presentation during ingest and open.
4. After the import/view path is solid, add validation and refinement workflows.

## Boundary Rule

This area should depend on `runtime/`, not reach around it directly into lower layers unless there is a deliberate and documented exception.
