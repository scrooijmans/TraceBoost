<svelte:options runes={true} />

<script lang="ts">
  import type { SurveyMapProbe } from "@geoviz/data-models";
  import { adaptOphioliteSurveyMapToGeoviz, type OphioliteResolvedSurveyMapSource } from "@geoviz/data-models";
  import { SurveyMapChart } from "@geoviz/svelte";
  import type { DatasetSummary, ResolvedSurveyMapSourceDto } from "@traceboost/seis-contracts";
  import { pickProjectFolder, pickVelocityFunctionsFile, pickWellTimeDepthJsonFile } from "../file-dialog";
  import { getViewerModelContext } from "../viewer-model.svelte";

  interface Props {
    showSidebar: boolean;
    hideSidebar: () => void;
  }

  let { showSidebar, hideSidebar }: Props = $props();

  const viewerModel = getViewerModelContext();
  let mapProbe = $state.raw<SurveyMapProbe | null>(null);
  let mapProbeSourceId = $state<string | null>(null);
  let volumeContextMenu = $state.raw<{
    entryId: string;
    label: string;
    x: number;
    y: number;
    exportable: boolean;
  } | null>(null);
  const activeSurveyMapSurvey = $derived(viewerModel.surveyMapSource?.surveys[0] ?? null);
  const activeSurveyMapPreviewHorizon = $derived.by(() => {
    const source = viewerModel.surveyMapSource;
    const horizonId = source?.scalar_field_horizon_id;
    if (!source || !horizonId) {
      return null;
    }
    return source.horizons.find((horizon) => horizon.id === horizonId) ?? null;
  });
  const sidebarSurveyMap = $derived.by(() => {
    if (viewerModel.surveyMapSource) {
      return adaptOphioliteSurveyMapToGeoviz(resolvedSurveyMapToGeovizSource(viewerModel.surveyMapSource));
    }

    const dataset = viewerModel.dataset ?? viewerModel.activeDatasetEntry?.last_dataset ?? null;
    return dataset ? adaptOphioliteSurveyMapToGeoviz(datasetSummaryToSurveyMapSource(dataset)) : null;
  });

  function basename(filePath: string): string {
    return filePath.split(/[\\/]/).pop() ?? filePath;
  }

  function fileStem(filePath: string | null | undefined): string {
    const filename = basename(filePath ?? "");
    return filename.replace(/\.[^.]+$/, "");
  }

  function stripGeneratedHashSuffix(value: string): string {
    return value.replace(/-[0-9a-f]{16}$/i, "");
  }

  function normalizeGeneratedSeparators(value: string): string {
    return value.replace(/\s*(?:Â·|·)\s*/g, " | ");
  }

  function datasetLabel(displayName: string, fallbackPath: string | null | undefined, entryId: string): string {
    const trimmedDisplayName = displayName.trim();
    if (trimmedDisplayName) {
      return normalizeGeneratedSeparators(stripGeneratedHashSuffix(trimmedDisplayName));
    }

    const preferredPathLabel = fileStem(fallbackPath);
    if (preferredPathLabel) {
      return normalizeGeneratedSeparators(stripGeneratedHashSuffix(preferredPathLabel));
    }

    return entryId;
  }

  function entryStorePath(entry: {
    last_dataset?: { store_path: string } | null;
    imported_store_path?: string | null;
    preferred_store_path?: string | null;
  }): string {
    return entry.last_dataset?.store_path ?? entry.imported_store_path ?? entry.preferred_store_path ?? "";
  }

  function closeVolumeContextMenu(): void {
    volumeContextMenu = null;
  }

  function openVolumeContextMenu(
    event: MouseEvent,
    entryId: string,
    label: string,
    exportable: boolean
  ): void {
    event.preventDefault();
    event.stopPropagation();
    const menuWidth = 192;
    const menuHeight = 54;
    volumeContextMenu = {
      entryId,
      label,
      x: Math.min(event.clientX, Math.max(12, window.innerWidth - menuWidth - 12)),
      y: Math.min(event.clientY, Math.max(12, window.innerHeight - menuHeight - 12)),
      exportable
    };
  }

  async function handleContextMenuExport(): Promise<void> {
    const context = volumeContextMenu;
    if (!context?.exportable) {
      return;
    }
    closeVolumeContextMenu();
    await viewerModel.openDatasetExportDialog(context.entryId);
  }

  function handleVolumeListKeyDown(event: KeyboardEvent): void {
    if (event.key === "Escape" && volumeContextMenu) {
      closeVolumeContextMenu();
      return;
    }

    if (!(event.ctrlKey || event.metaKey)) {
      return;
    }

    const key = event.key.toLowerCase();
    if (key === "c" && viewerModel.activeEntryId) {
      event.preventDefault();
      viewerModel.copyActiveWorkspaceEntry();
    }

    if (key === "v") {
      event.preventDefault();
      void viewerModel.pasteCopiedWorkspaceEntry();
    }
  }

  function datasetSummaryToSurveyMapSource(dataset: DatasetSummary): OphioliteResolvedSurveyMapSource {
    const inline = dataset.descriptor.geometry.summary.inline_axis;
    const xline = dataset.descriptor.geometry.summary.xline_axis;
    const spatial = dataset.descriptor.spatial;
    const footprint = spatial?.footprint?.exterior?.map((point) => ({ x: point.x, y: point.y })) ?? null;
    const outline =
      footprint && footprint.length >= 4
        ? footprint.slice(0, Math.max(footprint.length - 1, 4))
        : [
            { x: xline.first, y: inline.first },
            { x: xline.last, y: inline.first },
            { x: xline.last, y: inline.last },
            { x: xline.first, y: inline.last }
          ];
    const coordinateReference = spatial?.coordinate_reference;

    return {
      id: `${dataset.descriptor.id}-survey-map`,
      name: dataset.descriptor.label,
      x_label: footprint ? "X" : "Xline",
      y_label: footprint ? "Y" : "Inline",
      coordinate_unit: coordinateReference?.unit ?? (footprint ? "projected" : "index"),
      background: "#1b1b1b",
      surveys: [
        {
          id: `${dataset.descriptor.id}-outline`,
          name: dataset.descriptor.label,
          outline,
          stroke: "rgba(103, 196, 143, 0.95)",
          fill: "rgba(103, 196, 143, 0.12)"
        }
      ],
      wells: []
    };
  }

  function resolvedSurveyMapToGeovizSource(source: ResolvedSurveyMapSourceDto): OphioliteResolvedSurveyMapSource {
    const primarySurvey = source.surveys[0] ?? null;
    const primarySpatial = primarySurvey ? primarySurvey.display_spatial ?? primarySurvey.native_spatial : null;
    const usesProjectedSpace = Boolean(primarySpatial?.footprint);
    const coordinateReference =
      primarySpatial?.coordinate_reference ??
      primarySurvey?.coordinate_reference_binding?.effective ??
      primarySurvey?.coordinate_reference_binding?.detected ??
      null;

    return {
      id: source.id,
      name: source.name,
      x_label: usesProjectedSpace ? "X" : "Xline",
      y_label: usesProjectedSpace ? "Y" : "Inline",
      coordinate_unit: coordinateReference?.unit ?? (usesProjectedSpace ? "projected" : "index"),
      background: "#1b1b1b",
      scalar_field: source.scalar_field ? adaptResolvedSurveyMapScalarField(source.scalar_field) : null,
      surveys: source.surveys.map((survey) => {
        const spatial = survey.display_spatial ?? survey.native_spatial;
        const footprint = spatial.footprint?.exterior?.map((point) => ({ x: point.x, y: point.y })) ?? null;
        const outline =
          footprint && footprint.length >= 4
            ? footprint.slice(0, Math.max(footprint.length - 1, 4))
            : [
                { x: survey.index_grid.xline_axis.first, y: survey.index_grid.inline_axis.first },
                { x: survey.index_grid.xline_axis.last, y: survey.index_grid.inline_axis.first },
                { x: survey.index_grid.xline_axis.last, y: survey.index_grid.inline_axis.last },
                { x: survey.index_grid.xline_axis.first, y: survey.index_grid.inline_axis.last }
              ];

        return {
          id: survey.asset_id,
          name: survey.name,
          outline,
          stroke: "rgba(103, 196, 143, 0.95)",
          fill: "rgba(103, 196, 143, 0.12)"
        };
      }),
      wells: source.wells.flatMap((well) => {
        const surfaceLocation = well.surface_location;
        if (!surfaceLocation) {
          return [];
        }

        const firstTrajectory = well.trajectories[0];
        const trajectory =
          firstTrajectory && firstTrajectory.rows.length
            ? [
                { x: surfaceLocation.x, y: surfaceLocation.y },
                ...firstTrajectory.rows.flatMap((station) =>
                  station.easting_offset === null || station.northing_offset === null
                    ? []
                    : [
                        {
                          x: surfaceLocation.x + station.easting_offset,
                          y: surfaceLocation.y + station.northing_offset
                        }
                      ]
                )
              ]
            : undefined;

        return [
          {
            well_id: well.well_id,
            wellbore_id: well.wellbore_id,
            name: well.name,
            surface_position: surfaceLocation,
            plan_trajectory: trajectory,
            color: "rgba(236, 236, 236, 0.92)"
          }
        ];
      })
    };
  }

  function adaptResolvedSurveyMapScalarField(
    field: ResolvedSurveyMapSourceDto["scalar_field"] extends infer T ? NonNullable<T> : never
  ): NonNullable<OphioliteResolvedSurveyMapSource["scalar_field"]> {
    return {
      id: field.id,
      name: field.name,
      columns: field.columns,
      rows: field.rows,
      values: field.values,
      origin: field.origin,
      step: field.step,
      unit: field.unit ?? undefined,
      min_value: field.min_value ?? undefined,
      max_value: field.max_value ?? undefined
    };
  }

  function transformStatusLabel(status: string | null | undefined): string {
    switch (status) {
      case "display_equivalent":
        return "Display CRS matches native";
      case "display_transformed":
        return "Display transform active";
      case "display_degraded":
        return "Degraded display transform";
      case "display_unavailable":
        return "Display transform unavailable";
      case "native_only":
        return "Native coordinates only";
      default:
        return "No map transform";
    }
  }

  function velocitySourceKindLabel(sourceKind: string): string {
    switch (sourceKind) {
      case "velocity_grid3_d":
        return "3D grid";
      case "horizon_layer_model":
        return "Horizon model";
      case "checkshot_model1_d":
        return "Checkshot";
      case "sonic_log1_d":
        return "Sonic";
      case "vp_log1_d":
        return "Vp";
      case "velocity_function1_d":
        return "1D function";
      case "constant_velocity":
        return "Constant";
      default:
        return sourceKind;
    }
  }

  function wellTimeDepthAssetKindLabel(assetKind: string): string {
    switch (assetKind) {
      case "checkshot_vsp_observation_set":
        return "Checkshot/VSP";
      case "manual_time_depth_pick_set":
        return "Manual Picks";
      case "well_time_depth_authored_model":
        return "Authored";
      case "well_time_depth_model":
        return "Compiled";
      default:
        return assetKind;
    }
  }

  function selectedProjectWellBinding() {
    const selectedWellbore = viewerModel.selectedProjectWellboreInventoryItem;
    if (!selectedWellbore) {
      return null;
    }
    return {
      well_name: selectedWellbore.wellName,
      wellbore_name: selectedWellbore.wellboreName,
      operator_aliases: []
    };
  }

  async function handleImportVelocityFunctions(): Promise<void> {
    const inputPath = await pickVelocityFunctionsFile();
    if (!inputPath) {
      return;
    }
    await viewerModel.importVelocityFunctionsFile(inputPath, "interval");
  }

  async function handlePickProjectRoot(): Promise<void> {
    const projectRoot = await pickProjectFolder();
    if (!projectRoot) {
      return;
    }
    viewerModel.setProjectRoot(projectRoot);
  }

  async function handleResolveProjectWellOverlays(): Promise<void> {
    try {
      await viewerModel.resolveConfiguredProjectSectionWellOverlays();
    } catch (error) {
      viewerModel.note(
        "Failed to resolve configured project well overlays.",
        "backend",
        "warn",
        error instanceof Error ? error.message : String(error)
      );
    }
  }

  async function handleImportProjectWellTimeDepthAsset(
    assetKind:
      | "checkshot_vsp_observation_set"
      | "manual_time_depth_pick_set"
      | "well_time_depth_authored_model"
      | "well_time_depth_model",
    dialogTitle: string
  ): Promise<void> {
    const projectRoot = viewerModel.projectRoot.trim();
    const binding = selectedProjectWellBinding();
    if (!projectRoot || !binding) {
      return;
    }

    const jsonPath = await pickWellTimeDepthJsonFile(dialogTitle);
    if (!jsonPath) {
      return;
    }

    try {
      await viewerModel.importProjectWellTimeDepthAsset({
        projectRoot,
        jsonPath,
        binding,
        assetKind
      });
      await viewerModel.refreshProjectWellOverlayInventory(projectRoot);
    } catch (error) {
      viewerModel.note(
        "Failed to import project well time-depth asset.",
        "backend",
        "warn",
        error instanceof Error ? error.message : String(error)
      );
    }
  }

  async function handleCompileProjectWellTimeDepthAuthoredModel(assetId: string): Promise<void> {
    const projectRoot = viewerModel.projectRoot.trim();
    if (!projectRoot) {
      return;
    }

    try {
      await viewerModel.compileProjectWellTimeDepthAuthoredModel({
        projectRoot,
        assetId,
        setActive: true
      });
      await viewerModel.refreshProjectWellOverlayInventory(projectRoot);
    } catch (error) {
      viewerModel.note(
        "Failed to compile project well time-depth authored model.",
        "backend",
        "warn",
        error instanceof Error ? error.message : String(error)
      );
    }
  }
