import type {
  AmplitudeSpectrumRequest,
  AmplitudeSpectrumResponse,
  ExportSegyResponse,
  DatasetId,
  CancelProcessingJobResponse,
  DatasetRegistryEntry,
  DatasetRegistryStatus,
  GetProcessingJobResponse,
  ImportDatasetResponse,
  ImportHorizonXyzResponse,
  LoadWorkspaceStateResponse,
  ListPipelinePresetsResponse,
  OpenDatasetResponse,
  PreviewSubvolumeProcessingRequest,
  PreviewSubvolumeProcessingResponse,
  PreviewTraceLocalProcessingResponse as PreviewProcessingResponse,
  TraceLocalProcessingPreset as ProcessingPreset,
  RemoveDatasetEntryResponse,
  RunSubvolumeProcessingRequest,
  RunSubvolumeProcessingResponse,
  RunTraceLocalProcessingResponse as RunProcessingResponse,
  SaveWorkspaceSessionRequest,
  SaveWorkspaceSessionResponse,
  SavePipelinePresetResponse,
  SectionHorizonOverlayView,
  SegyGeometryOverride,
  SetDatasetNativeCoordinateReferenceRequest,
  SetDatasetNativeCoordinateReferenceResponse,
  SetActiveDatasetEntryResponse,
  SectionCoordinate,
  SectionAxis,
  SectionDisplayDefaults,
  SectionMetadata,
  SectionView,
  SectionUnits,
  SurveyPreflightResponse,
  PreviewTraceLocalProcessingRequest as PreviewProcessingRequest,
  ResolveSurveyMapRequest,
  ResolveSurveyMapResponse,
  RunTraceLocalProcessingRequest as RunProcessingRequest,
  SubvolumeProcessingPipeline,
  UpsertDatasetEntryRequest,
  UpsertDatasetEntryResponse,
  WorkspaceSession
} from "@traceboost/seis-contracts";
import { IPC_SCHEMA_VERSION as SCHEMA_VERSION } from "@traceboost/seis-contracts";

export interface DiagnosticsStatus {
  sessionId: string;
  sessionStartedAt: string;
  verboseEnabled: boolean;
  sessionLogPath: string;
}

export interface DiagnosticsEvent {
  sessionId: string;
  operationId: string;
  command: string;
  stage: string;
  level: string;
  timestamp: string;
  message: string;
  durationMs?: number | null;
  fields?: Record<string, unknown> | null;
}

export interface FrontendDiagnosticsEventRequest {
  stage: string;
  level: "debug" | "info" | "warn" | "error";
  message: string;
  fields?: Record<string, unknown> | null;
}

export type SectionBytePayload = Array<number> | Uint8Array;

export interface TransportSectionView
  extends Omit<
    SectionView,
    "horizontal_axis_f64le" | "inline_axis_f64le" | "xline_axis_f64le" | "sample_axis_f32le" | "amplitudes_f32le"
  > {
  horizontal_axis_f64le: SectionBytePayload;
  inline_axis_f64le: SectionBytePayload | null;
  xline_axis_f64le: SectionBytePayload | null;
  sample_axis_f32le: SectionBytePayload;
  amplitudes_f32le: SectionBytePayload;
}

export interface TransportPreviewView {
  section: TransportSectionView;
  processing_label: string;
  preview_ready: boolean;
}

export interface TransportPreviewProcessingResponse {
  preview: TransportPreviewView;
}

interface PackedPreviewSectionHeader {
  datasetId: DatasetId;
  axis: SectionAxis;
  coordinate: SectionCoordinate;
  traces: number;
  samples: number;
  horizontalAxisBytes: number;
  inlineAxisBytes: number | null;
  xlineAxisBytes: number | null;
  sampleAxisBytes: number;
  amplitudesBytes: number;
  units: SectionUnits | null;
  metadata: SectionMetadata | null;
  displayDefaults: SectionDisplayDefaults | null;
}

interface PackedSectionResponseHeader {
  section: PackedPreviewSectionHeader;
}

interface PackedPreviewResponseHeader {
  previewReady: boolean;
  processingLabel: string;
  section: PackedPreviewSectionHeader;
}

export function isTauriEnvironment(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const DATASET_REGISTRY_STORAGE_KEY = "traceboost.dataset-registry";
const WORKSPACE_SESSION_STORAGE_KEY = "traceboost.workspace-session";

async function invokeTauri<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

async function invokeTauriRaw(command: string, args: Record<string, unknown>): Promise<Uint8Array> {
  const { invoke } = await import("@tauri-apps/api/core");
  const response = await invoke<Uint8Array | ArrayBuffer>(command, args);
  return response instanceof Uint8Array ? response : new Uint8Array(response);
}

async function readJson<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || "Backend request failed");
  }
  return response.json() as Promise<T>;
}

