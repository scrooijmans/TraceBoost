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
  - product-facing application workflow and Tauri-facing desktop command surface

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

## Processing Flow

The current backend processing design is:

- canonical post-stack processing model is a typed, versioned `TraceLocalProcessingPipeline`
- preview requests run synchronously against the currently requested inline/xline section
- full-volume apply runs as a background job and always writes a new derived `tbvol`
- reusable operator sequences are persisted as pipeline presets
- derived stores persist full processing lineage, including the exact pipeline revision and operations used

This keeps the backend deterministic and frontend-safe without introducing a scripting language as the source of truth.

The current live shared operator family is trace-local:

- `amplitude_scalar`
- `trace_rms_normalize`
- `agc_rms`
- `phase_rotation`
- `lowpass_filter`
- `highpass_filter`
- `bandpass_filter`

This is a deliberate scope boundary:

- trace-local operators belong in the shared `TraceLocalProcessingOperation` path
- gather-native prestack operators belong in a separate `GatherProcessingOperation` path with dedicated `tbgath` ingest/store/preview/materialization APIs in Ophiolite
- section/gather-matrix operators should be modeled separately instead of being forced into the trace-local executor
- inverse-wavelet operators should also be treated as a separate scope because they carry different assumptions, parameters, and validation needs
- analysis flows such as amplitude spectrum inspection should stay separate from materializing processing operators

Current TraceBoost still only owns the trace-local post-stack UI. The prestack backend now exists in Ophiolite, including dedicated offset-gather materialization APIs plus separate velocity scan / semblance analysis requests with optional autopicked time-velocity estimates, but there is no sibling prestack app wired to it yet.

The detailed implementation plan for that work lives in `docs/spectral-processing-implementation.md`.

## Modeling, Analysis, and Display Separation

As TraceBoost expands beyond trace-local seismic processing, it should preserve the shared taxonomy owned by Ophiolite instead of collapsing everything into "operators" or frontend-only view models.

The working separation is:

- `Source Assets`
  - imported seismic volumes, horizons, sparse velocity functions, wells/logs, velocity cubes
- `Authored Models`
  - layered velocity models and future horizon-guided property models
- `Compiled Runtime Assets`
  - build outputs such as `SurveyTimeDepthTransform3D` and future property fields
- `Analysis APIs`
  - diagnostic or estimation workflows such as velocity scans and model QC
- `Display DTOs`
  - chart/map-facing resolved sections, overlays, and previews

Design rules for TraceBoost:

- TraceBoost owns workflow, activation, orchestration, and user-facing diagnostics for those families
- Ophiolite owns canonical model contracts, CRS/geometry/coverage checks, and model-build/runtime logic
- geoviz remains a rendering consumer of resolved DTOs and must not become the source of authored-model semantics
- not every future computation belongs in the shared operator family; authored-model and model-build workflows stay separate from processing operators

This matters directly for future velocity/property modeling:

- a velocity cube or sparse velocity functions are source assets
- a layered velocity model is an authored model
- the derived time-depth transform is a compiled runtime asset
- section/map overlays are display DTOs

## Workspace Persistence

The current desktop persistence split is:

- `seis-runtime` / shared Ophiolite seismic runtime
  - canonical seismic ingest/open/section/materialization logic
- `traceboost-frontend/src-tauri`
  - app-local dataset registry
  - app-local workspace session snapshot
  - persisted pipeline presets
- `traceboost-frontend`
  - reactive viewer/session state
  - active dataset selection from the remembered registry
  - restore of the last active dataset/section on startup
- `geoviz`
  - rendering only; it does not know about recent datasets or session persistence

This keeps “remember what I was working on” as a desktop/workspace concern rather than forcing it into canonical seismic storage or chart models.

## CRS And Map Workspace

For seismic map display and future survey/well overlays:

- Ophiolite owns native/effective CRS truth and any future reprojection
- TraceBoost owns the workspace display-CRS preference and related warnings
- geoviz consumes resolved geometry and does not assign or transform CRS metadata

The phase-1 contract for that split is documented in:

- `docs/crs-display-workspace-phase1.md`

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