</script>

<svelte:window onclick={closeVolumeContextMenu} />

<aside class:hidden={!showSidebar} class="sidebar">
  <div class="sidebar-header">
    <div class="logo-row">
      <svg
        class="logo-icon"
        viewBox="0 0 24 24"
        width="32"
        height="32"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
      >
        <path
          d="M3 20 L6 8 L9 14 L12 4 L15 16 L18 10 L21 20"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <div class="logo-copy">
        <h1>TraceBoost <span class="version">v0.1.0</span></h1>
        <p class="subtitle">Seismic Volumes</p>
      </div>
      <button class="collapse-button" onclick={hideSidebar} aria-label="Hide sidebar">
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
    </div>
  </div>

  <div class="volume-list-shell">
    {#if viewerModel.workspaceEntries.length}
      <div
        class="volume-list"
        role="listbox"
        tabindex="0"
        onkeydown={handleVolumeListKeyDown}
        aria-label="Seismic volumes"
      >
        {#each viewerModel.workspaceEntries as entry (entry.entry_id)}
          {@const visibleLabel = datasetLabel(
            entry.display_name,
            entry.source_path ?? entry.imported_store_path ?? entry.preferred_store_path,
            entry.entry_id
          )}
          <div class="volume-row">
            <button
              class:active={viewerModel.activeEntryId === entry.entry_id}
              class="volume-entry"
              onclick={() => void viewerModel.activateDatasetEntry(entry.entry_id)}
              oncontextmenu={(event) =>
                openVolumeContextMenu(
                  event,
                  entry.entry_id,
                  visibleLabel,
                  entryStorePath(entry).trim().length > 0
                )}
              disabled={viewerModel.loading}
              title={visibleLabel}
            >
              <span class="volume-entry-label">
                {visibleLabel}
              </span>
            </button>
            <button
              class="volume-remove"
              onclick={() => void viewerModel.removeWorkspaceEntry(entry.entry_id)}
              disabled={viewerModel.loading}
              aria-label={`Remove ${visibleLabel}`}
              title={`Remove ${visibleLabel}`}
            >
              X
            </button>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-state">
        <span class="empty-title">No volumes loaded</span>
        <p>Use <strong>File &gt; Open Volume...</strong> to open a `.tbvol` or import a `.segy`.</p>
      </div>
    {/if}

    <section class="velocity-models-panel">
      <div class="velocity-models-header">
        <div>
          <span>Velocity Models</span>
          <small>
            {#if viewerModel.activeVelocityModelDescriptor}
              Active: {viewerModel.activeVelocityModelDescriptor.name}
            {:else}
              Active: Global 1D fallback
            {/if}
          </small>
        </div>
        <div class="velocity-model-actions">
          <button
            type="button"
            class="secondary"
            disabled={!viewerModel.activeStorePath || viewerModel.loading}
            onclick={() => viewerModel.openVelocityModelWorkbench()}
          >
            Model...
          </button>
          <button
            type="button"
            class="secondary"
            disabled={!viewerModel.activeStorePath || viewerModel.velocityModelsLoading || viewerModel.loading}
            onclick={() => void handleImportVelocityFunctions()}
          >
            Import...
          </button>
          <button
            type="button"
            class="secondary"
            disabled={!viewerModel.activeStorePath || viewerModel.velocityModelsLoading || viewerModel.loading}
            onclick={() => void viewerModel.refreshVelocityModels()}
          >
            Refresh
          </button>
          <button
            type="button"
            disabled={!viewerModel.activeStorePath || viewerModel.velocityModelsLoading || viewerModel.loading}
            onclick={() => void viewerModel.createDemoVelocityModel()}
          >
            Create Demo
          </button>
        </div>
      </div>

      <button
        class:active={!viewerModel.activeVelocityModelAssetId}
        class="velocity-model-entry velocity-model-fallback"
        type="button"
        disabled={!viewerModel.activeStorePath || viewerModel.loading}
        onclick={() => void viewerModel.activateVelocityModel(null)}
      >
        <span class="velocity-model-name">Global 1D fallback</span>
        <small>Constant or 1D velocity function</small>
      </button>

      {#if viewerModel.velocityModelsError}
        <p class="velocity-model-error">{viewerModel.velocityModelsError}</p>
      {:else if viewerModel.velocityModelsLoading}
        <p class="velocity-model-empty">Loading velocity models...</p>
      {:else if viewerModel.availableVelocityModels.length}
        <div class="velocity-model-list">
          {#each viewerModel.availableVelocityModels as model (model.id)}
            <button
              class:active={viewerModel.activeVelocityModelAssetId === model.id}
              class="velocity-model-entry"
              type="button"
              disabled={viewerModel.loading}
              onclick={() => void viewerModel.activateVelocityModel(model.id)}
              title={model.name}
            >
              <span class="velocity-model-name">{model.name}</span>
              <small>
                {velocitySourceKindLabel(model.source_kind)} | {model.coverage.relationship}
              </small>
            </button>
          {/each}
        </div>
      {:else if viewerModel.activeStorePath}
        <p class="velocity-model-empty">
          No survey velocity models are registered for this volume yet.
        </p>
      {:else}
        <p class="velocity-model-empty">
          Open a seismic volume to manage velocity models.
        </p>
      {/if}
    </section>

    <section class="velocity-models-panel">
      <div class="velocity-models-header">
        <div>
          <span>Section Wells</span>
          <small>
            {#if viewerModel.sectionWellOverlays.length}
              Active overlays: {viewerModel.sectionWellOverlays.length}
            {:else}
              No active section well overlays
            {/if}
          </small>
        </div>
        <div class="velocity-model-actions">
          <button
            type="button"
            class="secondary"
            disabled={viewerModel.loading}
            onclick={() => void handlePickProjectRoot()}
          >
            Browse…
          </button>
          <button
            type="button"
            class="secondary"
            disabled={viewerModel.loading || !viewerModel.projectRoot}
            onclick={() => void viewerModel.refreshProjectWellOverlayInventory(viewerModel.projectRoot)}
          >
            Refresh
          </button>
          <button
            type="button"
            disabled={!viewerModel.canResolveConfiguredProjectSectionWellOverlays || viewerModel.projectSectionWellOverlaysLoading || viewerModel.loading}
            onclick={() => void handleResolveProjectWellOverlays()}
          >
            {viewerModel.projectSectionWellOverlaysLoading ? "Resolving…" : "Resolve"}
          </button>
        </div>
      </div>

      <label class="crs-field">
        <span>Project Root</span>
        <input
          bind:value={viewerModel.projectRoot}
          type="text"
          placeholder="C:\\data\\ophiolite-project"
          onblur={() => viewerModel.setProjectRoot(viewerModel.projectRoot)}
          onkeydown={(event) => {
            if (event.key === "Enter") {
              viewerModel.setProjectRoot(viewerModel.projectRoot);
            }
          }}
        />
      </label>

      <label class="crs-field">
        <span>Survey Asset</span>
        <select
          bind:value={viewerModel.projectSurveyAssetId}
          disabled={viewerModel.projectWellOverlayInventoryLoading || !viewerModel.projectSurveyAssets.length}
          onchange={() => viewerModel.setProjectSurveyAssetId(viewerModel.projectSurveyAssetId)}
        >
          {#if viewerModel.projectSurveyAssets.length}
            {#each viewerModel.projectSurveyAssets as survey (survey.assetId)}
              <option value={survey.assetId}>
                {survey.name} | {survey.wellboreName}
              </option>
            {/each}
          {:else}
            <option value="">
              {viewerModel.projectWellOverlayInventoryLoading ? "Loading surveys..." : "No survey assets found"}
            </option>
          {/if}
        </select>
      </label>

      <label class="crs-field">
        <span>Wellbore</span>
        <select
          bind:value={viewerModel.projectWellboreId}
          disabled={viewerModel.projectWellOverlayInventoryLoading || !viewerModel.projectWellboreInventory.length}
          onchange={() => viewerModel.setProjectWellboreId(viewerModel.projectWellboreId)}
        >
          {#if viewerModel.projectWellboreInventory.length}
            {#each viewerModel.projectWellboreInventory as wellbore (wellbore.wellboreId)}
              <option value={wellbore.wellboreId}>
                {wellbore.wellName} | {wellbore.wellboreName}
              </option>
            {/each}
          {:else}
            <option value="">
              {viewerModel.projectWellOverlayInventoryLoading ? "Loading wellbores..." : "No wellbores found"}
            </option>
          {/if}
        </select>
      </label>

      <label class="crs-field">
        <span>Tolerance (m)</span>
        <input
          bind:value={viewerModel.projectSectionToleranceM}
          type="number"
          min="0.1"
          step="0.1"
          onblur={() => viewerModel.setProjectSectionToleranceM(viewerModel.projectSectionToleranceM)}
        />
      </label>

      {#if viewerModel.projectWellOverlayInventoryError}
        <p class="velocity-model-error">{viewerModel.projectWellOverlayInventoryError}</p>
      {:else if viewerModel.projectWellOverlayInventoryLoading}
        <p class="velocity-model-empty">Loading project inventory...</p>
      {/if}

      {#if viewerModel.selectedProjectSurveyAsset}
        <div class="crs-meta">
          <span>Survey Asset Id</span>
          <strong>{viewerModel.selectedProjectSurveyAsset.assetId}</strong>
        </div>
      {/if}

      {#if viewerModel.selectedProjectWellboreInventoryItem}
        <div class="crs-meta">
          <span>Selected wellbore</span>
          <strong>
            {viewerModel.selectedProjectWellboreInventoryItem.wellName} | {viewerModel.selectedProjectWellboreInventoryItem.wellboreName}
          </strong>
        </div>
        <div class="crs-meta">
          <span>Current assets</span>
          <strong>
            Traj {viewerModel.selectedProjectWellboreInventoryItem.trajectoryAssetCount} | Models {viewerModel.selectedProjectWellboreInventoryItem.wellTimeDepthModelCount}
          </strong>
        </div>
        {#if viewerModel.selectedProjectWellboreInventoryItem.activeWellTimeDepthModelAssetId}
          <div class="crs-meta">
            <span>Project active model</span>
            <strong>{viewerModel.selectedProjectWellboreInventoryItem.activeWellTimeDepthModelAssetId}</strong>
          </div>
        {/if}

        <div class="crs-meta">
          <span>Well Time-Depth Assets</span>
          <strong>
            Obs {viewerModel.projectWellTimeDepthObservationSets.length} | Authored {viewerModel.projectWellTimeDepthAuthoredModels.length} | Compiled {viewerModel.projectWellTimeDepthModels.length}
          </strong>
        </div>

        <div class="velocity-model-actions velocity-model-actions-wrap">
          <button
            type="button"
            class="secondary"
            disabled={viewerModel.loading || viewerModel.projectWellTimeDepthModelsLoading}
            onclick={() =>
              void handleImportProjectWellTimeDepthAsset(
                "checkshot_vsp_observation_set",
                "Import Checkshot/VSP Observation Set"
              )}
          >
            Import Checkshot
          </button>
          <button
            type="button"
            class="secondary"
            disabled={viewerModel.loading || viewerModel.projectWellTimeDepthModelsLoading}
            onclick={() =>
              void handleImportProjectWellTimeDepthAsset(
                "manual_time_depth_pick_set",
                "Import Manual Time-Depth Picks"
              )}
          >
            Import Picks
          </button>
          <button
            type="button"
            class="secondary"
            disabled={viewerModel.loading || viewerModel.projectWellTimeDepthModelsLoading}
            onclick={() =>
              void handleImportProjectWellTimeDepthAsset(
                "well_time_depth_authored_model",
                "Import Well Time-Depth Authored Model"
              )}
          >
            Import Authored
          </button>
          <button
            type="button"
            class="secondary"
            disabled={viewerModel.loading || viewerModel.projectWellTimeDepthModelsLoading}
            onclick={() =>
              void handleImportProjectWellTimeDepthAsset(
                "well_time_depth_model",
                "Import Compiled Well Time-Depth Model"
              )}
          >
            Import Compiled
          </button>
        </div>
      {/if}

      {#if viewerModel.projectWellTimeDepthModelsError}
        <p class="velocity-model-error">{viewerModel.projectWellTimeDepthModelsError}</p>
      {:else if viewerModel.projectWellTimeDepthModelsLoading}
        <p class="velocity-model-empty">Loading well models...</p>
      {/if}

      {#if !viewerModel.projectWellTimeDepthModelsError && !viewerModel.projectWellTimeDepthModelsLoading && viewerModel.projectWellTimeDepthObservationSets.length}
        <div class="crs-meta">
          <span>Observation Sets</span>
          <strong>{viewerModel.projectWellTimeDepthObservationSets.length}</strong>
        </div>
        <div class="velocity-model-list">
          {#each viewerModel.projectWellTimeDepthObservationSets as sourceAsset (sourceAsset.assetId)}
            <div class="velocity-model-entry velocity-model-static">
              <span class="velocity-model-name">{sourceAsset.name}</span>
              <small>
                {wellTimeDepthAssetKindLabel(sourceAsset.assetKind)} | {sourceAsset.sampleCount} samples
              </small>
            </div>
          {/each}
        </div>
      {/if}

      {#if !viewerModel.projectWellTimeDepthModelsError && !viewerModel.projectWellTimeDepthModelsLoading && viewerModel.projectWellTimeDepthAuthoredModels.length}
        <div class="crs-meta">
          <span>Authored Models</span>
          <strong>{viewerModel.projectWellTimeDepthAuthoredModels.length}</strong>
        </div>
        <div class="velocity-model-list">
          {#each viewerModel.projectWellTimeDepthAuthoredModels as model (model.assetId)}
            <div class="well-time-depth-row">
              <div class="velocity-model-entry velocity-model-static">
                <span class="velocity-model-name">{model.name}</span>
                <small>
                  {model.sourceBindingCount} sources | {model.assumptionIntervalCount} assumptions
                </small>
              </div>
              <button
                type="button"
                class="secondary well-time-depth-action"
                disabled={viewerModel.loading || viewerModel.projectWellTimeDepthModelsLoading}
                onclick={() => void handleCompileProjectWellTimeDepthAuthoredModel(model.assetId)}
              >
                Compile + Activate
              </button>
            </div>
          {/each}
        </div>
      {/if}

      {#if !viewerModel.projectWellTimeDepthModelsError && !viewerModel.projectWellTimeDepthModelsLoading && viewerModel.projectWellTimeDepthModels.length}
        <div class="crs-meta">
          <span>Compiled Models</span>
          <strong>{viewerModel.projectWellTimeDepthModels.length}</strong>
        </div>
      {/if}

      {#if !viewerModel.projectWellTimeDepthModelsError && !viewerModel.projectWellTimeDepthModelsLoading}
        {#if viewerModel.projectWellTimeDepthModels.length}
          <div class="velocity-model-list">
            {#each viewerModel.projectWellTimeDepthModels as model (model.assetId)}
              <button
                class:active={viewerModel.selectedProjectWellTimeDepthModelAssetId === model.assetId}
                class="velocity-model-entry"
                type="button"
                onclick={() => viewerModel.setSelectedProjectWellTimeDepthModelAssetId(model.assetId)}
                title={model.name}
              >
                <span class="velocity-model-name">{model.name}</span>
                <small>
                  {velocitySourceKindLabel(model.sourceKind)} | {model.sampleCount} samples{model.isActiveProjectModel ? " | project active" : ""}
                </small>
              </button>
            {/each}
          </div>
        {:else}
          <p class="velocity-model-empty">
            Select a project wellbore to inspect compiled well time-depth models.
          </p>
        {/if}
      {/if}

      {#if viewerModel.projectSectionWellOverlaysError}
        <p class="velocity-model-error">{viewerModel.projectSectionWellOverlaysError}</p>
      {:else if viewerModel.projectSectionWellOverlays}
        <div class="crs-meta">
          <span>Resolved overlays</span>
          <strong>{viewerModel.projectSectionWellOverlays.overlays.length}</strong>
        </div>
      {/if}

      {#if viewerModel.selectedProjectWellTimeDepthModel}
        <div class="crs-meta">
          <span>Active well model</span>
          <strong>{viewerModel.selectedProjectWellTimeDepthModel.name}</strong>
        </div>
      {/if}

      <div class="crs-actions">
        <button
          type="button"
          class="secondary"
          disabled={!viewerModel.sectionWellOverlays.length && !viewerModel.projectSectionWellOverlays}
          onclick={() => viewerModel.clearProjectSectionWellOverlays()}
        >
          Clear
        </button>
      </div>
    </section>
  </div>

  <section class="sidebar-map-panel">
    <div class="sidebar-map-header">
      <span>Survey Map</span>
      <small>{sidebarSurveyMap?.name ?? "No active survey"}</small>
    </div>

    <div class="crs-panel">
      <label class="crs-field">
        <span>Display CRS</span>
        <input
          bind:value={viewerModel.displayCoordinateReferenceId}
          type="text"
          placeholder="EPSG:23031"
          onblur={() => viewerModel.setDisplayCoordinateReferenceId(viewerModel.displayCoordinateReferenceId)}
          onkeydown={(event) => {
            if (event.key === "Enter") {
              viewerModel.setDisplayCoordinateReferenceId(viewerModel.displayCoordinateReferenceId);
            }
          }}
        />
      </label>

      <div class="crs-meta">
        <span>Detected native CRS</span>
        <strong>{viewerModel.activeDetectedNativeCoordinateReferenceId ?? viewerModel.activeDetectedNativeCoordinateReferenceName ?? "Unknown"}</strong>
      </div>

      <div class="crs-meta">
        <span>Effective native CRS</span>
        <strong>{viewerModel.activeEffectiveNativeCoordinateReferenceId ?? viewerModel.activeEffectiveNativeCoordinateReferenceName ?? "Unknown"}</strong>
      </div>

      <div class="crs-meta">
        <span>Map transform</span>
        <strong>{transformStatusLabel(activeSurveyMapSurvey?.transform_status)}</strong>
      </div>

      <div class="crs-meta">
        <span>Imported horizons</span>
        <strong>{viewerModel.surveyMapSource?.horizons.length ?? 0}</strong>
      </div>

      {#if activeSurveyMapPreviewHorizon}
        <div class="crs-meta">
          <span>Preview horizon</span>
          <strong>{activeSurveyMapPreviewHorizon.name}</strong>
        </div>
      {/if}

      <label class="crs-field">
        <span>Override native CRS</span>
        <input
          bind:value={viewerModel.nativeCoordinateReferenceOverrideIdDraft}
          type="text"
          placeholder="EPSG:23031"
          disabled={!viewerModel.comparePrimaryStorePath || viewerModel.loading}
        />
      </label>

      <label class="crs-field">
        <span>Override label</span>
        <input
          bind:value={viewerModel.nativeCoordinateReferenceOverrideNameDraft}
          type="text"
          placeholder="ED50 / UTM zone 31N"
          disabled={!viewerModel.comparePrimaryStorePath || viewerModel.loading}
        />
      </label>

      <div class="crs-actions">
        <button
          type="button"
          disabled={
            !viewerModel.comparePrimaryStorePath ||
            viewerModel.loading ||
            !viewerModel.nativeCoordinateReferenceOverrideIdDraft.trim()
          }
          onclick={() =>
            void viewerModel.setActiveDatasetNativeCoordinateReference(
              viewerModel.nativeCoordinateReferenceOverrideIdDraft,
              viewerModel.nativeCoordinateReferenceOverrideNameDraft
            )}
        >
          Apply CRS
        </button>
        <button
          type="button"
          class="secondary"
          disabled={!viewerModel.comparePrimaryStorePath || viewerModel.loading}
          onclick={() => void viewerModel.setActiveDatasetNativeCoordinateReference(null, null)}
        >
          Clear Override
        </button>
      </div>

      {#if viewerModel.workspaceCoordinateReferenceWarnings.length}
        <div class="crs-warnings">
          {#each viewerModel.workspaceCoordinateReferenceWarnings as warning (warning)}
            <p>{warning}</p>
          {/each}
        </div>
      {/if}
    </div>

    <div class="sidebar-map-frame">
      <SurveyMapChart
        chartId="traceboost-sidebar-map"
        map={sidebarSurveyMap}
        stageScale={0.42}
        emptyMessage={viewerModel.surveyMapLoading ? "Resolving survey map..." : "Open a volume to preview its survey footprint."}
        onProbeChange={(event) => {
          mapProbeSourceId = sidebarSurveyMap?.id ?? null;
          mapProbe = event.probe;
        }}
      />
    </div>

    <div class="sidebar-map-readout">
      {#if mapProbe && mapProbeSourceId === sidebarSurveyMap?.id}
        x {mapProbe.x.toFixed(0)} y {mapProbe.y.toFixed(0)}
        {#if mapProbe.scalarValue !== undefined}
          | {mapProbe.scalarName ?? activeSurveyMapPreviewHorizon?.name ?? "Horizon"} {mapProbe.scalarValue.toFixed(1)}
        {/if}
      {:else if viewerModel.surveyMapLoading}
        Resolving active survey map geometry.
      {:else if activeSurveyMapPreviewHorizon}
        Previewing horizon surface for {activeSurveyMapPreviewHorizon.name}.
      {:else if viewerModel.surveyMapSource?.horizons.length}
        {viewerModel.surveyMapSource.horizons.length} imported horizons are available, but none can be previewed on the current map grid.
      {:else if sidebarSurveyMap}
        Move over the map to inspect coordinates.
      {:else}
        Survey footprint preview will appear here.
      {/if}
    </div>
  </section>
</aside>

{#if volumeContextMenu}
  <div
    class="volume-context-menu"
    style={`left:${volumeContextMenu.x}px; top:${volumeContextMenu.y}px;`}
    role="menu"
    tabindex="0"
    onclick={(event) => event.stopPropagation()}
    onkeydown={(event) => {
      event.stopPropagation();
      if (event.key === "Escape") {
        closeVolumeContextMenu();
      }
    }}
  >
    <button
      class="volume-context-item"
      type="button"
      role="menuitem"
      disabled={!volumeContextMenu.exportable}
      onclick={() => void handleContextMenuExport()}
      title={
        volumeContextMenu.exportable
          ? `Export ${volumeContextMenu.label}`
          : "Export is unavailable because this entry has no runtime store path."
      }
    >
      Export...
    </button>
  </div>
{/if}

<style>
  .sidebar {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    background: #181818;
    border-right: 1px solid #242424;
  }

  .sidebar.hidden {
    display: none;
  }

  .sidebar-header {
    padding: 10px 10px 8px;
    border-bottom: 1px solid #242424;
  }

  .logo-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
  }

  .logo-icon {
    color: #67c48f;
  }

  .logo-copy h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 650;
    color: #d7d7d7;
  }

  .version {
    font-size: 11px;
    color: #6c6c6c;
    font-weight: 500;
  }

  .subtitle {
    margin: 2px 0 0;
    font-size: 11px;
    color: #6f6f6f;
  }

  .collapse-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 2px;
    border: 1px solid #303030;
    background: #202020;
    color: #777;
    cursor: pointer;
  }

  .collapse-button:hover {
    background: #282828;
    color: #d0d0d0;
  }

  .volume-list-shell {
    min-height: 0;
    overflow: auto;
    padding: 10px;
    display: grid;
    gap: 12px;
  }

  .volume-list {
    display: grid;
    gap: 6px;
    outline: none;
  }

  .empty-state {
    border: 1px dashed #2d2d2d;
    background: #1c1c1c;
    padding: 14px;
    color: #828282;
  }

  .empty-title {
    display: block;
    margin-bottom: 6px;
    font-size: 12px;
    font-weight: 650;
    color: #c4c4c4;
  }

  .empty-state p {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
  }

  .volume-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
  }

  .volume-entry {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid #2b2b2b;
    background: #1d1d1d;
    color: #a9a9a9;
    text-align: left;
    cursor: pointer;
  }

  .volume-entry:hover:not(:disabled) {
    border-color: #3b3b3b;
    background: #242424;
    color: #dddddd;
  }

  .volume-entry.active {
    border-color: rgba(103, 196, 143, 0.45);
    background: rgba(33, 60, 44, 0.72);
    color: #f2fff7;
  }

  .volume-entry:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .volume-entry-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-weight: 600;
  }

  .velocity-models-panel {
    display: grid;
    gap: 8px;
    padding-top: 10px;
    border-top: 1px solid #242424;
  }

  .velocity-models-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .velocity-models-header span {
    display: block;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #8f8f8f;
  }

  .velocity-models-header small {
    display: block;
    margin-top: 4px;
    font-size: 11px;
    color: #a9a9a9;
  }

  .velocity-model-actions {
    display: flex;
    gap: 6px;
  }

  .velocity-model-actions-wrap {
    flex-wrap: wrap;
  }

  .velocity-model-actions button {
    padding: 6px 8px;
    border: 1px solid #2f2f2f;
    background: #202020;
    color: #d8d8d8;
    font-size: 11px;
    cursor: pointer;
  }

  .velocity-model-actions button.secondary {
    background: #191919;
    color: #b8b8b8;
  }

  .velocity-model-actions button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .velocity-model-list {
    display: grid;
    gap: 6px;
  }

  .velocity-model-entry {
    display: grid;
    gap: 3px;
    width: 100%;
    padding: 9px 10px;
    border: 1px solid #2b2b2b;
    background: #1d1d1d;
    color: #c8c8c8;
    text-align: left;
    cursor: pointer;
  }

  .velocity-model-entry:hover:not(:disabled) {
    border-color: #3b3b3b;
    background: #242424;
  }

  .velocity-model-entry.active {
    border-color: rgba(103, 196, 143, 0.45);
    background: rgba(33, 60, 44, 0.72);
    color: #f2fff7;
  }

  .velocity-model-entry:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .velocity-model-static {
    cursor: default;
  }

  .velocity-model-name {
    font-size: 12px;
    font-weight: 600;
  }

  .velocity-model-entry small {
    font-size: 10px;
    color: #8f8f8f;
  }

  .velocity-model-entry.active small {
    color: #d8f0df;
  }

  .well-time-depth-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
    align-items: stretch;
  }

  .well-time-depth-action {
    padding: 8px 10px;
    border: 1px solid #2f4537;
    background: #1d2c23;
    color: #dff7e6;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .well-time-depth-action:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .velocity-model-empty,
  .velocity-model-error {
    margin: 0;
    font-size: 11px;
    line-height: 1.45;
    color: #8f8f8f;
  }

  .velocity-model-error {
    color: #d29c9c;
  }

  .sidebar-map-panel {
    display: grid;
    gap: 8px;
    padding: 10px;
    border-top: 1px solid #242424;
    background: #151515;
  }

  .crs-panel {
    display: grid;
    gap: 8px;
    padding: 10px;
    border: 1px solid #262626;
    background: rgba(12, 12, 12, 0.72);
  }

  .crs-field {
    display: grid;
    gap: 4px;
  }

  .crs-field span,
  .crs-meta span {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #6a6a6a;
  }

  .crs-field input {
    min-width: 0;
    padding: 7px 8px;
    border: 1px solid #2f2f2f;
    background: #171717;
    color: #d7d7d7;
    font-size: 12px;
  }

  .crs-meta {
    display: grid;
    gap: 2px;
  }

  .crs-meta strong {
    font-size: 12px;
    font-weight: 600;
    color: #dadada;
  }

  .crs-actions {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .crs-actions button {
    padding: 8px 10px;
    border: 1px solid #2f4537;
    background: #1d2c23;
    color: #dff7e6;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
  }

  .crs-actions button.secondary {
    border-color: #303030;
    background: #1a1a1a;
    color: #bdbdbd;
  }

  .crs-actions button:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .crs-warnings {
    display: grid;
    gap: 6px;
    padding: 8px;
    border: 1px solid rgba(199, 141, 58, 0.28);
    background: rgba(86, 56, 22, 0.2);
  }

  .crs-warnings p {
    margin: 0;
    font-size: 11px;
    line-height: 1.4;
    color: #e6c48a;
  }

  .sidebar-map-header {
    display: grid;
    gap: 2px;
  }

  .sidebar-map-header span {
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #5e5e5e;
  }

  .sidebar-map-header small {
    color: #929292;
    font-size: 11px;
  }

  .sidebar-map-frame {
    height: 210px;
    border: 1px solid #272727;
    background: #101010;
    overflow: hidden;
  }

  .sidebar-map-frame :global(.geoviz-survey-map-shell) {
    width: 100%;
    height: 100%;
    min-height: 0;
  }

  .sidebar-map-readout {
    min-height: 14px;
    font-size: 11px;
    color: #787878;
  }

  .volume-context-menu {
    position: fixed;
    z-index: 40;
    min-width: 192px;
    padding: 6px;
    border: 1px solid rgba(120, 148, 167, 0.24);
    background: rgba(13, 18, 22, 0.98);
    box-shadow: 0 18px 42px rgba(0, 0, 0, 0.34);
    backdrop-filter: blur(10px);
  }

  .volume-context-item {
    width: 100%;
    min-height: 34px;
    padding: 8px 10px;
    border: none;
    background: transparent;
    color: #dde6eb;
    text-align: left;
    cursor: pointer;
  }

  .volume-context-item:hover:not(:disabled) {
    background: rgba(31, 76, 107, 0.34);
    color: #ffffff;
  }

  .volume-context-item:disabled {
    opacity: 0.48;
    cursor: not-allowed;
  }
</style>
