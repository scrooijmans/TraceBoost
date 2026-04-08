<svelte:options runes={true} />

<script lang="ts">
  import type { WorkspacePipelineEntry } from "@traceboost/seis-contracts";

  let {
    pipelines,
    activePipelineId,
    onSelect,
    onCreate,
    onDuplicate,
    onCopy,
    onPaste,
    onRemove,
    onRemoveItem,
    getLabel,
    canRemove
  }: {
    pipelines: WorkspacePipelineEntry[];
    activePipelineId: string | null;
    onSelect: (pipelineId: string) => void;
    onCreate: () => void;
    onDuplicate: () => void;
    onCopy: () => void;
    onPaste: () => void;
    onRemove: () => void;
    onRemoveItem: (pipelineId: string) => void;
    getLabel: (entry: WorkspacePipelineEntry, index: number) => string;
    canRemove: boolean;
  } = $props();

  function handleKeyDown(event: KeyboardEvent): void {
    if (!(event.ctrlKey || event.metaKey)) {
      return;
    }

    const key = event.key.toLowerCase();
    if (key === "c" && activePipelineId) {
      event.preventDefault();
      onCopy();
    }

    if (key === "v") {
      event.preventDefault();
      onPaste();
    }
  }
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

  <div class="pipeline-list" role="listbox" tabindex="0" onkeydown={handleKeyDown} aria-label="Session pipelines">
    {#each pipelines as entry, index (entry.pipeline_id)}
      {@const selected = entry.pipeline_id === activePipelineId}
      {@const label = getLabel(entry, index)}
      <div class="pipeline-row-shell">
        <button
          class:selected={selected}
          class="pipeline-row"
          onclick={() => onSelect(entry.pipeline_id)}
        >
          <span class="pipeline-index">{index + 1}</span>
          <span class="pipeline-copy">
            <strong>{label}</strong>
            <small>{entry.pipeline.operations.length} step{entry.pipeline.operations.length === 1 ? "" : "s"}</small>
          </span>
        </button>
        <button
          class="pipeline-remove"
          onclick={(event) => {
            event.stopPropagation();
            onRemoveItem(entry.pipeline_id);
          }}
          disabled={!canRemove}
          aria-label={`Remove ${label}`}
          title={`Remove ${label}`}
        >
          X
        </button>
      </div>
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
    background: #1a1a1a;
    border: 1px solid #2a2a2a;
    overflow: hidden;
  }

  .panel-header,
  .panel-footer {
    padding: 8px 10px;
  }

  .panel-header {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-bottom: 1px solid #242424;
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

  .action-row {
    display: flex;
    gap: 5px;
    flex-wrap: wrap;
  }

  .pipeline-list {
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow: auto;
    min-height: 0;
    flex: 1;
    outline: none;
  }

  .pipeline-row-shell {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
  }

  .pipeline-row {
    width: 100%;
    display: grid;
    grid-template-columns: 22px 1fr;
    gap: 8px;
    align-items: center;
    text-align: left;
    border: 1px solid #2a2a2a;
    background: #1e1e1e;
    color: inherit;
    padding: 7px 8px;
    cursor: pointer;
  }

  .pipeline-row:hover {
    background: #252525;
  }

  .pipeline-row.selected {
    border-color: rgba(74, 222, 128, 0.4);
    background: rgba(74, 222, 128, 0.06);
  }

  .pipeline-remove {
    width: 28px;
    border-radius: 2px;
    border: 1px solid #2c2c2c;
    background: #1b1b1b;
    color: #6f6f6f;
    cursor: pointer;
    opacity: 0;
    pointer-events: none;
    transition:
      opacity 120ms ease,
      border-color 120ms ease,
      background 120ms ease,
      color 120ms ease;
  }

  .pipeline-row-shell:hover .pipeline-remove,
  .pipeline-row:focus-visible + .pipeline-remove,
  .pipeline-remove:focus-visible {
    opacity: 1;
    pointer-events: auto;
  }

  .pipeline-remove:hover:not(:disabled) {
    border-color: #733838;
    background: #2a1b1b;
    color: #f08f8f;
  }

  .pipeline-remove:disabled {
    cursor: not-allowed;
    opacity: 0.28;
  }

  .pipeline-index {
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

  .pipeline-copy {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .pipeline-copy strong,
  .pipeline-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pipeline-copy strong {
    font-size: 12px;
    color: #c0c0c0;
  }

  .pipeline-copy small {
    font-size: 11px;
    color: #666;
  }

  .panel-footer {
    border-top: 1px solid #242424;
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
</style>
