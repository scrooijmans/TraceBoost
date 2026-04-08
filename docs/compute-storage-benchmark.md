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
- OpenVDS comparison runner:
  - [`scripts/openvds_storage_bench.cpp`](/Users/sc/dev/TraceBoost/scripts/openvds_storage_bench.cpp)

Planned external comparison candidate:

- SGZ / `seismic-zfp` comparison runner:
  - implemented as [`scripts/sgz_storage_bench.py`](/Users/crooijmanss/dev/TraceBoost/scripts/sgz_storage_bench.py)
  - intended as an external benchmark path similar in spirit to the OpenVDS runner
  - explicitly not a commitment to replace `tbvol` as the active runtime backend

Current state:

- the shared backend now lives in `ophiolite-seismic-runtime`
- TraceBoost runtime re-exports that shared backend as a compatibility layer
- ingest/open/preview/materialize in the shared runtime now operate on the `tbvol` path by default
- Zarr remains present only as a benchmark/reference backend and compatibility helper
- OpenVDS is benchmarked as an external local-file comparison path, not yet as an in-runtime backend

## Measured Findings

The benchmark was rerun in release mode through the storage-neutral compute path on:

- real control dataset: `test-data/f3.sgy`
- synthetic medium dataset: `256 x 256 x 1024`
- synthetic large dataset: `384 x 384 x 1024`
- OpenVDS brick sweep: `32`, `64`, `128`

Control finding:

- `f3.sgy` is too small to be decision-driving; storage/container overhead dominates

Measured backend comparison with `4 MiB` full-trace tiles for `tbvol`/flat/Zarr, plus local-file OpenVDS `.vds` runs:

- medium synthetic `256 x 256 x 1024`
  - `zarr_uncompressed_unsharded`
    - preview pipeline: `5.796 ms`
    - full apply pipeline: `560.661 ms`
    - file count: `67`
  - `tbvol`
    - preview pipeline: `0.286 ms`
    - full apply pipeline: `648.866 ms`
    - file count: `2`
  - `flat_binary_control`
    - preview pipeline: `0.477 ms`
    - full apply pipeline: `1466.344 ms`
    - file count: `1`
  - `openvds`
    - best balanced brick: `64`
    - preview pipeline: `2.960 ms`
    - full apply pipeline: `558.635 ms`
    - file count: `1`
    - sweep note: `brick_size=128` reached `537.918 ms` full apply, but preview regressed to `5.692 ms`
- large synthetic `384 x 384 x 1024`
  - `zarr_uncompressed_unsharded`
    - preview pipeline: `9.612 ms`
    - full apply pipeline: `900.962 ms`
    - file count: `147`
  - `tbvol`
    - preview pipeline: `0.229 ms`
    - full apply pipeline: `526.317 ms`
    - file count: `2`
  - `flat_binary_control`
    - preview pipeline: `0.567 ms`
    - full apply pipeline: `970.066 ms`
    - file count: `1`
  - `openvds`
    - best brick: `32`
    - preview pipeline: `4.759 ms`
    - full apply pipeline: `1033.193 ms`
    - file count: `1`
- small real control `f3.sgy`
  - still too small to drive the architectural decision
  - `tbvol` won preview and beat Zarr, but flat binary still had the cheapest full apply on this tiny case because container overhead dominates

Measured conclusion:

- the earlier “promote flat binary” direction is superseded
- OpenVDS is worth keeping in the benchmark set because it is materially stronger than generic local Zarr on file count and can be competitive on medium-size full apply
- `tbvol` remains the strongest interactive backend because its preview latency is still an order of magnitude lower than OpenVDS and much lower than Zarr
- `tbvol` also wins the larger synthetic full-apply case, which keeps it in front as the default local compute substrate

Chunk-size sweep findings for `tbvol`:

- `1 MiB` tiles were too small and increased tile-count overhead
- on the original synthetic benchmark corpus, `8 MiB` tiles were not consistently better and increased padding waste on non-divisible shapes
- the original synthetic sweet spot was `2-4 MiB`
- `4 MiB` was strongest on the medium synthetic dataset
- `2 MiB` and `4 MiB` were both strong on the larger synthetic dataset

Focused real-volume sweep update:

- on April 8, 2026, the new `sweep-tbvol` command was run against `C:\Users\crooijmanss\Downloads\archive\f3_dataset.sgy`
- dataset shape: `651 x 951 x 462`
- `1 MiB` won preview latency at `1.208 ms` but was unacceptable on full apply at `6907.067 ms`
- `2 MiB` reached `1.458 ms` preview and `1633.727 ms` full apply
- `4 MiB` reached `2.287 ms` preview and `1334.141 ms` full apply
- `8 MiB` reached `1.279 ms` preview, `1290.195 ms` full apply, and the best section-read I/O
- that made `8 MiB` the best balanced result on the first real customer-scale sweep
- the practical conclusion is no longer "always prefer `2-4 MiB`"
- the better conclusion is: keep `2`, `4`, and `8 MiB` in the candidate set and choose from benchmark evidence on real datasets

