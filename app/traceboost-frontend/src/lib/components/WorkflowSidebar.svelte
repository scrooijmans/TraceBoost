<svelte:options runes={true} />

<script lang="ts">
  import { getViewerModelContext } from "../viewer-model.svelte";

  interface Props {
    showSidebar: boolean;
    hideSidebar: () => void;
  }

  let { showSidebar, hideSidebar }: Props = $props();

  const viewerModel = getViewerModelContext();

  function basename(filePath: string): string {
    return filePath.split(/[\\/]/).pop() ?? filePath;
  }

  function fileStem(filePath: string | null | undefined): string {
    const filename = basename(filePath ?? "");
    return filename.replace(/\.[^.]+$/, "");
  }

  function stripGeneratedHashSuffix(value: string): string {
    return value.replace(/-[0-9a-f]{16}$/i, "");
  }

  function datasetLabel(displayName: string, fallbackPath: string | null | undefined, entryId: string): string {
    const preferredPathLabel = fileStem(fallbackPath);
    if (preferredPathLabel) {
      return stripGeneratedHashSuffix(preferredPathLabel);
    }

    const trimmedDisplayName = displayName.trim();
    if (trimmedDisplayName) {
      return stripGeneratedHashSuffix(trimmedDisplayName);
    }

    return entryId;
  }
</script>

<aside class:hidden={!showSidebar} class="sidebar">
  <div class="sidebar-header">
    <div class="logo-row">
      <svg
        class="logo-icon"
        viewBox="0 0 24 24"
        width="32"
        height="32"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
      >
        <path
          d="M3 20 L6 8 L9 14 L12 4 L15 16 L18 10 L21 20"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
      <div class="logo-copy">
        <h1>TraceBoost <span class="version">v0.1.0</span></h1>
        <p class="subtitle">Seismic Volumes</p>
      </div>
      <button class="collapse-button" onclick={hideSidebar} aria-label="Hide sidebar">
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
    </div>
  </div>

  <div class="volume-list-shell">
    {#if viewerModel.workspaceEntries.length}
      <div class="volume-list">
        {#each viewerModel.workspaceEntries as entry (entry.entry_id)}
          {@const visibleLabel = datasetLabel(
            entry.display_name,
            entry.source_path ?? entry.imported_store_path ?? entry.preferred_store_path,
            entry.entry_id
          )}
          <div class="volume-row">
            <button
              class:active={viewerModel.activeEntryId === entry.entry_id}
              class="volume-entry"
              onclick={() => void viewerModel.activateDatasetEntry(entry.entry_id)}
              disabled={viewerModel.loading}
              title={visibleLabel}
            >
              <span class="volume-entry-label">
                {visibleLabel}
              </span>
            </button>
            <button
              class="volume-remove"
              onclick={() => void viewerModel.removeWorkspaceEntry(entry.entry_id)}
              disabled={viewerModel.loading}
              aria-label={`Remove ${visibleLabel}`}
              title={`Remove ${visibleLabel}`}
            >
              ×
            </button>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-state">
        <span class="empty-title">No volumes loaded</span>
        <p>Use <strong>File &gt; Open Volume…</strong> to open a `.tbvol` or import a `.segy`.</p>
      </div>
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    background: #181818;
    border-right: 1px solid #242424;
  }

  .sidebar.hidden {
    display: none;
  }

  .sidebar-header {
    padding: 10px 10px 8px;
    border-bottom: 1px solid #242424;
  }

  .logo-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
  }

  .logo-icon {
    color: #67c48f;
  }

  .logo-copy h1 {
    margin: 0;
    font-size: 18px;
    font-weight: 650;
    color: #d7d7d7;
  }

  .version {
    font-size: 11px;
    color: #6c6c6c;
    font-weight: 500;
  }

  .subtitle {
    margin: 2px 0 0;
    font-size: 11px;
    color: #6f6f6f;
  }

  .collapse-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 2px;
    border: 1px solid #303030;
    background: #202020;
    color: #777;
    cursor: pointer;
  }

  .collapse-button:hover {
    background: #282828;
    color: #d0d0d0;
  }

  .volume-list-shell {
    min-height: 0;
    overflow: auto;
    padding: 10px;
  }

  .volume-list {
    display: grid;
    gap: 6px;
  }

  .volume-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
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

  .volume-entry-label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
    font-weight: 600;
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
  }

  .volume-remove:hover:not(:disabled) {
    border-color: #733838;
    background: #2a1b1b;
    color: #f08f8f;
  }

  .empty-state {
    border: 1px dashed #2d2d2d;
    background: #1c1c1c;
    padding: 14px;
    color: #828282;
  }

  .empty-title {
    display: block;
    margin-bottom: 6px;
    font-size: 12px;
    font-weight: 650;
    color: #c4c4c4;
  }

  .empty-state p {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
  }
</style>
