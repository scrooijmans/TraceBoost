# app

`app/` contains product-facing application crates and frontend hosts for TraceBoost.

Current crate:

- `traceboost-app`

This area owns:

- application workflow
- orchestration
- session and command surfaces
- future Tauri-facing app crates

Current contents:

- `traceboost-app`
  - Rust app shell / CLI over the monorepo runtime
- `traceboost-frontend`
  - Svelte/Vite frontend host that consumes generated contracts and embeds external `geoviz`

This area should depend on `runtime/`, not reach around it directly into lower layers unless there is a deliberate exception.
