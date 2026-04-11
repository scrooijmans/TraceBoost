# Lossless Compression Research And Benchmark Plan

## Purpose

This note narrows the next compression phase after the OpenVDS wavelet assessment.

The goal is not to chase every interesting compression paper. The goal is to answer a practical product question:

- can TraceBoost and Ophiolite gain meaningful exact-lossless storage reduction, and possibly processing benefits, without replacing `tbvol` as the active compute substrate?

## Resolved Starting Assumptions

- canonical runtime payloads remain exact `f32`
- `tbvol` remains the active working format unless benchmark evidence is unusually strong
- OpenVDS wavelet compression is out of scope for now because the current open-source SDK build cannot create compressed VDS
- the next phase should prioritize exact-lossless candidates before any custom implementation

## What Wavelet Compression Means Here

Wavelet compression applies a transform that rewrites a signal into coarse structure plus finer detail bands and then entropy-codes the result.

That can be either:

- lossy, if small detail terms are discarded or quantized
- lossless, if the transform is reversible and all transformed coefficients are encoded exactly

Wavelets are still a valid idea in principle, but they are not the recommended first implementation target for this stack right now.

Reason:

- the practical local benchmark path is blocked
- the public OpenVDS build only supports uncompressed create/write benchmarking
- there are lower-friction exact-lossless candidates that are better aligned with current `f32` volume handling

## Candidate Families

The current candidate space falls into three families.

### 1. Exact Float-Aware Compressors

These directly target floating-point arrays rather than generic byte streams.

Primary candidates:

- `fpzip`
- `ndzip`
- `ALP`

Why they matter:

- they are designed specifically for floating-point data
- they exploit correlation or structure that generic byte codecs miss
- they may provide materially better exact compression ratios than plain `zstd` on smooth seismic cubes

### 2. Pragmatic Chunked Codec Pipelines

These treat compression as a filter pipeline on chunked binary data.

Primary candidates:

- `bitshuffle + zstd`
- `bitshuffle + lz4`
- `Blosc2`

Why they matter:

- they are operationally simple
- they already match chunked storage models used in scientific array systems
- they are strong baseline candidates that any custom design must beat

### 3. Search / Generation Frameworks

These do not need to become production dependencies to be useful.

Primary candidates:

- `LC-framework`
- `libpressio`

Why they matter:

- `LC-framework` can search for reversible pipelines and generate standalone compressors
- `libpressio` can reduce benchmark friction across multiple compressors and metrics
- these tools are well suited to discovery work before custom implementation

## Current Recommendation

Benchmark in this order:

1. `fpzip`
2. `bitshuffle + zstd` and `bitshuffle + lz4`
3. `Blosc2`
4. `ndzip`
5. `ALP`
6. `LC-framework` only if the above do not give a clear answer

Recommended interpretation:

- `fpzip` is the best first exact-lossless specialist candidate
- `bitshuffle + zstd` is the most important pragmatic baseline
- `Blosc2` is the best chunked-system reference candidate
- `ndzip` is especially interesting if decompression throughput or GPU paths become important
- `ALP` is promising but is more clearly proven in analytical and database-style floating-point workloads than in strongly correlated seismic cubes
- `LC-framework` is the right bridge to custom implementation if packaged compressors are not good enough

## Why These Candidates

### fpzip

Strengths:

- exact lossless support for 2D and 3D floating-point arrays
- explicitly assumes spatial correlation
- very close to seismic-poststack cube structure

Risks:

- older codebase and API
- not a chunk store by itself
- random subvolume access would likely require block framing around fpzip streams

### bitshuffle + zstd

Strengths:

- simple
- widely used scientific baseline
- strong chance of a good engineering return for modest implementation effort

Risks:

- may leave compression ratio on the table relative to float-aware schemes
- may help storage more than processing unless chunk geometry and decompression paths are tuned carefully

### Blosc2

Strengths:

- chunked / block-oriented
- supports multiple codecs and filters
- has an N-dimensional store model and two-level partitioning

Risks:

- still a general-purpose framework rather than a seismic-specific codec
- adds system complexity compared with a narrow-purpose exact compressor

### ndzip

Strengths:

- exact, parallel, bit-identical
- explicitly built for scientific floating-point grids
- has CPU multi-threaded and GPU implementations

Risks:

- Linux/HPC-oriented research code
- integration cost may be higher than `fpzip` or byte-filter baselines

### ALP

Strengths:

- recent SIGMOD 2024 result
- strong lossless floating-point focus
- attractive for decimal-heavy or mixed-precision real-world data

Risks:

- likely a weaker fit for smoothly varying seismic amplitudes than for tabular/analytical floating-point columns
- should be treated as an empirical benchmark candidate, not a presumed winner

