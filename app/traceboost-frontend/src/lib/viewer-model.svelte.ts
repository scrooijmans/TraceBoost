import { createContext, tick } from "svelte";
import type { SectionHorizonOverlay as GeovizSectionHorizonOverlay } from "@geoviz/data-models";
import type { SeismicChartInteractionState, SeismicChartTool } from "@geoviz/svelte";
import type {
  DatasetRegistryEntry,
  DatasetSummary,
  ExportSegyResponse,
  ImportDatasetResponse,
  ImportedHorizonDescriptor,
  ResolvedSurveyMapSourceDto,
  SegyGeometryCandidate,
  SegyGeometryOverride,
  SegyHeaderField,
  SegyHeaderValueType,
  SectionAxis,
  SectionInteractionChanged,
  SectionHorizonOverlayView,
  SectionProbeChanged,
  SectionView,
  SectionViewportChanged,
  SurveyPreflightResponse,
  WorkspacePipelineEntry,
  WorkspaceSession
} from "@traceboost/seis-contracts";
import type { DiagnosticsEvent, DiagnosticsStatus, TransportSectionView } from "./bridge";
import {
  exportDatasetSegy,
  defaultImportStorePath,
  emitFrontendDiagnosticsEvent,
  fetchSectionHorizons,
  fetchSectionView,
  getDiagnosticsStatus,
  importDataset,
  importHorizonXyz,
  loadWorkspaceState,
  listenToDiagnosticsEvents,
  openDataset,
  preflightImport,
  removeDatasetEntry,
  resolveSurveyMap,
  saveWorkspaceSession,
  setActiveDatasetEntry,
  setDatasetNativeCoordinateReference,
  upsertDatasetEntry,
  setDiagnosticsVerbosity
} from "./bridge";
import { confirmOverwriteSegy, confirmOverwriteStore, pickSegyExportPath } from "./file-dialog";

type DisplaySectionView = SectionView | TransportSectionView;

export interface ViewerActivity {
  id: number;
  timestamp: string;
  source: "ui" | "backend" | "viewer";
  level: "info" | "warn" | "error";
  message: string;
  detail: string | null;
}

export interface ViewerModelOptions {
  tauriRuntime: boolean;
}

interface OpenDatasetOptions {
  entryId?: string | null;
  displayName?: string | null;
  sourcePath?: string | null;
  sessionPipelines?: WorkspacePipelineEntry[] | null;
  activeSessionPipelineId?: string | null;
  makeActive?: boolean;
  loadSection?: boolean;
}

interface ImportDatasetOptions extends OpenDatasetOptions {
  inputPath?: string;
  outputStorePath?: string;
  reuseExistingStore?: boolean;
  geometryOverride?: SegyGeometryOverride | null;
}

interface GeometryOverrideDraft {
  inlineByte: string;
  inlineType: SegyHeaderValueType;
  crosslineByte: string;
  crosslineType: SegyHeaderValueType;
  thirdAxisByte: string;
  thirdAxisType: SegyHeaderValueType;
}

interface ImportGeometryRecoveryState {
  inputPath: string;
  outputStorePath: string;
  preflight: SurveyPreflightResponse;
  importOptions: ImportDatasetOptions;
  mode: "candidate" | "manual";
  selectedCandidateIndex: number;
  draft: GeometryOverrideDraft;
  working: boolean;
  error: string | null;
}

export type CompareCompatibilityReason =
  | "primary_unset"
  | "missing_store_path"
  | "missing_dataset"
  | "missing_geometry_descriptor"
  | "compare_family_mismatch"
  | "geometry_fingerprint_mismatch";

export interface CompareCandidate {
  entryId: string;
  displayName: string;
  storePath: string;
  datasetId: string | null;
  compareFamily: string | null;
  fingerprint: string | null;
  compatible: boolean;
  isPrimary: boolean;
  reason: CompareCompatibilityReason | null;
}

export interface ComparePoolState {
  primaryStorePath: string | null;
  primaryDatasetId: string | null;
  primaryLabel: string | null;
  compareFamily: string | null;
  fingerprint: string | null;
  candidates: CompareCandidate[];
  compatibleStorePaths: string[];
  compatibleSecondaryStorePaths: string[];
}

function timestampLabel(): string {
  return new Date().toLocaleTimeString("en-GB", { hour12: false });
}

function capEntries<T>(entries: T[], next: T, limit: number): T[] {
  return [next, ...entries].slice(0, limit);
}

function errorMessage(error: unknown, fallback: string): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof (error as { message?: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }
  return fallback;
}

function nowMs(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

function nextAnimationFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()));
}

function bytePayloadLength(bytes: Array<number> | Uint8Array | null | undefined): number {
  if (!bytes) {
    return 0;
  }
  return bytes instanceof Uint8Array ? bytes.byteLength : bytes.length;
}

function estimateSectionPayloadBytes(section: DisplaySectionView): number {
  return (
    bytePayloadLength(section.horizontal_axis_f64le) +
    bytePayloadLength(section.inline_axis_f64le) +
    bytePayloadLength(section.xline_axis_f64le) +
    bytePayloadLength(section.sample_axis_f32le) +
    bytePayloadLength(section.amplitudes_f32le)
  );
}

function adaptSectionHorizonOverlays(overlays: SectionHorizonOverlayView[]): GeovizSectionHorizonOverlay[] {
  return overlays.map((overlay) => ({
    id: overlay.id,
    name: overlay.name ?? undefined,
    color: overlay.style.color,
    lineWidth: overlay.style.line_width ?? undefined,
    lineStyle: overlay.style.line_style,
    opacity: overlay.style.opacity ?? undefined,
    samples: overlay.samples.map((sample) => ({
      traceIndex: sample.trace_index,
      sampleIndex: sample.sample_index,
      sampleValue: sample.sample_value ?? undefined
    }))
  }));
}

function isExistingStoreError(message: string): boolean {
  return message.toLowerCase().includes("store root already exists:");
}

function isExistingSegyExportError(message: string): boolean {
  return message.toLowerCase().includes("output seg-y path already exists:");
}

function describePreflight(preflight: SurveyPreflightResponse): string {
  const gather = preflight.gather_axis_kind ? `, gather axis ${preflight.gather_axis_kind}` : "";
  return `${preflight.classification} (${preflight.stacking_state}, ${preflight.layout}${gather})`;
}

function canAutoImportPreflight(preflight: SurveyPreflightResponse): boolean {
  return (
    preflight.suggested_action === "direct_dense_ingest" ||
    preflight.suggested_action === "regularize_sparse_survey"
  );
}

function sameHeaderField(
  left: SegyHeaderField | null | undefined,
  right: SegyHeaderField | null | undefined
): boolean {
  if (!left && !right) {
    return true;
  }
  if (!left || !right) {
    return false;
  }
  return left.start_byte === right.start_byte && left.value_type === right.value_type;
}

function sameGeometryOverride(
  left: SegyGeometryOverride | null | undefined,
  right: SegyGeometryOverride | null | undefined
): boolean {
  if (!left && !right) {
    return true;
  }
  if (!left || !right) {
    return false;
  }
  return (
    sameHeaderField(left.inline_3d, right.inline_3d) &&
    sameHeaderField(left.crossline_3d, right.crossline_3d) &&
    sameHeaderField(left.third_axis, right.third_axis)
  );
}

function describeHeaderField(field: SegyHeaderField | null | undefined): string {
  if (!field) {
    return "unset";
  }
  return `${field.start_byte} (${field.value_type.toUpperCase()})`;
}

function describeGeometryOverride(geometry: SegyGeometryOverride | null | undefined): string {
  if (!geometry) {
    return "default SEG-Y mapping";
  }
  return `inline ${describeHeaderField(geometry.inline_3d)}, crossline ${describeHeaderField(geometry.crossline_3d)}`;
}

function geometryOverrideDraft(
  geometry: SegyGeometryOverride | null | undefined
): GeometryOverrideDraft {
  return {
    inlineByte: geometry?.inline_3d?.start_byte ? String(geometry.inline_3d.start_byte) : "",
    inlineType: geometry?.inline_3d?.value_type ?? "i32",
    crosslineByte: geometry?.crossline_3d?.start_byte ? String(geometry.crossline_3d.start_byte) : "",
    crosslineType: geometry?.crossline_3d?.value_type ?? "i32",
    thirdAxisByte: geometry?.third_axis?.start_byte ? String(geometry.third_axis.start_byte) : "",
    thirdAxisType: geometry?.third_axis?.value_type ?? "i32"
  };
}

function geometryOverrideFromDraft(draft: GeometryOverrideDraft): SegyGeometryOverride | null {
  const parseField = (startByteText: string, valueType: SegyHeaderValueType): SegyHeaderField | null => {
    const trimmed = startByteText.trim();
    if (!trimmed) {
      return null;
    }
    const parsed = Number.parseInt(trimmed, 10);
    if (!Number.isInteger(parsed) || parsed <= 0) {
      return null;
    }
    return { start_byte: parsed, value_type: valueType };
  };

  const geometry: SegyGeometryOverride = {
    inline_3d: parseField(draft.inlineByte, draft.inlineType),
    crossline_3d: parseField(draft.crosslineByte, draft.crosslineType),
    third_axis: parseField(draft.thirdAxisByte, draft.thirdAxisType)
  };

  if (!geometry.inline_3d && !geometry.crossline_3d && !geometry.third_axis) {
    return null;
  }
  return geometry;
}

function canRecoverPreflight(preflight: SurveyPreflightResponse): boolean {
  return Boolean(preflight.suggested_geometry_override) || preflight.geometry_candidates.length > 0;
}

function suggestedCandidateIndex(preflight: SurveyPreflightResponse): number {
  if (!preflight.suggested_geometry_override) {
    return preflight.geometry_candidates.length > 0 ? 0 : -1;
  }
  return preflight.geometry_candidates.findIndex((candidate) =>
    sameGeometryOverride(candidate.geometry, preflight.suggested_geometry_override)
  );
}

function trimPath(value: string): string {
  return value.trim();
}

