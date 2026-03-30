# traceboost-frontend

First Svelte/Vite frontend host inside the TraceBoost monorepo.

Purpose:

- consume the generated `@traceboost/seis-contracts` package
- consume external `@geoviz/svelte` as a real package dependency
- render a real seismic section contract inside the embedded chart component

Development:

```powershell
bun run setup:bun-links
bun install
bun run dev
```

In dev mode, the Vite server bridges to the Rust CLI app:

- it ingests `test-data/small.sgy` into a local demo store if needed
- it calls `traceboost-app view-section` on `/api/section`

This proves the contracts-first seam before the Tauri desktop shell is added.