async function postJson<T>(url: string, body: Record<string, unknown>): Promise<T> {
  const response = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json"
    },
    body: JSON.stringify(body)
  });
  return readJson<T>(response);
}

function operationSlug(
  operation:
    | ProcessingPreset["pipeline"]["steps"][number]["operation"]
    | RunProcessingRequest["pipeline"]["steps"][number]["operation"]
): string {
  if (typeof operation === "string") {
    return "trace-rms-normalize";
  }
  if ("amplitude_scalar" in operation) {
    return `amplitude-scalar-${String(operation.amplitude_scalar.factor).replace(".", "_")}`;
  }
  if ("agc_rms" in operation) {
    return `agc-rms-${String(operation.agc_rms.window_ms).replace(".", "_")}`;
  }
  if ("phase_rotation" in operation) {
    return `phase-rotation-${String(operation.phase_rotation.angle_degrees).replace(".", "_")}`;
  }
  if ("lowpass_filter" in operation) {
    return `lowpass-${[operation.lowpass_filter.f3_hz, operation.lowpass_filter.f4_hz]
      .map((value) => String(value).replace(".", "_"))
      .join("-")}`;
  }
  if ("highpass_filter" in operation) {
    return `highpass-${[operation.highpass_filter.f1_hz, operation.highpass_filter.f2_hz]
      .map((value) => String(value).replace(".", "_"))
      .join("-")}`;
  }
  if ("volume_arithmetic" in operation) {
    const secondaryStem =
      fileStem(operation.volume_arithmetic.secondary_store_path)
        .toLowerCase()
        .replace(/[^a-z0-9_-]+/g, "-")
        .replace(/^-+|-+$/g, "") || "volume";
    return `volume-${operation.volume_arithmetic.operator}-${secondaryStem}`;
  }
  return `bandpass-${[
    operation.bandpass_filter.f1_hz,
    operation.bandpass_filter.f2_hz,
    operation.bandpass_filter.f3_hz,
    operation.bandpass_filter.f4_hz
  ]
    .map((value) => String(value).replace(".", "_"))
    .join("-")}`;
}

function defaultWorkspaceSession(): WorkspaceSession {
  return {
    active_entry_id: null,
    active_store_path: null,
    active_axis: "inline",
    active_index: 0,
    selected_preset_id: null,
    display_coordinate_reference_id: null
  };
}

function storageAvailable(): boolean {
  return typeof window !== "undefined" && typeof window.localStorage !== "undefined";
}

function loadLocalRegistry(): DatasetRegistryEntry[] {
  if (!storageAvailable()) {
    return [];
  }
  const stored = window.localStorage.getItem(DATASET_REGISTRY_STORAGE_KEY);
  if (!stored) {
    return [];
  }
  try {
    return JSON.parse(stored) as DatasetRegistryEntry[];
  } catch {
    return [];
  }
}

function loadLocalSession(): WorkspaceSession {
  if (!storageAvailable()) {
    return defaultWorkspaceSession();
  }
  const stored = window.localStorage.getItem(WORKSPACE_SESSION_STORAGE_KEY);
  if (!stored) {
    return defaultWorkspaceSession();
  }
  try {
    return JSON.parse(stored) as WorkspaceSession;
  } catch {
    return defaultWorkspaceSession();
  }
}

function saveLocalRegistry(entries: DatasetRegistryEntry[]): void {
  if (!storageAvailable()) {
    return;
  }
  window.localStorage.setItem(DATASET_REGISTRY_STORAGE_KEY, JSON.stringify(entries));
}

function entryStoreIdentity(entry: DatasetRegistryEntry): { storeId: string; storePath: string } | null {
  const storeId = entry.last_dataset?.descriptor.store_id?.trim();
  const storePath =
    entry.last_dataset?.store_path?.trim() ||
    entry.imported_store_path?.trim() ||
    entry.preferred_store_path?.trim() ||
    "";
  if (!storeId || !storePath) {
    return null;
  }
  return { storeId, storePath };
}

