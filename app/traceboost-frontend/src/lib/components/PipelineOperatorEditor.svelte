<svelte:options runes={true} />

<script lang="ts">
  import type { ProcessingOperation } from "@traceboost/seis-contracts";
  import { isAmplitudeScalar, isBandpassFilter } from "../processing-model.svelte";

  let {
    selectedOperation,
    activeJob,
    processingError,
    onSetAmplitudeScalarFactor,
    onSetBandpassCorner = () => {},
    onMoveUp,
    onMoveDown,
    onRemove,
    onCancelJob
  }: {
    selectedOperation: ProcessingOperation | null;
    activeJob: { job_id: string; state: string; progress: { completed: number; total: number } } | null;
    processingError: string | null;
    onSetAmplitudeScalarFactor: (value: number) => void;
    onSetBandpassCorner?: (corner: "f1_hz" | "f2_hz" | "f3_hz" | "f4_hz", value: number) => void;
    onMoveUp: () => void;
    onMoveDown: () => void;
    onRemove: () => void;
    onCancelJob: () => void | Promise<void>;
  } = $props();
</script>

<section class="editor-panel">
  <header class="editor-header">
    <h3>Step Editor</h3>
    <p>Adjust the selected operator parameters and manage ordering.</p>
  </header>

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
      {:else if isBandpassFilter(selectedOperation)}
        <div class="field-grid">
          <label class="field">
            <span>F1 Low Stop</span>
            <input
              type="number"
              min="0"
              step="0.5"
              value={selectedOperation.bandpass_filter.f1_hz}
              oninput={(event) =>
                onSetBandpassCorner("f1_hz", Number((event.currentTarget as HTMLInputElement).value))}
            />
          </label>
          <label class="field">
            <span>F2 Low Pass</span>
            <input
              type="number"
              min="0"
              step="0.5"
              value={selectedOperation.bandpass_filter.f2_hz}
              oninput={(event) =>
                onSetBandpassCorner("f2_hz", Number((event.currentTarget as HTMLInputElement).value))}
            />
          </label>
          <label class="field">
            <span>F3 High Pass</span>
            <input
              type="number"
              min="0"
              step="0.5"
              value={selectedOperation.bandpass_filter.f3_hz}
              oninput={(event) =>
                onSetBandpassCorner("f3_hz", Number((event.currentTarget as HTMLInputElement).value))}
            />
          </label>
          <label class="field">
            <span>F4 High Stop</span>
            <input
              type="number"
              min="0"
              step="0.5"
              value={selectedOperation.bandpass_filter.f4_hz}
              oninput={(event) =>
                onSetBandpassCorner("f4_hz", Number((event.currentTarget as HTMLInputElement).value))}
            />
          </label>
        </div>
        <div class="info-block">
          <strong>Bandpass Filter</strong>
          <p>Zero-phase frequency-domain bandpass with cosine tapers. Runtime validation enforces f1 ≤ f2 ≤ f3 ≤ f4 ≤ Nyquist.</p>
          <p>Phase: {selectedOperation.bandpass_filter.phase}. Window: {selectedOperation.bandpass_filter.window}.</p>
        </div>
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
    gap: 2px;
  }

  .editor-header h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: #c0c0c0;
  }

  .editor-header p {
    margin: 0;
    color: #777;
    font-size: 11px;
  }

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

  .field-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .field span {
    font-size: 11px;
    color: #777;
  }

  .field input {
    background: #252525;
    border: 1px solid #333;
    border-radius: 2px;
    color: #d0d0d0;
    padding: 6px 8px;
    font: inherit;
    font-size: 12px;
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
    color: #c0c0c0;
    font-size: 12px;
  }

  .info-block p,
  .job-progress {
    margin: 0;
    color: #777;
    font-size: 11px;
    line-height: 1.5;
  }

  .job-header {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: center;
    margin-bottom: 6px;
  }

  .job-header span {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #6e6e6e;
  }

  .job-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .error-bar {
    border: 1px solid rgba(200, 60, 60, 0.25);
    background: rgba(80, 24, 24, 0.45);
    color: #d99999;
    font-size: 11px;
    padding: 8px 10px;
  }

  @media (max-width: 720px) {
    .field-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
