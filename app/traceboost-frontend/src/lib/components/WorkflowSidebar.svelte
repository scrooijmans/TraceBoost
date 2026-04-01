<svelte:options runes={true} />

<script lang="ts">
  import DiagnosticsPanel from "./DiagnosticsPanel.svelte";
  import { getViewerModelContext } from "../viewer-model.svelte";
  import { pickOutputFolder, pickSegyFile } from "../file-dialog";

  interface Props {
    showSidebar: boolean;
    hideSidebar: () => void;
    chartBound: boolean;
  }

  let { showSidebar, hideSidebar, chartBound }: Props = $props();

  const viewerModel = getViewerModelContext();

  function basename(filePath: string): string {
    return filePath.split(/[\\/]/).pop() ?? filePath;
  }

  async function handleSelectSegy() {
    const path = await pickSegyFile();
    if (path) {
      viewerModel.setInputPath(path);
      return;
    }

    viewerModel.note("SEG-Y file selection did not produce a usable path.", "ui", "warn");
  }

  async function handleSelectOutput() {
    const path = await pickOutputFolder();
    if (path) {
      viewerModel.setOutputStorePath(path);
      return;
    }

    viewerModel.note("Runtime store output selection did not produce a usable path.", "ui", "warn");
  }
</script>

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
        <p class="subtitle">Seismic Data Viewer</p>
      </div>
      <button class="collapse-button" onclick={hideSidebar} aria-label="Hide sidebar">
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
    </div>
  </div>

  <div class="steps">
    <div class="step">
      <div class="step-label">Step 1</div>
      <div class="step-title">Select SEG-Y File</div>
      <button class="btn btn-primary workflow-button" onclick={handleSelectSegy} disabled={viewerModel.loading}>
        {viewerModel.inputPath ? "Change File" : "Select File"}
      </button>
      {#if viewerModel.inputPath}
        <div class="selected-path" title={viewerModel.inputPath}>
          {basename(viewerModel.inputPath)}
        </div>
      {/if}
    </div>

    <div class="step">
      <div class="step-label">Step 2</div>
      <div class="step-title">Preflight Survey</div>
      <button
        class="btn btn-secondary workflow-button"
        onclick={() => void viewerModel.runPreflight()}
        disabled={viewerModel.loading || !viewerModel.inputPath}
      >
        Run Preflight
      </button>

      {#if viewerModel.preflight}
        <div class="info-card">
          <div class="info-row">
            <span>Classification</span>
            <span class="info-value">{viewerModel.preflight.classification}</span>
          </div>
          <div class="info-row">
            <span>Action</span>
            <span class="info-value">{viewerModel.preflight.suggested_action}</span>
          </div>
          <div class="info-row">
            <span>Traces</span>
            <span class="info-value">{viewerModel.preflight.trace_count}</span>
          </div>
          <div class="info-row">
            <span>Samples/trace</span>
            <span class="info-value">{viewerModel.preflight.samples_per_trace}</span>
          </div>
          <div class="info-row">
            <span>Completeness</span>
            <span class="info-value"
              >{(viewerModel.preflight.completeness_ratio * 100).toFixed(1)}%</span
            >
          </div>
          <div class="info-row">
            <span>Observed Traces</span>
            <span class="info-value">{viewerModel.preflight.observed_trace_count}</span>
          </div>
          <div class="info-row">
            <span>Expected Traces</span>
            <span class="info-value">{viewerModel.preflight.expected_trace_count}</span>
          </div>
        </div>

        {#if viewerModel.preflight.notes.length}
          <div class="notes-card">
            <div class="notes-title">Preflight Notes</div>
            <ul class="notes-list">
              {#each viewerModel.preflight.notes as note, index (`${index}:${note}`)}
                <li>{note}</li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if viewerModel.preflight.suggested_action !== "direct_dense_ingest"}
          <div class="warning-bar">
            Current backend policy only auto-imports direct dense surveys. This preflight path is likely
            to fail before any section reaches the viewer.
          </div>
        {/if}
      {/if}
    </div>

    <div class="step">
      <div class="step-label">Step 3</div>
      <div class="step-title">Runtime Store Output</div>
      <button class="btn btn-primary workflow-button" onclick={handleSelectOutput} disabled={viewerModel.loading}>
        {viewerModel.outputStorePath ? "Change Folder" : "Set Output Folder"}
      </button>
      {#if viewerModel.outputStorePath}
        <div class="selected-path" title={viewerModel.outputStorePath}>
          {basename(viewerModel.outputStorePath)}
        </div>
      {/if}
    </div>

    <div class="step">
      <div class="step-label">Step 4</div>
      <div class="step-title">Import & View</div>
      <div class="step-actions">
        <button
          class="btn btn-accent workflow-button"
          onclick={() => void viewerModel.importDataset()}
          disabled={viewerModel.loading || Boolean(viewerModel.importDisabledReason)}
        >
          Import SEG-Y
        </button>
        <button
          class="btn btn-secondary workflow-button"
          onclick={() => void viewerModel.openDataset()}
          disabled={viewerModel.loading || !viewerModel.outputStorePath}
        >
          Open Existing Store
        </button>
      </div>

      {#if !viewerModel.loading && viewerModel.importDisabledReason && viewerModel.inputPath && viewerModel.outputStorePath}
        <div class="step-hint">
          {viewerModel.importDisabledReason}
        </div>
      {/if}
    </div>

    {#if viewerModel.busyLabel}
      <div class="status-bar">
        <div class="spinner"></div>
        <span>{viewerModel.busyLabel}...</span>
      </div>
    {/if}

    {#if viewerModel.dataset}
      <div class="divider"></div>

      <div class="section-controls">
        <div class="step-title">Section Controls</div>

        <div class="info-card">
          <div class="info-row">
            <span>Label</span>
            <span class="info-value">{viewerModel.dataset.descriptor.label}</span>
          </div>
          <div class="info-row">
            <span>Shape</span>
            <span class="info-value"
              >{viewerModel.dataset.descriptor.shape[0]} x {viewerModel.dataset.descriptor.shape[1]} x
              {viewerModel.dataset.descriptor.shape[2]}</span
            >
          </div>
        </div>

        <div class="control-row">
          <label class="control-label">
            Axis
            <select
              bind:value={viewerModel.axis}
              disabled={!viewerModel.activeStorePath || viewerModel.loading}
              onchange={() => void viewerModel.load(viewerModel.axis, viewerModel.index)}
            >
              <option value="inline">Inline</option>
              <option value="xline">Xline</option>
            </select>
          </label>

          <label class="control-label">
            Index
            <input
              type="number"
              bind:value={viewerModel.index}
              min="0"
              disabled={!viewerModel.activeStorePath || viewerModel.loading}
              onchange={() => void viewerModel.load(viewerModel.axis, Number(viewerModel.index))}
            />
          </label>
        </div>

        <div class="control-row">
          <button
            class="btn btn-secondary btn-sm"
            disabled={!viewerModel.section}
            onclick={() =>
              viewerModel.setRenderMode(
                viewerModel.displayTransform.renderMode === "heatmap" ? "wiggle" : "heatmap"
              )}
          >
            {viewerModel.displayTransform.renderMode === "heatmap" ? "Wiggles" : "Heatmap"}
          </button>

          <button
            class="btn btn-secondary btn-sm"
            disabled={!viewerModel.section}
            onclick={() =>
              viewerModel.setColormap(
                viewerModel.displayTransform.colormap === "grayscale"
                  ? "red-white-blue"
                  : "grayscale"
              )}
          >
            {viewerModel.displayTransform.colormap === "grayscale" ? "R/W/B" : "Gray"}
          </button>
        </div>
      </div>

      <div class="probe-readout">
        {#if viewerModel.lastProbe?.probe}
          <div class="info-row">
            <span>Trace</span>
            <span class="info-value">{viewerModel.lastProbe.probe.trace_index}</span>
          </div>
          <div class="info-row">
            <span>Sample</span>
            <span class="info-value">{viewerModel.lastProbe.probe.sample_index}</span>
          </div>
          <div class="info-row">
            <span>Amplitude</span>
            <span class="info-value">{viewerModel.lastProbe.probe.amplitude.toFixed(4)}</span>
          </div>
        {:else}
          <div class="hint">Hover over the seismic chart for probe data.</div>
        {/if}
      </div>
    {/if}

    {#if viewerModel.error}
      <div class="error-bar">{viewerModel.error}</div>
    {/if}

    <DiagnosticsPanel {chartBound} />
  </div>

  <div class="sidebar-footer">
    <span>TraceBoost v0.1.0</span>
  </div>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    background: #0c1f2d;
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    overflow-y: auto;
    height: 100vh;
  }

  .sidebar.hidden {
    display: none;
  }

  .sidebar-header {
    padding: 20px 20px 0;
  }

  .logo-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .logo-copy {
    min-width: 0;
    flex: 1;
  }

  .logo-icon {
    color: #4ade80;
    flex-shrink: 0;
  }

  h1 {
    margin: 0;
    font-size: 20px;
    font-weight: 700;
    letter-spacing: -0.3px;
  }

  .version {
    font-size: 12px;
    font-weight: 400;
    color: rgba(255, 255, 255, 0.4);
    vertical-align: middle;
  }

  .subtitle {
    margin: 2px 0 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }

  .collapse-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    background: #102838;
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    flex-shrink: 0;
  }

  .collapse-button:hover {
    color: #fff;
    background: #1a3a50;
  }

  .steps {
    flex: 1;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .step {
    padding: 12px 0;
  }

  .step-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: rgba(255, 255, 255, 0.35);
    margin-bottom: 4px;
  }

  .step-title {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 10px;
  }

  .step-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 10px 16px;
    min-height: 44px;
    border-radius: 8px;
    border: 1px solid transparent;
    box-sizing: border-box;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, opacity 0.15s;
    color: #fff;
  }

  .workflow-button {
    width: 100%;
  }

  .btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .btn-primary {
    background: #1a6b3c;
  }

  .btn-primary:hover:not(:disabled) {
    background: #22874d;
  }

  .btn-secondary {
    background: #102838;
    border: 1px solid rgba(255, 255, 255, 0.12);
  }

  .btn-secondary:hover:not(:disabled) {
    background: #1a3a50;
  }

  .btn-accent {
    background: #2563eb;
  }

  .btn-accent:hover:not(:disabled) {
    background: #3b82f6;
  }

  .btn-sm {
    padding: 7px 12px;
    min-height: 36px;
    font-size: 12px;
    flex: 1;
  }

  .selected-path {
    margin-top: 8px;
    padding: 8px 10px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 6px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.6);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .step-hint {
    margin-top: 8px;
    font-size: 12px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.52);
  }

  .info-card {
    margin-top: 8px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
  }

  .notes-card {
    margin-top: 8px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
  }

  .notes-title {
    font-size: 12px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.8);
    margin-bottom: 6px;
  }

  .notes-list {
    margin: 0;
    padding-left: 18px;
    display: grid;
    gap: 6px;
    color: rgba(255, 255, 255, 0.6);
    font-size: 12px;
    line-height: 1.45;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    padding: 3px 0;
    color: rgba(255, 255, 255, 0.5);
  }

  .info-value {
    color: rgba(255, 255, 255, 0.8);
    font-variant-numeric: tabular-nums;
  }

  .status-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    background: rgba(74, 222, 128, 0.08);
    border-radius: 8px;
    font-size: 13px;
    color: #4ade80;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(74, 222, 128, 0.2);
    border-top-color: #4ade80;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 8px 0;
  }

  .section-controls {
    padding: 8px 0;
  }

  .control-row {
    display: flex;
    gap: 8px;
    margin-bottom: 8px;
  }

  .control-label {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
  }

  .control-label select,
  .control-label input {
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: #102838;
    color: inherit;
    font-size: 13px;
  }

  .probe-readout {
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
    margin-top: 4px;
  }

  .hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.35);
  }

  .error-bar {
    padding: 10px 12px;
    background: rgba(255, 100, 100, 0.1);
    border: 1px solid rgba(255, 100, 100, 0.2);
    border-radius: 8px;
    color: #ffb0b0;
    font-size: 13px;
    margin-top: 8px;
  }

  .warning-bar {
    margin-top: 12px;
    padding: 10px 12px;
    background: rgba(245, 158, 11, 0.12);
    border: 1px solid rgba(245, 158, 11, 0.24);
    border-radius: 8px;
    color: #fcd34d;
    font-size: 13px;
    line-height: 1.45;
  }

  .sidebar-footer {
    padding: 16px 20px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.25);
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }
</style>