function ensureUniqueStoreIdentityLocal(
  entries: DatasetRegistryEntry[],
  request: UpsertDatasetEntryRequest,
  existingIndex: number
): void {
  const storeId = request.dataset?.descriptor.store_id?.trim();
  const storePath = request.dataset?.store_path?.trim();
  if (!storeId || !storePath) {
    return;
  }

  for (let index = 0; index < entries.length; index += 1) {
    if (index === existingIndex) {
      continue;
    }
    const identity = entryStoreIdentity(entries[index]);
    if (!identity || identity.storeId !== storeId) {
      continue;
    }
    if (identity.storePath === storePath) {
      continue;
    }
    throw new Error(
      `Refusing to register duplicate store identity '${storeId}' for '${storePath}' because it is already used by '${entries[index].display_name}' at '${identity.storePath}'. This usually means a store folder was copied outside TraceBoost.`
    );
  }
}

function saveLocalSession(session: WorkspaceSession): void {
  if (!storageAvailable()) {
    return;
  }
  window.localStorage.setItem(WORKSPACE_SESSION_STORAGE_KEY, JSON.stringify(session));
}

function fileStem(filePath: string | null | undefined): string {
  const normalized = filePath?.trim() ?? "";
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

function userVisibleDatasetName(entry: DatasetRegistryEntry): string {
  const sourceStem = fileStem(entry.source_path);
  if (sourceStem) {
    return sourceStem;
  }
  const trimmedDisplayName = entry.display_name?.trim() ?? "";
  if (trimmedDisplayName) {
    return stripGeneratedHashSuffix(trimmedDisplayName);
  }
  const storeStem = fileStem(entry.imported_store_path ?? entry.preferred_store_path);
  if (storeStem) {
    return stripGeneratedHashSuffix(storeStem);
  }
  return entry.entry_id;
}

function sortEntries(entries: DatasetRegistryEntry[]): DatasetRegistryEntry[] {
  return [...entries].sort((left, right) => {
    const byName = userVisibleDatasetName(left).localeCompare(userVisibleDatasetName(right), undefined, {
      sensitivity: "base",
      numeric: true
    });
    if (byName !== 0) {
      return byName;
    }
    return left.entry_id.localeCompare(right.entry_id, undefined, { sensitivity: "base", numeric: true });
  });
}

function resolveEntryStatus(entry: DatasetRegistryEntry): DatasetRegistryStatus {
  if (entry.source_path) {
    return entry.imported_store_path ? "imported" : "linked";
  }
  return entry.imported_store_path ? "imported" : "linked";
}

export async function preflightImport(
  inputPath: string,
  geometryOverride: SegyGeometryOverride | null = null
): Promise<SurveyPreflightResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<SurveyPreflightResponse>("preflight_import_command", { inputPath, geometryOverride });
  }

  return postJson<SurveyPreflightResponse>("/api/preflight", { inputPath, geometryOverride });
}

export async function importDataset(
  inputPath: string,
  outputStorePath: string,
  overwriteExisting = false,
  geometryOverride: SegyGeometryOverride | null = null
): Promise<ImportDatasetResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<ImportDatasetResponse>("import_dataset_command", {
      inputPath,
      outputStorePath,
      geometryOverride,
      overwriteExisting
    });
  }

  return postJson<ImportDatasetResponse>("/api/import", {
    inputPath,
    outputStorePath,
    geometryOverride,
    overwriteExisting
  });
}

export async function defaultImportStorePath(inputPath: string): Promise<string> {
  if (isTauriEnvironment()) {
    return invokeTauri<string>("default_import_store_path_command", { inputPath });
  }

  const normalized = inputPath.trim();
  const separatorIndex = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  const directory = separatorIndex >= 0 ? normalized.slice(0, separatorIndex + 1) : "";
  const filename = separatorIndex >= 0 ? normalized.slice(separatorIndex + 1) : normalized;
  const basename = filename.replace(/\.[^.]+$/, "");
  return `${directory}${basename}.tbvol`;
}

export async function openDataset(storePath: string): Promise<OpenDatasetResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<OpenDatasetResponse>("open_dataset_command", { storePath });
  }

  return postJson<OpenDatasetResponse>("/api/open", { storePath });
}

export async function exportDatasetSegy(
  storePath: string,
  outputPath: string,
  overwriteExisting = false
): Promise<ExportSegyResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<ExportSegyResponse>("export_dataset_segy_command", {
      storePath,
      outputPath,
      overwriteExisting
    });
  }

  return postJson<ExportSegyResponse>("/api/export/segy", {
    storePath,
    outputPath,
    overwriteExisting
  });
}

export async function importHorizonXyz(
  storePath: string,
  inputPaths: string[]
): Promise<ImportHorizonXyzResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<ImportHorizonXyzResponse>("import_horizon_xyz_command", {
      storePath,
      inputPaths
    });
  }

  return postJson<ImportHorizonXyzResponse>("/api/horizons/import", {
    storePath,
    inputPaths
  });
}

