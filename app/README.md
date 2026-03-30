# app

`app/` contains product-facing application crates for TraceBoost.

Current crate:

- `traceboost-app`

This area owns:

- application workflow
- orchestration
- session and command surfaces
- future Tauri-facing app crates

This area should depend on `runtime/`, not reach around it directly into lower layers unless there is a deliberate exception.
