# Compute And Storage Benchmark Plan

## Purpose

This document defines the benchmark and decision rubric for the TraceBoost runtime compute layer.

The benchmark exists to answer one architectural question with measured data:

- should TraceBoost keep Zarr as the canonical runtime store for compute-heavy workflows, or should a different persistence layout replace it?

Current working assumption:

- benchmark the storage substrate through the same compute executor the product will use
- prefer the backend that wins on both preview latency and full-volume apply, even if that requires leaving Zarr behind

## Implemented Status

The benchmark, storage abstraction, and compute substrate are now implemented in-repo.

Implemented backend pieces:

- storage-neutral backend interfaces:
  - `VolumeStoreReader`
  - `VolumeStoreWriter`
- shared tile geometry and section assembly
- runtime compute kernels for:
  - `amplitude_scalar { factor }`
  - `trace_rms_normalize`
- preview execution on numeric section planes
- full-volume materialization to a derived store
- subset-based section reads that avoid loading the full volume
- `tbvol` backend:
  - fixed-layout tiled `f32` payload
  - mmap-backed reads
  - positioned writes
- Zarr adapter backend used as benchmark/reference path
- runtime manifest metadata for:
  - processing lineage
  - storage layout
- benchmark CLI:
  - `runtime/src/bin/compute_storage_bench.rs`
- Criterion harness:
  - `runtime/benches/compute_storage.rs`

Current state:

- the shared backend now lives in `ophiolite-seismic-runtime`
- TraceBoost runtime re-exports that shared backend as a compatibility layer
- ingest/open/preview/materialize in the shared runtime now operate on the `tbvol` path by default
- Zarr remains present only as a benchmark/reference backend and compatibility helper

## Measured Findings

The benchmark was rerun in release mode through the storage-neutral compute path on:

- real control dataset: `test-data/f3.sgy`
- synthetic medium dataset: `256 x 256 x 1024`
- synthetic large dataset: `384 x 384 x 1024`

Control finding:

- `f3.sgy` is too small to be decision-driving; storage/container overhead dominates

Measured backend comparison with `4 MiB` full-trace tiles:

- medium synthetic `256 x 256 x 1024`
  - `zarr_uncompressed_unsharded`
    - preview pipeline: `39.390 ms`
    - full apply pipeline: `688.762 ms`
    - file count: `67`
  - `tbvol`
    - preview pipeline: `0.503 ms`
    - full apply pipeline: `403.393 ms`
    - file count: `2`
  - `flat_binary_control`
    - preview pipeline: `1.043 ms`
    - full apply pipeline: `595.457 ms`
    - file count: `2`
- large synthetic `384 x 384 x 1024`
  - `zarr_uncompressed_unsharded`
    - preview pipeline: `61.177 ms`
    - full apply pipeline: `2573.365 ms`
    - file count: `147`
  - `tbvol`
    - preview pipeline: `1.756 ms`
    - full apply pipeline: `958.889 ms`
    - file count: `2`
  - `flat_binary_control`
    - preview pipeline: `1.831 ms`
    - full apply pipeline: `1572.882 ms`
    - file count: `2`
- small real control `f3.sgy`
  - still too small to drive the architectural decision
  - `tbvol` won preview and beat Zarr, but flat binary still had the cheapest full apply on this tiny case because container overhead dominates

Measured conclusion:

- the earlier “promote flat binary” direction is superseded
- the production-grade tiled implementation (`tbvol`) is now the best-performing backend overall
- `tbvol` beats Zarr decisively on both key workloads
- `tbvol` also beats the monolithic flat control on the larger synthetic case, which means the tile design is not just an overhead compromise; it is the correct compute substrate

Chunk-size sweep findings for `tbvol`:

- `1 MiB` tiles were too small and increased tile-count overhead
- `8 MiB` tiles were not consistently better and increased padding waste on non-divisible shapes
- the practical sweet spot is currently `2-4 MiB`
- `4 MiB` was strongest on the medium synthetic dataset
- `2 MiB` and `4 MiB` were both strong on the larger synthetic dataset

Implication:

- the old generic `8 MiB` chunk heuristic should not become the long-term `tbvol` policy
- future `tbvol` tile recommendation should be padding-aware and should target roughly `2-4 MiB`, not blindly maximize tile size

## Scope

This benchmark is about backend execution only.

It does not evaluate:

- UI or UX
- network services
- distributed execution
- GPU compute

It does evaluate:

- section read latency
- preview execution latency
- full-volume materialization throughput
- derived-store write costs
- file count and storage footprint

## Design Rules

- CPU-first compute remains the default path
- preview and full apply must use the same kernels
- source stores are immutable
- full apply writes a new derived store
- old stores remain readable even if new stores adopt improved chunking
- benchmark results, not preferences, determine the default storage policy

## V1 Operator Set

The first benchmark must use real backend kernels, not synthetic no-op transforms.

V1 operators:

- `amplitude_scalar { factor: f32 }`
- `trace_rms_normalize`

Constraints:

- `amplitude_scalar.factor` must validate to the inclusive range `[0.0, 10.0]`
- `trace_rms_normalize` computes RMS over the full sample axis of each trace
- traces with zero or near-zero RMS must use an epsilon-protected divisor
- sparse empty bins must be skipped when occupancy data exists

## Storage Candidates

The benchmark compares the following runtime-store candidates:

- `zarr_uncompressed_unsharded`
- `zarr_lz4_unsharded`
- `zarr_zstd_unsharded`
- `zarr_uncompressed_sharded`
- `zarr_lz4_sharded`
- `zarr_zstd_sharded`
- `tbvol`
- `flat_binary_control`

