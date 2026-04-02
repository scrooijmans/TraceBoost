import type {
  CancelProcessingJobResponse,
  GetProcessingJobResponse,
  ImportDatasetResponse,
  ListPipelinePresetsResponse,
  OpenDatasetResponse,
  PreviewProcessingResponse,
  ProcessingPreset,
  RunProcessingResponse,
  SavePipelinePresetResponse,
  SectionAxis,
  SectionView,
  SurveyPreflightResponse,
  PreviewProcessingRequest,
  RunProcessingRequest
} from "@traceboost/seis-contracts";

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
