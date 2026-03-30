# traceboost-app

`traceboost-app` is the Rust app/backend control plane inside the TraceBoost monorepo.

## Stack And Role

- Rust 2024 binary + library crate
- `clap` for the developer-facing CLI
- `serde` / `serde_json` for app-facing payloads
- depends on:
  - `seis-contracts-interop`
  - `seis-runtime`
  - `seis-io`

This crate is where product-level commands should live. It should orchestrate the runtime; it should not absorb raw SEG-Y parsing or runtime-store internals.

## Implemented

- reusable library helpers for:
  - survey preflight
  - dataset import
  - dataset open/summary
- CLI commands for:
  - backend info
  - inspect/analyze
  - ingest/validate
  - preflight-import
  - import-dataset
  - open-dataset
  - view-section

## Roadmap

1. Keep the current import/open/view workflow stable for both CLI and Tauri consumers.
2. Add app-facing error and progress surfaces suitable for desktop UX.
3. Add lightweight session/recent-dataset support here or in a closely related app crate.
4. Keep lower-level processing logic in `seis-runtime`, not here.
