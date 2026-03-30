# runtime

`runtime/` contains the `seis-runtime` package, which is the backend runtime layer inside the TraceBoost monorepo.

This area owns:

- ingest routing on top of `io/`
- canonical working-store creation and access
- shared-runtime dataset description helpers
- section/view generation for app-facing contracts
- processing, validation, and derived-store writeback

This area is CPU-first today, but it is the place where backend GPU compute would be introduced later if profiling justifies it.

Shared test fixtures live at the monorepo root in `test-data/`.
