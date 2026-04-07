import { createContext } from "svelte";
import type {
  AmplitudeSpectrumRequest,
  AmplitudeSpectrumResponse,
  PreviewProcessingRequest,
  ProcessingJobStatus,
  ProcessingOperation,
  ProcessingPipeline,
  ProcessingPreset,
  RunProcessingRequest,
  SectionView,
  WorkspacePipelineEntry
} from "@traceboost/seis-contracts";
import {
  cancelProcessingJob,
  defaultProcessingStorePath,
  deletePipelinePreset,
  fetchAmplitudeSpectrum,
  getProcessingJob,
  listPipelinePresets,
  previewProcessing,
  runProcessing,
  savePipelinePreset
} from "./bridge";
import { confirmOverwriteStore, pickOutputStorePath } from "./file-dialog";
import type { ViewerModel } from "./viewer-model.svelte";

type PreviewState = "raw" | "preview" | "stale";
type SpectrumAmplitudeScale = "db" | "linear";
export type OperatorCatalogId = "amplitude_scalar" | "trace_rms_normalize" | "bandpass_filter";

interface OperatorCatalogDefinition {
  id: OperatorCatalogId;
  label: string;
  description: string;
  keywords: string[];
  shortcut: "a" | "n" | "b";
  create: (section: SectionView | null) => ProcessingOperation;
}

const OPERATOR_CATALOG: readonly OperatorCatalogDefinition[] = [
  {
    id: "amplitude_scalar",
    label: "Amplitude Scalar",
    description: "Scale trace amplitudes by a constant factor.",
    keywords: ["scalar", "scale", "gain", "amplitude"],
    shortcut: "a",
    create: () => ({ amplitude_scalar: { factor: 1 } })
  },
  {
    id: "trace_rms_normalize",
    label: "Trace RMS Normalize",
    description: "Normalize each trace to unit RMS amplitude.",
    keywords: ["normalize", "rms", "trace", "balance"],
    shortcut: "n",
    create: () => "trace_rms_normalize"
  },
  {
    id: "bandpass_filter",
    label: "Bandpass Filter",
    description: "Zero-phase FFT bandpass with cosine tapers.",
    keywords: ["bandpass", "filter", "frequency", "spectral", "highcut", "lowcut"],
    shortcut: "b",
    create: (section) => defaultBandpassFilter(section)
  }
] as const;

export interface OperatorCatalogItem {
  id: OperatorCatalogId;
  label: string;
  description: string;
  keywords: string[];
  shortcut: "a" | "n" | "b";
}

export const operatorCatalogItems: readonly OperatorCatalogItem[] = OPERATOR_CATALOG.map(
  ({ id, label, description, keywords, shortcut }) => ({
    id,
    label,
    description,
    keywords,
    shortcut
  })
);

function createEmptyPipeline(): ProcessingPipeline {
  return {
    schema_version: 1,
    revision: 1,
    preset_id: null,
    name: null,
    description: null,
    operations: []
  };
}

function pipelineName(pipeline: ProcessingPipeline): string {
  return pipeline.name?.trim() || "Untitled pipeline";
}

function sectionKey(viewerModel: ViewerModel): string {
  return `${viewerModel.activeStorePath}:${viewerModel.axis}:${viewerModel.index}`;
}

function clonePipeline(pipeline: ProcessingPipeline): ProcessingPipeline {
  return {
    schema_version: pipeline.schema_version,
    revision: pipeline.revision,
    preset_id: pipeline.preset_id,
    name: pipeline.name,
    description: pipeline.description,
    operations: pipeline.operations.map((operation) => cloneOperation(operation))
  };
}

function cloneOperation(operation: ProcessingOperation): ProcessingOperation {
  if (typeof operation === "string") {
    return operation;
  }
  if ("amplitude_scalar" in operation) {
    return { amplitude_scalar: { ...operation.amplitude_scalar } };
  }
  return {
    bandpass_filter: {
      ...operation.bandpass_filter
    }
  };
}

