import type {
  CancelProcessingJobResponse,
  DatasetRegistryEntry,
  DatasetRegistryStatus,
  GetProcessingJobResponse,
  ImportDatasetResponse,
  LoadWorkspaceStateResponse,
  ListPipelinePresetsResponse,
  OpenDatasetResponse,
  PreviewProcessingResponse,
  ProcessingPreset,
  RemoveDatasetEntryResponse,
  RunProcessingResponse,
  SaveWorkspaceSessionRequest,
  SaveWorkspaceSessionResponse,
  SavePipelinePresetResponse,
  SetActiveDatasetEntryResponse,
  SectionAxis,
  SectionView,
  SurveyPreflightResponse,
  PreviewProcessingRequest,
  RunProcessingRequest,
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

export function isTauriEnvironment(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const DATASET_REGISTRY_STORAGE_KEY = "traceboost.dataset-registry";
const WORKSPACE_SESSION_STORAGE_KEY = "traceboost.workspace-session";

async function invokeTauri<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
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

function defaultWorkspaceSession(): WorkspaceSession {
  return {
    active_entry_id: null,
    active_store_path: null,
    active_axis: "inline",
    active_index: 0,
    selected_preset_id: null
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

function saveLocalSession(session: WorkspaceSession): void {
  if (!storageAvailable()) {
    return;
  }
  window.localStorage.setItem(WORKSPACE_SESSION_STORAGE_KEY, JSON.stringify(session));
}

function sortEntries(entries: DatasetRegistryEntry[]): DatasetRegistryEntry[] {
  return [...entries].sort((left, right) => right.updated_at_unix_s - left.updated_at_unix_s);
}

function resolveEntryStatus(entry: DatasetRegistryEntry): DatasetRegistryStatus {
  if (entry.source_path) {
    return entry.imported_store_path ? "imported" : "linked";
  }
  return entry.imported_store_path ? "imported" : "linked";
}

export async function preflightImport(inputPath: string): Promise<SurveyPreflightResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<SurveyPreflightResponse>("preflight_import_command", { inputPath });
  }

  return postJson<SurveyPreflightResponse>("/api/preflight", { inputPath });
}

export async function importDataset(
  inputPath: string,
  outputStorePath: string,
  overwriteExisting = false
): Promise<ImportDatasetResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<ImportDatasetResponse>("import_dataset_command", {
      inputPath,
      outputStorePath,
      overwriteExisting
    });
  }

  return postJson<ImportDatasetResponse>("/api/import", {
    inputPath,
    outputStorePath,
    overwriteExisting
  });
}

export async function openDataset(storePath: string): Promise<OpenDatasetResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<OpenDatasetResponse>("open_dataset_command", { storePath });
  }

  return postJson<OpenDatasetResponse>("/api/open", { storePath });
}

export async function fetchSectionView(
  storePath: string,
  axis: SectionAxis,
  index: number
): Promise<SectionView> {
  if (isTauriEnvironment()) {
    return invokeTauri<SectionView>("load_section_command", {
      storePath,
      axis,
      index
    });
  }

  const response = await fetch(
    `/api/section?storePath=${encodeURIComponent(storePath)}&axis=${encodeURIComponent(axis)}&index=${encodeURIComponent(index)}`
  );
  return readJson<SectionView>(response);
}

export async function previewProcessing(
  request: PreviewProcessingRequest
): Promise<PreviewProcessingResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<PreviewProcessingResponse>("preview_processing_command", { request });
  }

  return postJson<PreviewProcessingResponse>("/api/processing/preview", request as Record<string, unknown>);
}

export async function runProcessing(
  request: RunProcessingRequest
): Promise<RunProcessingResponse> {
  if (isTauriEnvironment()) {
    return invokeTauri<RunProcessingResponse>("run_processing_command", { request });
  }

  return postJson<RunProcessingResponse>("/api/processing/run", request as Record<string, unknown>);
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
  const trimmedSource = request.source_path?.trim() || null;
  const trimmedPreferredStore = request.preferred_store_path?.trim() || null;
  const trimmedImportedStore = request.imported_store_path?.trim() || null;
  const existingIndex =
    (request.entry_id
      ? entries.findIndex((entry) => entry.entry_id === request.entry_id)
      : -1) >= 0
      ? entries.findIndex((entry) => entry.entry_id === request.entry_id)
      : entries.findIndex(
          (entry) =>
            (trimmedSource && entry.source_path === trimmedSource) ||
            (trimmedImportedStore && entry.imported_store_path === trimmedImportedStore)
        );
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
          entry_id: request.entry_id ?? `dataset-${now}-${entries.length + 1}`,
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
    selected_preset_id: request.selected_preset_id ?? null
  };
  saveLocalSession(session);
  return {
    schema_version: SCHEMA_VERSION,
    session
  };
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
