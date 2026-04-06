# TraceBoost

TraceBoost is the backend/product monorepo for the seismic desktop application stack.

It is currently the implementation home for the seismic product workflow, but the longer-term target is for shared canonical seismic SDK layers to live in the sibling `ophiolite` repository while TraceBoost stays focused on product orchestration and desktop UX.

The current product milestone is:

`select SEG-Y -> preflight/ingest -> open runtime store -> view inline/xline sections in the app shell`

TraceBoost remains post-stack-first today, but preflight now distinguishes resolved seismic layout metadata such as post-stack vs prestack so unsupported surveys can be identified explicitly before ingest.

The backend now also supports the first processing flow:

`open tbvol -> define versioned operator pipeline -> preview on current 2D section -> materialize derived tbvol on full volume -> persist lineage/preset`

The desktop shell now also persists a lightweight workspace layer:

`remember linked SEG-Y paths + imported stores + last active section/preset across launches`

## Stack

- Rust 2024 workspace for contracts, I/O, runtime, and app/backend orchestration
- `serde`, `schemars`, and `ts-rs` for shared JSON and generated TypeScript contracts
- `clap` for developer-facing CLI surfaces
- shared seismic core/runtime crates in the sibling `ophiolite` repository
- Svelte 5 + Vite + Bun for the frontend host in `app/traceboost-frontend`
- Tauri 2 for the desktop shell in `app/traceboost-frontend/src-tauri`
- external `geoviz` packages for 2D seismic rendering

## Data Formats

- input survey format: SEG-Y
- working dataset format: `tbvol` tiled runtime store
- app/runtime boundary: JSON payloads typed by `seis-contracts-*` and generated into `@traceboost/seis-contracts`

## Monorepo Layout

- `contracts/`
  - shared Rust contracts and IPC-safe schemas
  - generated TypeScript artifact under `contracts/ts/seis-contracts/`
- `io/`
  - SEG-Y inspection, geometry extraction, chunked reads, and ingest-oriented helpers
- `runtime/`
  - TraceBoost compatibility wrapper over the shared Ophiolite seismic runtime
- `app/`
  - product-facing Rust orchestration, frontend host, and Tauri shell
- `test-data/`
  - shared fixtures used across `io`, `runtime`, and app tests
- `docs/`
  - canonical architecture docs plus archived legacy material
- `scripts/`
  - repository-level utilities such as contracts export

## What Works Today

- shared Rust and TypeScript contracts exist for:
  - dataset descriptions
  - section requests/responses
  - survey preflight including resolved stacking/layout metadata
  - dataset import/open flows
- `seis-io` can inspect SEG-Y files, load headers, analyze geometry, and feed ingest paths
- `seis-runtime` re-exports the shared Ophiolite seismic runtime used to preflight SEG-Y, ingest into `tbvol`, reopen stores, describe datasets, and generate section views
- `traceboost-app` now exposes reusable backend helpers and CLI commands for:
  - `preflight-import`
  - `import-dataset`
  - `open-dataset`
  - `view-section`
  - processing preview/materialization helpers over versioned operator pipelines
- `traceboost-frontend` can:
  - preflight a SEG-Y path
  - ingest to a runtime-store path
  - open an existing runtime store
  - keep a persisted workspace list of linked SEG-Y/store entries
  - restore the last active dataset, section position, and selected preset on relaunch
  - load inline/xline sections into the embedded `geoviz` chart
  - call backend commands for processing preview, processing jobs, and pipeline presets
- a Tauri shell scaffold exists and is wired to the same backend commands

## Immediate Roadmap

1. Tighten the first working desktop path in `traceboost-frontend` and `src-tauri`.
2. Expand the persisted workspace from one active dataset into richer multi-dataset session workflows.
3. Harden user-facing error handling and progress reporting around ingest/open/view flows.
4. After the desktop shell is stable, expand into validation, refinement, and richer processing workflows.

## Contributor Commands

Run the Rust workspace:

```powershell
cargo test
```

Regenerate the TypeScript contracts:

```powershell
.\scripts\generate-ts-contracts.ps1
```

Work on the frontend host:

```powershell
Set-Location app\traceboost-frontend
bun install
bun run dev
```

The frontend resolves sibling source from both `../geoviz` and `../ophiolite` during local development. Keep those repositories checked out next to `TraceBoost`.

Run the desktop shell:

```powershell
Set-Location app\traceboost-frontend
bun run tauri:dev
```

## Notes

- `traceboost-frontend` consumes the sibling `../geoviz` repository through direct local `file:` dependencies. Keep that checkout present next to `TraceBoost`.
- the longer-term shared-core direction is `TraceBoost app/product repo -> ophiolite shared subsurface core -> geoviz visualization SDK`
- the repository pins Rust `1.91.0` in `rust-toolchain.toml` so `cargo` and Tauri pick a supported compiler automatically.
- `geoviz` remains an external visualization SDK and is not vendored into this monorepo.
- `seisview-js` is not part of the production architecture here.
- legacy imported docs live under `docs/legacy/`.
