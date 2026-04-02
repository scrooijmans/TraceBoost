<svelte:options runes={true} />

<script lang="ts">
  import type { ProcessingOperation, ProcessingPipeline, ProcessingPreset } from "@traceboost/seis-contracts";
  import { isAmplitudeScalar } from "../processing-model.svelte";

  let {
    pipeline,
    selectedOperation,
    previewState,
    previewLabel,
    activeJob,
    presets,
    loadingPresets,
    canPreview,
    canRun,
    previewBusy,
    runBusy,
    processingError,
    onSetPipelineName,
    onSetAmplitudeScalarFactor,
    onMoveUp,
    onMoveDown,
    onRemove,
    onPreview,
    onShowRaw,
    onRun,
    onCancelJob,
    onLoadPreset,
    onSavePreset,
    onDeletePreset
  }: {
    pipeline: ProcessingPipeline;
    selectedOperation: ProcessingOperation | null;
    previewState: "raw" | "preview" | "stale";
    previewLabel: string | null;
    activeJob: { job_id: string; state: string; progress: { completed: number; total: number } } | null;
    presets: ProcessingPreset[];
    loadingPresets: boolean;
    canPreview: boolean;
    canRun: boolean;
    previewBusy: boolean;
    runBusy: boolean;
    processingError: string | null;
    onSetPipelineName: (value: string) => void;
    onSetAmplitudeScalarFactor: (value: number) => void;
    onMoveUp: () => void;
    onMoveDown: () => void;
    onRemove: () => void;
    onPreview: () => void | Promise<void>;
    onShowRaw: () => void;
    onRun: () => void | Promise<void>;
    onCancelJob: () => void | Promise<void>;
    onLoadPreset: (preset: ProcessingPreset) => void;
    onSavePreset: () => void | Promise<void>;
    onDeletePreset: (presetId: string) => void | Promise<void>;
  } = $props();

  let selectedPresetId = $state("");
</script>

<section class="editor-panel">
  <header class="editor-header">
    <div class="title-row">
      <div>
        <h3>Step Editor</h3>
        <p>{previewState === "preview" ? `Preview active: ${previewLabel ?? "processed"}` : previewState === "stale" ? "Preview stale" : "Showing raw section"}</p>
      </div>
      <div class="action-row">
        <button class="chip" onclick={onSavePreset}>Save Preset</button>
        <button class="chip" onclick={onPreview} disabled={!canPreview || previewBusy}>
          {previewBusy ? "Previewing..." : "Preview"}
        </button>
        <button class="chip primary" onclick={onRun} disabled={!canRun || runBusy}>
          {runBusy ? "Running..." : "Run Volume"}
        </button>
      </div>
    </div>
    <label class="field">
      <span>Pipeline Name</span>
      <input
        type="text"
        value={pipeline.name ?? ""}
        placeholder="Untitled pipeline"
        oninput={(event) => onSetPipelineName((event.currentTarget as HTMLInputElement).value)}
      />
    </label>
  </header>

  <div class="preset-row">
    <select bind:value={selectedPresetId} disabled={loadingPresets || !presets.length}>
      <option value="">Load preset...</option>
      {#each presets as preset (preset.preset_id)}
        <option value={preset.preset_id}>{preset.pipeline.name ?? preset.preset_id}</option>
      {/each}
    </select>
    <button
      class="chip"
      disabled={!selectedPresetId}
      onclick={() => {
        const preset = presets.find((candidate) => candidate.preset_id === selectedPresetId);
        if (preset) onLoadPreset(preset);
      }}
    >
      Load
    </button>
    <button class="chip danger" disabled={!selectedPresetId} onclick={() => onDeletePreset(selectedPresetId)}>
      Delete
    </button>
    <button class="chip" disabled={previewState === "raw"} onclick={onShowRaw}>Show Raw</button>
  </div>

  {#if selectedOperation}
    <div class="selected-card">
      <div class="selected-actions">
        <button class="chip" onclick={onMoveUp}>Move Up</button>
        <button class="chip" onclick={onMoveDown}>Move Down</button>
        <button class="chip danger" onclick={onRemove}>Delete Step</button>
      </div>

      {#if isAmplitudeScalar(selectedOperation)}
        <label class="field">
          <span>Amplitude Scalar Factor</span>
          <input
            type="number"
            min="0"
            max="10"
            step="0.1"
            value={selectedOperation.amplitude_scalar.factor}
            oninput={(event) =>
              onSetAmplitudeScalarFactor(Number((event.currentTarget as HTMLInputElement).value))}
          />
          <small>Valid range: 0.0 to 10.0</small>
        </label>
      {:else}
        <div class="info-block">
          <strong>Trace RMS Normalize</strong>
          <p>Scales each trace so its RMS amplitude becomes 1.0, with backend safeguards for zero-amplitude traces.</p>
        </div>
      {/if}
    </div>
  {:else}
    <div class="info-block empty">
      <strong>No step selected</strong>
      <p>Select a pipeline step to edit it.</p>
    </div>
  {/if}

  {#if activeJob}
    <div class="job-card">
      <div class="job-header">
        <strong>Background Job</strong>
        <span>{activeJob.state}</span>
      </div>
      <div class="job-progress">
        {activeJob.progress.completed} / {activeJob.progress.total || 0} tiles
      </div>
      {#if activeJob.state === "queued" || activeJob.state === "running"}
        <button class="chip danger" onclick={onCancelJob}>Cancel Job</button>
      {/if}
    </div>
  {/if}

  {#if processingError}
    <div class="error-bar">{processingError}</div>
  {/if}
</section>

<style>
  .editor-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 16px;
  }

  .editor-header {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .title-row {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .title-row h3 {
    margin: 0;
    font-size: 14px;
  }

  .title-row p {
    margin: 4px 0 0;
    color: rgba(255, 255, 255, 0.58);
    font-size: 12px;
  }

  .action-row,
  .preset-row,
  .selected-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field span {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.65);
  }

  .field input,
  .preset-row select {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 10px;
    color: inherit;
    padding: 10px 12px;
    font: inherit;
  }

  .field small {
    color: rgba(255, 255, 255, 0.46);
    font-size: 11px;
  }

  .chip {
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.04);
    color: inherit;
    border-radius: 999px;
    padding: 8px 10px;
    cursor: pointer;
  }

  .chip.primary {
    background: #1f8f5f;
    border-color: #1f8f5f;
  }

  .chip.danger {
    border-color: rgba(255, 120, 120, 0.35);
    color: #ff9a9a;
  }

  .chip:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .selected-card,
  .job-card,
  .info-block {
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    padding: 14px;
    background: rgba(255, 255, 255, 0.02);
  }

  .info-block strong,
  .job-header strong {
    display: block;
    margin-bottom: 6px;
  }

  .info-block p {
    margin: 0;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.62);
    line-height: 1.5;
  }

  .job-header {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: center;
  }

  .job-progress {
    margin: 8px 0 12px;
    color: rgba(255, 255, 255, 0.65);
    font-size: 12px;
  }

  .error-bar {
    border-radius: 12px;
    background: rgba(255, 89, 94, 0.16);
    border: 1px solid rgba(255, 89, 94, 0.25);
    padding: 10px 12px;
    color: #ffb4b7;
    font-size: 12px;
  }
</style>
