# io

`io/` contains `seis-io`, the SEG-Y-focused ingest layer inside the TraceBoost monorepo.

## Stack And Formats

- Rust 2024 library crate: `seis-io`
- `memmap2` for efficient file access
- `rayon` for parallel ingest-oriented work
- SEG-Y as the primary input format

This layer is responsible for understanding raw survey files well enough to hand them off to `runtime/`.

## Implemented

- SEG-Y inspection and metadata probing
- textual/EBCDIC and binary header loading
- trace-header-driven geometry analysis
- chunked trace reads
- cube assembly helpers used by ingest flows
- fixture-backed tests and benchmarks

Shared seismic fixtures live at the monorepo root in `test-data/`.

## Roadmap

1. Keep this layer focused on SEG-Y and adjacent raw-ingest concerns.
2. Tighten any gaps that block the first desktop workflow:
   robust preflight metadata, clearer geometry diagnostics, and stable ingest handoff to `runtime/`.
3. Defer any attempt to turn this crate into a browsing API or product orchestration layer.

## Non-Goals

This area does not own:

- the canonical runtime-store layout
- dataset/session management
- app orchestration
- viewer contracts

`SGYX_DESIGN.md` is retained as historical predecessor design material from the old standalone repo. It is not the canonical architecture document for the monorepo.
