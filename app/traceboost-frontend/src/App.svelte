<svelte:options runes={true} />

<script lang="ts">
  import { onMount } from "svelte";
  import WorkflowSidebar from "./lib/components/WorkflowSidebar.svelte";
  import ViewerMain from "./lib/components/ViewerMain.svelte";
  import { isTauriEnvironment } from "./lib/bridge";
  import { pickVolumeFile } from "./lib/file-dialog";
  import { ProcessingModel, setProcessingModelContext } from "./lib/processing-model.svelte";
  import { setViewerModelContext, ViewerModel } from "./lib/viewer-model.svelte";

  let showSidebar = $state(true);
  let viewerChart = $state.raw<{ fitToData?: () => void } | null>(null);

  const viewerModel = setViewerModelContext(new ViewerModel({ tauriRuntime: isTauriEnvironment() }));
  const processingModel = setProcessingModelContext(new ProcessingModel({ viewerModel }));

  function hideSidebar(): void {
    showSidebar = false;
  }

  function showSidebarPanel(): void {
    showSidebar = true;
  }

  async function handleNativeOpenVolumeMenu(): Promise<void> {
    showSidebarPanel();
    const path = await pickVolumeFile();

    if (path) {
      await viewerModel.openVolumePath(path);
      return;
    }

    viewerModel.note("Volume selection did not produce a usable path.", "ui", "warn");
  }

  onMount(() => {
    let disposed = false;
    let disposeNativeMenu = () => {};

    if (viewerModel.tauriRuntime) {
      void (async () => {
        const { listen } = await import("@tauri-apps/api/event");
        const unlisten = await listen("menu:file-open-volume", () => {
          void handleNativeOpenVolumeMenu();
        });

        if (disposed) {
          unlisten();
          return;
        }

        disposeNativeMenu = unlisten;
      })();
    }

    const disposeViewer = viewerModel.mountShell();
    const disposeProcessing = processingModel.mount();
    return () => {
      disposed = true;
      disposeNativeMenu();
      disposeProcessing();
      disposeViewer();
    };
  });
</script>

<svelte:head>
  <title>TraceBoost</title>
</svelte:head>

<div class:sidebar-hidden={!showSidebar} class="shell">
  <WorkflowSidebar {showSidebar} {hideSidebar} />
  <ViewerMain {showSidebar} {showSidebarPanel} bind:chartRef={viewerChart} />
</div>

<style>
  :global(body) {
    margin: 0;
    font-family: "Segoe UI", system-ui, -apple-system, sans-serif;
    background: #141414;
    color: #d0d0d0;
    font-size: 12px;
    -webkit-font-smoothing: antialiased;
  }

  .shell {
    display: grid;
    grid-template-columns: 280px 1fr;
    min-height: 100vh;
  }

  .shell.sidebar-hidden {
    grid-template-columns: 1fr;
  }
</style>