### LC-framework

Strengths:

- can search reversible pipelines automatically
- can optimize across ratio and throughput
- can generate standalone CPU/GPU compressors after discovery

Risks:

- research workflow rather than drop-in product dependency
- Linux/CUDA-oriented and likely heavier operationally

## Product Decision Tree

### Question 1

What problem is actually being solved first?

Recommended answer:

- exact-lossless storage reduction for large volumes

Not recommended as the first target:

- replacing the active runtime store
- speculative GPU compression work before a storage win exists
- building a custom codec before baseline evidence exists

### Question 2

Is the first benchmark about a file format or about reversible transforms?

Recommended answer:

- both, but keep them separate

Interpretation:

- benchmark `fpzip` and `ndzip` as specialist codecs
- benchmark `bitshuffle + zstd` and `Blosc2` as chunked filter/container approaches

### Question 3

Should the first phase target archive only, or archive plus ephemeral compressed processing?

Recommended answer:

- archive first, but always record decompression and full-apply timings

Reason:

- the archive case is easier to justify
- processing wins may appear, but should be treated as a measured upside, not as the premise

### Question 4

When should a custom implementation be considered?

Recommended answer:

- only if the best benchmarked candidate misses a clear product bar

That bar should be something like:

- at least `1.5x` to `2x` exact storage reduction on representative real datasets
- acceptable open/transcode latency
- no unacceptable regression in preview/full-apply workflows

## Proposed Benchmark Matrix

Use one real dataset first, then medium and large synthetic controls.

For each candidate, measure:

- exactness verification
- on-disk bytes
- compression throughput
- decompression throughput
- inline section read latency
- xline section read latency
- preview pipeline latency for representative trace-local operators
- full apply latency
- implementation complexity notes

## Recommended Immediate Next Steps

1. Add `fpzip` as the first external exact-lossless benchmark candidate.
2. Add `bitshuffle + zstd` as the byte-filter baseline.
3. Add `Blosc2` only if the first two results do not make the decision obvious.
4. Keep `ndzip` as the CPU/GPU throughput candidate if exact compression ratio alone is not enough.
5. Defer `ALP` and `LC-framework` until after the first exact-lossless pass unless a clear reason appears sooner.

## Initial Synthetic Benchmark Snapshot

An external archive-tier harness now exists at `scripts/lossless_float_storage_bench.cpp`.

It currently benchmarks:

- `fpzip`
- `blosc2-zstd-bitshuffle`
- `blosc2-lz4-bitshuffle`

Initial Windows runs on synthetic seismic-like cubes produced the following results.

### smoke128 `[128, 128, 512]`

| Codec | Ratio | Compress ms | Decompress ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | --- |
| `fpzip` | `10.377x` | `194.982` | `224.044` | `221.104` | yes |
| `blosc2-zstd-bitshuffle` | `1.418x` | `264.452` | `29.696` | `54.633` | yes |
| `blosc2-lz4-bitshuffle` | `1.287x` | `42.922` | `20.613` | `47.630` | yes |

### medium256 `[256, 256, 1024]`

| Codec | Ratio | Compress ms | Decompress ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | --- |
| `fpzip` | `11.428x` | `1647.929` | `1829.741` | `2029.621` | yes |
| `blosc2-zstd-bitshuffle` | `1.457x` | `2812.000` | `321.842` | `423.553` | yes |
| `blosc2-lz4-bitshuffle` | `1.348x` | `296.942` | `192.575` | `278.125` | yes |

Interpretation:

- `fpzip` is the strongest exact-lossless ratio candidate by a wide margin on smooth synthetic cubes
- `bitshuffle + lz4` is the fastest practical decode path in this first pass
- `bitshuffle + zstd` sits between them on ratio and latency
- these synthetic volumes are favorable to correlation-exploiting codecs, so the next real decision must be based on one or more representative customer datasets

Current recommendation after the first measured pass:

- keep `fpzip` as the leading exact archive-tier candidate
- keep `bitshuffle + lz4` as the leading low-friction decode-speed baseline
- do not make any adoption decision until the same matrix is run on real seismic volumes
- only add `ndzip` or a custom reversible preconditioner if real-data results leave the tradeoff unresolved

## First Real Dataset Snapshot

The benchmark harness now supports direct `tbvol` inputs, reconstructing the logical volume from tiled `amplitude.bin` storage using `manifest.json`.

Real-data run:

- source: `C:\Users\crooijmanss\AppData\Roaming\com.traceboost.app\volumes\f3_dataset-b06b2d1aa05e62ee.tbvol`
- logical shape: `[651, 951, 462]`
- logical exact payload: `1,144,098,648` bytes