Notes:

- `tbvol` is the current leading production backend candidate
- `flat_binary_control` remains a control artifact, not a production commitment
- HDF5 and TileDB remain intentionally deferred

## Chunking Policy Under Test

The current fixed chunk default in the runtime is not assumed to be correct for compute.

For newly written compute-friendly stores, candidate chunk shapes must follow this rule:

- logical array shape stays `[iline, xline, sample]`
- inner chunk shape is `[ci, cx, samples]`
- the sample axis spans the full trace in each chunk

Rationale:

- both v1 operators are trace-local
- splitting traces across sample-axis chunks adds avoidable I/O and compute overhead

Inner chunk target sweep:

- `1 MiB`
- `2 MiB`
- `4 MiB`
- `8 MiB`

Sharded Zarr shard target sweep:

- `32 MiB`
- `64 MiB`
- `128 MiB`
- `256 MiB`

## Dataset Matrix

The benchmark must run against at least three dataset classes:

- `small`
- `medium`
- `large`

Recommended initial shapes:

- `small`: existing fixture from `test-data/`
- `medium`: synthetic seismic-like volume near `256 x 256 x 1024`
- `large`: synthetic seismic-like volume near `512 x 512 x 1500` if workstation limits allow

Synthetic data requirements:

- deterministic across runs
- `f32`
- seismic-like structure instead of white-noise-only content
- stable enough to compare compression, preview latency, and full-volume throughput

## Workload Matrix

Each storage candidate must support the same workload matrix:

- inline section read
- xline section read
- preview `amplitude_scalar`
- preview `trace_rms_normalize`
- preview pipeline of both operators
- full-volume apply `amplitude_scalar`
- full-volume apply `trace_rms_normalize`
- full-volume apply pipeline of both operators

Preview rules:

- preview operates on the full current section, not only the visible viewport
- preview is ephemeral and returned as a processed section payload
- preview uses the exact same operator kernels as full apply

## Metrics

Each benchmark result row must record:

- dataset class
- storage candidate
- compression mode
- sharding mode
- inner chunk target
- shard target when applicable
- workload name
- elapsed wall-clock time in milliseconds
- throughput in bytes per second or voxels per second
- output store size on disk
- file count
- run mode: `warm` or `approx_cold`

Optional metrics if easily available:

- peak RSS
- bytes read
- bytes written

## Cold And Warm Runs

The benchmark must report both warm and approximate-cold behavior.

Definitions:

- `warm`: immediate repeat run in the same local environment
- `approx_cold`: fresh process plus best-effort cache disturbance before the measured workload

The benchmark must clearly label cold-cache results as approximate on desktop operating systems.

## Runtime Architecture Hooks

The benchmark is only valid if it runs through the same backend substrate the product will use.

Required runtime work before final benchmarking:

- add numeric section reads without materializing the whole 3D array
- add storage-neutral numeric access APIs for sections and chunks
- implement the real v1 kernels once
- benchmark preview and full apply through the runtime compute path, not through rendering-only payload transforms

## Repository Placement

Benchmark artifacts now live in the shared runtime:

- `ophiolite/crates/ophiolite-seismic-runtime/src/bin/compute_storage_bench.rs`
- `ophiolite/crates/ophiolite-seismic-runtime/benches/compute_storage.rs`

Supporting modules now live in:

- `ophiolite/crates/ophiolite-seismic-runtime/src/compute.rs`
- `ophiolite/crates/ophiolite-seismic-runtime/src/storage/`

The CLI runner should emit machine-readable summaries.

Expected outputs:

- JSON summary
- CSV table
- optional Markdown summary for architecture review

## Decision Rubric

The benchmark decides the default backend direction using this rule:

- keep Zarr only if the best Zarr configuration stays within about `20%` of the best overall result on the key workloads

Key workloads:

- full-volume two-operator materialization throughput
- section preview latency

Additional gates:

- file count must remain operationally reasonable
- storage footprint must not become disproportionately large without a clear throughput benefit
- if Zarr loses materially and consistently to the flat-binary control on both key workloads, then canonical persistence must be reconsidered

## Decision Update

The current measured result still does **not** satisfy the Zarr retention rule, but the recommended next phase has changed.

Reason:

- `tbvol` now beats Zarr decisively on both preview and full-volume apply
- `tbvol` also beats the flat-binary control on the larger synthetic workload, which removes the need to promote the monolithic control format further

Updated architectural conclusion:

- stop treating Zarr as the optimization target for this compute class
- stop treating the monolithic flat-binary control as the likely successor
- treat `tbvol` as the preferred extraction-ready runtime backend candidate
- the backend has now been moved into the shared core
- keep the current Zarr path only as a benchmark/reference adapter until it is no longer needed
- invest next in:
  - padding-aware `tbvol` tile-shape recommendation
  - backend cutover in the shared seismic core
  - product/runtime integration on top of that shared core

## Success Criteria

This plan is complete when the shared/runtime stack can:

- benchmark multiple runtime-store layouts in-repo
- compare them using real operators
- select a default tile policy for new stores
- justify with measured evidence that `tbvol` is the better runtime backend than Zarr for the current operator class
- expose that backend through the shared seismic core with TraceBoost consuming it as a wrapper

## Immediate Next Steps

1. Keep tuning `tbvol` around the padding-aware `2-4 MiB` tile regime if future operators demand it
2. Remove remaining Zarr-only assumptions in higher layers such as file naming and UI copy when those layers are touched
3. Keep Zarr only as long as the benchmark/reference path is still valuable
