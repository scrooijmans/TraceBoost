# contracts

`contracts/` is the shared schema layer for the TraceBoost monorepo.

## Stack And Formats

- Rust 2024 crates:
  - `seis-contracts-core`
  - `seis-contracts-views`
  - `seis-contracts-interop`
- `serde` for JSON serialization
- `schemars` for JSON Schema export
- `ts-rs` for generated TypeScript types
- generated frontend package at `ts/seis-contracts`

The contracts layer defines the typed payloads that cross:

- runtime <-> app/backend
- app/backend <-> Tauri frontend
- monorepo <-> external frontend consumers such as `geoviz` integrations

## Implemented

- dataset and volume descriptors
- section-axis and section-view contracts
- preview/view request-response contracts
- survey preflight request-response contracts
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
2. Add only the next app-facing contracts that the desktop workflow actually needs:
   error envelopes, progress events, and richer workspace/session payloads.
3. Avoid premature job-system or processing-batch schema growth until those features are implemented in the app.

## Non-Goals

This layer must not own:

- SEG-Y parsing
- runtime-store layout or chunk access
- processing kernels
- product workflow logic
