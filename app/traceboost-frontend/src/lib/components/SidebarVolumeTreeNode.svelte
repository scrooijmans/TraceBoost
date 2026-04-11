<svelte:options runes={true} />

<script lang="ts" module>
  export type VolumeTreeBadge = "source" | "final" | "checkpoint";

  export interface VolumeTreeNodeView {
    entryId: string;
    label: string;
    subtitle: string | null;
    badge: VolumeTreeBadge;
    children: VolumeTreeNodeView[];
  }
</script>

<script lang="ts">
  import SidebarVolumeTreeNode from "./SidebarVolumeTreeNode.svelte";

  interface Props {
    node: VolumeTreeNodeView;
    activeEntryId: string | null;
    loading: boolean;
    depth: number;
    isExpanded: (entryId: string) => boolean;
    toggleExpanded: (entryId: string) => void;
    activateEntry: (entryId: string) => void;
    removeEntry: (entryId: string) => void;
  }

  let { node, activeEntryId, loading, depth, isExpanded, toggleExpanded, activateEntry, removeEntry }: Props =
    $props();

  const hasChildren = $derived(node.children.length > 0);
  const expanded = $derived(hasChildren ? isExpanded(node.entryId) : false);

  function badgeLabel(badge: VolumeTreeBadge): string {
    switch (badge) {
      case "checkpoint":
        return "checkpoint";
      case "final":
        return "final";
      default:
        return "source";
    }
  }
</script>

<div
  class="tree-node"
  role="treeitem"
  aria-expanded={hasChildren ? expanded : undefined}
  aria-selected={activeEntryId === node.entryId}
>
  <div class="volume-row" style={`--depth:${depth}`}>
    <div class="volume-entry-shell">
      {#if hasChildren}
        <button
          class="tree-toggle"
          type="button"
          onclick={() => toggleExpanded(node.entryId)}
          aria-label={`${expanded ? "Collapse" : "Expand"} ${node.label}`}
          aria-expanded={expanded}
        >
          <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.7">
            {#if expanded}
              <polyline points="3 10 8 5 13 10" stroke-linecap="round" stroke-linejoin="round" />
            {:else}
              <polyline points="5 3 10 8 5 13" stroke-linecap="round" stroke-linejoin="round" />
            {/if}
          </svg>
        </button>
      {:else}
        <span class="tree-spacer" aria-hidden="true"></span>
      {/if}

      <button
        class:active={activeEntryId === node.entryId}
        class="volume-entry"
        type="button"
        onclick={() => activateEntry(node.entryId)}
        disabled={loading}
        title={node.subtitle ? `${node.label}\n${node.subtitle}` : node.label}
      >
        <span class="volume-entry-copy">
          <span class="volume-entry-head">
            <span class="volume-entry-label">{node.label}</span>
            <span class={`volume-badge ${node.badge}`}>{badgeLabel(node.badge)}</span>
          </span>
          {#if node.subtitle}
            <span class="volume-entry-subtitle">{node.subtitle}</span>
          {/if}
        </span>
      </button>
    </div>

    <button
      class="volume-remove"
      type="button"
      onclick={() => removeEntry(node.entryId)}
      disabled={loading}
      aria-label={`Remove ${node.label}`}
      title={`Remove ${node.label}`}
    >
      X
    </button>
  </div>

  {#if hasChildren && expanded}
    <div class="tree-children" role="group">
      {#each node.children as child (child.entryId)}
        <SidebarVolumeTreeNode
          node={child}
          activeEntryId={activeEntryId}
          loading={loading}
          depth={depth + 1}
          {isExpanded}
          {toggleExpanded}
          {activateEntry}
          {removeEntry}
        />
      {/each}
    </div>
  {/if}
</div>

<style>
  .tree-node {
    display: grid;
    gap: 6px;
  }

  .tree-children {
    display: grid;
    gap: 6px;
  }

  .volume-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
  }

  .volume-entry-shell {
    min-width: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 6px;
    align-items: stretch;
    padding-left: calc(var(--depth) * 14px);
  }

  .tree-toggle,
  .tree-spacer {
    width: 22px;
    min-width: 22px;
    height: 22px;
    margin-top: 8px;
  }

  .tree-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid #2a2a2a;
    background: #171717;
    color: #838383;
    cursor: pointer;
  }

  .tree-toggle:hover {
    border-color: #3a3a3a;
    background: #202020;
    color: #d8d8d8;
  }

  .volume-entry {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid #2b2b2b;
    background: #1d1d1d;
    color: #a9a9a9;
    text-align: left;
    cursor: pointer;
  }

  .volume-entry:hover:not(:disabled) {
    border-color: #3b3b3b;
    background: #242424;
    color: #dddddd;
  }

  .volume-entry.active {
    border-color: rgba(103, 196, 143, 0.45);
    background: rgba(33, 60, 44, 0.72);
    color: #f2fff7;
  }

  .volume-entry:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .volume-entry-copy {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .volume-entry-head {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .volume-entry-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-weight: 600;
  }

  .volume-entry-subtitle {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    color: #7d7d7d;
  }

  .volume-badge {
    flex: 0 0 auto;
    padding: 1px 5px;
    border-radius: 999px;
    border: 1px solid #3a3a3a;
    font-size: 9px;
    line-height: 1.4;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #c2c2c2;
    background: #232323;
  }

  .volume-badge.source {
    border-color: #35506c;
    background: rgba(42, 63, 84, 0.7);
    color: #cfe4ff;
  }

  .volume-badge.final {
    border-color: #35503f;
    background: rgba(42, 74, 56, 0.72);
    color: #d5f7dd;
  }

  .volume-badge.checkpoint {
    border-color: #6a5430;
    background: rgba(96, 73, 36, 0.72);
    color: #ffe4aa;
  }

  .volume-remove {
    width: 28px;
    height: 28px;
    margin-top: 4px;
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

  .volume-row:hover .volume-remove,
  .volume-row:focus-within .volume-remove {
    opacity: 1;
    pointer-events: auto;
  }

  .volume-remove:hover:not(:disabled) {
    border-color: #733838;
    background: #2a1b1b;
    color: #f08f8f;
  }
</style>
