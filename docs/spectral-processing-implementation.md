# Spectral Processing Implementation Spec

## Purpose

This document defines the concrete implementation plan for the first frequency-domain processing milestone across the shared Ophiolite seismic stack and the TraceBoost desktop product.

It answers these questions:

- where the canonical operator contract should live
- which first spectral capabilities should ship
- how TraceBoost should expose them without taking ownership of the math
- how the design remains reusable for future prestack products
- which APIs, tests, and benchmarks are required before rollout

## Decision Summary

The approved architecture is:

- `ophiolite-seismic`
  - owns canonical shared processing contracts
  - owns operator compatibility metadata
- `ophiolite-seismic-runtime`
  - owns operator validation
  - owns preview and materialization kernels
  - owns spectrum-analysis kernels
  - owns performance benchmarks
- `TraceBoost`
  - owns product orchestration and desktop UX
  - owns post-stack product gating
  - owns pipeline editing and spectrum-inspector UI
- `geoviz`
  - remains outside milestone 1
  - may later receive a reusable chart component only if a second product needs the same visualization

This keeps one operator DSL and one compute implementation while allowing multiple products to expose different subsets.

## Scope

Milestone 1 adds exactly two new capabilities:

- a runtime-backed `bandpass_filter` processing operator
- a runtime-backed read-only amplitude spectrum analysis API

Milestone 1 does not include:

- spectral decomposition attributes
- phase rotation
- whitening
- AGC redesign
- deconvolution
- 2D or 3D FFT operators
- GPU compute
- prestack UI inside `traceboost-frontend`

## Product And Layering Rules

### Shared Domain Layer

The canonical shared processing model remains in the Ophiolite contract crate:

- `ProcessingOperation`
- `ProcessingPipeline`
- operator compatibility metadata

TraceBoost must consume those types rather than redefining them.

### Product Layer

Product applications decide which operators they expose:

- `traceboost-frontend`
  - post-stack only
  - exposes only operators allowed for the current TraceBoost product
- future sibling prestack product
  - can expose additional operators already modeled in Ophiolite

This means operator compatibility and product availability are different concepts:

- compatibility says whether an operator is mathematically/legal for a seismic layout
- product availability says whether a specific app chooses to expose it

## Operator Model

### Canonical New Operator

The first shared spectral operator is:

```rust
pub enum ProcessingOperation {
    AmplitudeScalar { factor: f32 },
    TraceRmsNormalize,
    BandpassFilter {
        f1_hz: f32,
        f2_hz: f32,
        f3_hz: f32,
        f4_hz: f32,
        phase: FrequencyPhaseMode,
        window: FrequencyWindowShape,
    },
}

pub enum FrequencyPhaseMode {
    Zero,
}

pub enum FrequencyWindowShape {
    CosineTaper,
}
```

### Rationale

- `bandpass_filter` is modeled as a structured operator, not a string
- the four-corner form matches common seismic-processing practice
- `phase` and `window` are enums now so the contract can extend later without a breaking shape change
- milestone 1 allows only `Zero` and `CosineTaper`, but the type system is prepared for future additions

### Compatibility

`BandpassFilter` should be marked `AnyTraceMatrix`.

Reason:

- the kernel is trace-local
- the operator is reusable for post-stack and prestack trace matrices
- TraceBoost product policy, not the shared contract, will enforce the post-stack-only UI for now

## Validation Rules

The runtime must reject invalid parameters before execution.

Required validation:

- `f1_hz`, `f2_hz`, `f3_hz`, `f4_hz` must all be finite
- `f1_hz >= 0`
- `f1_hz <= f2_hz <= f3_hz <= f4_hz`
- `f4_hz <= nyquist_hz`
- sample interval must be resolvable from the dataset
- only supported enum values are accepted

Nyquist:

```text
nyquist_hz = 0.5 / sample_interval_seconds
```

Runtime errors should explain which constraint failed and include the offending value where practical.

## Bandpass Kernel

### Execution Policy

The hot path remains the existing trace-parallel executor in Ophiolite:

- parallelize over traces
- apply all operators for one trace in sequence
- reuse the same preview and materialization kernels

Milestone 1 should keep that design and add a spectral branch inside the operator executor.

### Dependency Policy

Use:

- `realfft`
- `rustfft` underneath
- existing `rayon`

Do not use:

- NumPy or Python in the product path
- `ndarray` in the per-trace FFT hot loop unless a later operator needs multidimensional transforms

### Conditioning Defaults

Milestone 1 `bandpass_filter` should:

