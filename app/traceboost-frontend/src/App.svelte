<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { SeismicSectionChart } from "@geoviz/svelte";
  import type { TraceBoostViewerState } from "./lib/viewer-store";
  import { viewerStore } from "./lib/viewer-store";

  let state: TraceBoostViewerState = {
    axis: "inline",
    index: 0,
    section: null,
    loading: false,
    error: null,
    resetToken: "inline:0",
    displayTransform: {
      renderMode: "heatmap",
      colormap: "grayscale",
      gain: 1,
      polarity: "normal"
    },
    lastProbe: null,
    lastViewport: null,
    lastInteraction: null
  };

  const unsubscribe = viewerStore.subscribe((value) => {
    state = value;
  });

  onMount(() => {
    viewerStore.load("inline", 0);
  });

  onDestroy(() => {
    unsubscribe();
  });
</script>

<svelte:head>
  <title>TraceBoost Frontend Host</title>
</svelte:head>

<div class="shell">
  <aside class="sidebar">
    <h1>TraceBoost</h1>
    <p>First frontend host consuming external `geoviz` through generated contracts.</p>

    <div class="controls">
      <label>
        Axis
        <select
          bind:value={state.axis}
          on:change={() => viewerStore.load(state.axis as "inline" | "xline", state.index)}
        >
          <option value="inline">Inline</option>
          <option value="xline">Xline</option>
        </select>
      </label>

      <label>
        Index
        <input
          type="number"
          bind:value={state.index}
          min="0"
          on:change={() => viewerStore.load(state.axis as "inline" | "xline", Number(state.index))}
        />
      </label>

      <button
        on:click={() =>
          viewerStore.setRenderMode(
            state.displayTransform.renderMode === "heatmap" ? "wiggle" : "heatmap"
          )}
      >
        {state.displayTransform.renderMode === "heatmap" ? "Switch To Wiggles" : "Switch To Heatmap"}
      </button>

      <button
        on:click={() =>
          viewerStore.setColormap(
            state.displayTransform.colormap === "grayscale" ? "red-white-blue" : "grayscale"
          )}
      >
        {state.displayTransform.colormap === "grayscale" ? "Switch To Red/White/Blue" : "Switch To Grayscale"}
      </button>
    </div>

    <div class="readout">
      {#if state.lastProbe?.probe}
        <div>Trace: {state.lastProbe.probe.trace_index}</div>
        <div>Sample: {state.lastProbe.probe.sample_index}</div>
        <div>Amplitude: {state.lastProbe.probe.amplitude.toFixed(4)}</div>
      {:else}
        <div>Move over the seismic chart.</div>
      {/if}
    </div>

    {#if state.lastViewport}
      <div class="readout">
        <div>Viewport traces: {state.lastViewport.viewport.trace_start} - {state.lastViewport.viewport.trace_end}</div>
        <div>Viewport samples: {state.lastViewport.viewport.sample_start} - {state.lastViewport.viewport.sample_end}</div>
      </div>
    {/if}

    {#if state.error}
      <div class="error">{state.error}</div>
    {/if}
  </aside>

  <main class="viewer-shell">
    <SeismicSectionChart
      chartId="traceboost-main"
      viewId={`${state.axis}:${state.index}`}
      section={state.section}
      displayTransform={state.displayTransform}
      loading={state.loading}
      errorMessage={state.error}
      resetToken={state.resetToken}
      on:probeChange={(event) => viewerStore.setProbe(event.detail)}
      on:viewportChange={(event) => viewerStore.setViewport(event.detail)}
      on:interactionChange={(event) => viewerStore.setInteraction(event.detail)}
    />
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: "Segoe UI", system-ui, sans-serif;
    background: #07151f;
    color: #e8edf0;
  }

  .shell {
    display: grid;
    grid-template-columns: 320px 1fr;
    min-height: 100vh;
  }

  .sidebar {
    padding: 20px;
    background: #0c1f2d;
    border-right: 1px solid rgba(255, 255, 255, 0.08);
  }

  .controls {
    display: grid;
    gap: 12px;
    margin: 20px 0;
  }

  .controls label {
    display: grid;
    gap: 6px;
    font-size: 13px;
  }

  .controls button,
  .controls select,
  .controls input {
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: #102838;
    color: inherit;
  }

  .viewer-shell {
    padding: 20px;
    min-height: 100vh;
  }

  .viewer-shell :global(.geoviz-svelte-chart-shell) {
    height: calc(100vh - 40px);
    border-radius: 16px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.08);
  }

  .readout {
    margin-top: 16px;
    padding: 12px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.04);
    font-size: 13px;
    line-height: 1.5;
  }

  .error {
    margin-top: 16px;
    color: #ffb0b0;
  }
</style>
