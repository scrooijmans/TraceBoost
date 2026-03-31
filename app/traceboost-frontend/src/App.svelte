<svelte:options runes={true} />

<script lang="ts">
  import { onMount } from "svelte";
  import WorkflowSidebar from "./lib/components/WorkflowSidebar.svelte";
  import ViewerMain from "./lib/components/ViewerMain.svelte";
  import { isTauriEnvironment } from "./lib/bridge";
  import { setViewerModelContext, ViewerModel } from "./lib/viewer-model.svelte";

  let showSidebar = $state(true);
  let viewerChart = $state.raw<{ fitToData?: () => void } | null>(null);

  const viewerModel = setViewerModelContext(new ViewerModel({ tauriRuntime: isTauriEnvironment() }));

  function hideSidebar(): void {
    showSidebar = false;
  }

  function showSidebarPanel(): void {
    showSidebar = true;
  }

  onMount(viewerModel.mountShell);
</script>

<svelte:head>
  <title>TraceBoost</title>
</svelte:head>

<div class:sidebar-hidden={!showSidebar} class="shell">
  <WorkflowSidebar {showSidebar} {hideSidebar} chartBound={Boolean(viewerChart)} />
  <ViewerMain {showSidebar} {showSidebarPanel} bind:chartRef={viewerChart} />
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
    background: #07151f;
    color: #e8edf0;
    -webkit-font-smoothing: antialiased;
  }

  .shell {
    display: grid;
    grid-template-columns: 320px 1fr;
    min-height: 100vh;
  }

  .shell.sidebar-hidden {
    grid-template-columns: 1fr;
  }
</style>
