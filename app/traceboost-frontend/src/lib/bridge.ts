import type {
  ImportDatasetResponse,
  OpenDatasetResponse,
  SectionAxis,
  SectionView,
  SurveyPreflightResponse
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