| Codec | Ratio | Compress ms | Decompress ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | --- |
| `blosc2-lz4-bitshuffle` | `1.966x` | `1129.894` | `793.288` | `1247.308` | yes |
| `blosc2-zstd-bitshuffle` | `2.106x` | `8550.779` | `1120.005` | `1563.925` | yes |
| `fpzip` | `1.210x` | `14642.185` | `13071.338` | `15969.388` | yes |

Interpretation:

- the synthetic benchmark materially overstated `fpzip` for this workload
- on real F3 data, `fpzip` is not competitive on either ratio or decode cost
- `bitshuffle + lz4` is the best current speed candidate
- `bitshuffle + zstd` is the best current ratio candidate, but with much higher compression cost and slower decode than `lz4`

Updated recommendation after the first real dataset:

- drop `fpzip` from the front of the queue for this seismic workload
- keep `bitshuffle + lz4` as the leading operational baseline
- keep `bitshuffle + zstd` as the ratio-oriented exact baseline
- only continue to `ndzip`, `ALP`, or a custom reversible preconditioner if one more real dataset suggests the F3 result is not representative
- do not start custom codec work yet

## Additional Real Dataset Proxies

Two additional SEG-Y files were converted into benchmark-only `tbvol` proxies using trace order plus a clean repeating `TraceNumber` cycle:

- `scripts/build_trace_order_tbvol_proxy.py`
- `C:\Users\crooijmanss\Downloads\3D-Waihapa.sgy` -> shape `[227, 305, 2501]`
- `C:\Users\crooijmanss\Downloads\3D-Waipuku.sgy` -> shape `[148, 312, 2001]`

Important limitation:

- these proxy volumes are valid exact amplitude arrays for compression benchmarking
- they are not a claim that survey geometry has been solved correctly
- standard `INLINE_3D` / `CROSSLINE_3D` headers in both files are zeroed, so these datasets still need real geometry work before normal ingest/runtime use

### Waihapa Trace-Order Proxy

| Codec | Ratio | Compress ms | Decompress ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | --- |
| `blosc2-lz4-bitshuffle` | `1.169x` | `1258.115` | `573.908` | `785.916` | yes |
| `blosc2-zstd-bitshuffle` | `1.246x` | `12060.231` | `774.976` | `1099.172` | yes |
| `fpzip` | `1.371x` | `8716.923` | `8977.882` | `7719.661` | yes |

### Waipuku Trace-Order Proxy

| Codec | Ratio | Compress ms | Decompress ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | --- |
| `blosc2-lz4-bitshuffle` | `1.814x` | `387.605` | `325.516` | `351.648` | yes |
| `blosc2-zstd-bitshuffle` | `1.924x` | `3582.491` | `295.189` | `396.011` | yes |
| `fpzip` | `1.867x` | `3533.871` | `3138.522` | `3350.889` | yes |

Interpretation across the three real datasets:

- `fpzip` is not robustly attractive for this workload
- on F3 it loses on both ratio and latency
- on Waihapa and Waipuku it sometimes improves ratio, but decode/apply cost is still far worse than the Blosc baselines
- `bitshuffle + lz4` remains the most operationally attractive exact baseline
- `bitshuffle + zstd` is the better storage-ratio baseline when slower compression is acceptable

Updated recommendation after three real datasets:

- stop treating `fpzip` as a serious front-runner for this seismic storage path
- keep `blosc2-lz4-bitshuffle` as the default benchmark winner on processing-aware grounds
- keep `blosc2-zstd-bitshuffle` as the archival-ratio alternative
- if further work continues, move next to `ndzip` or a reversible seismic-aware preconditioner ahead of `lz4/zstd`
- do not invest in a custom codec until one of those two paths clearly beats the Blosc baselines on real surveys

## ndzip Feasibility Check

`ndzip` was the next candidate to try after the first three real datasets, but the current Windows environment is not a practical fit.

Observed blockers:

- local `ndzip` documentation explicitly lists Linux + Clang as the tested prerequisite set
- no local `clang++` executable is present
- no local `Boost` toolchain or package root is present
- `ndzip` CMake requires `Boost::thread` and `Boost::program_options` up front

That does not mean `ndzip` is invalid. It means it is not the next lowest-friction benchmark on this machine.

## Reversible Preconditioner Probe

A benchmark-only reversible trace-wise XOR preconditioner was added ahead of the existing Blosc pipelines:

- `trace-xor-blosc2-lz4-bitshuffle`
- `trace-xor-blosc2-zstd-bitshuffle`

The transform is exact and reversible:

