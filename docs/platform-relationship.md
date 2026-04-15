# TraceBoost Platform Relationship

This document explains how TraceBoost relates to the Ophiolite platform.

## Product Roles

### TraceBoost

- owns workflow composition
- owns desktop UX and workspace behavior
- owns user-facing orchestration and app-local automation

### Ophiolite

- owns canonical subsurface contracts and DTO meaning
- owns reusable runtime primitives and local-first data foundations
- owns reusable seismic, map, well, and time-depth semantics

### Ophiolite Charts

- owns chart rendering and interaction behavior
- owns wrapper APIs for embedders
- consumes canonical contracts without becoming the semantic source of truth

## Dependency Direction

In practice the assembled application path is:

`TraceBoost app -> Ophiolite core + Ophiolite Charts`

That does not mean Ophiolite Charts depends on TraceBoost. It means TraceBoost is where the user sees the platform assembled into one application.

## Practical Boundary Rules

- if the feature is reusable subsurface meaning, it should move toward Ophiolite
- if the feature is chart behavior or embedder-facing rendering, it should live in Ophiolite Charts
- if the feature is workflow state, presets, orchestration, UX, or demo packaging, it belongs in TraceBoost
