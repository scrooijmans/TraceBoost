# contracts

`contracts/` is the shared schema layer for the TraceBoost monorepo.

These contracts are product-facing. They describe what the TraceBoost application needs to move across its own app/runtime/frontend boundaries, while canonical reusable subsurface meaning is pushed down into Ophiolite where that ownership is stable.

## Stack And Formats

- Rust 2024 crates:
  - `seis-contracts-core`
  - `seis-contracts-views`
  - `seis-contracts-operations`
  - `seis-contracts-interop`
- `serde` for JSON serialization
- `schemars` for JSON Schema export
- `ts-rs` for generated TypeScript types
- generated frontend package at `ts/seis-contracts`

The contracts layer defines the typed payloads that cross:

- runtime <-> app/backend
- app/backend <-> Tauri frontend
- monorepo <-> external frontend consumers such as `Ophiolite Charts` integrations

Current architectural direction:

- the existing `seis-contracts-core`, `seis-contracts-views`, `seis-contracts-operations`, and `seis-contracts-interop` split is a migration step toward clearer long-term ownership
- Ophiolite contract ownership is now split by concern under `ophiolite-seismic/src/contracts/`:
  - `domain.rs`
  - `processing.rs`
  - `models.rs`
  - `views.rs`
  - `operations.rs`
- TraceBoost crates now expose matching compatibility namespaces:
  - `seis-contracts-core::{domain, processing, models, operations, views}`
  - `seis-contracts-views::{section, gather}`
  - `seis-contracts-operations::{datasets, import_ops, processing_ops, workspace, resolve}`
  - `seis-contracts-interop::*` as a compatibility re-export of `seis-contracts-operations`
- the owning Rust source for app/workflow operations now lives in `seis-contracts-operations`; `seis-contracts-interop` remains only to avoid a breaking rename across downstream consumers
- packed frontend section transport is now explicit in `app/traceboost-frontend/src/lib/transport/packed-sections.ts` instead of living only as bridge-local helpers
- see `../articles/architecture/CONTRACT_ARCHITECTURE_AND_MIGRATION.md` for the target layout and migration plan

## Implemented

- dataset and volume descriptors
- section-axis and section-view contracts
- preview/view request-response contracts
- survey preflight request-response contracts
  - includes resolved stacking/layout metadata so apps can distinguish post-stack vs prestack before ingest
- dataset import request-response contracts
- dataset open/summary request-response contracts
- dataset registry and workspace-session payloads for the desktop shell
- schema-versioned IPC types for the first desktop workflow

Regenerate the TypeScript artifact from the repo root with:

```powershell
.\scripts\generate-ts-contracts.ps1
```

The generated output currently lives under:

- `ts/seis-contracts/src/generated/`
- `ts/seis-contracts/schemas/seis-contracts.schema.json`

## Roadmap

1. Keep this layer as the only source of truth for app/runtime/frontend payloads.
2. Incrementally separate semantic contracts from transport-specialized payloads instead of growing bridge-local wire shapes ad hoc.
3. Add only the next app-facing contracts that the desktop workflow actually needs:
   error envelopes, progress events, and richer workspace/session payloads.
4. Avoid premature job-system or processing-batch schema growth until those features are implemented in the app.

## Non-Goals

This layer must not own:

- SEG-Y parsing
- runtime-store layout or chunk access
- processing kernels
- product workflow logic
