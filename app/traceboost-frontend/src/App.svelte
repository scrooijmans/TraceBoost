<svelte:options runes={true} />

<script lang="ts">
  import { onDestroy } from "svelte";
  import { SeismicSectionChart } from "@geoviz/svelte";
  import type { TraceBoostViewerState } from "./lib/viewer-store";
  import { viewerStore } from "./lib/viewer-store";
  import { pickSegyFile, pickOutputFolder } from "./lib/file-dialog";

  let state = $state<TraceBoostViewerState>({
    inputPath: "",
    outputStorePath: "",
    activeStorePath: "",
    dataset: null,
    preflight: null,
    axis: "inline",
    index: 0,
    section: null,
    loading: false,
    busyLabel: null,
    error: null,
    resetToken: "inline:0",
    displayTransform: {
      renderMode: "heatmap",
      colormap: "grayscale",
      gain: 1,
      polarity: "normal"
    },
    lastProbe: null,
    lastViewport: null,
    lastInteraction: null
  });

  const unsubscribe = viewerStore.subscribe((value) => {
    state = value;
  });

  onDestroy(() => {
    unsubscribe();
  });

  let showSidebar = $state(true);

  async function handleSelectSegy() {
    const path = await pickSegyFile();
    if (path) {
      viewerStore.setInputPath(path);
    }
  }

  async function handleSelectOutput() {
    const path = await pickOutputFolder();
    if (path) {
      viewerStore.setOutputStorePath(path);
    }
  }

  function basename(filePath: string): string {
    return filePath.split(/[\\/]/).pop() ?? filePath;
  }
</script>

<svelte:head>
  <title>TraceBoost</title>
</svelte:head>