Implication:

- the old generic "always use `8 MiB`" heuristic should still not become the unqualified `tbvol` policy
- the old synthetic-only "target `2-4 MiB`" guidance is now too rigid
- future `tbvol` tile recommendation should stay padding-aware and benchmark-driven, with at least `2`, `4`, and `8 MiB` considered valid candidate regimes for large real volumes
- the shared runtime now uses a conservative adaptive fallback when no explicit chunk shape is supplied:
  - `4 MiB` below roughly `768 MiB` dense `f32` volume size
  - `8 MiB` at or above that threshold
- that fallback is intentionally conservative and should continue to be validated against additional real customer volumes

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
- `openvds` local-file comparison path
- `sgz` / `seismic-zfp` external comparison path planned for a future benchmark pass

Notes:

- `tbvol` is the current leading production backend candidate
- `flat_binary_control` remains a control artifact, not a production commitment
- `openvds` is not yet wired into the shared runtime backend layer; it is benchmarked through a standalone local-file runner
- SGZ should be treated like OpenVDS in the first phase:
  - benchmarked externally first
  - evaluated primarily for compressed storage and interchange merit
  - not assumed to be a drop-in replacement for the mmap-backed `tbvol` runtime path
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

For the SGZ external comparison path, the current runner emits JSON summaries and environment diagnostics via:

```powershell
python scripts/sgz_storage_bench.py check-env
python scripts/sgz_storage_bench.py benchmark-synthetic medium 256 256 1024 --bits-per-voxel 4
```

Current setup note:

- the SGZ runner depends on external Python packages from the `seismic-zfp` stack, notably `segyio` and `zfpy`
- if those are missing, `check-env` reports the exact blocker rather than failing silently

For targeted `tbvol` tile-shape tuning on one real source or existing store, the benchmark CLI now also supports a focused sweep:

```powershell
cargo run --release --manifest-path C:\Users\crooijmanss\dev\ophiolite\crates\ophiolite-seismic-runtime\Cargo.toml --bin compute_storage_bench -- sweep-tbvol C:\Users\crooijmanss\dev\TraceBoost\test-data\f3.sgy --format json
```

The `sweep-tbvol` command accepts either:

- a SEG-Y or SU source file, which is loaded once and retiled across the tested `tbvol` chunk targets
- an existing `tbvol` directory, which is read back into the canonical dense cube and then retiled across the tested chunk targets

The sweep currently benchmarks the default `1`, `2`, `4`, and `8 MiB` tile targets and reports, per tile policy:

- input `tbvol` bytes and file count
- inline and xline section read latency
- preview pipeline latency
- full-volume apply pipeline latency
- a simple balanced score across preview, apply, and section-read costs

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

- `tbvol` still dominates the interactive preview path
- `tbvol` still wins the larger synthetic full-volume apply case
- OpenVDS is a legitimate interoperability-oriented comparison point, but it does not overtake `tbvol` as the default local compute backend

Updated architectural conclusion:

- stop treating Zarr as the optimization target for this compute class
- stop treating the monolithic flat-binary control as the likely successor
- treat `tbvol` as the preferred extraction-ready runtime backend candidate
- keep OpenVDS in the benchmark set as a local-file interoperability/reference comparison
- add SGZ as a future external comparison target, with the default assumption that any value it has will be in a cold-storage or interchange tier rather than as the active compute substrate
- the backend has now been moved into the shared core
- keep the current Zarr path only as a benchmark/reference adapter until it is no longer needed
- invest next in:
  - padding-aware `tbvol` tile-shape recommendation
  - backend cutover in the shared seismic core
  - product/runtime integration on top of that shared core
  - an SGZ benchmark pass only after the external runner and acceptance bars are defined

## Success Criteria

This plan is complete when the shared/runtime stack can:

- benchmark multiple runtime-store layouts in-repo
- compare them using real operators
- select a default tile policy for new stores
- justify with measured evidence that `tbvol` is the better default local runtime backend than the benchmark alternatives for the current operator class
- expose that backend through the shared seismic core with TraceBoost consuming it as a wrapper

## Immediate Next Steps

1. Keep tuning `tbvol` around a padding-aware `2-8 MiB` candidate regime, with real-volume sweeps deciding the default for new stores
2. Remove remaining Zarr-only assumptions in higher layers such as file naming and UI copy when those layers are touched
3. Keep Zarr only as long as the benchmark/reference path is still valuable
4. Add SGZ as a measured external comparison candidate before considering any compressed-storage product work