function deriveStorePathFromInput(inputPath: string): string {
  const normalizedPath = trimPath(inputPath);
  if (!normalizedPath) {
    return "";
  }

  const separatorIndex = Math.max(normalizedPath.lastIndexOf("/"), normalizedPath.lastIndexOf("\\"));
  const directory = separatorIndex >= 0 ? normalizedPath.slice(0, separatorIndex + 1) : "";
  const filename = separatorIndex >= 0 ? normalizedPath.slice(separatorIndex + 1) : normalizedPath;
  const basename = filename.replace(/\.[^.]+$/, "");
  if (!basename) {
    return "";
  }

  return `${directory}${basename}.tbvol`;
}

function deriveSegyExportPathFromStore(storePath: string): string {
  const normalizedPath = trimPath(storePath);
  if (!normalizedPath) {
    return "";
  }

  const separatorIndex = Math.max(normalizedPath.lastIndexOf("/"), normalizedPath.lastIndexOf("\\"));
  const directory = separatorIndex >= 0 ? normalizedPath.slice(0, separatorIndex + 1) : "";
  const filename = separatorIndex >= 0 ? normalizedPath.slice(separatorIndex + 1) : normalizedPath;
  const basename = filename.replace(/\.[^.]+$/, "");
  if (!basename) {
    return "";
  }

  return `${directory}${basename}.export.sgy`;
}

function fileExtension(filePath: string): string {
  const normalized = trimPath(filePath);
  const separatorIndex = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  const filename = separatorIndex >= 0 ? normalized.slice(separatorIndex + 1) : normalized;
  const extensionIndex = filename.lastIndexOf(".");
  return extensionIndex >= 0 ? filename.slice(extensionIndex).toLowerCase() : "";
}

function fileStem(filePath: string | null | undefined): string {
  const normalized = trimPath(filePath ?? "");
  if (!normalized) {
    return "";
  }
  const separatorIndex = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  const filename = separatorIndex >= 0 ? normalized.slice(separatorIndex + 1) : normalized;
  return filename.replace(/\.[^.]+$/, "");
}

function stripGeneratedHashSuffix(value: string): string {
  return value.replace(/-[0-9a-f]{16}$/i, "");
}

function userVisibleDatasetName(
  displayName: string | null | undefined,
  sourcePath: string | null | undefined,
  storePath: string | null | undefined,
  fallbackId: string
): string {
  const trimmedDisplayName = trimPath(displayName ?? "");
  if (trimmedDisplayName) {
    return stripGeneratedHashSuffix(trimmedDisplayName);
  }

  const sourceStem = fileStem(sourcePath);
  if (sourceStem) {
    return sourceStem;
  }

  const storeStem = fileStem(storePath);
  if (storeStem) {
    return stripGeneratedHashSuffix(storeStem);
  }

  return fallbackId;
}

function nextDuplicateName(sourceName: string, existingNames: string[]): string {
  const trimmedSourceName = sourceName.trim() || "Dataset";
  const sourceMatch = /^(.*?)(?:_(\d+))?$/.exec(trimmedSourceName);
  const baseName = sourceMatch?.[1]?.trim() || trimmedSourceName;
  const lowerBaseName = baseName.toLowerCase();
  let maxSuffix = 0;

  for (const existingName of existingNames) {
    const trimmedExistingName = existingName.trim();
    if (!trimmedExistingName) {
      continue;
    }

    const existingMatch = /^(.*?)(?:_(\d+))?$/.exec(trimmedExistingName);
    const existingBaseName = existingMatch?.[1]?.trim() || trimmedExistingName;
    if (existingBaseName.toLowerCase() !== lowerBaseName) {
      continue;
    }

    const suffix = existingMatch?.[2] ? Number(existingMatch[2]) : 0;
    if (Number.isFinite(suffix)) {
      maxSuffix = Math.max(maxSuffix, suffix);
    }
  }

  return `${baseName}_${maxSuffix + 1}`;
}

function sortWorkspaceEntries(entries: DatasetRegistryEntry[]): DatasetRegistryEntry[] {
  return [...entries].sort((left, right) => {
    const leftName = userVisibleDatasetName(
      left.display_name,
      left.source_path,
      left.imported_store_path ?? left.preferred_store_path,
      left.entry_id
    );
    const rightName = userVisibleDatasetName(
      right.display_name,
      right.source_path,
      right.imported_store_path ?? right.preferred_store_path,
      right.entry_id
    );
    const byName = leftName.localeCompare(rightName, undefined, { sensitivity: "base", numeric: true });
    if (byName !== 0) {
      return byName;
    }
    return left.entry_id.localeCompare(right.entry_id, undefined, { sensitivity: "base", numeric: true });
  });
}

function mergeWorkspaceEntry(
  entries: DatasetRegistryEntry[],
  nextEntry: DatasetRegistryEntry
): DatasetRegistryEntry[] {
  const nextEntries = entries.filter((entry) => entry.entry_id !== nextEntry.entry_id);
  nextEntries.push(nextEntry);
  return sortWorkspaceEntries(nextEntries);
}

function entryStorePath(entry: DatasetRegistryEntry | null): string {
  return entry?.imported_store_path ?? entry?.preferred_store_path ?? "";
}

function cloneSessionPipelines(
  entries: WorkspacePipelineEntry[] | null | undefined
): WorkspacePipelineEntry[] | null {
  return entries ? structuredClone(entries) : null;
}

function datasetCompareFamily(dataset: DatasetSummary | null): string | null {
  return dataset?.descriptor.geometry?.compare_family ?? null;
}

function datasetGeometryFingerprint(dataset: DatasetSummary | null): string | null {
  return dataset?.descriptor.geometry?.fingerprint ?? null;
}

function compareCandidateReason(
  primary: DatasetSummary | null,
  candidate: DatasetSummary | null,
  candidateStorePath: string,
  isPrimary: boolean
): CompareCompatibilityReason | null {
  if (isPrimary) {
    return null;
  }

  if (!primary) {
    return "primary_unset";
  }

  if (!candidateStorePath) {
    return "missing_store_path";
  }

  if (!candidate) {
    return "missing_dataset";
  }

  const primaryGeometry = primary.descriptor.geometry;
  const candidateGeometry = candidate.descriptor.geometry;

  if (!primaryGeometry || !candidateGeometry) {
    return "missing_geometry_descriptor";
  }

  if (candidateGeometry.compare_family !== primaryGeometry.compare_family) {
    return "compare_family_mismatch";
  }

  if (candidateGeometry.fingerprint !== primaryGeometry.fingerprint) {
    return "geometry_fingerprint_mismatch";
  }

  return null;
}

export class ViewerModel {
  readonly tauriRuntime: boolean;

  inputPath = $state("");
  outputStorePath = $state("");
  activeStorePath = $state("");
  dataset = $state<DatasetSummary | null>(null);
  preflight = $state<SurveyPreflightResponse | null>(null);
  importGeometryRecovery = $state.raw<ImportGeometryRecoveryState | null>(null);
  axis = $state<SectionAxis>("inline");
  index = $state(0);
  section = $state.raw<DisplaySectionView | null>(null);
  sectionHorizons = $state.raw<GeovizSectionHorizonOverlay[]>([]);
  importedHorizons = $state.raw<ImportedHorizonDescriptor[]>([]);
  backgroundSection = $state.raw<DisplaySectionView | null>(null);
  loading = $state(false);
  backgroundLoading = $state(false);
  horizonImporting = $state(false);
  segyExporting = $state(false);
  busyLabel = $state<string | null>(null);
  error = $state<string | null>(null);
  backgroundError = $state<string | null>(null);
  resetToken = $state("inline:0");
  displayTransform = $state({
    renderMode: "heatmap" as "heatmap" | "wiggle",
    colormap: "grayscale" as "grayscale" | "red-white-blue",
    gain: 1,
    polarity: "normal" as "normal" | "reversed",
    clipMin: undefined as number | undefined,
    clipMax: undefined as number | undefined
  });
  chartTool = $state<SeismicChartTool>("crosshair");
  lastProbe = $state<SectionProbeChanged | null>(null);
  lastViewport = $state<SectionViewportChanged | null>(null);
  lastInteraction = $state<SectionInteractionChanged | null>(null);
  diagnosticsStatus = $state<DiagnosticsStatus | null>(null);
  verboseDiagnostics = $state(false);
  backendEvents = $state<DiagnosticsEvent[]>([]);
  recentActivity = $state<ViewerActivity[]>([]);
  lastImportedInputPath = $state("");
  lastImportedStorePath = $state("");
  workspaceEntries = $state.raw<DatasetRegistryEntry[]>([]);
  activeEntryId = $state<string | null>(null);
  selectedPresetId = $state<string | null>(null);
  displayCoordinateReferenceId = $state<string | null>(null);
  surveyMapSource = $state.raw<ResolvedSurveyMapSourceDto | null>(null);
  surveyMapLoading = $state(false);
  surveyMapError = $state<string | null>(null);
  nativeCoordinateReferenceOverrideIdDraft = $state("");
  nativeCoordinateReferenceOverrideNameDraft = $state("");
  workspaceReady = $state(false);
  restoringWorkspace = $state(false);
  compareBackgroundStorePath = $state<string | null>(null);
  compareSplitEnabled = $state(false);
  compareSplitPosition = $state(0.5);

  #activityCounter = 0;
  #diagnosticsUnlisten: (() => void) | null = null;
  #outputPathSource: "auto" | "manual" = "auto";
  #backgroundLoadRequestId = 0;
  #backgroundSectionKey: string | null = null;
  #surveyMapRequestId = 0;
  #copiedWorkspaceEntry: DatasetRegistryEntry | null = null;
  #workspaceEntryCounter = 0;

