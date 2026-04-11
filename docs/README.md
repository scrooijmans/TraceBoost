# Documentation

This directory contains the current TraceBoost architecture docs plus archived legacy imports.

Current source-of-truth document:

- `architecture.md`
- `crs-display-workspace-phase1.md` for the phase-1 workspace/display-CRS contract over canonical Ophiolite seismic CRS metadata
- `compute-storage-benchmark.md` for the runtime compute/storage benchmark and format decision plan
- `multi-format-io-runtime-strategy.md` for the runtime-store evolution, format-adapter priorities, licensing read, and next benchmark matrix
- `seismic-zfp-assessment-and-benchmark-plan.md` for the SGZ / `seismic-zfp` assessment and benchmark scope
- `spectral-processing-implementation.md` for the shared Ophiolite + TraceBoost frequency-domain operator and spectrum-analysis implementation plan

Supporting benchmark scripts now include:

- `scripts/openvds_storage_bench.cpp`
- `scripts/sgz_storage_bench.py`

Status of the rest of this folder:

- `legacy/` contains imported material preserved for historical reference
- `legacy/upscayl-import/` contains the unrelated Upscayl documentation baseline that existed before the monorepo reset
- current TraceBoost product truth should stay at the docs root, not inside `legacy/`

When docs conflict, prefer:

1. `README.md` at the repo root
2. `docs/architecture.md`
3. subsystem READMEs under `contracts/`, `io/`, `runtime/`, and `app/`
