# traceboost-frontend

`traceboost-frontend` is the current UI host for the first working TraceBoost desktop workflow.

It is the point where the other two products become visible in one application:

- TraceBoost owns the workflow and app state
- Ophiolite-backed contracts shape the data crossing the boundary
- Ophiolite Charts renders the embedded chart experience

## Stack

- Svelte 5
- Vite
- Bun
- generated `@traceboost/seis-contracts`
- external `@ophiolite/charts`
- Tauri 2 desktop shell under `src-tauri`

## Data Boundary

- inputs from the user:
  - SEG-Y file path
  - runtime-store output path
  - existing runtime-store path
- backend responses:
  - JSON payloads typed from `@traceboost/seis-contracts`
- rendered data:
  - section views returned by `traceboost-app` / `seis-runtime`

## Implemented

- form-driven workflow for:
  - preflighting a SEG-Y file
  - importing into the runtime store
  - opening an existing runtime store
  - loading inline/xline sections
- shared frontend bridge that can call:
  - Vite dev endpoints in browser mode
  - Tauri commands in desktop mode
- embedded Ophiolite Charts section rendering
- typechecked/generated contract consumption

## Development

```powershell
bun install
bun run dev
```

If you want an explicit prerequisite check before install:

```powershell
bun run doctor
```

Additional checks:

```powershell
bun run typecheck
bun run build
```

Run the desktop shell:

```powershell
bun run tauri:dev
```

## Local Prerequisites

- keep the sibling `ophiolite` repository checked out at `../ophiolite`
- use the repo-pinned Rust toolchain from `../../rust-toolchain.toml`

In browser dev mode, Vite exposes app-oriented endpoints that shell out to `traceboost-app` for:

- `/api/preflight`
- `/api/import`
- `/api/open`
- `/api/section`

## Roadmap

1. Replace manual path entry with native file and folder pickers through Tauri.
2. Add session/recent-dataset UX and better progress/error presentation.
3. Keep the first milestone narrow:
   import/open/view 2D sections only.
4. Defer broader processing UI until the desktop shell feels stable.
