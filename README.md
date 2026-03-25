# TraceBoost

TraceBoost is the product shell for a local-first seismic refinement application. The Rust backend now starts in this repo as `seisrefine`, a reusable library and CLI focused on conservative numeric upscaling of SEG-Y-derived volumes.

## Current Direction

- `TraceBoost` remains the future Tauri GUI and user-facing desktop application
- `seisrefine` is the backend crate that owns SEG-Y ingest, chunked derived storage, interpolation, and export primitives
- `sgyx` is used as the SEG-Y ingest dependency
- backend v1 targets regular post-stack cubes and 2x inter-trace densification
- backend v1 is CPU-first, Rust-native, and correctness/provenance-first

## Workspace

This repository now contains a Cargo workspace with:

- `crates/seisrefine`: Rust library + CLI

Example commands:

```bash
cargo run -p seisrefine -- inspect ../sgyx/test-data/small.sgy
cargo run -p seisrefine -- ingest ../sgyx/test-data/small.sgy ./small.zarr
cargo run -p seisrefine -- upscale ./small.zarr ./small-2x.zarr
cargo run -p seisrefine -- render ./small-2x.zarr ./inline.csv --axis inline --index 0
```

## Git Remotes

- `origin` remains the main `TraceBoost` repository
- `seisrefine` is the dedicated backend repository remote: `https://github.com/tuna-soup/seisrefine.git`

To publish only the backend crate to the separate repo, use:

```powershell
.\scripts\push-seisrefine.ps1
```

That command performs a `git subtree split` of `crates/seisrefine` and pushes only that history to the `seisrefine` remote.

## Notes

- the `docs/` directory remains the planning and research baseline
- the current implementation is intentionally backend-first; no Tauri code has been added yet
- learned super-resolution is deferred until the deterministic interpolation path and validation harness are stable
