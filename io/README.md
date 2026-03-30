# io

`io/` contains the `seis-io` package, which is the SEG-Y ingest layer inside the TraceBoost monorepo.

This area owns:

- SEG-Y inspection
- header loading
- geometry analysis
- chunked trace reads
- cube assembly and export-oriented handoff helpers

This area does not own:

- the canonical working-store layout
- app orchestration
- viewer contracts

Shared seismic fixtures now live at the monorepo root in `test-data/`.
