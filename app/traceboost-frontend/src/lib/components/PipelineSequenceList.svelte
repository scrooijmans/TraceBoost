<svelte:options runes={true} />

<script lang="ts">
  import type { ProcessingOperation, ProcessingPipeline } from "@traceboost/seis-contracts";
  import type { OperatorCatalogId } from "../processing-model.svelte";
  import { describeOperation, operatorCatalogItems } from "../processing-model.svelte";

  let {
    pipeline,
    selectedIndex,
    onSelect,
    onInsertOperator
  }: {
    pipeline: ProcessingPipeline;
    selectedIndex: number;
    onSelect: (index: number) => void;
    onInsertOperator: (operatorId: OperatorCatalogId) => void;
  } = $props();

  let query = $state("");
  let searchFocused = $state(false);
  let activeResultIndex = $state(0);
  let searchInput: HTMLInputElement | null = null;

  const normalizedQuery = $derived(query.trim().toLowerCase());
  const filteredCatalog = $derived(
    operatorCatalogItems.filter((item) => {
      if (!normalizedQuery) {
        return true;
      }
      const haystack = [item.label, item.description, ...item.keywords, item.shortcut].join(" ").toLowerCase();
      return haystack.includes(normalizedQuery);
    })
  );
  const showCatalog = $derived(searchFocused || normalizedQuery.length > 0);

  function summary(operation: ProcessingOperation): string {
    return describeOperation(operation);
  }

  function focusSearch(): void {
    searchInput?.focus();
    searchInput?.select();
  }

  function resetSearch(): void {
    query = "";
    activeResultIndex = 0;
  }

  function insertOperator(operatorId: OperatorCatalogId): void {
    onInsertOperator(operatorId);
    resetSearch();
    focusSearch();
  }

  function handleSearchKeydown(event: KeyboardEvent): void {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      if (filteredCatalog.length) {
        activeResultIndex = Math.min(activeResultIndex + 1, filteredCatalog.length - 1);
      }
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      activeResultIndex = Math.max(activeResultIndex - 1, 0);
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      const target = filteredCatalog[activeResultIndex] ?? filteredCatalog[0];
      if (target) {
        insertOperator(target.id);
      }
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      if (query) {
        resetSearch();
      } else {
        searchInput?.blur();
      }
    }
  }

  function handleWindowKeydown(event: KeyboardEvent): void {
    const target = event.target as HTMLElement | null;
    const tagName = target?.tagName?.toLowerCase();
    const editingText = Boolean(
      target?.isContentEditable ||
        tagName === "input" ||
        tagName === "textarea" ||
        tagName === "select"
    );

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      focusSearch();
      return;
    }

    if (editingText || event.ctrlKey || event.metaKey || event.altKey) {
      return;
    }

    if (event.key === "/") {
      event.preventDefault();
      focusSearch();
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<section class="sequence-panel">
  <header class="panel-header">
    <div>
      <h3>Pipeline</h3>
      <p>{pipeline.operations.length} step{pipeline.operations.length === 1 ? "" : "s"}</p>
    </div>
  </header>

  <div class="search-shell">
    <label class="search-label" for="pipeline-operator-search">Add Operator</label>
    <div class="search-input-shell">
      <span class="search-prompt">&gt;</span>
      <input
        bind:this={searchInput}
        id="pipeline-operator-search"
        type="text"
        placeholder="Search operators..."
        bind:value={query}
        onfocus={() => {
          searchFocused = true;
          activeResultIndex = 0;
        }}
        onblur={() => {
          searchFocused = false;
        }}
        oninput={() => {
          activeResultIndex = 0;
        }}
        onkeydown={handleSearchKeydown}
      />
    </div>
    <div class="search-meta">
      <span><code>/</code> or <code>Ctrl/Cmd+K</code> focus</span>
      <span><code>Enter</code> insert</span>
    </div>

    {#if showCatalog}
      <div class="catalog-list">
        {#if filteredCatalog.length}
          {#each filteredCatalog as item, index (item.id)}
            <button
              class:active={index === activeResultIndex}
              class="catalog-row"
              onmousedown={(event) => event.preventDefault()}
              onclick={() => insertOperator(item.id)}
              onmouseenter={() => {
                activeResultIndex = index;
              }}
            >
              <span class="catalog-copy">
                <strong>{item.label}</strong>
                <span>{item.description}</span>
              </span>
              <span class="catalog-meta">
                <kbd>{item.shortcut}</kbd>
              </span>
            </button>
          {/each}
        {:else}
          <div class="catalog-empty">No operators match “{query.trim()}”.</div>
        {/if}
      </div>
    {/if}
  </div>

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
      <p class="hint">Use the search above to add scalar, normalize, or bandpass operators.</p>
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

  .search-shell {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border-bottom: 1px solid #242424;
    background: #191919;
  }

  .search-label {
    font-size: 11px;
    color: #777;
  }

  .search-input-shell {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    border: 1px solid #333;
    background: #252525;
    padding: 8px 10px;
  }

  .search-prompt {
    color: #7d7d7d;
    font-family: "Cascadia Mono", "Consolas", monospace;
    font-size: 15px;
    font-weight: 700;
  }

  .search-input-shell input {
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: #d8d8d8;
    font: inherit;
    font-size: 13px;
  }

  .search-meta {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    color: #666;
    font-size: 10px;
  }

  .search-meta code,
  .catalog-meta kbd {
    font-family: "Cascadia Mono", "Consolas", monospace;
  }

  .catalog-list {
    border: 1px solid #2a2a2a;
    background: #171717;
    max-height: 180px;
    overflow: auto;
  }

  .catalog-row {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 8px 10px;
    border: none;
    border-bottom: 1px solid #232323;
    background: transparent;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .catalog-row:hover,
  .catalog-row.active {
    background: rgba(34, 126, 194, 0.22);
  }

  .catalog-copy {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .catalog-copy strong {
    color: #d2d8dc;
    font-size: 12px;
    font-weight: 600;
  }

  .catalog-copy span {
    color: #7f868a;
    font-size: 11px;
  }

  .catalog-meta kbd {
    border: 1px solid #3a3a3a;
    border-radius: 3px;
    padding: 2px 6px;
    background: #222;
    color: #9ba2a6;
    font-size: 10px;
  }

  .catalog-empty {
    padding: 10px;
    color: #777;
    font-size: 11px;
  }

  .sequence-list {
    list-style: none;
    margin: 0;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow: auto;
    min-height: 0;
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
</style>
