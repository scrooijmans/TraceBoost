<svelte:options runes={true} />

<script lang="ts">
  import type { ProcessingOperation, ProcessingPipeline } from "@traceboost/seis-contracts";
  import { describeOperation } from "../processing-model.svelte";

  let {
    pipeline,
    selectedIndex,
    onSelect,
    onAddAmplitudeScalar,
    onAddTraceNormalize
  }: {
    pipeline: ProcessingPipeline;
    selectedIndex: number;
    onSelect: (index: number) => void;
    onAddAmplitudeScalar: () => void;
    onAddTraceNormalize: () => void;
  } = $props();

  function summary(operation: ProcessingOperation): string {
    return describeOperation(operation);
  }
</script>

<section class="sequence-panel">
  <header class="panel-header">
    <div>
      <h3>Pipeline</h3>
      <p>{pipeline.operations.length} step{pipeline.operations.length === 1 ? "" : "s"}</p>
    </div>
    <div class="quick-add">
      <button class="chip" onclick={onAddAmplitudeScalar}>+ Scalar</button>
      <button class="chip" onclick={onAddTraceNormalize}>+ Normalize</button>
    </div>
  </header>

  {#if pipeline.operations.length}
    <ol class="sequence-list">
      {#each pipeline.operations as operation, index (`${index}:${summary(operation)}`)}
        <li>
          <button
            class:selected={index === selectedIndex}
            class="sequence-row"
            onclick={() => onSelect(index)}
          >
            <span class="step-index">{index + 1}</span>
            <span class="step-copy">
              <strong>{summary(operation)}</strong>
            </span>
          </button>
        </li>
      {/each}
    </ol>
  {:else}
    <div class="empty-state">
      <p>No operators in the pipeline.</p>
      <p class="hint">Press <code>a</code> for amplitude scalar or <code>n</code> for trace RMS normalize.</p>
    </div>
  {/if}
</section>

<style>
  .sequence-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  h3 {
    margin: 0;
    font-size: 14px;
  }

  .panel-header p {
    margin: 4px 0 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.55);
  }

  .quick-add {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .chip {
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.04);
    color: inherit;
    border-radius: 999px;
    padding: 7px 10px;
    font-size: 12px;
    cursor: pointer;
  }

  .chip:hover {
    background: rgba(255, 255, 255, 0.08);
  }

  .sequence-list {
    list-style: none;
    margin: 0;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow: auto;
  }

  li {
    margin: 0;
  }

  .sequence-row {
    width: 100%;
    display: grid;
    grid-template-columns: 32px 1fr;
    gap: 12px;
    align-items: center;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    border-radius: 12px;
    color: inherit;
    text-align: left;
    padding: 12px;
    cursor: pointer;
  }

  .sequence-row.selected {
    border-color: rgba(74, 222, 128, 0.6);
    background: rgba(74, 222, 128, 0.08);
  }

  .step-index {
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    font-size: 12px;
  }

  .step-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .step-copy strong {
    font-size: 13px;
    font-weight: 600;
  }

  .empty-state {
    padding: 18px 16px;
    color: rgba(255, 255, 255, 0.62);
    font-size: 13px;
  }

  .empty-state p {
    margin: 0 0 8px;
  }

  .hint code {
    font-family: "Cascadia Mono", "Consolas", monospace;
    font-size: 12px;
  }
</style>
