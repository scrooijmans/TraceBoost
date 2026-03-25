# TraceBoost

TraceBoost is the product shell for a local-first seismic refinement application.

The backend now lives as a separate project in the sibling repository:

- local path: `../seisrefine`
- remote: `https://github.com/tuna-soup/seisrefine.git`

## Current Direction

- `TraceBoost` remains the future Tauri GUI and user-facing desktop application
- `seisrefine` is the separate backend project that owns SEG-Y ingest, chunked derived storage, interpolation, validation, and export primitives
- `sgyx` is used as the SEG-Y ingest dependency
- backend v1 targets regular post-stack cubes and 2x inter-trace densification
- backend v1 is CPU-first, Rust-native, and correctness/provenance-first

## Repository Layout

This repository no longer carries the backend implementation directly. The authoritative backend development flow is:

- `TraceBoost/`: product shell, docs, future Tauri app
- `../seisrefine/`: backend Rust project
- `../sgyx/`: SEG-Y ingest library

Example commands:

```bash
cd ../seisrefine
cargo run -- inspect ../sgyx/test-data/small.sgy
cargo run -- validate ./target/validation-reports
```

## Notes

- the `docs/` directory remains the planning and research baseline
- the backend is now intentionally developed outside this repo; no Tauri code has been added yet
- learned super-resolution is deferred until the deterministic interpolation path and validation harness are stable
