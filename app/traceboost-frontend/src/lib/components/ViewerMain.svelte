<svelte:options runes={true} />

<script lang="ts">
  import type { ChartToolbarActionItem, ChartToolbarToolItem } from "@geoviz/svelte-toolbar";
  import { ChartInteractionToolbar } from "@geoviz/svelte-toolbar";
  import { SeismicSectionChart } from "@geoviz/svelte";
  import type { SegyGeometryCandidate, SegyHeaderField } from "@traceboost/seis-contracts";
  import PipelineControlBar from "./PipelineControlBar.svelte";
  import PipelineOperatorEditor from "./PipelineOperatorEditor.svelte";
  import PipelineSequenceList from "./PipelineSequenceList.svelte";
  import PipelineSessionList from "./PipelineSessionList.svelte";
  import SpectrumInspector from "./SpectrumInspector.svelte";
  import { pickHorizonFiles } from "../file-dialog";
  import { getProcessingModelContext } from "../processing-model.svelte";
  import { getViewerModelContext } from "../viewer-model.svelte";

  let {
    showSidebar,
    showSidebarPanel,
    chartRef = $bindable<{ fitToData?: () => void } | null>(null)
  }: {
    showSidebar: boolean;
    showSidebarPanel: () => void;
    chartRef?: { fitToData?: () => void } | null;
  } = $props();

  const viewerModel = getViewerModelContext();
  const processingModel = getProcessingModelContext();
  let displaySettingsOpen = $state(false);
  let draftGain = $state(1);
  let draftClipMode = $state<"auto" | "manual">("auto");
  let draftClipMin = $state("");
  let draftClipMax = $state("");
  let draftColormap = $state<"grayscale" | "red-white-blue">("grayscale");
  let draftPolarity = $state<"normal" | "reversed">("normal");
  let sectionIndexDraft = $state<number | undefined>(undefined);

  const compareViewport = $derived(viewerModel.lastViewport?.viewport ?? null);
  const geometryRecovery = $derived(viewerModel.importGeometryRecovery);
  const splitReady = $derived(
    viewerModel.compareSplitEnabled &&
      !!processingModel.displaySection &&
      !!viewerModel.backgroundSection &&
      viewerModel.displayTransform.renderMode === "heatmap"
  );
  const sectionAxisLimit = $derived(
    viewerModel.dataset
      ? viewerModel.axis === "inline"
        ? Math.max(0, viewerModel.dataset.descriptor.shape[0] - 1)
        : Math.max(0, viewerModel.dataset.descriptor.shape[1] - 1)
      : 0
  );
  const toolbarTools = $derived<ChartToolbarToolItem[]>([
    {
      id: "pointer",
      label: "Pointer",
      icon: "pointer",
      active: viewerModel.chartTool === "pointer",
      disabled: !processingModel.displaySection
    },
    {
      id: "crosshair",
      label: "Crosshair",
      icon: "crosshair",
      active: viewerModel.chartTool === "crosshair",
      disabled: !processingModel.displaySection
    },
    {
      id: "pan",
      label: "Pan",
      icon: "pan",
      active: viewerModel.chartTool === "pan",
      disabled: !processingModel.displaySection
    }
  ]);
  const toolbarActions = $derived<ChartToolbarActionItem[]>([
    {
      id: "fitToData",
      label: "Fit To Data",
      icon: "fitToData",
      disabled: !processingModel.displaySection
    }
  ]);

  function describeSegyHeaderField(field: SegyHeaderField | null | undefined): string {
    if (!field) {
      return "unset";
    }
    return `${field.start_byte} (${field.value_type.toUpperCase()})`;
  }

  function describeSegyGeometryCandidate(candidate: SegyGeometryCandidate): string {
    return `inline ${describeSegyHeaderField(candidate.geometry.inline_3d)}, crossline ${describeSegyHeaderField(candidate.geometry.crossline_3d)}`;
  }

  function handleToolbarToolSelect(toolId: string): void {
    if (toolId === "pointer" || toolId === "crosshair" || toolId === "pan") {
      viewerModel.setChartTool(toolId);
    }
  }

  function handleToolbarActionSelect(actionId: string): void {
    if (actionId === "fitToData") {
      chartRef?.fitToData?.();
    }
  }

  function handleAxisChange(nextAxis: "inline" | "xline"): void {
    if (!viewerModel.activeStorePath || viewerModel.loading) {
      return;
    }

    const clampedIndex = Math.min(
      viewerModel.index,
      nextAxis === "inline"
        ? Math.max(0, (viewerModel.dataset?.descriptor.shape[0] ?? 1) - 1)
        : Math.max(0, (viewerModel.dataset?.descriptor.shape[1] ?? 1) - 1)
    );
    void viewerModel.load(nextAxis, clampedIndex);
  }

  function commitSectionIndex(): void {
    const sectionIndexInput = sectionIndexDraft ?? viewerModel.index;
    if (!viewerModel.activeStorePath || viewerModel.loading) {
      sectionIndexDraft = undefined;
      return;
    }

    if (!Number.isFinite(sectionIndexInput)) {
      sectionIndexDraft = undefined;
      return;
    }

    const clamped = Math.min(Math.max(Math.round(sectionIndexInput), 0), sectionAxisLimit);
    sectionIndexDraft = undefined;
    if (clamped !== viewerModel.index) {
      void viewerModel.load(viewerModel.axis, clamped);
    }
  }

  function toggleRenderMode(nextMode: "heatmap" | "wiggle"): void {
    viewerModel.setRenderMode(nextMode);
    if (viewerModel.compareSplitEnabled && nextMode !== "heatmap") {
      viewerModel.setCompareSplitEnabled(false);
    }
  }

  function toggleColormap(): void {
    viewerModel.setColormap(
      viewerModel.displayTransform.colormap === "grayscale" ? "red-white-blue" : "grayscale"
    );
  }

  async function handleImportHorizons(): Promise<void> {
    const inputPaths = await pickHorizonFiles();
    if (inputPaths.length === 0) {
      viewerModel.note("Horizon import selection did not produce usable files.", "ui", "warn");
      return;
    }
    await viewerModel.importHorizonFiles(inputPaths);
  }

  function openDisplaySettings(): void {
    draftGain = viewerModel.displayTransform.gain;
    draftClipMode =
      typeof viewerModel.displayTransform.clipMin === "number" ||
      typeof viewerModel.displayTransform.clipMax === "number"
        ? "manual"
        : "auto";
    draftClipMin =
      typeof viewerModel.displayTransform.clipMin === "number"
        ? String(viewerModel.displayTransform.clipMin)
        : "";
    draftClipMax =
      typeof viewerModel.displayTransform.clipMax === "number"
        ? String(viewerModel.displayTransform.clipMax)
        : "";
    draftColormap = viewerModel.displayTransform.colormap;
    draftPolarity = viewerModel.displayTransform.polarity;
    displaySettingsOpen = true;
  }

  function closeDisplaySettings(): void {
    displaySettingsOpen = false;
  }

  function applyDisplaySettings(): void {
    const gain = Number(draftGain);
    if (Number.isFinite(gain) && gain > 0) {
      viewerModel.setGain(gain);
    }

    viewerModel.setColormap(draftColormap);
    viewerModel.setPolarity(draftPolarity);

    if (draftClipMode === "manual") {
      const clipMin = draftClipMin.trim() === "" ? undefined : Number(draftClipMin);
      const clipMax = draftClipMax.trim() === "" ? undefined : Number(draftClipMax);
      viewerModel.setClipRange(
        clipMin !== undefined && Number.isFinite(clipMin) ? clipMin : undefined,
        clipMax !== undefined && Number.isFinite(clipMax) ? clipMax : undefined
      );
    } else {
      viewerModel.setClipRange(undefined, undefined);
    }

    displaySettingsOpen = false;
  }

  function handleWindowKeyDown(event: KeyboardEvent): void {
    if (displaySettingsOpen && event.key === "Escape") {
      closeDisplaySettings();
      return;
    }

    if (processingModel.spectrumInspectorOpen && event.key === "Escape") {
      processingModel.closeSpectrumInspector();
      return;
    }

    if (displaySettingsOpen) {
      return;
    }

    void processingModel.handleKeydown(event);
  }
