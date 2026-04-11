# TraceBoost Articles

This folder collects long-form engineering notes and benchmark writeups that were previously kept in the repository root.

## Categories

### `architecture/`

- `GEOSPATIAL_CRS_ARCHITECTURE_AND_DESIGN.md`
- `CONTRACT_ARCHITECTURE_AND_MIGRATION.md`

### `benchmarking/`

- `PREVIEW_INCREMENTAL_EXECUTION_BENCHMARK_PLAN.md`

### `performance/`

- `PROCESSING_CACHE_ARCHITECTURE_AND_BENCHMARKING.md`
- `TRACEBOOST_PERFORMANCE_PROFILING_AND_OPTIMIZATIONS.md`

### `storage/`

- `SEISMIC_VOLUME_STORAGE_AND_BENCHMARKING.md`
- `SEISMIC_VOLUME_STORAGE_AND_BENCHMARKING_II.md`
- `TBVOL_EXACT_COMPRESSED_STORAGE_PROPOSAL.md`

## Notes

- The root `README.md` remains in the repository root as the entrypoint for contributors.
- The storage articles are intentionally split:
  - Part I explains why `tbvol` replaced the earlier runtime-store candidates for active compute.
  - Part II documents the later exact-lossless compression study and what it implies for `tbvol` as a processing and storage substrate.
  - The proposal article turns those findings into a concrete product shape for an optional exact compressed storage tier.
