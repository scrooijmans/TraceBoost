<svelte:options runes={true} />

<script lang="ts">
  import type { WorkspacePipelineEntry } from "@traceboost/seis-contracts";

  let {
    pipelines,
    activePipelineId,
    onSelect,
    onCreate,
    onDuplicate,
    onRemove,
    getLabel,
    canRemove
  }: {
    pipelines: WorkspacePipelineEntry[];
    activePipelineId: string | null;
    onSelect: (pipelineId: string) => void;
    onCreate: () => void;
    onDuplicate: () => void;
    onRemove: () => void;
    getLabel: (entry: WorkspacePipelineEntry, index: number) => string;
    canRemove: boolean;
  } = $props();
</script>

<section class="session-panel">
  <header class="panel-header">
    <div>
      <h3>Session Pipelines</h3>
      <p>{pipelines.length} pipeline{pipelines.length === 1 ? "" : "s"} in this dataset session</p>
    </div>
    <div class="action-row">
      <button class="chip" onclick={onCreate}>+ New</button>
      <button class="chip" onclick={onDuplicate} disabled={!activePipelineId}>Duplicate</button>
    </div>
  </header>

  <div class="pipeline-list">
    {#each pipelines as entry, index (entry.pipeline_id)}
      <button
        class:selected={entry.pipeline_id === activePipelineId}
        class="pipeline-row"
        onclick={() => onSelect(entry.pipeline_id)}
      >
        <span class="pipeline-index">{index + 1}</span>
        <span class="pipeline-copy">
          <strong>{getLabel(entry, index)}</strong>
          <small>{entry.pipeline.operations.length} step{entry.pipeline.operations.length === 1 ? "" : "s"}</small>
        </span>
      </button>
    {/each}
  </div>

  <div class="panel-footer">
    <button class="chip danger" onclick={onRemove} disabled={!canRemove}>
      Remove Active
    </button>
  </div>
</section>

<style>
  .session-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    overflow: hidden;
  }

  .panel-header,
  .panel-footer {
    padding: 14px 16px;
  }

  .panel-header {
    display: flex;
    flex-direction: column;
    gap: 12px;
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

  .action-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .pipeline-list {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow: auto;
    min-height: 0;
    flex: 1;
  }

  .pipeline-row {
    width: 100%;
    display: grid;
    grid-template-columns: 30px 1fr;
    gap: 10px;
    align-items: center;
    text-align: left;
    border: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
    color: inherit;
    border-radius: 12px;
    padding: 12px;
    cursor: pointer;
  }

  .pipeline-row.selected {
    border-color: rgba(74, 222, 128, 0.6);
    background: rgba(74, 222, 128, 0.08);
  }

  .pipeline-index {
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.08);
    font-size: 11px;
  }

  .pipeline-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .pipeline-copy strong,
  .pipeline-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pipeline-copy strong {
    font-size: 13px;
  }

  .pipeline-copy small {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
  }

  .panel-footer {
    border-top: 1px solid rgba(255, 255, 255, 0.08);
  }

  .chip {
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.04);
    color: inherit;
    border-radius: 999px;
    padding: 8px 10px;
    cursor: pointer;
  }

  .chip.danger {
    border-color: rgba(255, 120, 120, 0.35);
    color: #ff9a9a;
  }

  .chip:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