- each sample is reinterpreted as `u32`
- each trace is encoded as `current_bits XOR previous_bits`
- the result is then compressed with the same `bitshuffle + lz4/zstd` pipeline

### F3 with Trace-XOR

| Codec | Ratio | Compress ms | Decompress ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | --- |
| `blosc2-lz4-bitshuffle` | `1.966x` | `1129.894` | `793.288` | `1247.308` | yes |
| `trace-xor-blosc2-lz4-bitshuffle` | `1.986x` | `2584.462` | `1787.172` | `2462.842` | yes |
| `blosc2-zstd-bitshuffle` | `2.106x` | `8550.779` | `1120.005` | `1563.925` | yes |
| `trace-xor-blosc2-zstd-bitshuffle` | `2.157x` | `11647.344` | `2291.363` | `2450.200` | yes |

### Waipuku with Trace-XOR

| Codec | Ratio | Compress ms | Decompress ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | --- |
| `blosc2-lz4-bitshuffle` | `1.814x` | `387.605` | `325.516` | `351.648` | yes |
| `trace-xor-blosc2-lz4-bitshuffle` | `1.817x` | `695.842` | `455.712` | `590.368` | yes |
| `blosc2-zstd-bitshuffle` | `1.924x` | `3582.491` | `295.189` | `396.011` | yes |
| `trace-xor-blosc2-zstd-bitshuffle` | `1.935x` | `4105.041` | `499.040` | `643.532` | yes |

Interpretation:

- the simple reversible seismic-aware preconditioner does not justify itself
- it improves compression ratio only marginally
- it makes decompression and apply materially worse
- this is not a compelling direction for custom implementation in its current form

Updated recommendation after the preconditioner probe:

- keep `blosc2-lz4-bitshuffle` as the main exact processing-aware baseline
- keep `blosc2-zstd-bitshuffle` as the exact archival-ratio baseline
- deprioritize both `fpzip` and the current trace-XOR preconditioner
- only revisit custom reversible transforms if there is a more domain-specific predictor than naive per-trace XOR
- if benchmarking continues on this machine, the next useful step is likely parameter tuning and chunk-shape experiments around the existing Blosc baselines rather than a new codec family

## Tile-Wise tbvol Simulation

The benchmark harness now also supports tile-wise compression that mirrors current `tbvol` storage more closely:

- each logical `tbvol` tile is padded the same way the runtime already pads edge tiles
- each tile is compressed independently
- decompression reconstructs the logical volume tile-by-tile

This is a closer proxy for a hypothetical compressed-`tbvol` store than whole-volume compression.

### F3 with Current tbvol Tile Shape

Current F3 runtime tile shape from the real manifest:

- `[82, 56, 462]`

Tile-wise benchmark results:

| Codec | Ratio | Compress ms | Decompress ms | Inline ms | Apply ms | Exact |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| `tile-blosc2-lz4-bitshuffle` | `1.966x` | `2097.059` | `950.563` | `769.069` | `1154.858` | yes |
| `tile-blosc2-zstd-bitshuffle` | `2.107x` | `8995.632` | `1086.293` | `1100.516` | `1628.464` | yes |

Interpretation:

- the storage benefit survives tile-wise `tbvol`-style chunking almost unchanged
- `lz4` remains the better candidate when processing latency matters
- `zstd` remains the better candidate when exact storage ratio matters more than write-time CPU cost
- the numbers here still decompress all tiles during the benchmark path, so they are not yet the lower bound for a section reader that would only inflate touched tiles

## Compression-Level Sweep Notes

A quick F3 parameter sweep confirmed that extreme levels are not the right lever:

- `zstd` level `9` improved F3 ratio only slightly over level `5` (`2.158x` vs `2.106x`) but pushed compression time to roughly `160 s`
- `zstd` level `1` already landed near the same ratio band (`2.054x`)
- `lz4` did not show evidence that higher compression levels would materially change the conclusion for this workload

Practical interpretation:

- codec choice matters more than chasing extreme compression levels
- if this ever becomes a product feature, the useful operational choice is likely `lz4` for speed-sensitive workflows and moderate `zstd` for archive-oriented workflows

## Local Research Snapshot

Relevant local clones for this phase now exist under the parent `dev` folder:

- `fpzip`
- `c-blosc2`
- `ALP`
- `ndzip`
- `LC-framework`
- `libpressio`

These should be treated as benchmark and design references first, not as adoption commitments.

Implementation note:

- the local `libpressio` source already includes compressor plugins for `fpzip`, `ndzip`, `blosc2`, and `zfp`
- that makes `libpressio` a strong candidate for the benchmark harness layer because it can normalize metrics and reduce one-off wrapper work across candidates