  constructor(options: ViewerModelOptions) {
    this.tauriRuntime = options.tauriRuntime;

    $effect(() => {
      const backgroundStorePath = this.compareBackgroundStorePath;
      const foregroundStorePath = this.comparePrimaryStorePath;
      const axis = this.axis;
      const index = this.index;

      if (!backgroundStorePath || !foregroundStorePath || backgroundStorePath === foregroundStorePath) {
        this.backgroundSection = null;
        this.backgroundError = null;
        this.backgroundLoading = false;
        this.#backgroundSectionKey = null;
        return;
      }

      const nextKey = `${backgroundStorePath}:${axis}:${index}`;
      if (this.#backgroundSectionKey === nextKey) {
        return;
      }

      void this.loadBackgroundSection(backgroundStorePath, axis, index);
    });

    $effect(() => {
      const splitAllowed =
        this.compareSplitEnabled &&
        !!this.activeBackgroundCompareCandidate &&
        this.displayTransform.renderMode === "heatmap";

      if (!splitAllowed && this.compareSplitEnabled) {
        this.compareSplitEnabled = false;
      }
    });
  }

  #nextActivityId(): number {
    this.#activityCounter += 1;
    return this.#activityCounter;
  }

  note = (
    message: string,
    source: ViewerActivity["source"] = "ui",
    level: ViewerActivity["level"] = "info",
    detail: string | null = null
  ): void => {
    this.recentActivity = capEntries(
      this.recentActivity,
      {
        id: this.#nextActivityId(),
        timestamp: timestampLabel(),
        source,
        level,
        message,
        detail
      },
      24
    );
  };

  get activeDatasetEntry(): DatasetRegistryEntry | null {
    return this.workspaceEntries.find((entry) => entry.entry_id === this.activeEntryId) ?? null;
  }

  get activeDatasetDisplayName(): string {
    const activeEntry = this.activeDatasetEntry;
    return userVisibleDatasetName(
      activeEntry?.display_name ?? this.dataset?.descriptor.label ?? null,
      activeEntry?.source_path ?? null,
      activeEntry?.imported_store_path ?? activeEntry?.preferred_store_path ?? this.activeStorePath ?? null,
      activeEntry?.entry_id ?? this.dataset?.descriptor.id ?? "dataset"
    );
  }

  get canExportSegy(): boolean {
    return this.tauriRuntime && !!trimPath(this.activeStorePath) && !!this.dataset && !this.segyExporting;
  }

  get comparePrimaryDataset(): DatasetSummary | null {
    return this.dataset ?? this.activeDatasetEntry?.last_dataset ?? null;
  }

  get comparePrimaryStorePath(): string | null {
    const activeStorePath = trimPath(this.activeStorePath);
    if (activeStorePath) {
      return activeStorePath;
    }

    const fallbackStorePath = trimPath(entryStorePath(this.activeDatasetEntry));
    return fallbackStorePath || null;
  }

  get comparePoolState(): ComparePoolState {
    const primary = this.comparePrimaryDataset;
    const primaryStorePath = this.comparePrimaryStorePath;
    const primaryFingerprint = datasetGeometryFingerprint(primary);
    const primaryCompareFamily = datasetCompareFamily(primary);

    const candidates = this.workspaceEntries.map((entry) => {
      const storePath = trimPath(entryStorePath(entry));
      const candidateDataset = entry.last_dataset;
      const isPrimary = !!primaryStorePath && storePath === primaryStorePath;
      const reason = compareCandidateReason(primary, candidateDataset, storePath, isPrimary);

      return {
        entryId: entry.entry_id,
        displayName: userVisibleDatasetName(
          entry.display_name,
          entry.source_path,
          entry.imported_store_path ?? entry.preferred_store_path,
          entry.entry_id
        ),
        storePath,
        datasetId: candidateDataset?.descriptor.id ?? null,
        compareFamily: datasetCompareFamily(candidateDataset),
        fingerprint: datasetGeometryFingerprint(candidateDataset),
        compatible: reason === null,
        isPrimary,
        reason
      } satisfies CompareCandidate;
    });

    const compatibleStorePaths = candidates
      .filter((candidate) => candidate.compatible)
      .map((candidate) => candidate.storePath);

    const compatibleSecondaryStorePaths = candidates
      .filter((candidate) => candidate.compatible && !candidate.isPrimary)
      .map((candidate) => candidate.storePath);

    return {
      primaryStorePath,
      primaryDatasetId: primary?.descriptor.id ?? null,
      primaryLabel: this.activeDatasetDisplayName,
      compareFamily: primaryCompareFamily,
      fingerprint: primaryFingerprint,
      candidates,
      compatibleStorePaths,
      compatibleSecondaryStorePaths
    };
  }

  get compareCandidates(): CompareCandidate[] {
    return this.comparePoolState.candidates;
  }

  get compatibleCompareCandidates(): CompareCandidate[] {
    return this.compareCandidates.filter((candidate) => candidate.compatible);
  }

  get compatibleSecondaryCompareCandidates(): CompareCandidate[] {
    return this.compareCandidates.filter(
      (candidate) => candidate.compatible && !candidate.isPrimary
    );
  }

  get activeForegroundCompareCandidate(): CompareCandidate | null {
    const primaryStorePath = this.comparePrimaryStorePath;
    return this.compareCandidates.find((candidate) => candidate.storePath === primaryStorePath) ?? null;
  }

  get activeBackgroundCompareCandidate(): CompareCandidate | null {
    if (!this.compareBackgroundStorePath) {
      return null;
    }

    return (
      this.compatibleSecondaryCompareCandidates.find(
        (candidate) => candidate.storePath === this.compareBackgroundStorePath
      ) ?? null
    );
  }

  get canCycleForegroundCompareSurvey(): boolean {
    return this.compatibleCompareCandidates.length > 1;
  }

  get canEnableCompareSplit(): boolean {
    return !!this.activeBackgroundCompareCandidate && this.displayTransform.renderMode === "heatmap";
  }

  get activeCoordinateReferenceBinding() {
    return this.comparePrimaryDataset?.descriptor.coordinate_reference_binding ?? null;
  }

  get activeDetectedNativeCoordinateReferenceId(): string | null {
    return this.activeCoordinateReferenceBinding?.detected?.id ?? null;
  }

  get activeDetectedNativeCoordinateReferenceName(): string | null {
    return this.activeCoordinateReferenceBinding?.detected?.name ?? null;
  }

  get activeEffectiveNativeCoordinateReferenceId(): string | null {
    return this.activeCoordinateReferenceBinding?.effective?.id ?? null;
  }

  get activeEffectiveNativeCoordinateReferenceName(): string | null {
    return this.activeCoordinateReferenceBinding?.effective?.name ?? null;
  }

  get activeSurveyMapSurvey() {
    return this.surveyMapSource?.surveys[0] ?? null;
  }

  get workspaceCoordinateReferenceWarnings(): string[] {
    const warnings: string[] = [];
    const activeDataset = this.comparePrimaryDataset;
    if (!activeDataset) {
      return warnings;
    }

    if (!this.activeEffectiveNativeCoordinateReferenceId) {
      warnings.push("Active survey native CRS is unknown. Assign an override before relying on cross-survey map alignment.");
    }

    if (this.displayCoordinateReferenceId && !this.activeEffectiveNativeCoordinateReferenceId) {
      warnings.push(`Display CRS ${this.displayCoordinateReferenceId} is set, but the active survey has no effective native CRS.`);
    } else if (
      this.displayCoordinateReferenceId &&
      this.activeEffectiveNativeCoordinateReferenceId &&
      this.displayCoordinateReferenceId.toLowerCase() !==
        this.activeEffectiveNativeCoordinateReferenceId.toLowerCase()
    ) {
      const transformStatus = this.activeSurveyMapSurvey?.transform_status ?? "native_only";
      if (transformStatus === "display_unavailable") {
        warnings.push(
          `Display CRS ${this.displayCoordinateReferenceId} differs from active survey native CRS ${this.activeEffectiveNativeCoordinateReferenceId}, but no display transform is currently available.`
        );
      } else if (transformStatus === "display_degraded") {
        warnings.push(
          `Display CRS ${this.displayCoordinateReferenceId} differs from active survey native CRS ${this.activeEffectiveNativeCoordinateReferenceId}. The current map preview uses a degraded transform.`
        );
      } else if (transformStatus === "native_only") {
        warnings.push(
          `Display CRS ${this.displayCoordinateReferenceId} differs from active survey native CRS ${this.activeEffectiveNativeCoordinateReferenceId}. The current map preview is still in native coordinates.`
        );
      }
    }

    if (this.surveyMapError) {
      warnings.push(this.surveyMapError);
    }

    return warnings;
  }

  selectCompareBackground = (storePath: string | null): void => {
    const normalizedStorePath = trimPath(storePath ?? "") || null;

    if (!normalizedStorePath) {
      this.compareBackgroundStorePath = null;
      this.backgroundSection = null;
      this.backgroundError = null;
      this.backgroundLoading = false;
      this.#backgroundSectionKey = null;
      this.compareSplitEnabled = false;
      return;
    }

    const selectedCandidate = this.compatibleSecondaryCompareCandidates.find(
      (candidate) => candidate.storePath === normalizedStorePath
    );

    this.compareBackgroundStorePath = selectedCandidate?.storePath ?? null;
  };

  setCompareSplitEnabled = (enabled: boolean): void => {
    if (!enabled) {
      this.compareSplitEnabled = false;
      return;
    }

    if (!this.canEnableCompareSplit) {
      return;
    }

    this.compareSplitEnabled = true;
  };

  setCompareSplitPosition = (position: number): void => {
    this.compareSplitPosition = Math.min(Math.max(position, 0.1), 0.9);
  };

  cycleForegroundCompareSurvey = async (direction: -1 | 1): Promise<void> => {
    const candidates = this.compatibleCompareCandidates;
    if (candidates.length <= 1 || this.loading) {
      return;
    }

    const primaryStorePath = this.comparePrimaryStorePath;
    const currentIndex = candidates.findIndex((candidate) => candidate.storePath === primaryStorePath);
    const baseIndex = currentIndex >= 0 ? currentIndex : 0;
    const nextIndex = (baseIndex + direction + candidates.length) % candidates.length;
    const nextCandidate = candidates[nextIndex];

    if (!nextCandidate || !nextCandidate.storePath || nextCandidate.storePath === primaryStorePath) {
      return;
    }

    this.note("Cycling compare foreground survey.", "ui", "info", nextCandidate.displayName);
    await this.activateDatasetEntry(nextCandidate.entryId);
  };

  refreshCompareSelection = (): void => {
    if (!this.compareBackgroundStorePath) {
      return;
    }

    const stillCompatible = this.compatibleSecondaryCompareCandidates.some(
      (candidate) => candidate.storePath === this.compareBackgroundStorePath
    );

    if (!stillCompatible) {
      this.compareBackgroundStorePath = null;
      this.compareSplitEnabled = false;
      this.backgroundSection = null;
      this.backgroundError = null;
      this.backgroundLoading = false;
      this.#backgroundSectionKey = null;
    }
  };

  refreshSurveyMap = async (): Promise<void> => {
    const requestId = ++this.#surveyMapRequestId;
    const storePath = this.comparePrimaryStorePath;

    if (!storePath) {
      this.surveyMapSource = null;
      this.surveyMapError = null;
      this.surveyMapLoading = false;
      return;
    }

    if (!this.tauriRuntime) {
      this.surveyMapSource = null;
      this.surveyMapError = null;
      this.surveyMapLoading = false;
      return;
    }

    this.surveyMapLoading = true;
    this.surveyMapError = null;

    try {
      const response = await resolveSurveyMap({
        schema_version: 1,
        store_path: storePath,
        display_coordinate_reference_id: this.displayCoordinateReferenceId
      });

      if (requestId !== this.#surveyMapRequestId) {
        return;
      }

      this.surveyMapSource = response.survey_map;
      this.surveyMapError = null;
    } catch (error) {
      if (requestId !== this.#surveyMapRequestId) {
        return;
      }

      this.surveyMapSource = null;
      this.surveyMapError = errorMessage(error, "Failed to resolve the active survey map.");
      this.note("Failed to resolve survey map geometry.", "backend", "warn", this.surveyMapError);
    } finally {
      if (requestId === this.#surveyMapRequestId) {
        this.surveyMapLoading = false;
      }
    }
  };

  private async loadBackgroundSection(
    storePath: string,
    axis: SectionAxis,
    index: number
  ): Promise<void> {
    const requestId = ++this.#backgroundLoadRequestId;
    const sectionKey = `${storePath}:${axis}:${index}`;
    this.backgroundLoading = true;
    this.backgroundError = null;

    try {
      const section = await fetchSectionView(storePath, axis, index);
      if (requestId !== this.#backgroundLoadRequestId) {
        return;
      }

      this.backgroundSection = section;
      this.#backgroundSectionKey = sectionKey;
    } catch (error) {
      if (requestId !== this.#backgroundLoadRequestId) {
        return;
      }

      this.backgroundSection = null;
      this.#backgroundSectionKey = null;
      this.backgroundError = errorMessage(error, "Failed to load compare background section.");
      this.compareSplitEnabled = false;
      this.note("Failed to load compare background section.", "backend", "warn", this.backgroundError);
    } finally {
      if (requestId === this.#backgroundLoadRequestId) {
        this.backgroundLoading = false;
      }
    }
  }

  setSelectedPresetId = (presetId: string | null): void => {
    this.selectedPresetId = presetId?.trim() || null;
    if (!this.workspaceReady) {
      return;
    }
    void this.persistWorkspaceSession();
  };

  setDisplayCoordinateReferenceId = (coordinateReferenceId: string | null): void => {
    this.displayCoordinateReferenceId = coordinateReferenceId?.trim() || null;
    if (!this.workspaceReady) {
      void this.refreshSurveyMap();
      return;
    }
    void this.refreshSurveyMap();
    void this.persistWorkspaceSession();
  };

  #applyWorkspaceSession = (session: WorkspaceSession): void => {
    this.activeEntryId = session.active_entry_id;
    this.selectedPresetId = session.selected_preset_id;
    this.displayCoordinateReferenceId = session.display_coordinate_reference_id;
    this.axis = session.active_axis;
    this.index = session.active_index;
  };

  #applyWorkspaceEntry = (entry: DatasetRegistryEntry | null): void => {
    if (!entry) {
      this.nativeCoordinateReferenceOverrideIdDraft = "";
      this.nativeCoordinateReferenceOverrideNameDraft = "";
      return;
    }

    const sourcePath = entry.source_path ?? "";
    const storePath = entryStorePath(entry);
    this.inputPath = sourcePath;
    this.outputStorePath = storePath;
    this.activeStorePath = entry.imported_store_path ?? this.activeStorePath;
    this.#outputPathSource = storePath ? "manual" : "auto";
    this.error = null;
    this.preflight = null;
    this.nativeCoordinateReferenceOverrideIdDraft =
      entry.last_dataset?.descriptor.coordinate_reference_binding?.effective?.id ?? "";
    this.nativeCoordinateReferenceOverrideNameDraft =
      entry.last_dataset?.descriptor.coordinate_reference_binding?.effective?.name ?? "";
  };

  #clearLoadedDataset = (): void => {
    this.activeStorePath = "";
    this.dataset = null;
    this.surveyMapSource = null;
    this.surveyMapError = null;
    this.surveyMapLoading = false;
    this.section = null;
    this.backgroundSection = null;
    this.lastProbe = null;
    this.lastViewport = null;
    this.lastInteraction = null;
    this.resetToken = `${this.axis}:${this.index}`;
    this.compareBackgroundStorePath = null;
    this.compareSplitEnabled = false;
    this.compareSplitPosition = 0.5;
    this.backgroundError = null;
    this.backgroundLoading = false;
    this.#backgroundSectionKey = null;
    this.nativeCoordinateReferenceOverrideIdDraft = "";
    this.nativeCoordinateReferenceOverrideNameDraft = "";
  };

  #syncWorkspaceState = (entries: DatasetRegistryEntry[], session: WorkspaceSession): void => {
    this.workspaceEntries = sortWorkspaceEntries(entries);
    this.#applyWorkspaceSession(session);
    this.#applyWorkspaceEntry(
      this.workspaceEntries.find((entry) => entry.entry_id === session.active_entry_id) ?? null
    );
    this.refreshCompareSelection();
    this.workspaceReady = true;
    void this.refreshSurveyMap();
  };

  updateActiveEntryPipelines = async (
    sessionPipelines: WorkspacePipelineEntry[],
    activeSessionPipelineId: string | null
  ): Promise<void> => {
    const activeEntry = this.activeDatasetEntry;
    if (!activeEntry) {
      return;
    }

    try {
      const response = await upsertDatasetEntry({
        schema_version: 1,
        entry_id: activeEntry.entry_id,
        display_name: activeEntry.display_name,
        source_path: activeEntry.source_path,
        preferred_store_path: activeEntry.preferred_store_path,
        imported_store_path: activeEntry.imported_store_path,
        dataset: activeEntry.last_dataset,
        session_pipelines: sessionPipelines,
        active_session_pipeline_id: activeSessionPipelineId,
        make_active: true
      });
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, response.entry);
      this.#applyWorkspaceSession(response.session);
    } catch (error) {
      this.note(
        "Failed to persist session pipelines for the active dataset.",
        "backend",
        "warn",
        errorMessage(error, "Unknown pipeline workspace error")
      );
    }
  };

  refreshWorkspaceState = async (): Promise<void> => {
    const response = await loadWorkspaceState();
    this.#syncWorkspaceState(response.entries, response.session);
  };

  persistWorkspaceSession = async (): Promise<void> => {
    if (!this.workspaceReady) {
      return;
    }

    try {
      const response = await saveWorkspaceSession({
        schema_version: 1,
        active_entry_id: this.activeEntryId,
        active_store_path: trimPath(this.activeStorePath) || null,
        active_axis: this.axis,
        active_index: this.index,
        selected_preset_id: this.selectedPresetId,
        display_coordinate_reference_id: this.displayCoordinateReferenceId
      });
      this.#applyWorkspaceSession(response.session);
    } catch (error) {
      this.note(
        "Failed to persist workspace session state.",
        "backend",
        "warn",
        errorMessage(error, "Unknown workspace session error")
      );
    }
  };

  setActiveDatasetNativeCoordinateReference = async (
    coordinateReferenceId: string | null,
    coordinateReferenceName: string | null
  ): Promise<void> => {
    const storePath = this.comparePrimaryStorePath;
    if (!storePath) {
      this.note("Native CRS override blocked because no active runtime store is available.", "ui", "warn");
      return;
    }

    try {
      const response = await setDatasetNativeCoordinateReference({
        schema_version: 1,
        store_path: storePath,
        coordinate_reference_id: coordinateReferenceId?.trim() || null,
        coordinate_reference_name: coordinateReferenceName?.trim() || null
      });
      this.dataset = response.dataset;
      const activeEntry = this.activeDatasetEntry;
      if (activeEntry) {
        this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, {
          ...activeEntry,
          imported_store_path: activeEntry.imported_store_path ?? response.dataset.store_path,
          last_dataset: response.dataset
        });
      }
      this.nativeCoordinateReferenceOverrideIdDraft =
        response.dataset.descriptor.coordinate_reference_binding?.effective?.id ?? "";
      this.nativeCoordinateReferenceOverrideNameDraft =
        response.dataset.descriptor.coordinate_reference_binding?.effective?.name ?? "";
      void this.refreshSurveyMap();
      this.note(
        coordinateReferenceId?.trim()
          ? "Updated active dataset native CRS override."
          : "Cleared active dataset native CRS override.",
        "backend",
        "info",
        coordinateReferenceId?.trim() || null
      );
    } catch (error) {
      this.note(
        "Failed to update the active dataset native CRS override.",
        "backend",
        "error",
        errorMessage(error, "Unknown CRS override error")
      );
    }
  };

  setInputPath = (inputPath: string): void => {
    const normalizedPath = trimPath(inputPath);
    const previousInputPath = this.inputPath;
    const previousOutputStorePath = this.outputStorePath;
    const suggestedStorePath = deriveStorePathFromInput(normalizedPath);
    const shouldReplaceOutputPath =
      !previousOutputStorePath ||
      this.#outputPathSource === "auto" ||
      trimPath(previousOutputStorePath) === trimPath(this.lastImportedStorePath);

    this.inputPath = normalizedPath;
    this.preflight = null;
    this.importGeometryRecovery = null;
    this.error = null;

    if (shouldReplaceOutputPath && suggestedStorePath && suggestedStorePath !== previousOutputStorePath) {
      this.outputStorePath = suggestedStorePath;
      this.#outputPathSource = "auto";
      this.note("Suggested runtime store output path from the selected SEG-Y file.", "ui", "info", suggestedStorePath);
    }

    if (
      previousInputPath &&
      previousInputPath !== normalizedPath &&
      previousOutputStorePath &&
      previousOutputStorePath === this.lastImportedStorePath
    ) {
      this.note(
        "Replaced the previous active store path with a new suggested output path for the selected SEG-Y file.",
        "ui",
        "info",
        `${previousOutputStorePath} -> ${this.outputStorePath}`
      );
    }

    this.note("Selected SEG-Y input path.", "ui", "info", normalizedPath);
  };

  openVolumePath = async (volumePath: string): Promise<void> => {
    const normalizedPath = trimPath(volumePath);
    if (!normalizedPath) {
      this.error = "Volume path is required.";
      this.note("Open-volume blocked because no usable path was provided.", "ui", "error");
      return;
    }

    const extension = fileExtension(normalizedPath);
    const hasActiveDataset = Boolean(this.dataset && trimPath(this.activeStorePath));
    const shouldActivateOpenedVolume = !hasActiveDataset;
    if (extension === ".tbvol") {
      const matchingEntry =
        this.workspaceEntries.find(
          (entry) =>
            trimPath(entry.imported_store_path ?? entry.preferred_store_path ?? "") === normalizedPath
        ) ?? null;
      await this.openDatasetAt(normalizedPath, "inline", 0, {
        entryId: matchingEntry?.entry_id ?? null,
        sourcePath: matchingEntry?.source_path ?? null,
        sessionPipelines: cloneSessionPipelines(matchingEntry?.session_pipelines),
        activeSessionPipelineId: matchingEntry?.active_session_pipeline_id ?? null,
        makeActive: shouldActivateOpenedVolume,
        loadSection: shouldActivateOpenedVolume
      });
      return;
    }

    if (extension !== ".sgy" && extension !== ".segy") {
      this.error = "TraceBoost currently supports opening .tbvol, .sgy, and .segy volumes.";
      this.note("Open-volume blocked because the selected file type is unsupported.", "ui", "error", normalizedPath);
      return;
    }

    const matchingEntry =
      this.workspaceEntries.find((entry) => trimPath(entry.source_path ?? "") === normalizedPath) ?? null;
    const existingImportedStore = trimPath(matchingEntry?.imported_store_path ?? "");
    if (existingImportedStore) {
      this.note("Reusing existing imported runtime store for the selected SEG-Y volume.", "ui", "info", existingImportedStore);
      await this.openDatasetAt(existingImportedStore, "inline", 0, {
        entryId: matchingEntry?.entry_id ?? null,
        sourcePath: normalizedPath,
        sessionPipelines: cloneSessionPipelines(matchingEntry?.session_pipelines),
        activeSessionPipelineId: matchingEntry?.active_session_pipeline_id ?? null,
        makeActive: shouldActivateOpenedVolume,
        loadSection: shouldActivateOpenedVolume
      });
      return;
    }

    this.loading = true;
    this.busyLabel = "Inspecting volume";
    this.error = null;
    this.preflight = null;
    this.importGeometryRecovery = null;
    this.note("Started one-shot volume import.", "ui", "info", normalizedPath);

    try {
      const preflight = await preflightImport(normalizedPath);
      this.preflight = preflight;

      const outputStorePath =
        trimPath(matchingEntry?.imported_store_path ?? matchingEntry?.preferred_store_path ?? "") ||
        (await defaultImportStorePath(normalizedPath));

      if (!canAutoImportPreflight(preflight)) {
        this.loading = false;
        this.busyLabel = null;

        if (canRecoverPreflight(preflight)) {
          this.error = null;
          this.openImportGeometryRecovery(preflight, {
            inputPath: normalizedPath,
            outputStorePath,
            entryId: matchingEntry?.entry_id ?? null,
            sourcePath: normalizedPath,
            sessionPipelines: cloneSessionPipelines(matchingEntry?.session_pipelines),
            activeSessionPipelineId: matchingEntry?.active_session_pipeline_id ?? null,
            makeActive: shouldActivateOpenedVolume,
            loadSection: shouldActivateOpenedVolume,
            reuseExistingStore: true
          });
          this.note(
            "SEG-Y import requires geometry review; opened the mapping recovery dialog.",
            "ui",
            "warn",
            describePreflight(preflight)
          );
          return;
        }

        throw new Error(
          `This SEG-Y survey cannot be opened automatically yet. Resolved layout: ${describePreflight(preflight)}. Suggested action: ${preflight.suggested_action}.`
        );
      }
      this.loading = false;
      this.busyLabel = null;
      await this.importDataset({
        inputPath: normalizedPath,
        outputStorePath,
        entryId: matchingEntry?.entry_id ?? null,
        sourcePath: normalizedPath,
        sessionPipelines: cloneSessionPipelines(matchingEntry?.session_pipelines),
        activeSessionPipelineId: matchingEntry?.active_session_pipeline_id ?? null,
        makeActive: shouldActivateOpenedVolume,
        loadSection: shouldActivateOpenedVolume,
        reuseExistingStore: true
      });
    } catch (error) {
      this.loading = false;
      this.busyLabel = null;
      this.error = errorMessage(error, "Failed to open the selected volume.");
      this.note("One-shot volume open failed.", "backend", "error", this.error);
    }
  };

  openImportGeometryRecovery = (
    preflight: SurveyPreflightResponse,
    importOptions: ImportDatasetOptions
  ): void => {
    const preferredIndex = suggestedCandidateIndex(preflight);
    const initialGeometry =
      preferredIndex >= 0
        ? preflight.geometry_candidates[preferredIndex]?.geometry
        : preflight.suggested_geometry_override ?? preflight.resolved_geometry;
    this.importGeometryRecovery = {
      inputPath: trimPath(importOptions.inputPath ?? ""),
      outputStorePath: trimPath(importOptions.outputStorePath ?? ""),
      preflight,
      importOptions: {
        ...importOptions,
        inputPath: trimPath(importOptions.inputPath ?? ""),
        outputStorePath: trimPath(importOptions.outputStorePath ?? "")
      },
      mode: preferredIndex >= 0 ? "candidate" : "manual",
      selectedCandidateIndex: preferredIndex,
      draft: geometryOverrideDraft(initialGeometry),
      working: false,
      error: null
    };
  };

  closeImportGeometryRecovery = (): void => {
    if (this.importGeometryRecovery?.working) {
      return;
    }
    this.importGeometryRecovery = null;
  };

  selectImportGeometryCandidate = (candidateIndex: number): void => {
    const state = this.importGeometryRecovery;
    if (!state || !state.preflight.geometry_candidates[candidateIndex]) {
      return;
    }
    const candidate = state.preflight.geometry_candidates[candidateIndex];
    this.importGeometryRecovery = {
      ...state,
      mode: "candidate",
      selectedCandidateIndex: candidateIndex,
      draft: geometryOverrideDraft(candidate.geometry),
      error: null
    };
  };

  setImportGeometryRecoveryMode = (mode: "candidate" | "manual"): void => {
    const state = this.importGeometryRecovery;
    if (!state) {
      return;
    }
    this.importGeometryRecovery = {
      ...state,
      mode,
      error: null
    };
  };

  setImportGeometryRecoveryDraft = (
    field: keyof GeometryOverrideDraft,
    value: string | SegyHeaderValueType
  ): void => {
    const state = this.importGeometryRecovery;
    if (!state) {
      return;
    }
    this.importGeometryRecovery = {
      ...state,
      mode: "manual",
      draft: {
        ...state.draft,
        [field]: value
      },
      error: null
    };
  };

  confirmImportGeometryRecovery = async (): Promise<void> => {
    const state = this.importGeometryRecovery;
    if (!state) {
      return;
    }

    const selectedCandidate =
      state.mode === "candidate" && state.selectedCandidateIndex >= 0
        ? state.preflight.geometry_candidates[state.selectedCandidateIndex] ?? null
        : null;
    const geometryOverride =
      selectedCandidate?.geometry ?? geometryOverrideFromDraft(state.draft);

    if (!geometryOverride?.inline_3d || !geometryOverride.crossline_3d) {
      this.importGeometryRecovery = {
        ...state,
        error: "Both inline and crossline header mappings are required before import."
      };
      return;
    }

    this.importGeometryRecovery = {
      ...state,
      working: true,
      error: null
    };

    try {
      const validatedPreflight = await preflightImport(state.inputPath, geometryOverride);
      this.preflight = validatedPreflight;
      if (!canAutoImportPreflight(validatedPreflight)) {
        throw new Error(
          `The selected geometry mapping still resolves as ${describePreflight(validatedPreflight)}.`
        );
      }

      this.importGeometryRecovery = null;
      await this.importDataset({
        ...state.importOptions,
        inputPath: state.inputPath,
        outputStorePath: state.outputStorePath,
        geometryOverride
      });
    } catch (error) {
      const message = errorMessage(error, "Failed to validate the selected geometry mapping.");
      const current = this.importGeometryRecovery;
      if (current) {
        this.importGeometryRecovery = {
          ...current,
          working: false,
          error: message
        };
      }
      this.note("Geometry recovery import failed.", "backend", "error", message);
    }
  };

  selectInputPath = async (inputPath: string): Promise<void> => {
    this.setInputPath(inputPath);
    const normalizedInputPath = trimPath(this.inputPath);
    const existingEntry = this.activeDatasetEntry;
    const reuseActiveEntry = existingEntry?.source_path === normalizedInputPath;
    const matchingEntry =
      this.workspaceEntries.find((entry) => entry.source_path === normalizedInputPath) ?? null;

    if (!reuseActiveEntry) {
      const suggestedStorePath = entryStorePath(matchingEntry) || deriveStorePathFromInput(normalizedInputPath);
      this.outputStorePath = suggestedStorePath;
      this.#outputPathSource = matchingEntry && entryStorePath(matchingEntry) ? "manual" : "auto";
      this.#clearLoadedDataset();
    }

    try {
      const response = await upsertDatasetEntry({
        schema_version: 1,
        entry_id: reuseActiveEntry ? this.activeEntryId : matchingEntry?.entry_id ?? null,
        display_name: null,
        source_path: normalizedInputPath || null,
        preferred_store_path: trimPath(this.outputStorePath) || null,
        imported_store_path: reuseActiveEntry ? existingEntry?.imported_store_path ?? null : null,
        dataset: reuseActiveEntry ? existingEntry?.last_dataset ?? null : null,
        session_pipelines: reuseActiveEntry ? existingEntry?.session_pipelines ?? [] : matchingEntry?.session_pipelines ?? null,
        active_session_pipeline_id:
          reuseActiveEntry
            ? existingEntry?.active_session_pipeline_id ?? null
            : matchingEntry?.active_session_pipeline_id ?? null,
        make_active: true
      });
      this.activeEntryId = response.entry.entry_id;
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, response.entry);
      this.#applyWorkspaceSession(response.session);
      this.refreshCompareSelection();
    } catch (error) {
      this.note(
        "Failed to register the selected SEG-Y path in the workspace.",
        "backend",
        "error",
        errorMessage(error, "Unknown workspace registry error")
      );
    }
  };

  setOutputStorePath = (outputStorePath: string): void => {
    const normalizedPath = trimPath(outputStorePath);
    this.outputStorePath = normalizedPath;
    this.error = null;
    this.#outputPathSource = "manual";
    this.note("Selected runtime store output path.", "ui", "info", normalizedPath);
  };

  selectOutputStorePath = async (outputStorePath: string): Promise<void> => {
    this.setOutputStorePath(outputStorePath);
    if (!this.activeEntryId && !trimPath(this.inputPath)) {
      return;
    }

    try {
      const response = await upsertDatasetEntry({
        schema_version: 1,
        entry_id: this.activeEntryId,
        display_name: null,
        source_path: trimPath(this.inputPath) || null,
        preferred_store_path: trimPath(this.outputStorePath) || null,
        imported_store_path: this.activeDatasetEntry?.imported_store_path ?? null,
        dataset: this.activeDatasetEntry?.last_dataset ?? null,
        session_pipelines: this.activeDatasetEntry?.session_pipelines ?? null,
        active_session_pipeline_id: this.activeDatasetEntry?.active_session_pipeline_id ?? null,
        make_active: true
      });
      this.activeEntryId = response.entry.entry_id;
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, response.entry);
      this.#applyWorkspaceSession(response.session);
      this.refreshCompareSelection();
    } catch (error) {
      this.note(
        "Failed to persist the selected runtime store path.",
        "backend",
        "error",
        errorMessage(error, "Unknown workspace registry error")
      );
    }
  };

  get importIsRedundant(): boolean {
    return (
      trimPath(this.inputPath).length > 0 &&
      trimPath(this.outputStorePath).length > 0 &&
      trimPath(this.inputPath) === trimPath(this.lastImportedInputPath) &&
      trimPath(this.outputStorePath) === trimPath(this.lastImportedStorePath)
    );
  }

  get importDisabledReason(): string | null {
    if (!trimPath(this.inputPath) || !trimPath(this.outputStorePath)) {
      return "Select a SEG-Y file and output store path.";
    }

    if (this.importIsRedundant) {
      return "This SEG-Y is already imported to the selected runtime store. Change the file or output path to import again.";
    }

    return null;
  }

  setDiagnosticsStatus = (status: DiagnosticsStatus | null): void => {
    this.diagnosticsStatus = status;
    this.verboseDiagnostics = status?.verboseEnabled ?? this.verboseDiagnostics;
    if (status) {
      this.note("Connected to desktop diagnostics session.", "backend", "info", status.sessionLogPath);
    }
  };

  setVerboseDiagnostics = (enabled: boolean): void => {
    this.verboseDiagnostics = enabled;
  };

  addDiagnosticsEvent = (event: DiagnosticsEvent): void => {
    this.backendEvents = capEntries(this.backendEvents, event, 20);
  };

  setRenderMode = (renderMode: (typeof this.displayTransform)["renderMode"]): void => {
    this.displayTransform.renderMode = renderMode;
  };

  setColormap = (colormap: (typeof this.displayTransform)["colormap"]): void => {
    this.displayTransform.colormap = colormap;
  };

  setGain = (gain: number): void => {
    this.displayTransform.gain = gain;
  };

  setPolarity = (polarity: (typeof this.displayTransform)["polarity"]): void => {
    this.displayTransform.polarity = polarity;
  };

  setClipRange = (clipMin: number | undefined, clipMax: number | undefined): void => {
    this.displayTransform.clipMin = clipMin;
    this.displayTransform.clipMax = clipMax;
  };

  setChartTool = (tool: SeismicChartTool): void => {
    this.chartTool = tool;
  };

  setProbe = (event: SectionProbeChanged): void => {
    this.lastProbe = event;
  };

  setViewport = (event: SectionViewportChanged): void => {
    this.lastViewport = event;
  };

  setInteraction = (event: SectionInteractionChanged): void => {
    this.lastInteraction = event;
  };

  setInteractionState = (state: SeismicChartInteractionState): void => {
    this.chartTool = state.tool;
  };

  mountShell = (): (() => void) => {
    this.note(
      `App shell mounted in ${this.tauriRuntime ? "Tauri" : "browser"} mode.`,
      "viewer",
      "info"
    );

    if (!this.tauriRuntime) {
      return () => {};
    }

    let cancelled = false;

    void (async () => {
      const workspace = await loadWorkspaceState();
      if (cancelled) {
        return;
      }

      this.#syncWorkspaceState(workspace.entries, workspace.session);
      if (workspace.session.active_store_path) {
        this.restoringWorkspace = true;
        this.note("Restoring previous workspace dataset.", "viewer", "info", workspace.session.active_store_path);
        try {
          await this.openDatasetAt(
            workspace.session.active_store_path,
            workspace.session.active_axis,
            workspace.session.active_index
          );
        } catch (error) {
          this.note(
            "Failed to restore the previous active dataset automatically.",
            "backend",
            "warn",
            errorMessage(error, "Unknown workspace restore error")
          );
        } finally {
          this.restoringWorkspace = false;
        }
      }

      const status = await getDiagnosticsStatus();
      if (cancelled) {
        return;
      }

      this.setDiagnosticsStatus(status);
      const unlisten = await listenToDiagnosticsEvents((event) => {
        this.addDiagnosticsEvent(event);
      });

      if (cancelled) {
        unlisten();
        return;
      }

      this.#diagnosticsUnlisten = unlisten;
    })();

    return () => {
      cancelled = true;
      this.#diagnosticsUnlisten?.();
      this.#diagnosticsUnlisten = null;
    };
  };

  updateDiagnosticsVerbosity = async (enabled: boolean): Promise<void> => {
    this.setVerboseDiagnostics(enabled);
    this.note(
      enabled ? "Requested verbose backend diagnostics." : "Requested standard backend diagnostics.",
      "ui",
      "info"
    );

    try {
      await setDiagnosticsVerbosity(enabled);
    } catch (error) {
      this.setVerboseDiagnostics(!enabled);
      this.note(
        "Failed to update diagnostics verbosity.",
        "backend",
        "error",
        error instanceof Error ? error.message : "Unknown verbosity error"
      );
    }
  };

  activateDatasetEntry = async (entryId: string): Promise<void> => {
    try {
      const response = await setActiveDatasetEntry(entryId);
      this.activeEntryId = response.entry.entry_id;
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, response.entry);
      this.#applyWorkspaceSession(response.session);
      this.#applyWorkspaceEntry(response.entry);
      this.refreshCompareSelection();
      this.note("Activated dataset entry from the workspace list.", "ui", "info", response.entry.display_name);

      if (response.entry.imported_store_path) {
        await this.openDatasetAt(
          response.entry.imported_store_path,
          this.axis,
          this.index
        );
      }
    } catch (error) {
      this.error = errorMessage(error, "Failed to activate dataset entry.");
      this.note("Failed to activate dataset entry.", "backend", "error", this.error);
    }
  };

  removeWorkspaceEntry = async (entryId: string): Promise<void> => {
    try {
      const response = await removeDatasetEntry(entryId);
      const removedActive = this.activeEntryId === entryId;
      this.workspaceEntries = this.workspaceEntries.filter((entry) => entry.entry_id !== entryId);
      this.#applyWorkspaceSession(response.session);
      this.refreshCompareSelection();
      if (removedActive) {
        this.inputPath = "";
        this.outputStorePath = "";
        this.#clearLoadedDataset();
        this.preflight = null;
      }
      this.note("Removed dataset entry from the workspace list.", "ui", "warn", entryId);
    } catch (error) {
      this.error = errorMessage(error, "Failed to remove dataset entry.");
      this.note("Failed to remove dataset entry.", "backend", "error", this.error);
    }
  };

  copyActiveWorkspaceEntry = (): void => {
    const activeEntry = this.activeDatasetEntry;
    if (!activeEntry) {
      return;
    }

    this.#copiedWorkspaceEntry = structuredClone(activeEntry);
    this.note(
      "Copied active dataset entry.",
      "ui",
      "info",
      userVisibleDatasetName(
        activeEntry.display_name,
        activeEntry.source_path,
        activeEntry.imported_store_path ?? activeEntry.preferred_store_path,
        activeEntry.entry_id
      )
    );
  };

  pasteCopiedWorkspaceEntry = async (): Promise<void> => {
    const copiedEntry = this.#copiedWorkspaceEntry;
    if (!copiedEntry) {
      return;
    }

    const storePath = trimPath(entryStorePath(copiedEntry));
    if (!storePath) {
      this.note("Copied dataset entry has no runtime store path.", "ui", "warn", copiedEntry.entry_id);
      return;
    }

    const nextDisplayName = nextDuplicateName(
      userVisibleDatasetName(
        copiedEntry.display_name,
        copiedEntry.source_path,
        copiedEntry.imported_store_path ?? copiedEntry.preferred_store_path,
        copiedEntry.entry_id
      ),
      this.workspaceEntries.map((entry) =>
        userVisibleDatasetName(
          entry.display_name,
          entry.source_path,
          entry.imported_store_path ?? entry.preferred_store_path,
          entry.entry_id
        )
      )
    );

    await this.openDatasetAt(storePath, this.axis, this.index, {
      entryId: this.nextWorkspaceEntryId(),
      displayName: nextDisplayName,
      sourcePath: copiedEntry.source_path,
      sessionPipelines: cloneSessionPipelines(copiedEntry.session_pipelines),
      activeSessionPipelineId: copiedEntry.active_session_pipeline_id,
      makeActive: true,
      loadSection: true
    });
  };

  openDatasetAt = async (
    storePath: string,
    axis: SectionAxis = "inline",
    index = 0,
    options: OpenDatasetOptions = {}
  ): Promise<void> => {
    const normalizedStorePath = trimPath(storePath);
    if (!normalizedStorePath) {
      throw new Error("Store path is required.");
    }

    this.loading = true;
    this.busyLabel = this.restoringWorkspace ? "Restoring dataset" : "Opening dataset";
    this.error = null;
    this.note("Opening runtime store.", "ui", "info", normalizedStorePath);

    const hasOwnOption = (key: keyof OpenDatasetOptions): boolean =>
      Object.prototype.hasOwnProperty.call(options, key);
    const matchingActiveEntry =
      trimPath(entryStorePath(this.activeDatasetEntry)) === normalizedStorePath ? this.activeDatasetEntry : null;
    const nextEntryId: string | null = hasOwnOption("entryId")
      ? options.entryId ?? null
      : this.activeEntryId;
    const nextDisplayName: string | null = hasOwnOption("displayName")
      ? options.displayName ?? null
      : matchingActiveEntry?.display_name ?? null;
    const nextSourcePath: string = hasOwnOption("sourcePath")
      ? options.sourcePath ?? ""
      : matchingActiveEntry?.source_path ?? this.inputPath;
    const nextSessionPipelines: WorkspacePipelineEntry[] | null = hasOwnOption("sessionPipelines")
      ? cloneSessionPipelines(options.sessionPipelines)
      : this.activeDatasetEntry?.session_pipelines ?? null;
    const nextActiveSessionPipelineId: string | null = hasOwnOption("activeSessionPipelineId")
      ? options.activeSessionPipelineId ?? null
      : this.activeDatasetEntry?.active_session_pipeline_id ?? null;
    const makeActive = options.makeActive ?? true;
    const loadSection = options.loadSection ?? makeActive;

    try {
      const response = await openDataset(normalizedStorePath);

      const workspaceResponse = await upsertDatasetEntry({
        schema_version: 1,
        entry_id: nextEntryId,
        display_name:
          nextDisplayName?.trim() ||
          userVisibleDatasetName(
            response.dataset.descriptor.label,
            trimPath(nextSourcePath) || null,
            response.dataset.store_path,
            nextEntryId ?? response.dataset.descriptor.id
          ),
        source_path: trimPath(nextSourcePath) || null,
        preferred_store_path: response.dataset.store_path,
        imported_store_path: response.dataset.store_path,
        dataset: response.dataset,
        session_pipelines: nextSessionPipelines,
        active_session_pipeline_id: nextActiveSessionPipelineId,
        make_active: makeActive
      });
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, workspaceResponse.entry);
      this.refreshCompareSelection();

      if (makeActive) {
        this.dataset = response.dataset;
        this.activeStorePath = response.dataset.store_path;
        this.outputStorePath = response.dataset.store_path;
        this.#outputPathSource = "manual";
        this.inputPath = trimPath(nextSourcePath);
        this.error = null;
        this.activeEntryId = workspaceResponse.entry.entry_id;
        this.#applyWorkspaceSession(workspaceResponse.session);
        void this.refreshSurveyMap();

        this.note(
          "Runtime store opened.",
          "backend",
          "info",
          `${response.dataset.descriptor.label} @ ${response.dataset.store_path}`
        );
        if (loadSection) {
          await this.load(axis, index, response.dataset.store_path);
        } else {
          this.loading = false;
          this.busyLabel = null;
        }
      } else {
        this.loading = false;
        this.busyLabel = null;
        this.error = null;
        this.note(
          "Volume added to the workspace without changing the active seismic view.",
          "backend",
          "info",
          `${response.dataset.descriptor.label} @ ${response.dataset.store_path}`
        );
      }
    } catch (error) {
      this.loading = false;
      this.busyLabel = null;
      this.error = errorMessage(error, "Unknown open-store error");
      this.note("Opening runtime store failed.", "backend", "error", this.error);
      throw error;
    }
  };

  openDerivedDatasetAt = async (
    storePath: string,
    axis: SectionAxis = "inline",
    index = 0
  ): Promise<void> => {
    const activeEntry = this.activeDatasetEntry;
    await this.openDatasetAt(storePath, axis, index, {
      entryId: null,
      sourcePath: null,
      sessionPipelines: cloneSessionPipelines(activeEntry?.session_pipelines),
      activeSessionPipelineId: activeEntry?.active_session_pipeline_id ?? null
    });
  };

  private nextWorkspaceEntryId(): string {
    this.#workspaceEntryCounter += 1;
    return `dataset-copy-${Date.now()}-${this.#workspaceEntryCounter}`;
  }

  runPreflight = async (): Promise<void> => {
    const inputPath = this.inputPath.trim();
    this.loading = true;
    this.busyLabel = "Preflighting survey";
    this.error = null;
    this.note("Started survey preflight.", "ui", "info", inputPath || null);

    if (!inputPath) {
      this.loading = false;
      this.busyLabel = null;
      this.error = "Input SEG-Y path is required.";
      this.note("Preflight blocked because no SEG-Y path was provided.", "ui", "error");
      return;
    }

    try {
      const preflight = await preflightImport(inputPath);
      this.loading = false;
      this.busyLabel = null;
      this.preflight = preflight;
      this.error = null;
      this.note(
        `Preflight completed as ${describePreflight(preflight)}.`,
        "backend",
        preflight.suggested_action === "direct_dense_ingest" ? "info" : "warn",
        `Suggested action: ${preflight.suggested_action}`
      );
    } catch (error) {
      this.loading = false;
      this.busyLabel = null;
      this.error = error instanceof Error ? error.message : "Unknown preflight error";
      this.note(
        "Preflight failed.",
        "backend",
        "error",
        error instanceof Error ? error.message : "Unknown preflight error"
      );
    }
  };

  importDataset = async (options: ImportDatasetOptions = {}): Promise<void> => {
    const hasOwnOption = (key: keyof ImportDatasetOptions): boolean =>
      Object.prototype.hasOwnProperty.call(options, key);
    const inputPath = trimPath(hasOwnOption("inputPath") ? options.inputPath ?? "" : this.inputPath);
    const outputStorePath = trimPath(
      hasOwnOption("outputStorePath") ? options.outputStorePath ?? "" : this.outputStorePath
    );
    const nextEntryId: string | null = hasOwnOption("entryId") ? options.entryId ?? null : this.activeEntryId;
    const nextSourcePath = hasOwnOption("sourcePath") ? options.sourcePath ?? inputPath : inputPath;
    const nextSessionPipelines: WorkspacePipelineEntry[] | null = hasOwnOption("sessionPipelines")
      ? cloneSessionPipelines(options.sessionPipelines)
      : this.activeDatasetEntry?.session_pipelines ?? null;
    const nextActiveSessionPipelineId: string | null = hasOwnOption("activeSessionPipelineId")
      ? options.activeSessionPipelineId ?? null
      : this.activeDatasetEntry?.active_session_pipeline_id ?? null;
    const makeActive = options.makeActive ?? true;
    const loadSection = options.loadSection ?? makeActive;
    const reuseExistingStore = options.reuseExistingStore ?? false;
    const geometryOverride = hasOwnOption("geometryOverride") ? options.geometryOverride ?? null : null;
    this.loading = true;
    this.busyLabel = "Importing survey";
    this.error = null;
    this.note(
      "Started dataset import.",
      "ui",
      "info",
      `${inputPath || "(missing input)"} -> ${outputStorePath || "(missing output)"}`
    );

    if (!inputPath || !outputStorePath) {
      this.loading = false;
      this.busyLabel = null;
      this.error = "Both input SEG-Y path and output store path are required.";
      this.note("Import blocked because input or output path is missing.", "ui", "error");
      return;
    }

    try {
      let response: ImportDatasetResponse;

      try {
        response = await importDataset(inputPath, outputStorePath, false, geometryOverride);
      } catch (error) {
        const message = errorMessage(error, "Unknown import error");
        if (!isExistingStoreError(message)) {
          throw error;
        }

        if (reuseExistingStore) {
          this.loading = false;
          this.busyLabel = null;
          this.error = null;
          this.note(
            "An imported runtime store already exists for this SEG-Y file; reusing it instead of re-importing.",
            "backend",
            "info",
            outputStorePath
          );
          await this.openDatasetAt(outputStorePath, "inline", 0, {
            entryId: nextEntryId,
            sourcePath: nextSourcePath,
            sessionPipelines: nextSessionPipelines,
            activeSessionPipelineId: nextActiveSessionPipelineId,
            makeActive,
            loadSection
          });
          return;
        }

        this.loading = false;
        this.busyLabel = null;
        this.error = message;
        this.note(
          "Runtime store already exists; waiting for overwrite confirmation.",
          "backend",
          "warn",
          outputStorePath
        );

        const confirmed = await confirmOverwriteStore(outputStorePath);
        if (!confirmed) {
          this.error = "Import cancelled because the selected runtime store already exists.";
          this.note(
            "Overwrite of the existing runtime store was cancelled.",
            "ui",
            "warn",
            outputStorePath
          );
          return;
        }

        this.loading = true;
        this.busyLabel = "Overwriting runtime store";
        this.error = null;
        this.note("Confirmed overwrite of the existing runtime store.", "ui", "warn", outputStorePath);
        response = await importDataset(inputPath, outputStorePath, true, geometryOverride);
      }

      this.loading = false;
      this.busyLabel = null;
      this.lastImportedInputPath = inputPath;
      this.lastImportedStorePath = response.dataset.store_path;
      const workspaceResponse = await upsertDatasetEntry({
        schema_version: 1,
        entry_id: nextEntryId,
        display_name: userVisibleDatasetName(
          response.dataset.descriptor.label,
          trimPath(nextSourcePath) || null,
          response.dataset.store_path,
          nextEntryId ?? response.dataset.descriptor.id
        ),
        source_path: trimPath(nextSourcePath) || null,
        preferred_store_path: response.dataset.store_path,
        imported_store_path: response.dataset.store_path,
        dataset: response.dataset,
        session_pipelines: nextSessionPipelines,
        active_session_pipeline_id: nextActiveSessionPipelineId,
        make_active: makeActive
      });
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, workspaceResponse.entry);
      this.refreshCompareSelection();
      if (makeActive) {
        this.dataset = response.dataset;
        this.activeStorePath = response.dataset.store_path;
        this.outputStorePath = response.dataset.store_path;
        this.#outputPathSource = "manual";
        this.inputPath = inputPath;
        this.error = null;
        this.activeEntryId = workspaceResponse.entry.entry_id;
        this.#applyWorkspaceSession(workspaceResponse.session);
        void this.refreshSurveyMap();
        this.note(
          "Dataset import completed.",
          "backend",
          "info",
          `${response.dataset.descriptor.label} @ ${response.dataset.store_path}`
        );
        if (loadSection) {
          await this.load("inline", 0, response.dataset.store_path);
        }
      } else {
        this.error = null;
        this.note(
          "Survey import completed and the volume was added to the workspace without changing the active seismic view.",
          "backend",
          "info",
          `${response.dataset.descriptor.label} @ ${response.dataset.store_path}`
        );
      }
    } catch (error) {
      this.loading = false;
      this.busyLabel = null;
      this.error = errorMessage(error, "Unknown import error");
      this.note(
        "Dataset import failed.",
        "backend",
        "error",
        errorMessage(error, "Unknown import error")
      );
    }
  };

  openDataset = async (): Promise<void> => {
    const storePath = this.outputStorePath.trim() || this.activeStorePath.trim();
    if (!storePath) {
      this.error = "Store path is required.";
      this.note("Open-store blocked because no runtime store path was provided.", "ui", "error");
      return;
    }

    try {
      await this.openDatasetAt(storePath, "inline", 0);
    } catch (error) {
      this.error = errorMessage(error, "Unknown open-store error");
    }
  };

  importHorizonFiles = async (inputPaths: string[]): Promise<void> => {
    const activeStorePath = this.activeStorePath.trim();
    const normalizedPaths = inputPaths.map(trimPath).filter((value) => value.length > 0);
    if (!activeStorePath) {
      this.error = "Open a runtime store before importing horizons.";
      this.note("Horizon import blocked because no active runtime store is open.", "ui", "error");
      return;
    }
    if (normalizedPaths.length === 0) {
      return;
    }

    this.horizonImporting = true;
    this.error = null;
    this.note(
      "Started horizon xyz import.",
      "ui",
      "info",
      `${normalizedPaths.length} file${normalizedPaths.length === 1 ? "" : "s"}`
    );

    try {
      const response = await importHorizonXyz(activeStorePath, normalizedPaths);
      this.importedHorizons = response.imported;
      this.sectionHorizons = adaptSectionHorizonOverlays(
        await fetchSectionHorizons(activeStorePath, this.axis, this.index)
      );
      this.note(
        "Imported horizon xyz files into the active runtime store.",
        "backend",
        "info",
        response.imported.map((item) => item.name).join(", ")
      );
    } catch (error) {
      this.error = errorMessage(error, "Unknown horizon import error");
      this.note("Horizon import failed.", "backend", "error", this.error);
    } finally {
      this.horizonImporting = false;
    }
  };

  exportActiveDatasetSegy = async (): Promise<void> => {
    const activeStorePath = trimPath(this.activeStorePath);
    if (!this.tauriRuntime) {
      this.note("SEG-Y export is only available in the desktop app.", "ui", "warn");
      return;
    }
    if (!activeStorePath) {
      this.error = "Open a runtime store before exporting SEG-Y.";
      this.note("SEG-Y export blocked because no active runtime store is open.", "ui", "error");
      return;
    }

    const selectedOutputPath = await pickSegyExportPath(
      deriveSegyExportPathFromStore(activeStorePath) || "survey.export.sgy"
    );
    const outputPath = trimPath(selectedOutputPath ?? "");
    if (!outputPath) {
      this.note("SEG-Y export was cancelled before an output path was chosen.", "ui", "warn");
      return;
    }

    this.segyExporting = true;
    this.error = null;
    this.note("Started SEG-Y export.", "ui", "info", `${activeStorePath} -> ${outputPath}`);

    try {
      let response: ExportSegyResponse;

      try {
        response = await exportDatasetSegy(activeStorePath, outputPath, false);
      } catch (error) {
        const message = errorMessage(error, "Unknown SEG-Y export error");
        if (!isExistingSegyExportError(message)) {
          throw error;
        }

        this.note(
          "SEG-Y export target already exists; waiting for overwrite confirmation.",
          "backend",
          "warn",
          outputPath
        );
        const confirmed = await confirmOverwriteSegy(outputPath);
        if (!confirmed) {
          this.note("SEG-Y export overwrite was cancelled.", "ui", "warn", outputPath);
          return;
        }

        this.note("Confirmed overwrite of the existing SEG-Y export target.", "ui", "warn", outputPath);
        response = await exportDatasetSegy(activeStorePath, outputPath, true);
      }

      this.note("Exported active runtime store to SEG-Y.", "backend", "info", response.output_path);
    } catch (error) {
      this.error = errorMessage(error, "Failed to export SEG-Y from the active runtime store.");
      this.note("SEG-Y export failed.", "backend", "error", this.error);
    } finally {
      this.segyExporting = false;
    }
  };

  load = async (axis: SectionAxis, index: number, storePathOverride?: string): Promise<void> => {
    const activeStorePath = (storePathOverride ?? this.activeStorePath).trim();
    this.activeStorePath = storePathOverride ?? this.activeStorePath;
    this.axis = axis;
    this.index = index;
    this.loading = true;
    this.busyLabel = "Loading section";
    this.error = null;
    this.note("Requested section load.", "ui", "info", `${axis}:${index}`);

    if (!activeStorePath) {
      this.loading = false;
      this.busyLabel = null;
      this.error = "Open or import a dataset before loading sections.";
      this.sectionHorizons = [];
      this.note("Section load blocked because no active store is open.", "ui", "error");
      return;
    }

    try {
      const loadStartedMs = nowMs();
      const [section, sectionHorizons] = await Promise.all([
        fetchSectionView(activeStorePath, axis, index),
        fetchSectionHorizons(activeStorePath, axis, index)
      ]);
      const loadResolvedMs = nowMs();
      this.axis = axis;
      this.index = index;
      this.section = section;
      this.sectionHorizons = adaptSectionHorizonOverlays(sectionHorizons);
      const stateAssignedMs = nowMs();
      this.loading = false;
      this.busyLabel = null;
      this.error = null;
      this.resetToken = `${axis}:${index}`;
      await this.persistWorkspaceSession();
      const afterPersistMs = nowMs();
      await tick();
      const afterTickMs = nowMs();
      await nextAnimationFrame();
      const afterFirstFrameMs = nowMs();
      await nextAnimationFrame();
      const afterSecondFrameMs = nowMs();
      void emitFrontendDiagnosticsEvent({
        stage: "load_section",
        level: "info",
        message: "Frontend section load timings recorded",
        fields: {
          storePath: activeStorePath,
          datasetId: section.dataset_id,
          axis,
          index,
          traces: section.traces,
          samples: section.samples,
          payloadBytes: estimateSectionPayloadBytes(section),
          frontendAwaitMs: loadResolvedMs - loadStartedMs,
          frontendStateAssignMs: stateAssignedMs - loadResolvedMs,
          frontendPersistWorkspaceMs: afterPersistMs - stateAssignedMs,
          frontendTickMs: afterTickMs - afterPersistMs,
          frontendFirstFrameMs: afterFirstFrameMs - afterTickMs,
          frontendSecondFrameMs: afterSecondFrameMs - afterFirstFrameMs,
          frontendCommitToSecondFrameMs: afterSecondFrameMs - stateAssignedMs,
          frontendTotalMs: afterSecondFrameMs - loadStartedMs,
          frontendStage: "viewer_load_section"
        }
      }).catch((error) => {
        this.note(
          "Failed to record frontend section load timings.",
          "backend",
          "warn",
          error instanceof Error ? error.message : String(error)
        );
      });
      this.note(
        "Section payload loaded.",
        "backend",
        "info",
        `${axis}:${index} | traces=${section.traces} samples=${section.samples} coordinate=${section.coordinate.value}`
      );
    } catch (error) {
      this.axis = axis;
      this.index = index;
      this.loading = false;
      this.busyLabel = null;
      this.error = error instanceof Error ? error.message : "Unknown section load error";
      this.note(
        "Section load failed.",
        "backend",
        "error",
        error instanceof Error ? error.message : "Unknown section load error"
      );
    }
  };
}

const [internalGetViewerModelContext, internalSetViewerModelContext] = createContext<ViewerModel>();

export function getViewerModelContext(): ViewerModel {
  const viewerModel = internalGetViewerModelContext();

  if (!viewerModel) {
    throw new Error("Viewer model context not found");
  }

  return viewerModel;
}

export function setViewerModelContext(viewerModel: ViewerModel): ViewerModel {
  internalSetViewerModelContext(viewerModel);
  return viewerModel;
}
