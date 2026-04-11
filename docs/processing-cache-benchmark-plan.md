# Processing Cache Benchmark Plan

## Purpose

This benchmark exists to answer one product question with measured data:

- does processing-cache materially reduce rerun time enough to justify the additional code and storage complexity?

If the answer is no, the cache path should be removed and TraceBoost should return to the simpler non-cached implementation.

## Success Criteria

The cache work is justified only if all of the following hold:

- exact full-rerun reuse returns immediately and avoids recompute in practice
- prefix reuse reduces wall-clock rerun time for late-pipeline edits by a meaningful margin
- the disk growth introduced by checkpoints stays within the configured cache budget
- added overhead on cache misses is small relative to the baseline uncached run

Recommended acceptance bar for late-edit reruns:

- at least `25%` wall-clock improvement on the representative multi-step rerun cases
- no more than `10%` regression on first-run cache-miss cases

If these bars are not met on representative data, remove or heavily simplify the cache path.

## Datasets

Use both real and synthetic data.

Real datasets:

- `C:\Users\crooijmanss\dev\TraceBoost\test-data\f3.tbvol` if present
- otherwise ingest `C:\Users\crooijmanss\dev\TraceBoost\test-data\f3.sgy` into a local `tbvol`

Synthetic datasets:

- small synthetic `tbvol`
- medium synthetic `tbvol`
- large synthetic `tbvol`

Synthetic stores should be deterministic and sized to make multi-step runs visible in timings.

## Pipeline Matrix

Run at least these trace-local pipeline shapes:

1. `3-step`
- `amplitude_scalar`
- `agc_rms`
- `phase_rotation`

2. `5-step`
- `highpass_filter`
- `agc_rms`
- `phase_rotation`
- `bandpass_filter`
- `amplitude_scalar`

3. `8-step`
- mix of scalar, AGC, phase rotation, and spectral filters

4. `10-step`
- same operator families, with repeated filter/AGC phases to simulate realistic tuning

## Scenarios

Measure each pipeline under these scenarios:

1. Baseline uncached first run
- cache disabled

2. Cached first run
- cache enabled
- measures cache-miss overhead

3. Exact rerun
- same pipeline, same source
- should short-circuit to existing output

4. Late-edit rerun
- modify the final operator only

5. Mid-late edit rerun
- modify operator `N-1`

6. Middle edit rerun
- modify operator near the center of the pipeline

7. Checkpointed rerun
- explicit user checkpoints enabled
- modify a later operator and confirm prefix reuse from the latest matching checkpoint

## Metrics

Record at minimum:

- wall-clock job duration
- time spent before first progress update
- total output bytes
- checkpoint bytes written
- cache hit or miss classification
- reused prefix length
- exact-output reuse versus prefix reuse

Recommended additional metrics:

- CPU time if easy to capture
- number of materialized stores written per run
- cache directory size before and after each run

## Benchmark Procedure

For each dataset and pipeline:

1. clear the processing cache
2. run the uncached baseline
3. enable cache and run the first cached pass
4. rerun exact
5. rerun after editing one late operator
6. rerun after editing one middle operator

Repeat each case at least `3` times and report median time.

## Interpretation

The cache path is worth keeping if:

- exact reruns are effectively instant relative to baseline
- late-edit reruns show strong speedup
- cache-miss overhead is modest
- disk growth remains bounded and predictable

The cache path is not worth keeping if:

- late-edit reruns do not materially improve
- cache lookup/registration overhead erodes first-run performance too much
- disk amplification is too large for the achieved speedup
- the implementation becomes too complex relative to measured benefit

## Rollback Rule

If the benchmark does not show clear improvement on the representative `f3` and synthetic cases, revert the cache path and keep the simpler pre-cache processing implementation.