</script>

<svelte:window onkeydown={handleWindowKeyDown} />

{#snippet chartDisplayOverlay()}
  <div class="chart-display-overlay">
    <div class="display-chip-row">
      <label class="display-chip field">
        <span>{viewerModel.axis === "inline" ? "Inline" : "Xline"}</span>
        <select
          value={viewerModel.axis}
          disabled={!viewerModel.activeStorePath || viewerModel.loading}
          onchange={(event) => handleAxisChange((event.currentTarget as HTMLSelectElement).value as "inline" | "xline")}
        >
          <option value="inline">Inline</option>
          <option value="xline">Xline</option>
        </select>
      </label>

      <label class="display-chip field">
        <span>Index</span>
        <input
          bind:value={
            () => sectionIndexDraft ?? viewerModel.index,
            (value) => {
              sectionIndexDraft = value;
            }
          }
          type="number"
          min="0"
          max={sectionAxisLimit}
          disabled={!viewerModel.activeStorePath || viewerModel.loading}
          onblur={commitSectionIndex}
          onkeydown={(event) => {
            if (event.key === "Enter") {
              commitSectionIndex();
            }
          }}
        />
      </label>
    </div>

    <div class="display-chip-row">
      <button
        class:active={viewerModel.sectionHorizons.length > 0}
        class="display-chip action"
        onclick={() => void handleImportHorizons()}
        disabled={!viewerModel.activeStorePath || viewerModel.horizonImporting}
      >
        {viewerModel.horizonImporting ? "Importing…" : `Horizons ${viewerModel.sectionHorizons.length}`}
      </button>
      <button
        class="display-chip action"
        onclick={() => void viewerModel.exportActiveDatasetSegy()}
        disabled={!viewerModel.canExportSegy}
      >
        {viewerModel.segyExporting ? "Exporting…" : "Export SEG-Y"}
      </button>
      <button
        class:active={viewerModel.displayTransform.renderMode === "heatmap"}
        class="display-chip action"
        onclick={() => toggleRenderMode("heatmap")}
        disabled={!processingModel.displaySection}
      >
        Heatmap
      </button>
      <button
        class:active={viewerModel.displayTransform.renderMode === "wiggle"}
        class="display-chip action"
        onclick={() => toggleRenderMode("wiggle")}
        disabled={!processingModel.displaySection}
      >
        Wiggle
      </button>
      <button
        class="display-chip action"
        onclick={toggleColormap}
        disabled={!processingModel.displaySection}
      >
        {viewerModel.displayTransform.colormap === "grayscale" ? "R/W/B" : "Gray"}
      </button>
      <button
        class:active={processingModel.spectrumInspectorOpen}
        class="display-chip icon"
        onclick={processingModel.openSpectrumInspector}
        aria-label="Open frequency spectrum inspector"
        disabled={!processingModel.canInspectSpectrum}
      >
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8">
          <path d="M4 18.5V14" />
          <path d="M9 18.5V8" />
          <path d="M14 18.5V11" />
          <path d="M19 18.5V5" />
          <path d="M3 18.5h18" />
        </svg>
      </button>
      <button
        class="display-chip icon"
        onclick={openDisplaySettings}
        aria-label="Open display settings"
        disabled={!processingModel.displaySection}
      >
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8">
          <path d="M10.3 2.5h3.4l.5 2.2a7.9 7.9 0 012 .8l1.9-1.2 2.4 2.4-1.2 1.9c.35.63.61 1.3.78 2l2.23.52v3.4l-2.23.52a7.9 7.9 0 01-.78 2l1.2 1.9-2.4 2.4-1.9-1.2a7.9 7.9 0 01-2 .78l-.52 2.23h-3.4l-.52-2.23a7.9 7.9 0 01-2-.78l-1.9 1.2-2.4-2.4 1.2-1.9a7.9 7.9 0 01-.78-2L2.5 13.7v-3.4l2.23-.52a7.9 7.9 0 01.78-2L4.26 5.9l2.4-2.4 1.9 1.2a7.9 7.9 0 012-.78z" />
          <circle cx="12" cy="12" r="3.1" />
        </svg>
      </button>
    </div>
  </div>
{/snippet}

{#snippet chartToolbarOverlay()}
  <div class="chart-toolbar-overlay">
    <ChartInteractionToolbar
      variant="overlay"
      iconOnly={true}
      tools={toolbarTools}
      actions={toolbarActions}
      onToolSelect={handleToolbarToolSelect}
      onActionSelect={handleToolbarActionSelect}
    />
  </div>
{/snippet}

{#snippet compareCycleOverlay()}
  <div class="compare-cycle-overlay">
    <button
      class="compare-arrow"
      onclick={() => void viewerModel.cycleForegroundCompareSurvey(-1)}
      aria-label="Show previous compatible survey"
      disabled={viewerModel.loading}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 5v14" />
        <path d="M7 10l5-5 5 5" />
      </svg>
    </button>
    <div class="compare-cycle-copy">
      <small>
        {viewerModel.compatibleCompareCandidates.findIndex(
          (candidate) => candidate.storePath === viewerModel.comparePrimaryStorePath
        ) + 1}
        / {viewerModel.compatibleCompareCandidates.length}
      </small>
    </div>
    <button
      class="compare-arrow"
      onclick={() => void viewerModel.cycleForegroundCompareSurvey(1)}
      aria-label="Show next compatible survey"
      disabled={viewerModel.loading}
    >
      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M12 19V5" />
        <path d="M7 14l5 5 5-5" />
      </svg>
    </button>
  </div>
{/snippet}

{#snippet compareLabelOverlay()}
  <div class="compare-label-overlay">
    <div class="compare-label-line">
      <strong>{viewerModel.activeForegroundCompareCandidate?.displayName ?? viewerModel.activeDatasetDisplayName}</strong>
    </div>

    {#if viewerModel.activeBackgroundCompareCandidate}
      <div class="compare-label-line secondary">
        <strong>{viewerModel.activeBackgroundCompareCandidate.displayName}</strong>
      </div>
    {/if}
  </div>
{/snippet}

{#if !showSidebar}
  <button class="sidebar-toggle" onclick={showSidebarPanel} aria-label="Show sidebar">
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
      <polyline points="9 18 15 12 9 6" />
    </svg>
  </button>
{/if}

<main class="viewer-shell">
  <div class="workspace-columns">
    <aside class="session-column">
      <div class="session-column-header">
        <span class="eyebrow">Processing Workspace</span>
        <h2>{processingModel.pipelineTitle}</h2>
        <p>
          {viewerModel.dataset
            ? `Working on ${viewerModel.activeDatasetDisplayName} at ${viewerModel.axis}:${viewerModel.index}`
            : "Open a runtime store to preview processing on the current section."}
        </p>
      </div>

        <PipelineSessionList
          pipelines={processingModel.sessionPipelineItems}
          activePipelineId={processingModel.activeSessionPipelineId}
          onSelect={processingModel.activateSessionPipeline}
          onCreate={processingModel.createSessionPipeline}
          onDuplicate={processingModel.duplicateActiveSessionPipeline}
          onCopy={processingModel.copyActiveSessionPipeline}
          onPaste={processingModel.pasteCopiedSessionPipeline}
          onRemove={processingModel.removeActiveSessionPipeline}
          onRemoveItem={processingModel.removeSessionPipeline}
          getLabel={processingModel.sessionPipelineLabel}
          canRemove={processingModel.canRemoveSessionPipeline}
        />
    </aside>

    <div class="main-column">
      <div class="definition-pane">
        <PipelineControlBar
          pipeline={processingModel.pipeline}
          previewState={processingModel.previewState}
          previewLabel={processingModel.previewLabel}
          presets={processingModel.presets}
          loadingPresets={processingModel.loadingPresets}
          canPreview={processingModel.canPreview}
          canRun={processingModel.canRun}
          previewBusy={processingModel.previewBusy}
          runBusy={processingModel.runBusy}
          runOutputSettingsOpen={processingModel.runOutputSettingsOpen}
          runOutputPathMode={processingModel.runOutputPathMode}
          runOutputPath={processingModel.resolvedRunOutputPath}
          resolvingRunOutputPath={processingModel.resolvingRunOutputPath}
          overwriteExistingRunOutput={processingModel.overwriteExistingRunOutput}
          onSetPipelineName={processingModel.setPipelineName}
          onPreview={() => processingModel.previewCurrentSection()}
          onShowRaw={processingModel.showRawSection}
          onRun={() => processingModel.runOnVolume()}
          onToggleRunOutputSettings={() =>
            processingModel.setRunOutputSettingsOpen(!processingModel.runOutputSettingsOpen)}
          onSetRunOutputPathMode={processingModel.setRunOutputPathMode}
          onSetCustomRunOutputPath={processingModel.setCustomRunOutputPath}
          onBrowseRunOutputPath={() => processingModel.browseRunOutputPath()}
          onResetRunOutputPath={processingModel.resetRunOutputPath}
          onSetOverwriteExistingRunOutput={processingModel.setOverwriteExistingRunOutput}
          onLoadPreset={processingModel.loadPreset}
          onSavePreset={() => processingModel.savePreset()}
          onDeletePreset={(presetId) => processingModel.deletePreset(presetId)}
        />

        <div class="definition-grid">
          <PipelineSequenceList
            operations={processingModel.workspaceOperations}
            traceLocalOperationCount={processingModel.pipeline.steps.length}
            hasSubvolumeCrop={processingModel.hasSubvolumeCrop}
            selectedIndex={processingModel.selectedStepIndex}
            checkpointAfterOperationIndexes={processingModel.checkpointAfterOperationIndexes}
            checkpointWarning={processingModel.checkpointWarning}
            onSelect={processingModel.selectStep}
            onInsertOperator={processingModel.insertOperatorById}
            onCopy={processingModel.copySelectedOperation}
            onPaste={processingModel.pasteCopiedOperation}
            onRemove={processingModel.removeOperationAt}
            onToggleCheckpoint={processingModel.toggleCheckpointAfterOperation}
          />

          <PipelineOperatorEditor
            selectedOperation={processingModel.selectedOperation}
            activeJob={processingModel.activeJob}
            processingError={processingModel.error}
            primaryVolumeLabel={processingModel.activePrimaryVolumeLabel}
            sourceSubvolumeBounds={processingModel.sourceSubvolumeBounds}
            secondaryVolumeOptions={processingModel.volumeArithmeticSecondaryOptions}
            selectedStepCanCheckpoint={processingModel.canToggleSelectedCheckpoint}
            selectedStepCheckpoint={processingModel.selectedStepCheckpoint}
            onSetAmplitudeScalarFactor={processingModel.setSelectedAmplitudeScalarFactor}
            onSetAgcWindow={processingModel.setSelectedAgcWindow}
            onSetPhaseRotationAngle={processingModel.setSelectedPhaseRotationAngle}
            onSetLowpassCorner={processingModel.setSelectedLowpassCorner}
            onSetHighpassCorner={processingModel.setSelectedHighpassCorner}
            onSetBandpassCorner={processingModel.setSelectedBandpassCorner}
            onSetVolumeArithmeticOperator={processingModel.setSelectedVolumeArithmeticOperator}
            onSetVolumeArithmeticSecondaryStorePath={processingModel.setSelectedVolumeArithmeticSecondaryStorePath}
            onSetSubvolumeCropBound={processingModel.setSelectedSubvolumeCropBound}
            onSetSelectedCheckpoint={processingModel.setSelectedCheckpoint}
            canMoveUp={processingModel.canMoveSelectedUp}
            canMoveDown={processingModel.canMoveSelectedDown}
            onMoveUp={processingModel.moveSelectedUp}
            onMoveDown={processingModel.moveSelectedDown}
            onRemove={processingModel.removeSelected}
            onCancelJob={() => processingModel.cancelActiveJob()}
            onOpenArtifact={(storePath) => processingModel.openProcessingArtifact(storePath)}
          />
        </div>
      </div>

      <div class="viewer-pane">
      {#if processingModel.displaySection}
        <div class="chart-frame">
          <SeismicSectionChart
            bind:this={chartRef}
            chartId="traceboost-main"
            viewId={`${viewerModel.axis}:${viewerModel.index}:${processingModel.displaySectionMode}`}
            section={processingModel.displaySection}
            sectionHorizons={viewerModel.sectionHorizons}
            secondarySection={splitReady ? viewerModel.backgroundSection : null}
            compareMode={splitReady ? "split" : "single"}
            splitPosition={viewerModel.compareSplitPosition}
            viewport={compareViewport}
            displayTransform={viewerModel.displayTransform}
            interactions={{ tool: viewerModel.chartTool }}
            loading={viewerModel.loading || processingModel.previewBusy || (splitReady && viewerModel.backgroundLoading)}
            errorMessage={viewerModel.error ?? (splitReady ? viewerModel.backgroundError : null)}
            resetToken={processingModel.displayResetToken}
            onProbeChange={viewerModel.setProbe}
            onViewportChange={viewerModel.setViewport}
            onInteractionChange={viewerModel.setInteraction}
            onInteractionStateChange={viewerModel.setInteractionState}
            onSplitPositionChange={(ratio) => viewerModel.setCompareSplitPosition(ratio)}
            stageScale={2}
            stageTopLeft={chartDisplayOverlay}
            plotTopCenter={chartToolbarOverlay}
            plotTopRight={viewerModel.canCycleForegroundCompareSurvey ? compareCycleOverlay : undefined}
            plotBottomLeft={compareLabelOverlay}
          />

          {#if processingModel.spectrumInspectorOpen}
            <div class="spectrum-inspector-layer">
              <SpectrumInspector
                floating={true}
                canInspectSpectrum={processingModel.canInspectSpectrum}
                spectrumBusy={processingModel.spectrumBusy}
                spectrumStale={processingModel.spectrumStale}
                spectrumError={processingModel.spectrumError}
                spectrumSelectionSummary={processingModel.spectrumSelectionSummary}
                spectrumAmplitudeScale={processingModel.spectrumAmplitudeScale}
                rawSpectrum={processingModel.rawSpectrum}
                processedSpectrum={processingModel.processedSpectrum}
                onSetSpectrumAmplitudeScale={processingModel.setSpectrumAmplitudeScale}
                onRefreshSpectrum={() => processingModel.refreshSpectrum()}
                onClose={processingModel.closeSpectrumInspector}
              />
            </div>
          {/if}
        </div>
      {:else}
        <div class="welcome-card">
          <svg
            class="welcome-icon"
            viewBox="0 0 24 24"
            width="64"
            height="64"
            fill="none"
            stroke="currentColor"
            stroke-width="1"
          >
            <path
              d="M3 20 L6 8 L9 14 L12 4 L15 16 L18 10 L21 20"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
            <line x1="3" y1="20" x2="21" y2="20" />
          </svg>
          <h2>Open a Volume</h2>
          <p>
            Use <strong>File &gt; Open Volume…</strong> to open a `.tbvol` directly or import a
            `.segy`/`.sgy` into the runtime store automatically, then start viewing and processing.
          </p>
          <span class="welcome-version">TraceBoost v0.1.0</span>
        </div>
      {/if}
      </div>
    </div>
  </div>
</main>

{#if displaySettingsOpen}
  <div
    class="display-settings-backdrop"
    role="presentation"
    onclick={closeDisplaySettings}
  >
    <div
      class="display-settings-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Display settings"
      tabindex="0"
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => event.stopPropagation()}
    >
      <div class="display-settings-header">
        <h3>Display Settings</h3>
      </div>

      <div class="display-settings-grid">
        <label class="settings-field">
          <span>Gain</span>
          <input type="number" min="0.01" step="0.05" bind:value={draftGain} />
        </label>

        <label class="settings-field">
          <span>Color Scale</span>
          <select bind:value={draftColormap}>
            <option value="grayscale">Grayscale</option>
            <option value="red-white-blue">Red / White / Blue</option>
          </select>
        </label>

        <label class="settings-field">
          <span>Polarity</span>
          <select bind:value={draftPolarity}>
            <option value="normal">Normal</option>
            <option value="reversed">Reversed</option>
          </select>
        </label>

        <label class="settings-field">
          <span>Amplitude Range</span>
          <select bind:value={draftClipMode}>
            <option value="auto">Auto</option>
            <option value="manual">Manual</option>
          </select>
        </label>

        <label class="settings-field">
          <span>Minimum</span>
          <input type="number" step="0.01" bind:value={draftClipMin} disabled={draftClipMode !== "manual"} />
        </label>

        <label class="settings-field">
          <span>Maximum</span>
          <input type="number" step="0.01" bind:value={draftClipMax} disabled={draftClipMode !== "manual"} />
        </label>
      </div>

      <div class="display-settings-actions">
        <button class="settings-btn secondary" onclick={closeDisplaySettings}>Cancel</button>
        <button class="settings-btn primary" onclick={applyDisplaySettings}>Apply</button>
      </div>
    </div>
  </div>
{/if}

{#if geometryRecovery}
  <div class="import-geometry-backdrop" role="presentation" onclick={() => viewerModel.closeImportGeometryRecovery()}>
    <div
      class="import-geometry-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Review SEG-Y geometry mapping"
      tabindex="0"
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => event.stopPropagation()}
    >
      <div class="import-geometry-header">
        <h3>Review SEG-Y Geometry Mapping</h3>
        <p>
          TraceBoost could not import this SEG-Y with the default inline/crossline mapping. Review a suggested
          mapping or enter the header bytes manually, then continue the import.
        </p>
      </div>

      <div class="import-geometry-summary">
        <div class="import-geometry-summary-row">
          <span>Resolved default mapping</span>
          <strong>
            inline {describeSegyHeaderField(geometryRecovery.preflight.resolved_geometry.inline_3d)} /
            crossline {describeSegyHeaderField(geometryRecovery.preflight.resolved_geometry.crossline_3d)}
          </strong>
        </div>
        <div class="import-geometry-summary-row">
          <span>Current result</span>
          <strong>{geometryRecovery.preflight.classification} • {geometryRecovery.preflight.layout}</strong>
        </div>
      </div>

      {#if geometryRecovery.preflight.geometry_candidates.length}
        <div class="import-geometry-mode">
          <label class="import-geometry-mode-option">
            <input
              type="radio"
              name="geometry-mode"
              checked={geometryRecovery.mode === "candidate"}
              onchange={() => viewerModel.setImportGeometryRecoveryMode("candidate")}
            />
            <span>Use suggested mappings</span>
          </label>
          <label class="import-geometry-mode-option">
            <input
              type="radio"
              name="geometry-mode"
              checked={geometryRecovery.mode === "manual"}
              onchange={() => viewerModel.setImportGeometryRecoveryMode("manual")}
            />
            <span>Enter bytes manually</span>
          </label>
        </div>

        {#if geometryRecovery.mode === "candidate"}
          <div class="import-geometry-candidates">
            {#each geometryRecovery.preflight.geometry_candidates as candidate, candidateIndex (candidate.label)}
              <label
                class:selected={geometryRecovery.selectedCandidateIndex === candidateIndex}
                class="import-geometry-candidate"
              >
                <input
                  type="radio"
                  name="geometry-candidate"
                  checked={geometryRecovery.selectedCandidateIndex === candidateIndex}
                  onchange={() => viewerModel.selectImportGeometryCandidate(candidateIndex)}
                />
                <div class="import-geometry-candidate-copy">
                  <strong>{candidate.label}</strong>
                  <span>
                    {candidate.classification} • {candidate.inline_count} x {candidate.crossline_count}
                    {#if candidate.auto_selectable}
                      • recommended
                    {/if}
                  </span>
                  <code>{describeSegyGeometryCandidate(candidate)}</code>
                </div>
              </label>
            {/each}
          </div>
        {/if}
      {/if}

      <div class="import-geometry-manual">
        <div class="import-geometry-manual-header">
          <span>Manual Override</span>
          <small>Advanced: use this when the suggested list does not match the survey.</small>
        </div>

        <div class="import-geometry-grid">
          <label class="import-geometry-field">
            <span>Inline byte</span>
            <input
              type="number"
              min="1"
              value={geometryRecovery.draft.inlineByte}
              disabled={geometryRecovery.mode !== "manual" || geometryRecovery.working}
              oninput={(event) =>
                viewerModel.setImportGeometryRecoveryDraft(
                  "inlineByte",
                  (event.currentTarget as HTMLInputElement).value
                )}
            />
          </label>

          <label class="import-geometry-field">
            <span>Inline type</span>
            <select
              value={geometryRecovery.draft.inlineType}
              disabled={geometryRecovery.mode !== "manual" || geometryRecovery.working}
              onchange={(event) =>
                viewerModel.setImportGeometryRecoveryDraft(
                  "inlineType",
                  (event.currentTarget as HTMLSelectElement).value as "i16" | "i32"
                )}
            >
              <option value="i32">I32</option>
              <option value="i16">I16</option>
            </select>
          </label>

          <label class="import-geometry-field">
            <span>Crossline byte</span>
            <input
              type="number"
              min="1"
              value={geometryRecovery.draft.crosslineByte}
              disabled={geometryRecovery.mode !== "manual" || geometryRecovery.working}
              oninput={(event) =>
                viewerModel.setImportGeometryRecoveryDraft(
                  "crosslineByte",
                  (event.currentTarget as HTMLInputElement).value
                )}
            />
          </label>

          <label class="import-geometry-field">
            <span>Crossline type</span>
            <select
              value={geometryRecovery.draft.crosslineType}
              disabled={geometryRecovery.mode !== "manual" || geometryRecovery.working}
              onchange={(event) =>
                viewerModel.setImportGeometryRecoveryDraft(
                  "crosslineType",
                  (event.currentTarget as HTMLSelectElement).value as "i16" | "i32"
                )}
            >
              <option value="i32">I32</option>
              <option value="i16">I16</option>
            </select>
          </label>
        </div>
      </div>

      {#if geometryRecovery.error}
        <p class="import-geometry-error">{geometryRecovery.error}</p>
      {/if}

      <div class="import-geometry-actions">
        <button
          class="settings-btn secondary"
          onclick={() => viewerModel.closeImportGeometryRecovery()}
          disabled={geometryRecovery.working}
        >
          Cancel
        </button>
        <button
          class="settings-btn primary"
          onclick={() => viewerModel.confirmImportGeometryRecovery()}
          disabled={geometryRecovery.working}
        >
          {geometryRecovery.working ? "Validating Mapping..." : "Use Mapping And Import"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .sidebar-toggle {
    position: fixed;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    z-index: 10;
    background: #1a1a1a;
    border: 1px solid #333;
    border-left: none;
    border-radius: 0 2px 2px 0;
    padding: 10px 5px;
    color: #777;
    cursor: pointer;
  }

  .sidebar-toggle:hover {
    color: #d0d0d0;
    background: #252525;
  }

  .viewer-shell {
    min-height: 100vh;
    background: #141414;
  }

  .workspace-columns {
    min-height: 100vh;
    display: grid;
    grid-template-columns: minmax(260px, 300px) minmax(0, 1fr);
  }

  .session-column {
    min-height: 0;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 8px;
    padding: 10px 10px 12px;
    border-right: 1px solid #242424;
    background: #141414;
  }

  .session-column-header {
    display: grid;
    gap: 2px;
  }

  .eyebrow {
    display: inline-block;
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #555;
  }

  .session-column-header h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #c0c0c0;
  }

  .session-column-header p {
    margin: 0;
    font-size: 11px;
    color: #777;
    line-height: 1.45;
  }

  .main-column {
    min-height: 0;
    padding: 10px 14px 14px;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 8px;
  }

  .definition-pane {
    min-height: 0;
    display: grid;
    gap: 8px;
    position: relative;
    z-index: 8;
  }

  .definition-grid {
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(320px, 0.95fr) minmax(420px, 1.25fr);
    gap: 8px;
    align-items: stretch;
  }

  .viewer-pane {
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    position: relative;
    z-index: 1;
  }

  .chart-frame {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  .chart-display-overlay {
    display: grid;
    gap: 6px;
    justify-items: start;
  }

  .display-chip-row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .display-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 4px;
    border: none;
    background: transparent;
    color: #d7dde1;
  }

  .display-chip.field {
    padding-right: 6px;
  }

  .display-chip.field span {
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #7d7d7d;
  }

  .display-chip.field select,
  .display-chip.field input {
    min-width: 56px;
    border: none;
    outline: none;
    background: transparent;
    color: #f3f5f6;
    font: inherit;
  }

  .display-chip.field input {
    width: 52px;
  }

  .display-chip.action,
  .display-chip.icon {
    cursor: pointer;
  }

  .display-chip.action:hover:not(:disabled),
  .display-chip.icon:hover:not(:disabled) {
    color: #ffffff;
  }

  .display-chip.action.active {
    color: #effff5;
  }

  .display-chip.icon.active {
    color: #d9ffe9;
  }

  .display-chip:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .chart-toolbar-overlay {
    display: flex;
    justify-content: center;
  }

  .spectrum-inspector-layer {
    position: absolute;
    right: 14px;
    bottom: 16px;
    z-index: 6;
    pointer-events: none;
  }

  .chart-toolbar-overlay :global(.toolbar-group) {
    padding: 0;
    background: transparent;
    box-shadow: none;
    backdrop-filter: none;
  }

  .chart-toolbar-overlay :global(.toolbar-button) {
    background: transparent;
    color: #d7dde1;
  }

  .chart-toolbar-overlay :global(.toolbar-button:hover:not(:disabled)) {
    background: transparent;
    color: #ffffff;
  }

  .chart-toolbar-overlay :global(.toolbar-button.active) {
    background: transparent;
    box-shadow: none;
    color: #effff5;
  }

  .viewer-shell :global(.geoviz-svelte-chart-shell) {
    height: 100%;
    width: 100%;
    border-radius: 0;
    overflow: hidden;
    border: 1px solid #2a2a2a;
  }

  .compare-cycle-overlay {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 6px;
    align-items: center;
    padding: 0;
  }

  .compare-arrow {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 2px;
    border: none;
    background: transparent;
    color: #cfd6db;
    cursor: pointer;
  }

  .compare-arrow:hover:not(:disabled) {
    color: #ffffff;
  }

  .compare-arrow:disabled {
    opacity: 0.38;
    cursor: not-allowed;
  }

  .compare-cycle-copy {
    min-width: 0;
    display: grid;
    gap: 1px;
    text-align: center;
  }

  .compare-cycle-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .compare-cycle-copy small {
    font-size: 11px;
    color: #cfd6db;
  }

  .compare-label-overlay {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    pointer-events: none;
  }

  .compare-label-line strong {
    font-size: 12px;
    font-weight: 600;
    color: #eef3f6;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
  }

  .compare-label-line.secondary strong {
    color: #bed8ff;
  }

  .welcome-card {
    text-align: center;
    max-width: 380px;
    padding: 36px 32px;
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    margin: auto;
  }

  .welcome-icon {
    color: #333;
    margin-bottom: 16px;
  }

  .welcome-card h2 {
    margin: 0 0 10px;
    font-size: 16px;
    font-weight: 600;
    color: #c0c0c0;
  }

  .welcome-card p {
    margin: 0 0 18px;
    font-size: 12px;
    line-height: 1.55;
    color: #777;
  }

  .welcome-version {
    font-size: 11px;
    color: #444;
  }

  .display-settings-backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(3, 8, 12, 0.56);
    backdrop-filter: blur(6px);
  }

  .display-settings-dialog {
    width: min(520px, calc(100vw - 32px));
    padding: 18px;
    background: #0d1f2b;
    border: 1px solid rgba(173, 196, 208, 0.2);
    box-shadow: 0 24px 48px rgba(0, 0, 0, 0.32);
  }

  .display-settings-header h3 {
    margin: 0 0 14px;
    font-size: 16px;
    color: rgba(240, 246, 250, 0.96);
  }

  .display-settings-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .settings-field {
    display: grid;
    gap: 6px;
    font-size: 12px;
    color: rgba(194, 209, 218, 0.82);
  }

  .settings-field input,
  .settings-field select {
    min-height: 34px;
    padding: 7px 9px;
    border: 1px solid rgba(158, 183, 196, 0.18);
    background: rgba(6, 17, 24, 0.86);
    color: rgba(240, 246, 250, 0.96);
  }

  .settings-field input:disabled {
    opacity: 0.45;
  }

  .display-settings-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
    margin-top: 18px;
  }

  .settings-btn {
    min-width: 92px;
    min-height: 34px;
    padding: 7px 14px;
    border: 1px solid rgba(158, 183, 196, 0.18);
    cursor: pointer;
  }

  .settings-btn.secondary {
    background: rgba(10, 24, 33, 0.92);
    color: rgba(224, 235, 241, 0.92);
  }

  .settings-btn.primary {
    background: rgba(25, 79, 117, 0.94);
    color: white;
    border-color: rgba(107, 166, 206, 0.36);
  }

  .import-geometry-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(7, 10, 12, 0.7);
    backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    z-index: 30;
  }

  .import-geometry-dialog {
    width: min(720px, 100%);
    max-height: min(84vh, 820px);
    overflow: auto;
    background: linear-gradient(180deg, rgba(22, 27, 31, 0.98), rgba(14, 18, 21, 0.98));
    border: 1px solid rgba(122, 167, 142, 0.35);
    border-radius: 18px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.45);
    padding: 22px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .import-geometry-header h3 {
    margin: 0 0 8px;
    font-size: 18px;
    color: #eef4ef;
  }

  .import-geometry-header p {
    margin: 0;
    color: #b8c3bc;
    line-height: 1.45;
  }

  .import-geometry-summary {
    display: grid;
    gap: 10px;
    padding: 12px 14px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .import-geometry-summary-row {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: baseline;
  }

  .import-geometry-summary-row span {
    color: #90a29a;
  }

  .import-geometry-summary-row strong {
    color: #edf5f0;
    text-align: right;
  }

  .import-geometry-mode {
    display: flex;
    gap: 18px;
    flex-wrap: wrap;
  }

  .import-geometry-mode-option {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: #d6ded8;
  }

  .import-geometry-candidates {
    display: grid;
    gap: 10px;
  }

  .import-geometry-candidate {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 12px;
    align-items: start;
    padding: 12px 14px;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    cursor: pointer;
  }

  .import-geometry-candidate.selected {
    border-color: rgba(115, 190, 146, 0.65);
    background: rgba(79, 145, 104, 0.12);
  }

  .import-geometry-candidate-copy {
    display: grid;
    gap: 4px;
  }

  .import-geometry-candidate-copy strong {
    color: #edf5f0;
  }

  .import-geometry-candidate-copy span {
    color: #9fb0a9;
  }

  .import-geometry-candidate-copy code {
    color: #d7e8dc;
    font-size: 11px;
  }

  .import-geometry-manual {
    display: grid;
    gap: 12px;
    padding: 14px;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.07);
  }

  .import-geometry-manual-header {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: baseline;
  }

  .import-geometry-manual-header span {
    color: #edf5f0;
    font-weight: 600;
  }

  .import-geometry-manual-header small {
    color: #90a29a;
  }

  .import-geometry-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .import-geometry-field {
    display: grid;
    gap: 6px;
  }

  .import-geometry-field span {
    color: #aebbb4;
  }

  .import-geometry-field input,
  .import-geometry-field select {
    width: 100%;
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(10, 13, 15, 0.7);
    color: #eef4ef;
    padding: 8px 10px;
  }

  .import-geometry-error {
    margin: 0;
    color: #ff9f9f;
  }

  .import-geometry-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  @media (max-width: 900px) {
    .workspace-columns {
      grid-template-columns: 1fr;
    }

    .session-column {
      grid-template-rows: auto minmax(220px, auto);
      border-right: none;
      border-bottom: 1px solid #242424;
      padding-bottom: 10px;
    }

    .main-column {
      padding-inline: 10px;
      padding-bottom: 10px;
    }

    .definition-grid {
      grid-template-columns: 1fr;
    }

    .display-settings-grid {
      grid-template-columns: minmax(0, 1fr);
    }

    .import-geometry-grid {
      grid-template-columns: 1fr;
    }

    .import-geometry-summary-row,
    .import-geometry-manual-header {
      flex-direction: column;
      align-items: flex-start;
    }

    .import-geometry-summary-row strong {
      text-align: left;
    }
  }
</style>
