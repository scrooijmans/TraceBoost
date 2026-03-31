<svelte:options runes={true} />

<script lang="ts">
  import { SeismicSectionChart } from "@geoviz/svelte";
  import { getViewerModelContext } from "../viewer-model.svelte";

  let {
    showSidebar,
    showSidebarPanel,
    chartRef = $bindable<{ fitToData?: () => void } | null>(null)
  }: {
    showSidebar: boolean;
    showSidebarPanel: () => void;
    chartRef?: { fitToData?: () => void } | null;
  } = $props();

  const viewerModel = getViewerModelContext();
</script>

{#if !showSidebar}
  <button class="sidebar-toggle" onclick={showSidebarPanel} aria-label="Show sidebar">
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="2">
      <polyline points="9 18 15 12 9 6" />
    </svg>
  </button>
{/if}

<main class="viewer-shell">
  {#if viewerModel.section}
    <SeismicSectionChart
      bind:this={chartRef}
      chartId="traceboost-main"
      viewId={`${viewerModel.axis}:${viewerModel.index}`}
      section={viewerModel.section}
      displayTransform={viewerModel.displayTransform}
      loading={viewerModel.loading}
      errorMessage={viewerModel.error}
      resetToken={viewerModel.resetToken}
      onProbeChange={viewerModel.setProbe}
      onViewportChange={viewerModel.setViewport}
      onInteractionChange={viewerModel.setInteraction}
    />
  {:else}
    <div class="welcome-card">
      <svg
        class="welcome-icon"
        viewBox="0 0 24 24"
        width="64"
        height="64"
        fill="none"
        stroke="currentColor"
        stroke-width="1"
      >
        <path
          d="M3 20 L6 8 L9 14 L12 4 L15 16 L18 10 L21 20"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
        <line x1="3" y1="20" x2="21" y2="20" />
      </svg>
      <h2>Select a SEG-Y File</h2>
      <p>
        Use the sidebar to select a SEG-Y file, run a preflight check, set an output folder, then
        import or open a runtime store to view seismic sections.
      </p>
      <span class="welcome-version">TraceBoost v0.1.0</span>
    </div>
  {/if}
</main>

<style>
  .sidebar-toggle {
    position: fixed;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    z-index: 10;
    background: #0c1f2d;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-left: none;
    border-radius: 0 8px 8px 0;
    padding: 12px 6px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
  }

  .sidebar-toggle:hover {
    color: #fff;
    background: #1a3a50;
  }

  .viewer-shell {
    padding: 20px;
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .viewer-shell :global(.geoviz-svelte-chart-shell) {
    height: calc(100vh - 40px);
    width: 100%;
    border-radius: 16px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .welcome-card {
    text-align: center;
    max-width: 420px;
    padding: 48px 40px;
    background: #0c1f2d;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 20px;
  }

  .welcome-icon {
    color: rgba(255, 255, 255, 0.15);
    margin-bottom: 20px;
  }

  h2 {
    margin: 0 0 12px;
    font-size: 22px;
    font-weight: 600;
  }

  p {
    margin: 0 0 24px;
    font-size: 14px;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.5);
  }

  .welcome-version {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.25);
  }
</style>
