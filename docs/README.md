# Documentation

This directory contains the current TraceBoost architecture docs plus archived legacy imports.

Current source-of-truth document:

- `architecture.md`

Status of the rest of this folder:

- `legacy/` contains imported material preserved for historical reference
- `legacy/upscayl-import/` contains the unrelated Upscayl documentation baseline that existed before the monorepo reset
- current TraceBoost product truth should stay at the docs root, not inside `legacy/`

When docs conflict, prefer:

1. `README.md` at the repo root
2. `docs/architecture.md`
3. subsystem READMEs under `contracts/`, `io/`, `runtime/`, and `app/`
