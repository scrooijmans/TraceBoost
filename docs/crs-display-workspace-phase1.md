# TraceBoost CRS Display Workspace Phase 1

This document captures the exact TraceBoost-side contract and UX changes required to consume the canonical CRS model defined in Ophiolite ADR-0014.

TraceBoost does not become the owner of native CRS truth. It owns:

- workspace display CRS preference
- user-facing warnings and override workflows
- cache invalidation for display-derived geometry

## Design Rules

- native/effective CRS truth comes from the Ophiolite dataset/store descriptor
- dataset registry entries do not become a second source of CRS truth
- workspace display CRS is persisted as session state
- display-space survey geometry is derived output and must be invalidated when display CRS changes

## Exact Contract Changes

### 1. Workspace session contract

Current `WorkspaceSession` and `SaveWorkspaceSessionRequest` are the correct persistence boundary for the display CRS.

Phase-1 target in `contracts/seis-contracts-interop/src/lib.rs`:

```rust
pub struct WorkspaceSession {
    pub active_entry_id: Option<String>,
    pub active_store_path: Option<String>,
    pub active_axis: SectionAxis,
    pub active_index: usize,
    pub selected_preset_id: Option<String>,
    pub display_coordinate_reference_id: Option<String>,
}

pub struct SaveWorkspaceSessionRequest {
    pub schema_version: u32,
    pub active_entry_id: Option<String>,
    pub active_store_path: Option<String>,
    pub active_axis: SectionAxis,
    pub active_index: usize,
    pub selected_preset_id: Option<String>,
    pub display_coordinate_reference_id: Option<String>,
}
```

No separate app-global settings type is needed for phase 1. The display CRS is workspace/session state.

### 2. Dataset registry contract

`DatasetRegistryEntry` should remain unchanged in phase 1.

Rationale:

- `last_dataset.descriptor` already carries the canonical Ophiolite descriptor
- duplicating CRS fields into the registry would create drift between app-local state and store metadata

Registry consumers should read native/effective CRS from the dataset descriptor when available.

### 3. New override command surface

TraceBoost needs an app command that forwards a native-CRS override into Ophiolite-managed dataset metadata.

Recommended request/response shape:

```rust
pub struct SetDatasetNativeCoordinateReferenceRequest {
    pub schema_version: u32,
    pub store_path: String,
    pub coordinate_reference_id: Option<String>,
    pub coordinate_reference_name: Option<String>,
}

pub struct SetDatasetNativeCoordinateReferenceResponse {
    pub schema_version: u32,
    pub dataset: DatasetSummary,
}
```

This should be added to the interop contract layer if TraceBoost owns the IPC boundary for it.

The UI should call this operation instead of patching registry/session files directly.

## Frontend State Expectations

The viewer/workspace model should derive these states from `DatasetSummary.descriptor` and the current workspace session:

- native CRS unknown
- native CRS overridden
- mixed effective native CRSs across loaded datasets
- display CRS set
- display CRS unset
- display transform unavailable for one or more datasets

## UX Requirements

### Asset details workflow

Each loaded dataset should expose:

- detected native CRS
- effective native CRS
- CRS source:
  - header
  - import manifest
  - user override
  - unknown
- an override control that writes through the backend command
- a clear action to clear the override

### Workspace settings workflow

The workspace/session settings should expose:

- `Display CRS`
- suggested default when all loaded datasets share one effective native CRS
- a warning when multiple effective native CRSs exist and no display CRS is selected

### Map/chart warnings

TraceBoost should warn, not block, in phase 1:

- when a dataset has unknown native CRS
- when a display CRS is set but one or more datasets cannot be transformed into it yet
- when multiple effective native CRSs are present without a display CRS

## Display-Geometry Cache Rules

Any display-derived survey map cache must be invalidated when:

- workspace `display_coordinate_reference_id` changes
- dataset effective native CRS changes
- dataset survey-map response schema version changes

Native-space-only cache entries may remain valid across display-CRS changes.

## Phase-1 Behavior Matrix

- known effective native CRS, no display CRS:
  - render native-space map

- unknown effective native CRS, no display CRS:
  - render native-space map if native geometry exists
  - warn that the native CRS is unknown

- known effective native CRS, display CRS equals native CRS:
  - render display-space map using the same geometry

- known effective native CRS, display CRS differs, reprojection not implemented:
  - keep native-space map available
  - show display-space unavailable warning

- mixed effective native CRSs, no display CRS:
  - allow isolated dataset map views
  - warn before implying overlay/alignment

## What Phase 1 Explicitly Does Not Do

- no app-side reprojection logic
- no geoviz-side CRS reasoning
- no automatic adoption of display CRS as native CRS
- no registry-only CRS overrides

## Implementation Order

1. Extend the workspace session contract with `display_coordinate_reference_id`.
2. Add the native-CRS override command surface.
3. Add frontend state and warnings derived from the canonical descriptor.
4. Add the asset-details override UI.
5. Add the workspace display-CRS setting UI.
