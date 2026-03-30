# TraceBoost

TraceBoost is now the monorepo for the backend/product side of the seismic application stack.

## Monorepo Layout

- `contracts/`
  - shared Rust contracts and IPC-safe schemas
  - generated TypeScript contract artifact under `contracts/ts/seis-contracts/`
- `io/`
  - SEG-Y ingest and geometry extraction
- `runtime/`
  - working-store, processing, validation, and runtime-facing APIs
- `app/`
  - product-facing application crates and frontend hosts
- `test-data/`
  - shared seismic fixtures used across `io`, `runtime`, and app integration tests
- `docs/`
  - current architecture notes plus explicitly archived legacy imports
- `scripts/`
  - repository-level support tooling

## Current Direction

- CPU-first processing remains the default path
- the backend keeps a deliberate path open for future GPU compute
- the canonical working-volume layout remains the current chunked Zarr-backed store in `runtime/`
- the product app stays separate from the visualization SDK; `geoviz` remains outside this repo

## Workspace

This repo uses one root Cargo workspace for the Rust/backend side:

- `seis-contracts-core`
- `seis-contracts-views`
- `seis-contracts-interop`
- `seis-io`
- `seis-runtime`
- `traceboost-app`
- `contracts-export`

Frontend-facing generated contract artifact:

- `contracts/ts/seis-contracts`

Current frontend host:

- `app/traceboost-frontend`
  - Svelte/Vite host that consumes `@traceboost/seis-contracts`
  - embeds external `@geoviz/svelte`

Run the full backend/product test suite with:

```bash
cargo test
```

Regenerate the TypeScript contract artifact with:

```powershell
.\scripts\generate-ts-contracts.ps1
```

Layered CI now treats this repo as the only active backend/product source of truth:

- contracts job
- I/O job
- runtime job
- app job
- full workspace integration job

## Notes

- `geoviz` remains an external visualization SDK and is not vendored into this monorepo
- `seisview-js` is not part of the production architecture here
- old standalone repos are being deprecated in favor of this monorepo layout
- legacy imported docs now live under `docs/legacy/`