export async function fetchSectionHorizons(
  storePath: string,
  axis: SectionAxis,
  index: number
): Promise<SectionHorizonOverlayView[]> {
  if (isTauriEnvironment()) {
    const response = await invokeTauri<{ schema_version: number; overlays: SectionHorizonOverlayView[] }>(
      "load_section_horizons_command",
      {
        storePath,
        axis,
        index
      }
    );
    return response.overlays;
  }

  const response = await fetch(
    `/api/horizons/section?storePath=${encodeURIComponent(storePath)}&axis=${encodeURIComponent(axis)}&index=${encodeURIComponent(index)}`
  );
  const payload = await readJson<{ schema_version: number; overlays: SectionHorizonOverlayView[] }>(response);
  return payload.overlays;
}

export async function fetchSectionView(
  storePath: string,
  axis: SectionAxis,
  index: number
): Promise<TransportSectionView | SectionView> {
  if (isTauriEnvironment()) {
    const payload = await invokeTauriRaw("load_section_binary_command", {
      storePath,
      axis,
      index
    });
    return parsePackedSectionViewResponse(payload);
  }

  const response = await fetch(
    `/api/section?storePath=${encodeURIComponent(storePath)}&axis=${encodeURIComponent(axis)}&index=${encodeURIComponent(index)}`
  );
  return readJson<SectionView>(response);
}

export async function previewProcessing(
  request: PreviewProcessingRequest
): Promise<TransportPreviewProcessingResponse> {
  if (isTauriEnvironment()) {
    const payload = await invokeTauriRaw("preview_processing_binary_command", { request });
    return parsePackedPreviewProcessingResponse(payload);
  }

  return postJson<PreviewProcessingResponse>("/api/processing/preview", request as Record<string, unknown>);
}

export async function previewSubvolumeProcessing(
  request: PreviewSubvolumeProcessingRequest
): Promise<TransportPreviewProcessingResponse> {
  if (isTauriEnvironment()) {
    const payload = await invokeTauriRaw("preview_subvolume_processing_binary_command", { request });
    return parsePackedPreviewProcessingResponse(payload);
  }

  return postJson<PreviewSubvolumeProcessingResponse>("/api/processing/subvolume/preview", request as Record<string, unknown>);
}