1. copy the trace into a worker-local real input buffer
2. remove the trace mean to suppress DC leakage
3. run real-to-complex FFT
4. apply a trapezoidal bandpass with cosine tapers:
   - stopband below `f1_hz`
   - taper up between `f1_hz` and `f2_hz`
   - passband between `f2_hz` and `f3_hz`
   - taper down between `f3_hz` and `f4_hz`
   - stopband above `f4_hz`
5. run inverse FFT
6. normalize inverse output correctly for the chosen FFT library behavior
7. write the filtered samples back into the trace

Milestone 1 does not add:

- phase rotation
- minimum-phase variants
- edge-padding controls
- spectral whitening
- automatic passband inference

### Workspace Reuse

The runtime must avoid fresh FFT allocation per trace.

Required design:

- each compute worker owns reusable FFT plans and scratch buffers for a given trace length
- repeated traces of the same sample count reuse those plans and buffers
- plan creation stays outside the innermost trace loop

This is required to keep preview latency and full-volume throughput competitive.

## Spectrum Analysis API

### Why This Is Separate From `ProcessingOperation`

Amplitude spectrum extraction is analysis, not processing:

- it changes output shape from trace samples to frequency bins
- it is used for inspection and parameter selection
- it should not appear in dataset lineage or materialized pipelines

Therefore it must not be modeled as a `ProcessingOperation`.

### Canonical Analysis Modes

Milestone 1 supports:

- `single_trace`
- `average_over_trace_range`

The second mode computes the mean amplitude spectrum across a contiguous trace window for a selected section.

### Proposed Contract Shapes

Canonical shared analysis request/response types should be added in Ophiolite contracts:

```rust
pub enum SpectrumAggregation {
    SingleTrace { trace_index: usize },
    AverageTraceRange { trace_start: usize, trace_end: usize },
}

pub struct AmplitudeSpectrumRequest {
    pub schema_version: u32,
    pub store_path: String,
    pub section: SectionRequest,
    pub aggregation: SpectrumAggregation,
    pub pipeline: Option<ProcessingPipeline>,
}

pub struct AmplitudeSpectrumCurve {
    pub frequencies_hz: Vec<f32>,
    pub amplitudes: Vec<f32>,
}

pub struct AmplitudeSpectrumResponse {
    pub schema_version: u32,
    pub section: SectionRequest,
    pub aggregation: SpectrumAggregation,
    pub sample_interval_ms: f32,
    pub curve: AmplitudeSpectrumCurve,
    pub processing_label: Option<String>,
}
```

### Request Semantics

- `pipeline: None`
  - compute the spectrum on raw section data
- `pipeline: Some(...)`
  - compute the spectrum after applying the supplied processing pipeline to the selected traces

This allows the spectrum panel to compare raw and filtered behavior without materializing a new volume.

### Range Semantics

`AverageTraceRange` should use:

- inclusive `trace_start`
- exclusive `trace_end`

Validation:

- `trace_start < trace_end`
- `trace_end <= section trace count`

## API Wiring

### Ophiolite Runtime Surface

Add runtime functions alongside the existing preview helpers:

```rust
pub fn amplitude_spectrum_from_section_view(...);
pub fn amplitude_spectrum_from_section_plane(...);
pub fn amplitude_spectrum_for_processing_section(...);
```

The exact naming can be adjusted, but the separation should be:

- raw section access
- optional processed section access
- analysis result construction

### TraceBoost App And Tauri Surface

TraceBoost should expose the spectrum analysis as a new command, parallel to the existing preview command:

- app helper in `app/traceboost-app`
- Tauri command in `app/traceboost-frontend/src-tauri`
- frontend bridge function in `app/traceboost-frontend/src/lib/bridge.ts`

Suggested command name:

- `amplitude_spectrum_command`

### TypeScript Boundary

Generated TypeScript contracts should mirror the Rust request/response types so the frontend stays schema-driven.

## TraceBoost Frontend Responsibilities

TraceBoost owns:

- adding `bandpass_filter` to the pipeline editor
- editing its parameters
- hiding unsupported operators from the post-stack product
- adding a spectrum-inspector panel
- selection UX for:
  - current trace
  - selected trace range
  - raw vs processed spectrum comparison

TraceBoost does not own:

- FFT execution
- bandpass kernel math
- canonical operator schema

### UI Constraints For Milestone 1

The first UI should expose:

- quick-add `Bandpass`
- editable corners `f1/f2/f3/f4`
- a read-only label for `Zero phase`
- a read-only label for `Cosine taper`
- spectrum panel for:
  - selected trace
  - average visible trace range or selected trace range

The first UI should not expose:

- prestack layout choices
- advanced taper families
- phase rotation controls
- arbitrary spectrum transforms

## Persistence And Lineage

Persist:

- `bandpass_filter` inside `ProcessingPipeline`
- pipeline presets containing `bandpass_filter`
- derived volume lineage containing the exact bandpass parameters used

