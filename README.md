# TraceBoost

TraceBoost is the first-party seismic workflow application built on top of the Ophiolite platform.

In the current stack:

- `Ophiolite` owns canonical subsurface contracts, runtime primitives, and platform automation
- `Ophiolite Charts` owns embeddable chart rendering and interaction wrappers
- `TraceBoost` composes those lower layers into one opinionated workflow application

Start here for the product-facing docs:

- `docs/product-overview.md`
- `docs/platform-relationship.md`
- `docs/interfaces-and-automation.md`

## Current Focus

The clearest end-to-end path today is:

`select SEG-Y -> preflight/ingest -> open runtime store -> inspect inline/xline sections -> preview processing -> materialize derived output`

TraceBoost remains post-stack-first, local-first, and desktop-first.

## Repository Layout

- `contracts/` shared Rust contracts and generated TypeScript artifacts
- `io/` SEG-Y inspection and ingest-oriented helpers
- `runtime/` compatibility wrapper over shared Ophiolite runtime primitives
- `app/` application orchestration, frontend host, and Tauri shell
- `python/` thin application-facing automation wrapper
- `docs/` product and architecture docs

## Boundary Rule

TraceBoost should own workflow composition, workspace/session behavior, presets, demo packaging, and app-local automation.

Reusable semantics should move toward `ophiolite`. Reusable chart behavior should move toward `Ophiolite Charts`.

## Development

Frontend local dependencies resolve through the sibling `../ophiolite` checkout, including `../ophiolite/charts`.
