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
    runOutputSettingsOpen,
    runOutputPathMode,
    runOutputPath,
    resolvingRunOutputPath,
    overwriteExistingRunOutput,
    onSetPipelineName,
    onSetAmplitudeScalarFactor,
    onMoveUp,
    onMoveDown,
    onRemove,
    onPreview,
    onShowRaw,
    onRun,
    onToggleRunOutputSettings,
    onSetRunOutputPathMode,
    onSetCustomRunOutputPath,
    onBrowseRunOutputPath,
    onResetRunOutputPath,
    onSetOverwriteExistingRunOutput,
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
    runOutputSettingsOpen: boolean;
    runOutputPathMode: "default" | "custom";
    runOutputPath: string | null;
    resolvingRunOutputPath: boolean;
    overwriteExistingRunOutput: boolean;
    onSetPipelineName: (value: string) => void;
    onSetAmplitudeScalarFactor: (value: number) => void;
    onMoveUp: () => void;
    onMoveDown: () => void;
    onRemove: () => void;
    onPreview: () => void | Promise<void>;
    onShowRaw: () => void;
    onRun: () => void | Promise<void>;
    onToggleRunOutputSettings: () => void;
    onSetRunOutputPathMode: (mode: "default" | "custom") => void;
    onSetCustomRunOutputPath: (value: string) => void;
    onBrowseRunOutputPath: () => void | Promise<void>;
    onResetRunOutputPath: () => void;
    onSetOverwriteExistingRunOutput: (value: boolean) => void;
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
        <button class="chip" onclick={onToggleRunOutputSettings} disabled={!canRun || runBusy}>
          {runOutputSettingsOpen ? "Hide Output" : "Output Settings"}
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

  {#if runOutputSettingsOpen}
    <section class="output-settings">
      <div class="output-settings-header">
        <strong>Volume Output</strong>
        <span>{runOutputPathMode === "default" ? "Managed default" : "Custom path"}</span>
      </div>

      <div class="mode-row">
        <button
          class:active={runOutputPathMode === "default"}
          class="chip"
          onclick={() => onSetRunOutputPathMode("default")}
          disabled={runBusy}
        >
          Default
        </button>
        <button
          class:active={runOutputPathMode === "custom"}
          class="chip"
          onclick={() => onSetRunOutputPathMode("custom")}
          disabled={runBusy}
        >
          Custom
        </button>
      </div>

      <label class="field">
        <span>Output Store Path</span>
        <div class="path-row">
          <input
            type="text"
            value={runOutputPath ?? ""}
            placeholder={resolvingRunOutputPath ? "Resolving managed output path..." : "No output path selected"}
            readonly={runOutputPathMode === "default"}
            oninput={(event) => onSetCustomRunOutputPath((event.currentTarget as HTMLInputElement).value)}
          />
          <button class="chip" onclick={onBrowseRunOutputPath} disabled={runBusy}>
            Browse
          </button>
          <button class="chip" onclick={onResetRunOutputPath} disabled={runBusy || runOutputPathMode === "default"}>
            Reset
          </button>
        </div>
        <small>
          {#if runOutputPathMode === "default"}
            TraceBoost writes a unique derived `.tbvol` into its managed output library.
          {:else}
            Use a custom `.tbvol` path when you need to control naming or overwrite an existing store.
          {/if}
        </small>
      </label>

      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={overwriteExistingRunOutput}
          onchange={(event) => onSetOverwriteExistingRunOutput((event.currentTarget as HTMLInputElement).checked)}
        />
        <span>Allow overwrite if the output store already exists</span>
      </label>
    </section>
  {/if}

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
    gap: 8px;
    min-height: 0;
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    padding: 10px;
    overflow: auto;
  }

  .editor-header {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .title-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: flex-start;
  }

  .title-row h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: #c0c0c0;
  }

  .title-row p {
    margin: 2px 0 0;
    color: #777;
    font-size: 11px;
  }

  .action-row,
  .preset-row,
  .selected-actions {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field span {
    font-size: 11px;
    color: #777;
  }

  .field input,
  .preset-row select {
    background: #252525;
    border: 1px solid #333;
    border-radius: 2px;
    color: #d0d0d0;
    padding: 6px 8px;
    font: inherit;
    font-size: 12px;
  }

  .output-settings {
    border: 1px solid #2f2f2f;
    background: #171717;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .output-settings-header {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: baseline;
  }

  .output-settings-header strong {
    font-size: 11px;
    color: #d1d1d1;
    font-weight: 650;
  }

  .output-settings-header span {
    font-size: 10px;
    color: #6e6e6e;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .mode-row,
  .path-row {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
  }

  .path-row input {
    flex: 1 1 220px;
    min-width: 0;
  }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: #909090;
  }

  .checkbox-row input {
    margin: 0;
  }

  .field small {
    color: #555;
    font-size: 11px;
  }

  .chip {
    border: 1px solid #333;
    background: #252525;
    color: #aaa;
    border-radius: 2px;
    padding: 4px 8px;
    font-size: 11px;
    cursor: pointer;
  }

  .chip:hover:not(:disabled) {
    background: #2e2e2e;
    color: #d0d0d0;
  }

  .chip.primary {
    background: #1a5c33;
    border-color: #236b3d;
    color: #d0d0d0;
  }

  .chip.active {
    border-color: #4d6a57;
    background: #213c2c;
    color: #f2fff7;
  }

  .chip.primary:hover:not(:disabled) {
    background: #1f6e3d;
  }

  .chip.danger {
    border-color: rgba(200, 60, 60, 0.3);
    color: #c07070;
  }

  .chip:disabled {
    opacity: 0.38;
    cursor: not-allowed;
  }

  .selected-card,
  .job-card,
  .info-block {
    border: 1px solid #2a2a2a;
    padding: 10px;
    background: #1e1e1e;
  }

  .info-block strong,
  .job-header strong {
    display: block;
    margin-bottom: 4px;
    font-size: 12px;
    color: #c0c0c0;
  }

  .info-block p {
    margin: 0;
    font-size: 12px;
    color: #777;
    line-height: 1.45;
  }

  .job-header {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: center;
  }

  .job-progress {
    margin: 5px 0 8px;
    color: #888;
    font-size: 11px;
  }

  .error-bar {
    background: rgba(180, 40, 40, 0.1);
    border: 1px solid rgba(200, 60, 60, 0.22);
    padding: 7px 10px;
    color: #c08080;
    font-size: 11px;
  }
</style>
