# runtime

`runtime/` contains `seis-runtime`, the backend runtime layer for TraceBoost.

## Stack And Formats

- Rust 2024 library crate: `seis-runtime`
- `zarrs` for the current chunked runtime-store implementation
- `ndarray` and `rayon` for CPU-first data handling and processing
- `serde` / JSON at the boundary to app-facing contracts

Runtime takes raw ingest output from `io/` and turns it into the canonical working dataset used by the app.

## Implemented

- SEG-Y preflight helpers on top of `seis-io`
- ingest into the current chunked Zarr-backed runtime store
- reopen/describe existing stores
- `VolumeDescriptor` and `DatasetSummary`-ready metadata for app surfaces
- section-view generation for inline/xline browsing
- validation and processing entry points that can expand later

Shared fixtures live in `test-data/`.

## Roadmap

1. Keep the first app path stable:
   preflight -> ingest -> open -> section view.
2. Add small app-facing helpers around recent stores, error mapping, and session-friendly dataset summaries.
3. Preserve store compatibility while cleaning internal naming and runtime APIs.
4. Defer deeper cache/residency and backend GPU work until the first desktop workflow is proven and profiled.

## Constraints

- This layer owns the runtime-store contract, not the frontend.
- It is CPU-first today by design.
- Existing stores still use compatibility-oriented metadata such as `seisrefine.manifest.json`; that should not be broken casually.