Do not persist:

- spectrum panel state in lineage
- inspected raw/processed spectrum curves

Spectrum inspection is an ephemeral analysis concern, not a canonical processing-history concern.

## Performance Requirements

Milestone 1 must preserve the current architectural rules:

- preview and full apply use the same kernels
- preview remains synchronous on the selected section
- full-volume apply remains a background materialization job
- source volumes remain immutable
- full apply writes a new derived `tbvol`

### Benchmark Requirements

Before frontend rollout, add benchmark coverage for:

- preview `bandpass_filter`
- full-volume apply `bandpass_filter`
- preview `bandpass_filter + trace_rms_normalize`
- full-volume apply `bandpass_filter + trace_rms_normalize`
- amplitude spectrum extraction on:
  - single trace
  - averaged trace range

Comparison baseline:

- current scalar-only operator cost
- current RMS-normalize cost
- new bandpass cost

The new benchmark rows should live beside the existing Ophiolite Criterion and CLI benchmark surfaces.

## Testing Requirements

### Unit Tests

Add tests for:

- invalid corner ordering rejection
- Nyquist rejection
- NaN/infinite parameter rejection
- zero-width passband acceptance or rejection policy
- compatibility classification

Milestone 1 recommendation:

- allow equality at corners
- a degenerate taper or passband simply collapses to the corresponding limiting case

### Kernel Correctness Tests

Add tests for:

- strong attenuation outside passband on synthetic traces
- passband preservation on synthetic traces
- mean removal behavior
- inverse normalization correctness
- preview and materialize using the same output kernel

Suggested fixtures:

- single-frequency sine
- mixed-frequency trace
- zero trace
- impulse-like trace

### Integration Tests

Add integration tests for:

- pipeline validation including `bandpass_filter`
- preview request returning processed section views
- materialized derived volume lineage including `bandpass_filter`
- spectrum request returning stable bin/amplitude lengths

## Rollout Order

Implementation order should be:

1. extend Ophiolite contracts
2. regenerate shared contracts artifacts
3. add Ophiolite runtime validation and kernel
4. add spectrum analysis request/response and runtime surface
5. add unit tests and benchmarks
6. wire TraceBoost app and Tauri commands
7. wire TraceBoost frontend editor and spectrum panel
8. ship the post-stack product subset

This order keeps the shared core stable before product UI work starts.

## Files Expected To Change

### Ophiolite

- `ophiolite/crates/ophiolite-seismic/src/contracts.rs`
- `ophiolite/crates/ophiolite-seismic-runtime/src/compute.rs`
- `ophiolite/crates/ophiolite-seismic-runtime/src/lib.rs`
- `ophiolite/crates/ophiolite-seismic-runtime/Cargo.toml`
- `ophiolite/crates/ophiolite-seismic-runtime/benches/compute_storage.rs`
- optional new runtime test modules if the FFT code is split out

### TraceBoost

- `contracts/seis-contracts-*` generated artifacts after export
- `app/traceboost-app/src/lib.rs`
- `app/traceboost-frontend/src-tauri/src/lib.rs`
- `app/traceboost-frontend/src/lib/bridge.ts`
- `app/traceboost-frontend/src/lib/processing-model.svelte.ts`
- `app/traceboost-frontend/src/lib/components/PipelineOperatorEditor.svelte`
- `app/traceboost-frontend/src/lib/components/PipelineSequenceList.svelte`
- likely one new frontend component for the spectrum panel

## Future Extension Path

This design is intentionally scalable toward richer shared operators.

Likely future additions:

- phase rotation
- notch filters
- spectral whitening
- FK-domain filters
- prestack-only operators
- angle/offset-conditioned trace operators

The current design supports that by:

- keeping the operator DSL shared in Ophiolite
- separating operator compatibility from product exposure
- keeping analysis APIs separate from materializing operators
- using explicit enums for spectral policy instead of raw ad hoc strings

## Acceptance Criteria

Milestone 1 is complete when all of the following are true:

- shared contracts support `bandpass_filter`
- generated TypeScript contracts include the new operator and spectrum-analysis payloads
- preview and full materialization can execute `bandpass_filter`
- TraceBoost can request and display raw and processed amplitude spectra
- the TraceBoost product UI exposes `bandpass_filter` only for post-stack workflows
- correctness tests cover validation and core signal behavior
- benchmarks exist and show acceptable throughput relative to current operators

## Non-Goals For This Spec

This document does not define:

- the exact final frontend visual design of the spectrum panel
- a GPU roadmap
- a distributed processing roadmap
- the future sibling prestack product UX

Those remain separate follow-on design tasks.