function parsePackedPreviewProcessingResponse(bytes: Uint8Array): TransportPreviewProcessingResponse {
  const MAGIC = "TBPRV001";
  if (bytes.byteLength < 16) {
    throw new Error("Packed preview response is too small.");
  }

  const magic = new TextDecoder().decode(bytes.subarray(0, 8));
  if (magic !== MAGIC) {
    throw new Error(`Unexpected packed preview response magic: ${magic}`);
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const headerLength = view.getUint32(8, true);
  const dataOffset = view.getUint32(12, true);
  const headerStart = 16;
  const headerEnd = headerStart + headerLength;
  if (headerEnd > bytes.byteLength || dataOffset > bytes.byteLength || dataOffset < headerEnd) {
    throw new Error("Packed preview response header is invalid.");
  }

  const header = JSON.parse(
    new TextDecoder().decode(bytes.subarray(headerStart, headerEnd))
  ) as PackedPreviewResponseHeader;

  let cursor = dataOffset;
  const nextBytes = (length: number | null): Uint8Array | null => {
    if (length === null) {
      return null;
    }
    const next = bytes.subarray(cursor, cursor + length);
    cursor += length;
    return next;
  };

  return {
    preview: {
      preview_ready: header.previewReady,
      processing_label: header.processingLabel,
      section: {
        dataset_id: header.section.datasetId,
        axis: header.section.axis,
        coordinate: header.section.coordinate,
        traces: header.section.traces,
        samples: header.section.samples,
        horizontal_axis_f64le: nextBytes(header.section.horizontalAxisBytes) ?? new Uint8Array(0),
        inline_axis_f64le: nextBytes(header.section.inlineAxisBytes),
        xline_axis_f64le: nextBytes(header.section.xlineAxisBytes),
        sample_axis_f32le: nextBytes(header.section.sampleAxisBytes) ?? new Uint8Array(0),
        amplitudes_f32le: nextBytes(header.section.amplitudesBytes) ?? new Uint8Array(0),
        units: header.section.units,
        metadata: header.section.metadata,
        display_defaults: header.section.displayDefaults
      }
    }
  };
}

function parsePackedSectionViewResponse(bytes: Uint8Array): TransportSectionView {
  const MAGIC = "TBSEC001";
  if (bytes.byteLength < 16) {
    throw new Error("Packed section response is too small.");
  }

  const magic = new TextDecoder().decode(bytes.subarray(0, 8));
  if (magic !== MAGIC) {
    throw new Error(`Unexpected packed section response magic: ${magic}`);
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const headerLength = view.getUint32(8, true);
  const dataOffset = view.getUint32(12, true);
  const headerStart = 16;
  const headerEnd = headerStart + headerLength;
  if (headerEnd > bytes.byteLength || dataOffset > bytes.byteLength || dataOffset < headerEnd) {
    throw new Error("Packed section response header is invalid.");
  }

  const header = JSON.parse(
    new TextDecoder().decode(bytes.subarray(headerStart, headerEnd))
  ) as PackedSectionResponseHeader;

  let cursor = dataOffset;
  const nextBytes = (length: number | null): Uint8Array | null => {
    if (length === null) {
      return null;
    }
    const next = bytes.subarray(cursor, cursor + length);
    cursor += length;
    return next;
  };

  return {
    dataset_id: header.section.datasetId,
    axis: header.section.axis,
    coordinate: header.section.coordinate,
    traces: header.section.traces,
    samples: header.section.samples,
    horizontal_axis_f64le: nextBytes(header.section.horizontalAxisBytes) ?? new Uint8Array(0),
    inline_axis_f64le: nextBytes(header.section.inlineAxisBytes),
    xline_axis_f64le: nextBytes(header.section.xlineAxisBytes),
    sample_axis_f32le: nextBytes(header.section.sampleAxisBytes) ?? new Uint8Array(0),
    amplitudes_f32le: nextBytes(header.section.amplitudesBytes) ?? new Uint8Array(0),
    units: header.section.units,
    metadata: header.section.metadata,
    display_defaults: header.section.displayDefaults
  };
}

export async function emitFrontendDiagnosticsEvent(request: FrontendDiagnosticsEventRequest): Promise<void> {
  if (!isTauriEnvironment()) {
    return;
  }

  await invokeTauri<void>("emit_frontend_diagnostics_event_command", { request });
}

export async function fetchAmplitudeSpectrum(
  request: AmplitudeSpectrumRequest
): Promise<AmplitudeSpectrumResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<AmplitudeSpectrumResponse>("amplitude_spectrum_command", { request });
  }

  return postJson<AmplitudeSpectrumResponse>("/api/processing/spectrum", request as Record<string, unknown>);
}

export async function runProcessing(
  request: RunProcessingRequest
): Promise<RunProcessingResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<RunProcessingResponse>("run_processing_command", { request });
  }

  return postJson<RunProcessingResponse>("/api/processing/run", request as Record<string, unknown>);
}

export async function runSubvolumeProcessing(
  request: RunSubvolumeProcessingRequest
): Promise<RunSubvolumeProcessingResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<RunSubvolumeProcessingResponse>("run_subvolume_processing_command", { request });
  }

  return postJson<RunSubvolumeProcessingResponse>("/api/processing/subvolume/run", request as Record<string, unknown>);
}

export async function defaultProcessingStorePath(
  storePath: string,
  pipeline: ProcessingPreset["pipeline"] | RunProcessingRequest["pipeline"]
): Promise<string> {
  if (isTauriEnvironment()) {
    return invokeTauri<string>("default_processing_store_path_command", {
      storePath,
      pipeline
    });
  }

  const normalizedStorePath = storePath.trim();
  const separatorIndex = Math.max(normalizedStorePath.lastIndexOf("/"), normalizedStorePath.lastIndexOf("\\"));
  const directory = separatorIndex >= 0 ? normalizedStorePath.slice(0, separatorIndex + 1) : "";
  const filename = separatorIndex >= 0 ? normalizedStorePath.slice(separatorIndex + 1) : normalizedStorePath;
  const sourceStem = filename.replace(/\.[^.]+$/, "") || "dataset";
  const namedPipeline = pipeline.name?.trim();
  const pipelineOperationSlug =
    pipeline.steps.map(({ operation }) => operationSlug(operation)).join("-") || "pipeline";
  const pipelineStem = (namedPipeline || pipelineOperationSlug)
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  const timestamp = new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\..+$/, "")
    .replace("T", "-");
  return `${directory}${sourceStem}.${pipelineStem || "pipeline"}.${timestamp}.tbvol`;
}