<div class="shell">
  <aside class="sidebar" class:hidden={!showSidebar}>
    <div class="sidebar-header">
      <div class="logo-row">
        <svg class="logo-icon" viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M3 20 L6 8 L9 14 L12 4 L15 16 L18 10 L21 20" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
        <div>
          <h1>TraceBoost <span class="version">v0.1.0</span></h1>
          <p class="subtitle">Seismic Data Viewer</p>
        </div>
      </div>
    </div>

    <div class="steps">
      <!-- Step 1: Select SEG-Y -->
      <div class="step">
        <div class="step-label">Step 1</div>
        <div class="step-title">Select SEG-Y File</div>
        <button class="btn btn-primary" onclick={handleSelectSegy} disabled={state.loading}>
          {state.inputPath ? "Change File" : "Select File"}
        </button>
        {#if state.inputPath}
          <div class="selected-path" title={state.inputPath}>
            {basename(state.inputPath)}
          </div>
        {/if}
      </div>

      <!-- Step 2: Preflight -->
      <div class="step">
        <div class="step-label">Step 2</div>
        <div class="step-title">Preflight Survey</div>
        <button
          class="btn btn-secondary"
          onclick={() => viewerStore.runPreflight()}
          disabled={state.loading || !state.inputPath}
        >
          Run Preflight
        </button>

        {#if state.preflight}
          <div class="info-card">
            <div class="info-row">
              <span>Classification</span>
              <span class="info-value">{state.preflight.classification}</span>
            </div>
            <div class="info-row">
              <span>Action</span>
              <span class="info-value">{state.preflight.suggested_action}</span>
            </div>
            <div class="info-row">
              <span>Traces</span>
              <span class="info-value">{state.preflight.trace_count}</span>
            </div>
            <div class="info-row">
              <span>Samples/trace</span>
              <span class="info-value">{state.preflight.samples_per_trace}</span>
            </div>
            <div class="info-row">
              <span>Completeness</span>
              <span class="info-value">{(state.preflight.completeness_ratio * 100).toFixed(1)}%</span>
            </div>
          </div>
        {/if}
      </div>

      <!-- Step 3: Set Output -->
      <div class="step">
        <div class="step-label">Step 3</div>
        <div class="step-title">Runtime Store Output</div>
        <button class="btn btn-primary" onclick={handleSelectOutput} disabled={state.loading}>
          {state.outputStorePath ? "Change Folder" : "Set Output Folder"}
        </button>
        {#if state.outputStorePath}
          <div class="selected-path" title={state.outputStorePath}>
            {basename(state.outputStorePath)}
          </div>
        {/if}
      </div>

      <!-- Step 4: Import & View -->
      <div class="step">
        <div class="step-label">Step 4</div>
        <div class="step-title">Import & View</div>
        <div class="step-actions">
          <button
            class="btn btn-accent"
            onclick={() => viewerStore.importDataset()}
            disabled={state.loading || !state.inputPath || !state.outputStorePath}
          >
            Import SEG-Y
          </button>
          <button
            class="btn btn-secondary"
            onclick={() => viewerStore.openDataset()}
            disabled={state.loading || !state.outputStorePath}
          >
            Open Existing Store
          </button>
        </div>
      </div>

      {#if state.busyLabel}
        <div class="status-bar">
          <div class="spinner"></div>
          <span>{state.busyLabel}...</span>
        </div>
      {/if}

      <!-- Section Controls (visible when dataset loaded) -->
      {#if state.dataset}
        <div class="divider"></div>

        <div class="section-controls">
          <div class="step-title">Section Controls</div>

          {#if state.dataset}
            <div class="info-card">
              <div class="info-row">
                <span>Label</span>
                <span class="info-value">{state.dataset.descriptor.label}</span>
              </div>
              <div class="info-row">
                <span>Shape</span>
                <span class="info-value">
                  {state.dataset.descriptor.shape[0]} x {state.dataset.descriptor.shape[1]} x {state.dataset.descriptor.shape[2]}
                </span>
              </div>
            </div>
          {/if}

          <div class="control-row">
            <label class="control-label">
              Axis
              <select
                bind:value={state.axis}
                disabled={!state.activeStorePath || state.loading}
                onchange={() => viewerStore.load(state.axis as "inline" | "xline", state.index)}
              >
                <option value="inline">Inline</option>
                <option value="xline">Xline</option>
              </select>
            </label>

            <label class="control-label">
              Index
              <input
                type="number"
                bind:value={state.index}
                min="0"
                disabled={!state.activeStorePath || state.loading}
                onchange={() => viewerStore.load(state.axis as "inline" | "xline", Number(state.index))}
              />
            </label>
          </div>

          <div class="control-row">
            <button
              class="btn btn-secondary btn-sm"
              disabled={!state.section}
              onclick={() =>
                viewerStore.setRenderMode(
                  state.displayTransform.renderMode === "heatmap" ? "wiggle" : "heatmap"
                )}
            >
              {state.displayTransform.renderMode === "heatmap" ? "Wiggles" : "Heatmap"}
            </button>

            <button
              class="btn btn-secondary btn-sm"
              disabled={!state.section}
              onclick={() =>
                viewerStore.setColormap(
                  state.displayTransform.colormap === "grayscale" ? "red-white-blue" : "grayscale"
                )}
            >
              {state.displayTransform.colormap === "grayscale" ? "R/W/B" : "Gray"}
            </button>
          </div>
        </div>

        <!-- Probe readout -->
        <div class="probe-readout">
          {#if state.lastProbe?.probe}
            <div class="info-row">
              <span>Trace</span>
              <span class="info-value">{state.lastProbe.probe.trace_index}</span>
            </div>
            <div class="info-row">
              <span>Sample</span>
              <span class="info-value">{state.lastProbe.probe.sample_index}</span>
            </div>
            <div class="info-row">
              <span>Amplitude</span>
              <span class="info-value">{state.lastProbe.probe.amplitude.toFixed(4)}</span>
            </div>
          {:else}
            <div class="hint">Hover over the seismic chart for probe data.</div>
          {/if}
        </div>
      {/if}

      {#if state.error}
        <div class="error-bar">{state.error}</div>
      {/if}
    </div>

    <div class="sidebar-footer">
      <span>TraceBoost v0.1.0</span>
    </div>
  </aside>

  <!-- Sidebar toggle -->
  {#if !showSidebar}
    <button class="sidebar-toggle" onclick={() => (showSidebar = true)}>
      <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="9 18 15 12 9 6" />
      </svg>
    </button>
  {/if}

  <main class="viewer-shell">
    {#if state.section}
      <SeismicSectionChart
        chartId="traceboost-main"
        viewId={`${state.axis}:${state.index}`}
        section={state.section}
        displayTransform={state.displayTransform}
        loading={state.loading}
        errorMessage={state.error}
        resetToken={state.resetToken}
        onProbeChange={(event) => viewerStore.setProbe(event)}
        onViewportChange={(event) => viewerStore.setViewport(event)}
        onInteractionChange={(event) => viewerStore.setInteraction(event)}
      />
    {:else}
      <div class="welcome-card">
        <svg class="welcome-icon" viewBox="0 0 24 24" width="64" height="64" fill="none" stroke="currentColor" stroke-width="1">
          <path d="M3 20 L6 8 L9 14 L12 4 L15 16 L18 10 L21 20" stroke-linecap="round" stroke-linejoin="round" />
          <line x1="3" y1="20" x2="21" y2="20" />
        </svg>
        <h2>Select a SEG-Y File</h2>
        <p>
          Use the sidebar to select a SEG-Y file, run a preflight check,
          set an output folder, then import or open a runtime store to view seismic sections.
        </p>
        <span class="welcome-version">TraceBoost v0.1.0</span>
      </div>
    {/if}
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
    background: #07151f;
    color: #e8edf0;
    -webkit-font-smoothing: antialiased;
  }

  .shell {
    display: grid;
    grid-template-columns: 320px 1fr;
    min-height: 100vh;
  }

  /* Sidebar */
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

  .sidebar.hidden + .viewer-shell {
    grid-column: 1 / -1;
  }

  .sidebar-header {
    padding: 20px 20px 0;
  }

  .logo-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .logo-icon {
    color: #4ade80;
    flex-shrink: 0;
  }

  .sidebar-header h1 {
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

  /* Steps */
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

  /* Buttons */
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 10px 16px;
    border-radius: 8px;
    border: none;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s, opacity 0.15s;
    color: #fff;
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
    font-size: 12px;
    flex: 1;
  }

  /* Selected path display */
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

  /* Info cards */
  .info-card {
    margin-top: 8px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 8px;
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

  /* Status / busy */
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
    to { transform: rotate(360deg); }
  }

  /* Divider */
  .divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 8px 0;
  }

  /* Section controls */
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

  /* Probe readout */
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

  /* Error */
  .error-bar {
    padding: 10px 12px;
    background: rgba(255, 100, 100, 0.1);
    border: 1px solid rgba(255, 100, 100, 0.2);
    border-radius: 8px;
    color: #ffb0b0;
    font-size: 13px;
    margin-top: 8px;
  }

  /* Footer */
  .sidebar-footer {
    padding: 16px 20px;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.25);
    border-top: 1px solid rgba(255, 255, 255, 0.06);
  }

  /* Sidebar toggle */
  .sidebar-toggle {
    position: fixed;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    z-index: 10;
    background: #0c1f2d;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-left: none;
    border-radius: 0 8px 8px 0;
    padding: 12px 6px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
  }

  .sidebar-toggle:hover {
    color: #fff;
    background: #1a3a50;
  }

  /* Main viewer */
  .viewer-shell {
    padding: 20px;
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .viewer-shell :global(.geoviz-svelte-chart-shell) {
    height: calc(100vh - 40px);
    width: 100%;
    border-radius: 16px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  /* Welcome card */
  .welcome-card {
    text-align: center;
    max-width: 420px;
    padding: 48px 40px;
    background: #0c1f2d;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 20px;
  }

  .welcome-icon {
    color: rgba(255, 255, 255, 0.15);
    margin-bottom: 20px;
  }

  .welcome-card h2 {
    margin: 0 0 12px;
    font-size: 22px;
    font-weight: 600;
  }

  .welcome-card p {
    margin: 0 0 24px;
    font-size: 14px;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.5);
  }

  .welcome-version {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.25);
  }
</style>
