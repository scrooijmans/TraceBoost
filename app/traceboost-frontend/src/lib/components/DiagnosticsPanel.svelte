<svelte:options runes={true} />

<script lang="ts">
  import { getViewerModelContext } from "../viewer-model.svelte";

  interface Props {
    chartBound: boolean;
  }

  let { chartBound }: Props = $props();

  const viewerModel = getViewerModelContext();

  function basename(filePath: string): string {
    return filePath.split(/[\\/]/).pop() ?? filePath;
  }

  function formatDiagnosticsFields(fields: Record<string, unknown> | null | undefined): string | null {
    if (!fields) {
      return null;
    }

    return Object.entries(fields)
      .map(([key, value]) => `${key}=${String(value)}`)
      .join(" ");
  }

  function handleVerboseDiagnosticsToggle(event: Event): void {
    const enabled = (event.currentTarget as HTMLInputElement).checked;
    void viewerModel.updateDiagnosticsVerbosity(enabled);
  }
</script>

<details class="diagnostics-panel" open>
  <summary>Diagnostics</summary>

  <div class="info-card diagnostics-card">
    <div class="info-row">
      <span>Runtime</span>
      <span class="info-value">{viewerModel.tauriRuntime ? "tauri" : "browser"}</span>
    </div>
    <div class="info-row">
      <span>Chart Bound</span>
      <span class="info-value">{chartBound ? "yes" : "no"}</span>
    </div>
    <div class="info-row">
      <span>Dataset Loaded</span>
      <span class="info-value">{viewerModel.dataset ? "yes" : "no"}</span>
    </div>
    <div class="info-row">
      <span>Section Loaded</span>
      <span class="info-value"
        >{viewerModel.section ? `${viewerModel.section.traces} x ${viewerModel.section.samples}` : "no"}</span
      >
    </div>
    <div class="info-row">
      <span>Active Store</span>
      <span class="info-value"
        >{viewerModel.activeStorePath ? basename(viewerModel.activeStorePath) : "none"}</span
      >
    </div>
    {#if viewerModel.diagnosticsStatus}
      <div class="info-row">
        <span>Session</span>
        <span class="info-value">{basename(viewerModel.diagnosticsStatus.sessionLogPath)}</span>
      </div>
    {/if}
  </div>

  {#if viewerModel.tauriRuntime}
    <label class="diagnostics-toggle">
      <input
        type="checkbox"
        checked={viewerModel.verboseDiagnostics}
        onchange={handleVerboseDiagnosticsToggle}
      />
      <span>Verbose backend diagnostics</span>
    </label>
  {/if}

  <div class="diagnostics-list">
    <div class="step-title">Recent Activity</div>
    {#if viewerModel.recentActivity.length}
      {#each viewerModel.recentActivity as entry (entry.id)}
        <div class={`diagnostics-entry diagnostics-entry-${entry.level}`}>
          <div class="diagnostics-meta">
            <span>{entry.timestamp}</span>
            <span>{entry.source}</span>
          </div>
          <div>{entry.message}</div>
          {#if entry.detail}
            <div class="diagnostics-detail">{entry.detail}</div>
          {/if}
        </div>
      {/each}
    {:else}
      <div class="hint">No app activity recorded yet.</div>
    {/if}
  </div>

  <div class="diagnostics-list">
    <div class="step-title">Backend Events</div>
    {#if viewerModel.backendEvents.length}
      {#each viewerModel.backendEvents as event (event.operationId + event.timestamp + event.stage)}
        <div
          class={`diagnostics-entry diagnostics-entry-${event.level === "ERROR" ? "error" : event.level === "WARN" ? "warn" : "info"}`}
        >
          <div class="diagnostics-meta">
            <span>{event.timestamp}</span>
            <span>{event.command}:{event.stage}</span>
          </div>
          <div>{event.message}</div>
          {#if formatDiagnosticsFields(event.fields)}
            <div class="diagnostics-detail">{formatDiagnosticsFields(event.fields)}</div>
          {/if}
        </div>
      {/each}
    {:else if viewerModel.tauriRuntime}
      <div class="hint">No backend diagnostics events received yet.</div>
    {:else}
      <div class="hint">Backend events are only available in the Tauri desktop shell.</div>
    {/if}
  </div>
</details>

<style>
  .diagnostics-panel {
    margin-top: 16px;
    display: grid;
    gap: 12px;
  }

  .diagnostics-panel summary {
    cursor: pointer;
    font-size: 13px;
    font-weight: 700;
    color: #bfd0da;
    list-style: none;
  }

  .diagnostics-panel summary::-webkit-details-marker {
    display: none;
  }

  .diagnostics-card {
    margin-top: 10px;
  }

  .diagnostics-toggle {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: #c7d6de;
  }

  .diagnostics-toggle input {
    margin: 0;
  }

  .diagnostics-list {
    display: grid;
    gap: 8px;
  }

  .diagnostics-entry {
    border-radius: 12px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 12px;
    line-height: 1.45;
  }

  .diagnostics-entry-info {
    border-color: rgba(59, 130, 246, 0.22);
  }

  .diagnostics-entry-warn {
    border-color: rgba(245, 158, 11, 0.22);
  }

  .diagnostics-entry-error {
    border-color: rgba(248, 113, 113, 0.22);
  }

  .diagnostics-meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 4px;
    color: #8fa3af;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .diagnostics-detail {
    margin-top: 6px;
    color: #bfd0da;
    word-break: break-word;
  }

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

  .step-title {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: 10px;
  }

  .hint {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.35);
  }
</style>
