# TraceBoost Product Overview

TraceBoost is the first-party workflow application in the current stack.

It is the application users operate directly to ingest seismic data, inspect it, run processing workflows, manage workspace state, and automate repeatable local tasks.

## What TraceBoost Is

- a desktop-oriented seismic workflow application
- the owner of ingest, open, view, process, export, and workspace flows
- the owner of app-local automation surfaces such as the current CLI and Python wrapper
- the place where the broader platform becomes one coherent end-user experience

## What TraceBoost Is Not

- not the canonical owner of reusable subsurface semantics
- not the chart-rendering SDK
- not the platform brand

## Current Customer-Facing Interfaces

- desktop application shell
- app/backend CLI through `traceboost-app`
- Python automation wrapper through `traceboost-automation`

## Relationship To The Platform

- `Ophiolite` is the subsurface core beneath TraceBoost
- `Ophiolite Charts` is the visualization SDK embedded inside TraceBoost
- `TraceBoost` is where those lower layers become a user-facing workflow application