function normalizePresetId(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

function isExistingOutputStoreError(message: string): boolean {
  return message.toLowerCase().includes("output processing store already exists:");
}

function pipelineTimestamp(): number {
  return Math.floor(Date.now() / 1000);
}

function pipelineRunOutputSignature(pipeline: ProcessingPipeline): string {
  return JSON.stringify({
    name: pipeline.name ?? null,
    operations: pipeline.operations.map((operation) =>
      typeof operation === "string"
        ? operation
        : "amplitude_scalar" in operation
          ? { amplitude_scalar: { factor: operation.amplitude_scalar.factor } }
          : {
              bandpass_filter: {
                f1_hz: operation.bandpass_filter.f1_hz,
                f2_hz: operation.bandpass_filter.f2_hz,
                f3_hz: operation.bandpass_filter.f3_hz,
                f4_hz: operation.bandpass_filter.f4_hz,
                phase: operation.bandpass_filter.phase,
                window: operation.bandpass_filter.window
              }
            }
    )
  });
}

function defaultBandpassFilter(section: SectionView | null): ProcessingOperation {
  const sampleAxis = section?.sample_axis_f32le ?? [];
  const sampleIntervalMs =
    sampleAxis.length >= 2 ? Math.abs((sampleAxis[1] ?? 0) - (sampleAxis[0] ?? 0)) : 2;
  const safeSampleIntervalMs =
    Number.isFinite(sampleIntervalMs) && sampleIntervalMs > 0 ? sampleIntervalMs : 2;
  const nyquistHz = 500.0 / safeSampleIntervalMs;
  const f1_hz = Math.max(4, nyquistHz * 0.06);
  const f2_hz = Math.max(f1_hz + 1, nyquistHz * 0.1);
  const f4_hz = Math.min(nyquistHz, Math.max(f2_hz + 6, nyquistHz * 0.45));
  const f3_hz = Math.min(f4_hz, Math.max(f2_hz + 4, nyquistHz * 0.32));

  return {
    bandpass_filter: {
      f1_hz: Number(f1_hz.toFixed(1)),
      f2_hz: Number(f2_hz.toFixed(1)),
      f3_hz: Number(f3_hz.toFixed(1)),
      f4_hz: Number(f4_hz.toFixed(1)),
      phase: "zero",
      window: "cosine_taper"
    }
  };
}

export interface ProcessingModelOptions {
  viewerModel: ViewerModel;
}

export class ProcessingModel {
  readonly viewerModel: ViewerModel;

  pipeline = $state<ProcessingPipeline>(createEmptyPipeline());
  sessionPipelines = $state.raw<WorkspacePipelineEntry[]>([]);
  activeSessionPipelineId = $state<string | null>(null);
  selectedStepIndex = $state(0);
  editingParams = $state(false);
  previewState = $state<PreviewState>("raw");
  previewSection = $state<SectionView | null>(null);
  previewLabel = $state<string | null>(null);
  previewedSectionKey = $state<string | null>(null);
  previewBusy = $state(false);
  spectrumInspectorOpen = $state(false);
  spectrumAmplitudeScale = $state<SpectrumAmplitudeScale>("db");
  spectrumBusy = $state(false);
  spectrumStale = $state(false);
  spectrumError = $state<string | null>(null);
  rawSpectrum = $state.raw<AmplitudeSpectrumResponse | null>(null);
  processedSpectrum = $state.raw<AmplitudeSpectrumResponse | null>(null);
  spectrumSectionKey = $state<string | null>(null);
  runBusy = $state(false);
  error = $state<string | null>(null);
  presets = $state.raw<ProcessingPreset[]>([]);
  activeJob = $state<ProcessingJobStatus | null>(null);
  loadingPresets = $state(false);
  runOutputSettingsOpen = $state(false);
  runOutputPathMode = $state<"default" | "custom">("default");
  customRunOutputPath = $state("");
  overwriteExistingRunOutput = $state(false);
  defaultRunOutputPath = $state<string | null>(null);
  resolvingRunOutputPath = $state(false);

  #jobPollTimer: number | null = null;
  #presetCounter = 0;
  #sessionPipelineCounter = 0;
  #hydratedDatasetEntryId: string | null = null;
  #runOutputPathRequestId = 0;
  #copiedSessionPipeline: ProcessingPipeline | null = null;

  constructor(options: ProcessingModelOptions) {
    this.viewerModel = options.viewerModel;

    $effect(() => {
      const key = sectionKey(this.viewerModel);
      const currentSection = this.viewerModel.section;
      const activeStorePath = this.viewerModel.activeStorePath;
      if (!activeStorePath || !currentSection) {
        this.previewSection = null;
        this.previewState = "raw";
        this.previewedSectionKey = null;
        this.spectrumInspectorOpen = false;
        this.clearSpectrumState();
        return;
      }

      if (this.previewedSectionKey && this.previewedSectionKey !== key) {
        this.previewState = "stale";
      }
      if (this.spectrumSectionKey && this.spectrumSectionKey !== key) {
        this.clearSpectrumState();
      }
    });

    $effect(() => {
      const activeEntryId = this.viewerModel.activeEntryId;
      const activeEntry = this.viewerModel.activeDatasetEntry;

      if (!activeEntryId || !activeEntry) {
        this.#hydratedDatasetEntryId = null;
        if (!this.sessionPipelines.length) {
          const fallback = this.createSessionPipelineEntry(this.nextEmptySessionPipelineName());
          this.sessionPipelines = [fallback];
          this.activeSessionPipelineId = fallback.pipeline_id;
          this.pipeline = clonePipeline(fallback.pipeline);
        }
        return;
      }

       if (this.#hydratedDatasetEntryId === activeEntryId) {
        return;
      }
      this.#hydratedDatasetEntryId = activeEntryId;

      const nextSessionPipelines =
        activeEntry.session_pipelines.length > 0
          ? activeEntry.session_pipelines.map((entry) => ({
              pipeline_id: entry.pipeline_id,
              pipeline: clonePipeline(entry.pipeline),
              updated_at_unix_s: entry.updated_at_unix_s
            }))
          : [this.createSessionPipelineEntry("Pipeline 1")];
      const activePipelineId =
        activeEntry.active_session_pipeline_id &&
        nextSessionPipelines.some((entry) => entry.pipeline_id === activeEntry.active_session_pipeline_id)
          ? activeEntry.active_session_pipeline_id
          : nextSessionPipelines[0]?.pipeline_id ?? null;
      const activePipeline =
        nextSessionPipelines.find((entry) => entry.pipeline_id === activePipelineId) ?? nextSessionPipelines[0];

      this.sessionPipelines = nextSessionPipelines;
      this.activeSessionPipelineId = activePipeline?.pipeline_id ?? null;
      this.pipeline = clonePipeline(activePipeline?.pipeline ?? createEmptyPipeline());
      this.selectedStepIndex = 0;
      this.editingParams = false;
      this.clearPreviewState();
    });

    $effect(() => {
      const activeStorePath = this.viewerModel.activeStorePath;
      const signature = pipelineRunOutputSignature(this.pipeline);

      if (!activeStorePath) {
        this.defaultRunOutputPath = null;
        this.resolvingRunOutputPath = false;
        return;
      }

      void this.refreshDefaultRunOutputPath(activeStorePath, clonePipeline(this.pipeline), signature);
    });
  }

  mount = (): (() => void) => {
    void this.refreshPresets();
    return () => {
      if (this.#jobPollTimer !== null && typeof window !== "undefined") {
        window.clearTimeout(this.#jobPollTimer);
      }
      this.#jobPollTimer = null;
    };
  };

  get selectedOperation(): ProcessingOperation | null {
    return this.pipeline.operations[this.selectedStepIndex] ?? null;
  }

  get activeSessionPipeline(): WorkspacePipelineEntry | null {
    return this.sessionPipelines.find((entry) => entry.pipeline_id === this.activeSessionPipelineId) ?? null;
  }

  get sessionPipelineItems(): WorkspacePipelineEntry[] {
    return this.sessionPipelines;
  }

  get hasOperations(): boolean {
    return this.pipeline.operations.length > 0;
  }

  get selectedStepLabel(): string | null {
    return this.selectedOperation ? describeOperation(this.selectedOperation) : null;
  }

  get displaySection(): SectionView | null {
    if (this.previewState === "preview" && this.previewSection) {
      return this.previewSection;
    }
    return this.viewerModel.section;
  }

  get displaySectionMode(): PreviewState {
    return this.previewState;
  }

  get displayResetToken(): string {
    return `${this.viewerModel.resetToken}:${this.previewState}:${this.previewedSectionKey ?? "raw"}`;
  }

  get canPreview(): boolean {
    return this.hasOperations && Boolean(this.viewerModel.section && this.viewerModel.activeStorePath);
  }

  get canRun(): boolean {
    return this.hasOperations && Boolean(this.viewerModel.activeStorePath);
  }

  get canInspectSpectrum(): boolean {
    return Boolean(this.viewerModel.section && this.viewerModel.activeStorePath && this.viewerModel.dataset);
  }

  get spectrumSelectionSummary(): string {
    const section = this.viewerModel.section;
    if (!section) {
      return "Open a dataset and load a section to inspect spectra.";
    }

    return `Whole ${this.viewerModel.axis} section ${this.viewerModel.index} · ${section.traces} traces × ${section.samples} samples`;
  }

  get pipelineDirty(): boolean {
    return this.previewState !== "preview";
  }

  get pipelineTitle(): string {
    return pipelineName(this.pipeline);
  }

  get canRemoveSessionPipeline(): boolean {
    return this.sessionPipelines.length > 1;
  }

  get resolvedRunOutputPath(): string | null {
    if (this.runOutputPathMode === "custom") {
      const nextPath = this.customRunOutputPath.trim();
      return nextPath.length > 0 ? nextPath : null;
    }
    return this.defaultRunOutputPath;
  }

  sessionPipelineLabel = (entry: WorkspacePipelineEntry, index: number): string => {
    return pipelineName(entry.pipeline) || `Pipeline ${index + 1}`;
  };

  setRunOutputSettingsOpen = (open: boolean): void => {
    this.runOutputSettingsOpen = open;
    if (open && this.viewerModel.activeStorePath && !this.defaultRunOutputPath && !this.resolvingRunOutputPath) {
      void this.refreshDefaultRunOutputPath(
        this.viewerModel.activeStorePath,
        clonePipeline(this.pipeline),
        pipelineRunOutputSignature(this.pipeline)
      );
    }
  };

  setRunOutputPathMode = (mode: "default" | "custom"): void => {
    this.runOutputPathMode = mode;
  };

  setCustomRunOutputPath = (value: string): void => {
    this.customRunOutputPath = value;
  };

  resetRunOutputPath = (): void => {
    this.runOutputPathMode = "default";
    this.customRunOutputPath = "";
  };

  browseRunOutputPath = async (): Promise<void> => {
    const selected = await pickOutputStorePath(this.resolvedRunOutputPath ?? this.defaultRunOutputPath ?? "processed.tbvol");
    if (!selected) {
      return;
    }
    this.runOutputPathMode = "custom";
    this.customRunOutputPath = selected;
  };

  setOverwriteExistingRunOutput = (value: boolean): void => {
    this.overwriteExistingRunOutput = value;
  };

  refreshPresets = async (): Promise<void> => {
    this.loadingPresets = true;
    try {
      const response = await listPipelinePresets();
      this.presets = response.presets;
    } catch (error) {
      this.error = errorMessage(error, "Failed to load pipeline presets.");
      this.viewerModel.note("Failed to load pipeline presets.", "backend", "error", this.error);
    } finally {
      this.loadingPresets = false;
    }
  };

  createSessionPipeline = (): void => {
    const nextEntry = this.createSessionPipelineEntry(this.nextEmptySessionPipelineName());
    this.sessionPipelines = [...this.sessionPipelines, nextEntry];
    this.activeSessionPipelineId = nextEntry.pipeline_id;
    this.pipeline = clonePipeline(nextEntry.pipeline);
    this.viewerModel.setSelectedPresetId(null);
    this.selectedStepIndex = 0;
    this.editingParams = false;
    this.clearPreviewState();
    void this.persistSessionPipelines();
  };

  duplicateActiveSessionPipeline = (): void => {
    const source = this.activeSessionPipeline;
    if (!source) {
      return;
    }
    const duplicate = this.createCopiedSessionPipelineEntry(source.pipeline);
    this.sessionPipelines = [...this.sessionPipelines, duplicate];
    this.activeSessionPipelineId = duplicate.pipeline_id;
    this.pipeline = clonePipeline(duplicate.pipeline);
    this.viewerModel.setSelectedPresetId(null);
    this.selectedStepIndex = 0;
    this.editingParams = false;
    this.clearPreviewState();
    void this.persistSessionPipelines();
  };

  activateSessionPipeline = (pipelineId: string): void => {
    const entry = this.sessionPipelines.find((candidate) => candidate.pipeline_id === pipelineId);
    if (!entry) {
      return;
    }

    this.activeSessionPipelineId = pipelineId;
    this.pipeline = clonePipeline(entry.pipeline);
    this.viewerModel.setSelectedPresetId(entry.pipeline.preset_id ?? null);
    this.selectedStepIndex = 0;
    this.editingParams = false;
    this.clearPreviewState();
    void this.persistSessionPipelines();
  };

  removeActiveSessionPipeline = (): void => {
    const activePipelineId = this.activeSessionPipelineId;
    if (!activePipelineId) {
      return;
    }

    if (this.sessionPipelines.length <= 1) {
      const replacement = this.createSessionPipelineEntry(this.nextEmptySessionPipelineName());
      this.sessionPipelines = [replacement];
      this.activeSessionPipelineId = replacement.pipeline_id;
      this.pipeline = clonePipeline(replacement.pipeline);
      this.selectedStepIndex = 0;
      this.editingParams = false;
      this.clearPreviewState();
      void this.persistSessionPipelines();
      return;
    }

    const activeIndex = this.sessionPipelines.findIndex((entry) => entry.pipeline_id === activePipelineId);
    const nextSessionPipelines = this.sessionPipelines.filter((entry) => entry.pipeline_id !== activePipelineId);
    const fallbackEntry = nextSessionPipelines[Math.max(0, activeIndex - 1)] ?? nextSessionPipelines[0];
    this.sessionPipelines = nextSessionPipelines;
    this.activeSessionPipelineId = fallbackEntry.pipeline_id;
    this.pipeline = clonePipeline(fallbackEntry.pipeline);
    this.viewerModel.setSelectedPresetId(fallbackEntry.pipeline.preset_id ?? null);
    this.selectedStepIndex = 0;
    this.editingParams = false;
    this.clearPreviewState();
    void this.persistSessionPipelines();
  };

  private createSessionPipelineEntry(
    suggestedName: string,
    template: ProcessingPipeline = createEmptyPipeline()
  ): WorkspacePipelineEntry {
    this.#sessionPipelineCounter += 1;
    const pipeline = clonePipeline(template);
    pipeline.name = pipeline.name?.trim() || suggestedName;
    return {
      pipeline_id: `session-pipeline-${Date.now()}-${this.#sessionPipelineCounter}`,
      pipeline,
      updated_at_unix_s: pipelineTimestamp()
    };
  }

  private nextEmptySessionPipelineName(): string {
    const existingNames = this.sessionPipelines.map((entry) => pipelineName(entry.pipeline).trim().toLowerCase());
    if (!existingNames.includes("pipeline")) {
      return "Pipeline";
    }

    let index = 2;
    while (existingNames.includes(`pipeline ${index}`)) {
      index += 1;
    }
    return `Pipeline ${index}`;
  }

  private createCopiedSessionPipelineEntry(source: ProcessingPipeline): WorkspacePipelineEntry {
    const pipeline = clonePipeline(source);
    pipeline.preset_id = null;
    pipeline.name = `${pipelineName(source)}_copy`;
    return this.createSessionPipelineEntry(pipeline.name, pipeline);
  }

  copyActiveSessionPipeline = (): void => {
    const activePipeline = this.activeSessionPipeline?.pipeline ?? this.pipeline;
    this.#copiedSessionPipeline = clonePipeline(activePipeline);
    this.viewerModel.note("Copied active session pipeline.", "ui", "info", pipelineName(activePipeline));
  };

  pasteCopiedSessionPipeline = (): void => {
    if (!this.#copiedSessionPipeline) {
      return;
    }

    const duplicate = this.createCopiedSessionPipelineEntry(this.#copiedSessionPipeline);
    this.sessionPipelines = [...this.sessionPipelines, duplicate];
    this.activeSessionPipelineId = duplicate.pipeline_id;
    this.pipeline = clonePipeline(duplicate.pipeline);
    this.viewerModel.setSelectedPresetId(null);
    this.selectedStepIndex = 0;
    this.editingParams = false;
    this.clearPreviewState();
    void this.persistSessionPipelines();
  };

  private persistSessionPipelines(): Promise<void> {
    return this.viewerModel.updateActiveEntryPipelines(
      this.sessionPipelines.map((entry) => ({
        pipeline_id: entry.pipeline_id,
        updated_at_unix_s: entry.updated_at_unix_s,
        pipeline: clonePipeline(entry.pipeline)
      })),
      this.activeSessionPipelineId
    );
  }

  private updateActiveSessionPipeline(nextPipeline: ProcessingPipeline): void {
    const activePipelineId = this.activeSessionPipelineId;
    const snapshot = clonePipeline(nextPipeline);
    this.pipeline = snapshot;

    if (!activePipelineId) {
      return;
    }

    this.sessionPipelines = this.sessionPipelines.map((entry) =>
      entry.pipeline_id === activePipelineId
        ? {
            pipeline_id: entry.pipeline_id,
            pipeline: clonePipeline(snapshot),
            updated_at_unix_s: pipelineTimestamp()
          }
        : entry
    );
    void this.persistSessionPipelines();
  }

  private clearPreviewState(): void {
    this.previewState = "raw";
    this.previewSection = null;
    this.previewLabel = null;
    this.previewedSectionKey = null;
  }

  private clearSpectrumState(): void {
    this.rawSpectrum = null;
    this.processedSpectrum = null;
    this.spectrumStale = false;
    this.spectrumError = null;
    this.spectrumSectionKey = null;
  }

  openSpectrumInspector = (): void => {
    this.spectrumInspectorOpen = true;
  };

  closeSpectrumInspector = (): void => {
    this.spectrumInspectorOpen = false;
  };

  toggleSpectrumInspector = (): void => {
    this.spectrumInspectorOpen = !this.spectrumInspectorOpen;
  };

  setSpectrumAmplitudeScale = (scale: SpectrumAmplitudeScale): void => {
    this.spectrumAmplitudeScale = scale;
  };

  selectStep = (index: number): void => {
    if (this.pipeline.operations.length === 0) {
      this.selectedStepIndex = 0;
      return;
    }
    this.selectedStepIndex = Math.max(0, Math.min(index, this.pipeline.operations.length - 1));
  };

  selectNextStep = (): void => {
    this.selectStep(this.selectedStepIndex + 1);
  };

  selectPreviousStep = (): void => {
    this.selectStep(this.selectedStepIndex - 1);
  };

  addAmplitudeScalarAfterSelected = (): void => {
    this.insertOperatorById("amplitude_scalar");
  };

  addTraceRmsNormalizeAfterSelected = (): void => {
    this.insertOperatorById("trace_rms_normalize");
  };

  addBandpassAfterSelected = (): void => {
    this.insertOperatorById("bandpass_filter");
  };

  insertOperatorById = (operatorId: OperatorCatalogId): void => {
    const operator = OPERATOR_CATALOG.find((candidate) => candidate.id === operatorId);
    if (!operator) {
      return;
    }
    this.insertOperation(operator.create(this.viewerModel.section));
  };

  insertOperation = (operation: ProcessingOperation): void => {
    const next = clonePipeline(this.pipeline);
    const insertIndex = this.pipeline.operations.length === 0 ? 0 : this.selectedStepIndex + 1;
    next.operations.splice(insertIndex, 0, cloneOperation(operation));
    next.revision += 1;
    this.updateActiveSessionPipeline(next);
    this.selectedStepIndex = insertIndex;
    this.editingParams = true;
    this.invalidatePreview();
  };

  removeSelected = (): void => {
    if (!this.selectedOperation) {
      return;
    }
    const next = clonePipeline(this.pipeline);
    next.operations.splice(this.selectedStepIndex, 1);
    next.revision += 1;
    this.updateActiveSessionPipeline(next);
    this.selectedStepIndex = Math.max(0, Math.min(this.selectedStepIndex, next.operations.length - 1));
    this.editingParams = false;
    this.invalidatePreview();
  };

  moveSelectedUp = (): void => {
    if (this.selectedStepIndex <= 0 || !this.selectedOperation) {
      return;
    }
    const next = clonePipeline(this.pipeline);
    const [operation] = next.operations.splice(this.selectedStepIndex, 1);
    next.operations.splice(this.selectedStepIndex - 1, 0, operation);
    next.revision += 1;
    this.updateActiveSessionPipeline(next);
    this.selectedStepIndex -= 1;
    this.invalidatePreview();
  };

  moveSelectedDown = (): void => {
    if (!this.selectedOperation || this.selectedStepIndex >= this.pipeline.operations.length - 1) {
      return;
    }
    const next = clonePipeline(this.pipeline);
    const [operation] = next.operations.splice(this.selectedStepIndex, 1);
    next.operations.splice(this.selectedStepIndex + 1, 0, operation);
    next.revision += 1;
    this.updateActiveSessionPipeline(next);
    this.selectedStepIndex += 1;
    this.invalidatePreview();
  };

  beginParamEdit = (): void => {
    this.editingParams = Boolean(this.selectedOperation);
  };

  endParamEdit = (): void => {
    this.editingParams = false;
  };

  setPipelineName = (value: string): void => {
    this.updateActiveSessionPipeline({
      ...clonePipeline(this.pipeline),
      name: value.trim() || null
    });
  };

  setSelectedAmplitudeScalarFactor = (value: number): void => {
    const selected = this.selectedOperation;
    if (!selected || !isAmplitudeScalar(selected)) {
      return;
    }
    const next = clonePipeline(this.pipeline);
    const operation = next.operations[this.selectedStepIndex];
    if (!isAmplitudeScalar(operation)) {
      return;
    }
    operation.amplitude_scalar.factor = value;
    next.revision += 1;
    this.updateActiveSessionPipeline(next);
    this.invalidatePreview();
  };

  setSelectedBandpassCorner = (
    corner: "f1_hz" | "f2_hz" | "f3_hz" | "f4_hz",
    value: number
  ): void => {
    const selected = this.selectedOperation;
    if (!selected || !isBandpassFilter(selected) || !Number.isFinite(value)) {
      return;
    }

    const next = clonePipeline(this.pipeline);
    const operation = next.operations[this.selectedStepIndex];
    if (!isBandpassFilter(operation)) {
      return;
    }

    operation.bandpass_filter[corner] = value;
    next.revision += 1;
    this.updateActiveSessionPipeline(next);
    this.invalidatePreview();
  };

  replacePipeline = (pipeline: ProcessingPipeline): void => {
    this.updateActiveSessionPipeline(clonePipeline(pipeline));
    this.selectedStepIndex = 0;
    this.editingParams = false;
    this.invalidatePreview();
  };

  loadPreset = (preset: ProcessingPreset): void => {
    this.replacePipeline(preset.pipeline);
    this.viewerModel.setSelectedPresetId(preset.preset_id);
    this.viewerModel.note("Applied library template to the active pipeline.", "ui", "info", preset.preset_id);
  };

  savePreset = async (): Promise<void> => {
    const presetId =
      normalizePresetId(this.pipeline.preset_id ?? this.pipeline.name ?? `pipeline-${++this.#presetCounter}`) ||
      `pipeline-${++this.#presetCounter}`;
    const preset: ProcessingPreset = {
      preset_id: presetId,
      pipeline: {
        ...clonePipeline(this.pipeline),
        preset_id: presetId
      },
      created_at_unix_s: 0,
      updated_at_unix_s: 0
    };
    try {
      const response = await savePipelinePreset(preset);
      this.updateActiveSessionPipeline(clonePipeline(response.preset.pipeline));
      this.viewerModel.setSelectedPresetId(response.preset.preset_id);
      await this.refreshPresets();
      this.viewerModel.note("Saved pipeline as a library template.", "ui", "info", response.preset.preset_id);
    } catch (error) {
      this.error = errorMessage(error, "Failed to save library template.");
      this.viewerModel.note("Failed to save library template.", "backend", "error", this.error);
    }
  };

  deletePreset = async (presetId: string): Promise<void> => {
    try {
      const deleted = await deletePipelinePreset(presetId);
      if (deleted) {
        if (this.viewerModel.selectedPresetId === presetId) {
          this.viewerModel.setSelectedPresetId(null);
        }
        await this.refreshPresets();
        this.viewerModel.note("Deleted library template.", "ui", "warn", presetId);
      }
    } catch (error) {
      this.error = errorMessage(error, "Failed to delete library template.");
      this.viewerModel.note("Failed to delete library template.", "backend", "error", this.error);
    }
  };

  previewCurrentSection = async (): Promise<void> => {
    if (!this.canPreview || !this.viewerModel.dataset || !this.viewerModel.activeStorePath) {
      this.error = "Open a dataset and load a section before previewing.";
      return;
    }

    this.previewBusy = true;
    this.error = null;
    try {
      const request: PreviewProcessingRequest = {
        schema_version: 1,
        store_path: this.viewerModel.activeStorePath,
        section: {
          dataset_id: this.viewerModel.dataset.descriptor.id,
          axis: this.viewerModel.axis,
          index: this.viewerModel.index
        },
        pipeline: clonePipeline(this.pipeline)
      };
      const response = await previewProcessing(request);
      this.previewSection = response.preview.section;
      this.previewState = "preview";
      this.previewLabel = response.preview.processing_label;
      this.previewedSectionKey = sectionKey(this.viewerModel);
      this.viewerModel.note("Processing preview generated.", "backend", "info", this.previewLabel);
    } catch (error) {
      this.error = errorMessage(error, "Failed to preview processing pipeline.");
      this.viewerModel.note("Processing preview failed.", "backend", "error", this.error);
    } finally {
      this.previewBusy = false;
    }
  };

  refreshSpectrum = async (): Promise<void> => {
    const currentSection = this.viewerModel.section;
    if (!this.canInspectSpectrum || !this.viewerModel.dataset || !this.viewerModel.activeStorePath || !currentSection) {
      this.spectrumError = "Open a dataset and load a section before inspecting the spectrum.";
      return;
    }

    this.spectrumBusy = true;
    this.spectrumError = null;
    try {
      const baseRequest: AmplitudeSpectrumRequest = {
        schema_version: 1,
        store_path: this.viewerModel.activeStorePath,
        section: {
          dataset_id: this.viewerModel.dataset.descriptor.id,
          axis: this.viewerModel.axis,
          index: this.viewerModel.index
        },
        selection: "whole_section",
        pipeline: null
      };

      const rawResponse = await fetchAmplitudeSpectrum(baseRequest);
      this.rawSpectrum = rawResponse;

      if (this.hasOperations) {
        this.processedSpectrum = await fetchAmplitudeSpectrum({
          ...baseRequest,
          pipeline: clonePipeline(this.pipeline)
        });
      } else {
        this.processedSpectrum = null;
      }

      this.spectrumStale = false;
      this.spectrumSectionKey = sectionKey(this.viewerModel);
      this.viewerModel.note("Amplitude spectrum generated.", "backend", "info", this.spectrumSelectionSummary);
    } catch (error) {
      this.spectrumError = errorMessage(error, "Failed to inspect amplitude spectrum.");
      this.viewerModel.note("Amplitude spectrum failed.", "backend", "error", this.spectrumError);
    } finally {
      this.spectrumBusy = false;
    }
  };

  showRawSection = (): void => {
    this.previewState = this.previewedSectionKey === sectionKey(this.viewerModel) ? "stale" : "raw";
  };

  runOnVolume = async (): Promise<void> => {
    if (!this.canRun || !this.viewerModel.activeStorePath) {
      this.error = "Open a dataset before running processing on the full volume.";
      return;
    }
    this.runBusy = true;
    this.error = null;
    try {
      const outputStorePath =
        this.runOutputPathMode === "custom"
          ? this.customRunOutputPath.trim()
          : await defaultProcessingStorePath(this.viewerModel.activeStorePath, this.pipeline);
      if (!outputStorePath) {
        this.error = "Select an output runtime store path before running the full volume.";
        this.runBusy = false;
        return;
      }
      await this.startRunOnVolume(outputStorePath, this.overwriteExistingRunOutput);
    } catch (error) {
      this.error = errorMessage(error, "Failed to start processing job.");
      if (!this.overwriteExistingRunOutput && isExistingOutputStoreError(this.error)) {
        const confirmed = await confirmOverwriteStore(
          this.resolvedRunOutputPath ?? this.customRunOutputPath.trim()
        );
        if (confirmed) {
          this.overwriteExistingRunOutput = true;
          const outputStorePath =
            this.resolvedRunOutputPath ??
            (this.viewerModel.activeStorePath
              ? await defaultProcessingStorePath(this.viewerModel.activeStorePath, this.pipeline)
              : null);
          if (outputStorePath) {
            try {
              await this.startRunOnVolume(outputStorePath, true);
              return;
            } catch (retryError) {
              this.error = errorMessage(retryError, "Failed to start processing job.");
            }
          }
        }
      }
      this.runBusy = false;
      this.viewerModel.note("Failed to start processing job.", "backend", "error", this.error);
    }
  };

  cancelActiveJob = async (): Promise<void> => {
    if (!this.activeJob) {
      return;
    }
    try {
      const response = await cancelProcessingJob(this.activeJob.job_id);
      this.activeJob = response.job;
      this.viewerModel.note("Requested processing job cancellation.", "ui", "warn", response.job.job_id);
    } catch (error) {
      this.error = errorMessage(error, "Failed to cancel processing job.");
    }
  };

  handleKeydown = async (event: KeyboardEvent): Promise<void> => {
    const target = event.target as HTMLElement | null;
    const tagName = target?.tagName?.toLowerCase();
    const editingText = Boolean(
      target?.isContentEditable ||
        tagName === "input" ||
        tagName === "textarea" ||
        tagName === "select"
    );
    if (editingText && !event.ctrlKey && !event.metaKey && event.key !== "Escape") {
      return;
    }

    if (event.ctrlKey || event.metaKey) {
      if (event.key.toLowerCase() === "s") {
        event.preventDefault();
        await this.savePreset();
      }
      return;
    }

    switch (event.key) {
      case "j":
        event.preventDefault();
        this.selectNextStep();
        break;
      case "k":
        event.preventDefault();
        this.selectPreviousStep();
        break;
      case "J":
        event.preventDefault();
        this.moveSelectedDown();
        break;
      case "K":
        event.preventDefault();
        this.moveSelectedUp();
        break;
      case "a":
        event.preventDefault();
        this.addAmplitudeScalarAfterSelected();
        break;
      case "n":
        event.preventDefault();
        this.addTraceRmsNormalizeAfterSelected();
        break;
      case "b":
        event.preventDefault();
        this.addBandpassAfterSelected();
        break;
      case "x":
      case "Delete":
        event.preventDefault();
        this.removeSelected();
        break;
      case "Enter":
        event.preventDefault();
        this.beginParamEdit();
        break;
      case "Escape":
        event.preventDefault();
        this.endParamEdit();
        break;
      case "p":
        event.preventDefault();
        await this.previewCurrentSection();
        break;
      case "s":
        event.preventDefault();
        this.openSpectrumInspector();
        if (!this.rawSpectrum && !this.spectrumBusy) {
          await this.refreshSpectrum();
        }
        break;
      case "r":
        event.preventDefault();
        await this.runOnVolume();
        break;
    }
  };

  private scheduleJobPoll(): void {
    if (!this.activeJob || typeof window === "undefined") {
      return;
    }
    if (this.#jobPollTimer !== null) {
      window.clearTimeout(this.#jobPollTimer);
    }
    this.#jobPollTimer = window.setTimeout(() => {
      void this.pollActiveJob();
    }, 500);
  }

  private async pollActiveJob(): Promise<void> {
    if (!this.activeJob) {
      this.runBusy = false;
      return;
    }
    try {
      const response = await getProcessingJob(this.activeJob.job_id);
      this.activeJob = response.job;
      switch (response.job.state) {
        case "queued":
        case "running":
          this.runBusy = true;
          this.scheduleJobPoll();
          break;
        case "completed":
          this.runBusy = false;
          if (response.job.output_store_path) {
            await this.viewerModel.openDerivedDatasetAt(
              response.job.output_store_path,
              this.viewerModel.axis,
              this.viewerModel.index
            );
          }
          this.viewerModel.note(
            "Processing job completed.",
            "backend",
            "info",
            response.job.output_store_path ?? response.job.job_id
          );
          break;
        case "cancelled":
          this.runBusy = false;
          this.viewerModel.note("Processing job cancelled.", "backend", "warn", response.job.job_id);
          break;
        case "failed":
          this.runBusy = false;
          this.error = response.job.error_message ?? "Processing job failed.";
          this.viewerModel.note("Processing job failed.", "backend", "error", this.error);
          break;
      }
    } catch (error) {
      this.runBusy = false;
      this.error = errorMessage(error, "Failed to poll processing job.");
      this.viewerModel.note("Processing job polling failed.", "backend", "error", this.error);
    }
  }

  private invalidatePreview(): void {
    if (this.previewSection) {
      this.previewState = "stale";
    } else {
      this.previewState = "raw";
    }
    if (this.rawSpectrum || this.processedSpectrum) {
      this.spectrumStale = true;
      this.spectrumError = null;
    }
  }

  private async refreshDefaultRunOutputPath(
    activeStorePath: string,
    pipeline: ProcessingPipeline,
    signature: string
  ): Promise<void> {
    const requestId = ++this.#runOutputPathRequestId;
    this.resolvingRunOutputPath = true;
    try {
      const nextPath = await defaultProcessingStorePath(activeStorePath, pipeline);
      if (
        requestId !== this.#runOutputPathRequestId ||
        activeStorePath !== this.viewerModel.activeStorePath ||
        signature !== pipelineRunOutputSignature(this.pipeline)
      ) {
        return;
      }
      this.defaultRunOutputPath = nextPath;
    } catch {
      if (requestId !== this.#runOutputPathRequestId) {
        return;
      }
      this.defaultRunOutputPath = null;
    } finally {
      if (requestId === this.#runOutputPathRequestId) {
        this.resolvingRunOutputPath = false;
      }
    }
  }

  private async startRunOnVolume(outputStorePath: string, overwriteExisting: boolean): Promise<void> {
    if (!this.viewerModel.activeStorePath) {
      throw new Error("Open a dataset before running processing on the full volume.");
    }

    const request: RunProcessingRequest = {
      schema_version: 1,
      store_path: this.viewerModel.activeStorePath,
      output_store_path: outputStorePath,
      overwrite_existing: overwriteExisting,
      pipeline: clonePipeline(this.pipeline)
    };
    const response = await runProcessing(request);
    this.activeJob = response.job;
    this.viewerModel.note(
      "Started full-volume processing job.",
      "backend",
      "info",
      response.job.output_store_path ?? response.job.job_id
    );
    this.scheduleJobPoll();
  }
}

export function describeOperation(operation: ProcessingOperation): string {
  if (isAmplitudeScalar(operation)) {
    return `amplitude scalar (${operation.amplitude_scalar.factor})`;
  }
  if (isBandpassFilter(operation)) {
    const { f1_hz, f2_hz, f3_hz, f4_hz } = operation.bandpass_filter;
    return `bandpass (${f1_hz}/${f2_hz}/${f3_hz}/${f4_hz} Hz)`;
  }
  return "trace RMS normalize";
}

export function isAmplitudeScalar(
  operation: ProcessingOperation
): operation is { amplitude_scalar: { factor: number } } {
  return typeof operation !== "string" && "amplitude_scalar" in operation;
}

export function isBandpassFilter(
  operation: ProcessingOperation
): operation is {
  bandpass_filter: {
    f1_hz: number;
    f2_hz: number;
    f3_hz: number;
    f4_hz: number;
    phase: "zero";
    window: "cosine_taper";
  };
} {
  return typeof operation !== "string" && "bandpass_filter" in operation;
}

const [internalGetProcessingModelContext, internalSetProcessingModelContext] =
  createContext<ProcessingModel>();

export function getProcessingModelContext(): ProcessingModel {
  const processingModel = internalGetProcessingModelContext();
  if (!processingModel) {
    throw new Error("Processing model context not found");
  }
  return processingModel;
}

export function setProcessingModelContext(processingModel: ProcessingModel): ProcessingModel {
  internalSetProcessingModelContext(processingModel);
  return processingModel;
}
