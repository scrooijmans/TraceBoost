# Seismic-ZFP Assessment And Benchmark Plan

## Purpose

This note records the current TraceBoost assessment of `seismic-zfp` / `.sgz` and turns that assessment into an explicit benchmark plan.

The goal is not to promote SGZ by intuition. The goal is to answer a narrower engineering question:

- does SGZ have enough merit in any TraceBoost storage tier to justify more integration work?

## Resolved Decisions

The current recommended decisions are:

- do **not** replace `tbvol` as the active runtime compute store
- do **not** add SGZ to the processing-cache production path
- do add SGZ as an external storage benchmark candidate
- do treat SGZ as a possible cold-storage or interchange tier
- do leave open one experimental question:
  - whether direct preview or direct full apply from SGZ is unexpectedly competitive

These decisions are intentionally asymmetric. SGZ may still be useful even if it is a poor active runtime backend.

## Why SGZ Is Not The Active Runtime Default

The current TraceBoost runtime path is optimized around:

- raw tiled `f32` amplitude payloads
- mmap-backed reads
- full-sample-axis tiles
- direct tile reuse through preview and materialization
- deterministic tile geometry for derived outputs and multi-volume arithmetic

That matches the current trace-local operator family well.

SGZ is a different storage bet:

- fixed-rate ZFP-compressed blocks
- bitrate-dependent precision
- fast arbitrary subvolume access in a compressed format
- stronger fit for compact storage and transfer than for zero-overhead local compute

That mismatch does not make SGZ bad. It means SGZ should earn its place through the right benchmark question.

## Product-Level Non-Goals

The following are out of scope for the first SGZ phase:

- switching the frontend default working format from `tbvol` to SGZ
- making SGZ the hidden processing-cache artifact format
- rewriting the shared runtime so preview and full apply execute primarily on compressed SGZ blocks
- changing lineage semantics or processing reproducibility rules to accommodate lossy storage by default

If any of those become desirable later, they should be reopened only after benchmark evidence exists.

## Candidate Roles For SGZ

SGZ should be evaluated only in these roles at first:

### 1. External Benchmark Candidate

Compare SGZ to:

- `tbvol`
- existing Zarr benchmark variants
- OpenVDS where relevant

This is the cheapest and safest first step.

### 2. Cold-Storage Or Interchange Tier

Potential workflow:

- ingest source SEG-Y
- optionally generate an SGZ archival copy
- transcode to `tbvol` when opening for interactive processing

This is the most plausible near-term value if customer datasets are large and local storage or transfer becomes painful.

### 3. Experimental Direct-Read Path

Potential workflow:

- open SGZ directly
- benchmark section preview and full apply without first transcoding to `tbvol`

This is explicitly an experiment, not an architectural commitment.

## SGZ Questions We Actually Need Answered

The benchmark phase must answer these questions:

1. How much disk reduction does SGZ provide relative to `tbvol` on representative datasets?
2. How much latency does SGZ add to first preview and repeated preview?
3. How much throughput does SGZ lose on full-volume apply?
4. Is `SGZ -> tbvol` transcode time acceptable compared with the storage savings?
5. How much numeric drift appears at realistic bitrates before and after representative operators?
6. Are there dataset classes where SGZ is clearly useful even if it is not the active compute backend?

## Benchmark Matrix

The first SGZ benchmark pass should measure:

- on-disk bytes
- file count
- open-to-first-preview latency
- inline section read latency
- xline section read latency
- preview `amplitude_scalar`
- preview `trace_rms_normalize`
- preview `phase_rotation`
- preview `bandpass_filter`
- preview `bandpass_filter + phase_rotation`
- full apply of the same operator set
- SGZ to `tbvol` transcode time
- numeric drift versus the original source amplitudes

The current benchmark already exercises most of these operator classes for other storage candidates. SGZ should be added in a way that preserves comparability.

## Dataset Matrix

The first SGZ pass should use:

- the existing real small control dataset
- the existing medium synthetic benchmark dataset
- the existing large synthetic benchmark dataset
- at least one larger real customer-like dataset if available later