export async function defaultSubvolumeProcessingStorePath(
  storePath: string,
  pipeline: SubvolumeProcessingPipeline
): Promise<string> {
  if (isTauriEnvironment()) {
    return invokeTauri<string>("default_subvolume_processing_store_path_command", {
      storePath,
      pipeline
    });
  }

  const normalizedStorePath = storePath.trim();
  const separatorIndex = Math.max(normalizedStorePath.lastIndexOf("/"), normalizedStorePath.lastIndexOf("\\"));
  const directory = separatorIndex >= 0 ? normalizedStorePath.slice(0, separatorIndex + 1) : "";
  const filename = separatorIndex >= 0 ? normalizedStorePath.slice(separatorIndex + 1) : normalizedStorePath;
  const sourceStem = filename.replace(/\.[^.]+$/, "") || "dataset";
  const namedPipeline = pipeline.name?.trim();
  const prefixLabel =
    pipeline.trace_local_pipeline?.steps.map(({ operation }) => operationSlug(operation)).join("-") ?? "";
  const cropLabel = `crop-il-${pipeline.crop.inline_min}-${pipeline.crop.inline_max}-xl-${pipeline.crop.xline_min}-${pipeline.crop.xline_max}-z-${pipeline.crop.z_min_ms}-${pipeline.crop.z_max_ms}`;
  const pipelineStem = (namedPipeline || [prefixLabel, cropLabel].filter(Boolean).join("-") || "crop-subvolume")
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  const timestamp = new Date()
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\..+$/, "")
    .replace("T", "-");
  return `${directory}${sourceStem}.${pipelineStem || "crop-subvolume"}.${timestamp}.tbvol`;
}

export async function getProcessingJob(jobId: string): Promise<GetProcessingJobResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<GetProcessingJobResponse>("get_processing_job_command", {
      request: { schema_version: 1, job_id: jobId }
    });
  }

  return postJson<GetProcessingJobResponse>("/api/processing/job", { schema_version: 1, job_id: jobId });
}

export async function cancelProcessingJob(jobId: string): Promise<CancelProcessingJobResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<CancelProcessingJobResponse>("cancel_processing_job_command", {
      request: { schema_version: 1, job_id: jobId }
    });
  }

  return postJson<CancelProcessingJobResponse>("/api/processing/cancel", {
    schema_version: 1,
    job_id: jobId
  });
}

export async function listPipelinePresets(): Promise<ListPipelinePresetsResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<ListPipelinePresetsResponse>("list_pipeline_presets_command", {});
  }

  const response = await fetch("/api/processing/presets");
  return readJson<ListPipelinePresetsResponse>(response);
}

export async function savePipelinePreset(
  preset: ProcessingPreset
): Promise<SavePipelinePresetResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<SavePipelinePresetResponse>("save_pipeline_preset_command", {
      request: { schema_version: 1, preset }
    });
  }

  return postJson<SavePipelinePresetResponse>("/api/processing/presets/save", {
    schema_version: 1,
    preset
  });
}

export async function deletePipelinePreset(presetId: string): Promise<boolean> {
  if (isTauriEnvironment()) {
    const response = await invokeTauri<{ schema_version: number; deleted: boolean }>(
      "delete_pipeline_preset_command",
      {
        request: { schema_version: 1, preset_id: presetId }
      }
    );
    return response.deleted;
  }

  const response = await postJson<{ schema_version: number; deleted: boolean }>(
    "/api/processing/presets/delete",
    {
      schema_version: 1,
      preset_id: presetId
    }
  );
  return response.deleted;
}

export async function loadWorkspaceState(): Promise<LoadWorkspaceStateResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<LoadWorkspaceStateResponse>("load_workspace_state_command", {});
  }

  return {
    schema_version: SCHEMA_VERSION,
    entries: sortEntries(loadLocalRegistry()),
    session: loadLocalSession()
  };
}

