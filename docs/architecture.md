# TraceBoost Architecture

## Summary

TraceBoost is the backend/product monorepo for the seismic application stack.

The active backend/product architecture is:

- `contracts/`
  - shared contracts and IPC-safe schemas
  - generated frontend artifact at `contracts/ts/seis-contracts/`
- `io/`
  - SEG-Y inspection, header loading, geometry analysis, and ingest-oriented reads
- `runtime/`
  - TraceBoost compatibility wrapper over the shared Ophiolite seismic runtime
- `app/`
  - product-facing application workflow and future Tauri-facing crates

`geoviz` remains outside this repository as the visualization SDK boundary.

The target ecosystem boundary is:

- `ophiolite` owns shared subsurface SDK layers and canonical domain/app-boundary types over time
- `TraceBoost` owns product-facing seismic workflow composition, desktop UX, and app-specific orchestration
- `geoviz` remains the visualization SDK and adapter boundary

## Design Rules

- CPU-first processing is the default path
- backend GPU compute remains a deliberate future option
- one root Cargo workspace governs the Rust/backend side
- one shared top-level `test-data/` directory is used across backend/product tests
- dependency direction is strict:
  - `app -> runtime -> io -> contracts`
- no generic `shared/` or `common/` bucket is allowed
- as shared seismic core concerns move into `ophiolite`, TraceBoost should consume them rather than recreate a second canonical core here

## Current Package Map

- `seis-contracts-core`
- `seis-contracts-views`
- `seis-contracts-interop`
- `seis-io`
- `seis-runtime`
- `traceboost-app`

## Compatibility Notes

- old standalone repos for contracts, I/O, and runtime have been deprecated in favor of this monorepo
- canonical runtime storage now lives in the shared Ophiolite seismic runtime and uses `tbvol`
- internal Rust import names may still lag behind package names in some places; package identity and repo boundary are the authoritative naming layer

## Testing And CI

- package-level CI exists for contracts, I/O, runtime, and app
- generated TypeScript contracts are regenerated and checked in CI
- one full workspace integration run validates the monorepo as a whole
- local verification entrypoint remains:

```bash
cargo test
```

TypeScript contract regeneration entrypoint:

```powershell
.\scripts\generate-ts-contracts.ps1
```
