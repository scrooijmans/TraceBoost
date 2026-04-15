# TraceBoost Interfaces And Automation

TraceBoost exposes one application through multiple interfaces.

## Desktop Application

The desktop shell is the primary user-facing interface.

It owns:

- ingest and open flows
- section viewing and workspace state
- processing preview and apply flows
- product-local diagnostics and recent-session behavior

## CLI

`traceboost-app` is the application control plane for local commands and app/backend orchestration.

It exposes workflow-shaped commands for local use.

## Python

`traceboost-automation` is a thin Python wrapper over the same local CLI workflows.

It is meant for notebooks, repeatable scripts, local batch jobs, and demo preparation.

## Boundary Rule

These interfaces are application-facing. Platform-stable automation should continue moving toward Ophiolite over time, while TraceBoost keeps the workflow-specific wrappers it actually needs.