The small control remains useful for smoke testing, but it must not drive the decision.

## SGZ Compression Sweep

The first SGZ pass should include at least these fixed-rate settings:

- `8` bits per voxel
- `4` bits per voxel
- `2` bits per voxel
- optionally `1` bit per voxel for an aggressive lower-bound data point

If SGZ also exposes materially different blockshape choices worth testing, keep the matrix narrow and choose only layouts that represent:

- a balanced default
- one layout biased toward inline/xline section access
- one layout biased toward z-slice access if that matters for the SGZ implementation

## Acceptance Bars

SGZ should be judged differently depending on the role.

### Active Runtime Candidate

This is the hardest bar.

SGZ should not be considered a serious active runtime candidate unless all of the following are true:

- preview latency stays within roughly `20%` to `30%` of `tbvol` on representative datasets
- full apply stays within roughly `20%` to `30%` of `tbvol`
- the numeric drift remains acceptable for the current operator family
- the implementation complexity does not explode the shared runtime design

This outcome is currently considered unlikely.

### Cold-Storage Candidate

This is the realistic bar.

SGZ is useful as a cold-storage or interchange tier if:

- size reduction is material
- SGZ to `tbvol` transcode time is operationally acceptable
- the numeric drift at the chosen bitrate is acceptable for the intended workflow
- the product can keep `tbvol` as the interactive working format

## Processing Cache Decision

SGZ should stay out of the processing-cache benchmark and architecture for now.

Reason:

- the processing-cache work already rejected hidden complexity that did not pay for itself
- compression is a different concern than rerun reuse
- adding SGZ there now would blur two decisions that should remain separate

This can be reopened only if visible checkpoints or derived outputs become a measured storage problem on large real datasets.

## Implementation Plan

### Phase 1: Benchmark Planning

- add this SGZ assessment note
- update the compute-storage benchmark doc so SGZ is an explicit future external candidate
- keep SGZ out of the production architecture until measured

### Phase 2: External Benchmark Runner

- create an SGZ benchmark runner outside the core runtime path
- mirror the existing OpenVDS comparison pattern
- emit machine-readable summaries that can be compared to the existing benchmark outputs

Status:

- implemented as [`scripts/sgz_storage_bench.py`](/Users/crooijmanss/dev/TraceBoost/scripts/sgz_storage_bench.py)
- currently supports:
  - environment checking
  - synthetic SGZ generation
  - direct SGZ section-read and preview/apply benchmarking
  - optional SGZ -> SEG-Y -> `tbvol` timing if the local toolchain is available
- local validation status on the current workstation:
  - `segyio` and `zfpy` were installed successfully
  - synthetic smoke benchmarks complete end-to-end
  - the optional SGZ -> SEG-Y -> `tbvol` timing path also completes on the smoke dataset

### Phase 3: Decision Review

- compare `tbvol`, Zarr, OpenVDS, and SGZ results
- decide whether SGZ has merit only as cold storage or whether any direct-read path deserves more work

OpenVDS note:

- the local open-source OpenVDS SDK build can still be used for `CompressionMethod::None` benchmarking
- lossless wavelet create/write benchmarking is not available in that OSS build even though the API exposes wavelet lossless enums
- practical consequence: a serious `WaveletLossless` benchmark pass requires OpenVDS+ or another binary distribution that includes the Bluware compression implementation

### Phase 4: Optional Product Follow-Up

Only if the numbers justify it:

- add SGZ import or transcode support
- add an archival or compact-storage workflow
- keep `tbvol` as the default interactive compute substrate unless the evidence is unusually strong

## Current Recommendation

The current recommendation is:

- benchmark SGZ
- do not architect around SGZ yet
- assume its most plausible role is compressed cold storage or interchange
- assume `tbvol` remains the active runtime compute format unless benchmark evidence proves otherwise

The next concrete execution step after this document is:

- install the missing `seismic-zfp` Python prerequisites on the benchmark machine
- run the synthetic SGZ benchmark matrix with representative bitrate and blockshape settings