export async function upsertDatasetEntry(
  request: UpsertDatasetEntryRequest
): Promise<UpsertDatasetEntryResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<UpsertDatasetEntryResponse>("upsert_dataset_entry_command", { request });
  }

  const entries = loadLocalRegistry();
  const explicitEntryId = request.entry_id?.trim() || null;
  const trimmedSource = request.source_path?.trim() || null;
  const trimmedPreferredStore = request.preferred_store_path?.trim() || null;
  const trimmedImportedStore = request.imported_store_path?.trim() || null;
  const existingIndex = explicitEntryId
    ? entries.findIndex((entry) => entry.entry_id === explicitEntryId)
    : entries.findIndex(
        (entry) =>
          (trimmedSource && entry.source_path === trimmedSource) ||
          (trimmedImportedStore && entry.imported_store_path === trimmedImportedStore)
      );
  ensureUniqueStoreIdentityLocal(entries, request, existingIndex);
  const now = Math.floor(Date.now() / 1000);
  const entry: DatasetRegistryEntry =
    existingIndex >= 0
      ? {
          ...entries[existingIndex],
          display_name:
            request.display_name?.trim() ||
            entries[existingIndex].display_name,
          source_path: trimmedSource ?? entries[existingIndex].source_path,
          preferred_store_path: trimmedPreferredStore ?? entries[existingIndex].preferred_store_path,
          imported_store_path: trimmedImportedStore ?? entries[existingIndex].imported_store_path,
          last_dataset: request.dataset ?? entries[existingIndex].last_dataset,
          session_pipelines: request.session_pipelines ?? entries[existingIndex].session_pipelines,
          active_session_pipeline_id:
            request.active_session_pipeline_id ?? entries[existingIndex].active_session_pipeline_id,
          last_imported_at_unix_s:
            request.dataset || trimmedImportedStore ? now : entries[existingIndex].last_imported_at_unix_s,
          updated_at_unix_s: now,
          status: entries[existingIndex].status
        }
      : {
          entry_id: explicitEntryId ?? `dataset-${now}-${entries.length + 1}`,
          display_name:
            request.display_name?.trim() ||
            request.dataset?.descriptor.label ||
            trimmedSource?.split(/[\\/]/).pop() ||
            trimmedImportedStore?.split(/[\\/]/).pop() ||
            `Dataset ${entries.length + 1}`,
          source_path: trimmedSource,
          preferred_store_path: trimmedPreferredStore,
          imported_store_path: trimmedImportedStore,
          last_dataset: request.dataset ?? null,
          session_pipelines: request.session_pipelines ?? [],
          active_session_pipeline_id: request.active_session_pipeline_id ?? null,
          status: "linked",
          last_opened_at_unix_s: null,
          last_imported_at_unix_s: request.dataset || trimmedImportedStore ? now : null,
          updated_at_unix_s: now
        };
  entry.status = resolveEntryStatus(entry);

  const nextEntries = existingIndex >= 0 ? [...entries] : [...entries, entry];
  if (existingIndex >= 0) {
    nextEntries[existingIndex] = entry;
  }

  let session = loadLocalSession();
  if (request.make_active) {
    session = {
      ...session,
      active_entry_id: entry.entry_id,
      active_store_path: entry.imported_store_path ?? entry.preferred_store_path ?? null
    };
    saveLocalSession(session);
  }

  saveLocalRegistry(sortEntries(nextEntries));

  return {
    schema_version: SCHEMA_VERSION,
    entry,
    session
  };
}

export async function removeDatasetEntry(entryId: string): Promise<RemoveDatasetEntryResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<RemoveDatasetEntryResponse>("remove_dataset_entry_command", {
      request: { schema_version: SCHEMA_VERSION, entry_id: entryId }
    });
  }

  const currentEntries = loadLocalRegistry();
  const entries = currentEntries.filter((entry) => entry.entry_id !== entryId);
  saveLocalRegistry(entries);
  const currentSession = loadLocalSession();
  const session =
    currentSession.active_entry_id === entryId
      ? { ...currentSession, active_entry_id: null, active_store_path: null }
      : currentSession;
  saveLocalSession(session);
  return { schema_version: SCHEMA_VERSION, deleted: entries.length !== currentEntries.length, session };
}

export async function setActiveDatasetEntry(entryId: string): Promise<SetActiveDatasetEntryResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<SetActiveDatasetEntryResponse>("set_active_dataset_entry_command", {
      request: { schema_version: SCHEMA_VERSION, entry_id: entryId }
    });
  }

  const entries = loadLocalRegistry();
  const index = entries.findIndex((entry) => entry.entry_id === entryId);
  if (index < 0) {
    throw new Error(`Unknown dataset entry: ${entryId}`);
  }
  const now = Math.floor(Date.now() / 1000);
  const entry = {
    ...entries[index],
    last_opened_at_unix_s: now,
    updated_at_unix_s: now
  };
  entries[index] = entry;
  saveLocalRegistry(sortEntries(entries));
  const session = {
    ...loadLocalSession(),
    active_entry_id: entry.entry_id,
    active_store_path: entry.imported_store_path ?? entry.preferred_store_path ?? null
  };
  saveLocalSession(session);
  return {
    schema_version: SCHEMA_VERSION,
    entry,
    session
  };
}

