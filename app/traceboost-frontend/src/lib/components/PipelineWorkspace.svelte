<svelte:options runes={true} />

<script lang="ts">
  import PipelineOperatorEditor from "./PipelineOperatorEditor.svelte";
  import PipelineSequenceList from "./PipelineSequenceList.svelte";
  import PipelineSessionList from "./PipelineSessionList.svelte";
  import { getProcessingModelContext } from "../processing-model.svelte";
  import { getViewerModelContext } from "../viewer-model.svelte";

  const viewerModel = getViewerModelContext();
  const processingModel = getProcessingModelContext();
</script>

<svelte:window onkeydown={(event) => void processingModel.handleKeydown(event)} />

<div class="workspace-shell">
  <div class="workspace-header">
    <div>
      <span class="eyebrow">Processing Workspace</span>
      <h2>{processingModel.pipelineTitle}</h2>
      <p>
        {viewerModel.dataset
          ? `Working on ${viewerModel.dataset.descriptor.label} at ${viewerModel.axis}:${viewerModel.index}`
          : "Open a runtime store to preview processing on the current section."}
      </p>
    </div>

    <div class="shortcut-card">
      <span>Shortcuts</span>
      <p><code>a</code> add scalar, <code>n</code> add normalize, <code>p</code> preview, <code>r</code> run volume</p>
    </div>
  </div>

  <div class="workspace-grid">
    <PipelineSessionList
      pipelines={processingModel.sessionPipelineItems}
      activePipelineId={processingModel.activeSessionPipelineId}
      onSelect={processingModel.activateSessionPipeline}
      onCreate={processingModel.createSessionPipeline}
      onDuplicate={processingModel.duplicateActiveSessionPipeline}
      onRemove={processingModel.removeActiveSessionPipeline}
      getLabel={processingModel.sessionPipelineLabel}
      canRemove={processingModel.canRemoveSessionPipeline}
    />

    <PipelineSequenceList
      pipeline={processingModel.pipeline}
      selectedIndex={processingModel.selectedStepIndex}
      onSelect={processingModel.selectStep}
      onAddAmplitudeScalar={processingModel.addAmplitudeScalarAfterSelected}
      onAddTraceNormalize={processingModel.addTraceRmsNormalizeAfterSelected}
    />

    <PipelineOperatorEditor
      pipeline={processingModel.pipeline}
      selectedOperation={processingModel.selectedOperation}
      previewState={processingModel.previewState}
      previewLabel={processingModel.previewLabel}
      activeJob={processingModel.activeJob}
      presets={processingModel.presets}
      loadingPresets={processingModel.loadingPresets}
      canPreview={processingModel.canPreview}
      canRun={processingModel.canRun}
      previewBusy={processingModel.previewBusy}
      runBusy={processingModel.runBusy}
      processingError={processingModel.error}
      onSetPipelineName={processingModel.setPipelineName}
      onSetAmplitudeScalarFactor={processingModel.setSelectedAmplitudeScalarFactor}
      onMoveUp={processingModel.moveSelectedUp}
      onMoveDown={processingModel.moveSelectedDown}
      onRemove={processingModel.removeSelected}
      onPreview={() => processingModel.previewCurrentSection()}
      onShowRaw={processingModel.showRawSection}
      onRun={() => processingModel.runOnVolume()}
      onCancelJob={() => processingModel.cancelActiveJob()}
      onLoadPreset={processingModel.loadPreset}
      onSavePreset={() => processingModel.savePreset()}
      onDeletePreset={(presetId) => processingModel.deletePreset(presetId)}
    />
  </div>
</div>

<style>
  .workspace-shell {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
    padding: 18px 20px 12px;
    outline: none;
  }

  .workspace-header {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: flex-start;
  }

  .eyebrow {
    display: inline-block;
    margin-bottom: 6px;
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 700;
  }

  .workspace-header p {
    margin: 6px 0 0;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.58);
  }

  .shortcut-card {
    flex-shrink: 0;
    min-width: 260px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.03);
    padding: 12px 14px;
  }

  .shortcut-card span {
    display: block;
    font-size: 11px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.46);
    margin-bottom: 6px;
  }

  .shortcut-card p {
    margin: 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.68);
  }

  code {
    font-family: "Cascadia Mono", "Consolas", monospace;
  }

  .workspace-grid {
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(220px, 0.7fr) minmax(280px, 0.95fr) minmax(340px, 1.1fr);
    gap: 14px;
    flex: 1;
  }

  @media (max-width: 1100px) {
    .workspace-header {
      flex-direction: column;
    }

    .shortcut-card {
      min-width: 0;
      width: 100%;
    }

    .workspace-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
