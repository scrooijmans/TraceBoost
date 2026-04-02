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
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 10px;
    border-bottom: 1px solid #242424;
    align-items: center;
  }

  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: #c0c0c0;
  }

  .panel-header p {
    margin: 0;
    font-size: 11px;
    color: #666;
  }

  .quick-add {
    display: flex;
    gap: 5px;
    align-items: center;
    flex-shrink: 0;
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

  .chip:hover {
    background: #2e2e2e;
    color: #d0d0d0;
  }

  .sequence-list {
    list-style: none;
    margin: 0;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow: auto;
  }

  li {
    margin: 0;
  }

  .sequence-row {
    width: 100%;
    display: grid;
    grid-template-columns: 22px 1fr;
    gap: 8px;
    align-items: center;
    border: 1px solid #2a2a2a;
    background: #1e1e1e;
    color: inherit;
    text-align: left;
    padding: 7px 8px;
    cursor: pointer;
  }

  .sequence-row:hover {
    background: #252525;
  }

  .sequence-row.selected {
    border-color: rgba(74, 222, 128, 0.4);
    background: rgba(74, 222, 128, 0.06);
  }

  .step-index {
    width: 20px;
    height: 20px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 2px;
    background: #2a2a2a;
    font-size: 10px;
    color: #888;
    flex-shrink: 0;
  }

  .step-copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .step-copy strong {
    font-size: 12px;
    font-weight: 500;
    color: #c0c0c0;
  }

  .empty-state {
    padding: 14px 10px;
    color: #777;
    font-size: 12px;
  }

  .empty-state p {
    margin: 0 0 5px;
  }

  .hint code {
    font-family: "Cascadia Mono", "Consolas", monospace;
    font-size: 11px;
  }
</style>