export async function saveWorkspaceSession(
  request: SaveWorkspaceSessionRequest
): Promise<SaveWorkspaceSessionResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<SaveWorkspaceSessionResponse>("save_workspace_session_command", { request });
  }

  const session: WorkspaceSession = {
    active_entry_id: request.active_entry_id ?? null,
    active_store_path: request.active_store_path ?? null,
    active_axis: request.active_axis,
    active_index: request.active_index,
    selected_preset_id: request.selected_preset_id ?? null,
    display_coordinate_reference_id: request.display_coordinate_reference_id ?? null
  };
  saveLocalSession(session);
  return {
    schema_version: SCHEMA_VERSION,
    session
  };
}

export async function setDatasetNativeCoordinateReference(
  request: SetDatasetNativeCoordinateReferenceRequest
): Promise<SetDatasetNativeCoordinateReferenceResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<SetDatasetNativeCoordinateReferenceResponse>(
      "set_dataset_native_coordinate_reference_command",
      { request }
    );
  }

  const entries = loadLocalRegistry();
  const index = entries.findIndex(
    (entry) =>
      entry.imported_store_path === request.store_path || entry.preferred_store_path === request.store_path
  );
  if (index >= 0 && entries[index]) {
    const entry = entries[index];
    entries[index] = {
      ...entry,
      last_dataset: entry.last_dataset
        ? {
            ...entry.last_dataset,
            descriptor: {
              ...entry.last_dataset.descriptor,
              spatial: entry.last_dataset.descriptor.spatial
                ? {
                    ...entry.last_dataset.descriptor.spatial,
                    coordinate_reference: request.coordinate_reference_id
                      ? {
                          ...(entry.last_dataset.descriptor.spatial.coordinate_reference ?? {
                            id: null,
                            name: null,
                            geodetic_datum: null,
                            unit: null
                          }),
                          id: request.coordinate_reference_id,
                          name:
                            request.coordinate_reference_name ??
                            entry.last_dataset.descriptor.spatial.coordinate_reference?.name ??
                            null
                        }
                      : entry.last_dataset.descriptor.coordinate_reference_binding?.detected ??
                        entry.last_dataset.descriptor.spatial.coordinate_reference
                  }
                : entry.last_dataset.descriptor.spatial,
              coordinate_reference_binding: entry.last_dataset.descriptor.coordinate_reference_binding
                ? {
                    ...entry.last_dataset.descriptor.coordinate_reference_binding,
                    effective: request.coordinate_reference_id
                      ? {
                          ...(entry.last_dataset.descriptor.coordinate_reference_binding.effective ??
                            entry.last_dataset.descriptor.coordinate_reference_binding.detected ?? {
                              id: null,
                              name: null,
                              geodetic_datum: null,
                              unit: null
                            }),
                          id: request.coordinate_reference_id,
                          name:
                            request.coordinate_reference_name ??
                            entry.last_dataset.descriptor.coordinate_reference_binding.effective?.name ??
                            entry.last_dataset.descriptor.coordinate_reference_binding.detected?.name ??
                            null
                        }
                      : entry.last_dataset.descriptor.coordinate_reference_binding.detected,
                    source: request.coordinate_reference_id ? "user_override" : "header"
                  }
                : entry.last_dataset.descriptor.coordinate_reference_binding
            }
          }
        : null
    };
    saveLocalRegistry(sortEntries(entries));
  }

  const datasetResponse = await openDataset(request.store_path);
  return {
    schema_version: SCHEMA_VERSION,
    dataset: datasetResponse.dataset
  };
}

export async function resolveSurveyMap(
  request: ResolveSurveyMapRequest
): Promise<ResolveSurveyMapResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<ResolveSurveyMapResponse>("resolve_survey_map_command", { request });
  }

  throw new Error("Survey map resolution is only available in the desktop runtime.");
}

export async function getDiagnosticsStatus(): Promise<DiagnosticsStatus | null> {
  if (!isTauriEnvironment()) {
    return null;
  }

  return invokeTauri<DiagnosticsStatus>("get_diagnostics_status_command", {});
}

export async function setDiagnosticsVerbosity(enabled: boolean): Promise<void> {
  if (!isTauriEnvironment()) {
    return;
  }

  await invokeTauri<void>("set_diagnostics_verbosity_command", { enabled });
}

export async function listenToDiagnosticsEvents(
  listener: (event: DiagnosticsEvent) => void
): Promise<() => void> {
  if (!isTauriEnvironment()) {
    return () => {};
  }

  const { listen } = await import("@tauri-apps/api/event");
  return listen<DiagnosticsEvent>("diagnostics:event", (event) => {
    listener(event.payload);
  });
}
