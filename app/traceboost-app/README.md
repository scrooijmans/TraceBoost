# traceboost-app

`traceboost-app` is the Rust app/backend control plane inside the TraceBoost monorepo.

It is the product-orchestration layer above the shared core. This crate turns canonical runtime and contract capabilities into TraceBoost workflows, and it feeds both the desktop shell and the automation surface.

## Stack And Role

- Rust 2024 binary + library crate
- `clap` for the developer-facing CLI
- `serde` / `serde_json` for app-facing payloads
- depends on:
  - `seis-contracts-operations`
  - `seis-runtime`
  - `seis-io`

This crate is where product-level commands should live. It should orchestrate the runtime; it should not absorb raw SEG-Y parsing or runtime-store internals.

The intended shape is one Rust workflow layer with multiple control surfaces on top of it:

- desktop/Tauri commands
- CLI commands
- thin Python automation wrappers

Those surfaces should reuse shared workflow orchestration here rather than reimplementing product flows independently.

## Implemented

- reusable library helpers for:
  - survey preflight
  - dataset import
  - dataset open/summary
  - survey-map resolution
  - native coordinate-reference assignment
  - survey time-depth demo/model workflows
  - a shared `TraceBoostWorkflowService` for app-facing orchestration
- CLI commands for:
  - backend info
  - inspect/analyze
  - ingest/validate
  - preflight-import
  - import-dataset
  - open-dataset
  - set-native-coordinate-reference
  - resolve-survey-map
  - view-section
  - load-velocity-models
  - ensure-demo-survey-time-depth-transform
  - prepare-survey-demo
  - import-velocity-functions-model

## Roadmap

1. Keep the current import/open/view workflow stable for both CLI and Tauri consumers.
2. Add app-facing error and progress surfaces suitable for desktop UX.
3. Add lightweight session/recent-dataset support here or in a closely related app crate.
4. Keep lower-level processing logic in `seis-runtime`, not here.
