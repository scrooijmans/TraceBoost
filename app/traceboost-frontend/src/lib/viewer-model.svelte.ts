import { createContext } from "svelte";
import type {
  DatasetSummary,
  ImportDatasetResponse,
  SectionAxis,
  SectionInteractionChanged,
  SectionProbeChanged,
  SectionView,
  SectionViewportChanged,
  SurveyPreflightResponse
} from "@traceboost/seis-contracts";
import type { DiagnosticsEvent, DiagnosticsStatus } from "./bridge";
import {
  fetchSectionView,
  getDiagnosticsStatus,
  importDataset,
  listenToDiagnosticsEvents,
  openDataset,
  preflightImport,
  setDiagnosticsVerbosity
} from "./bridge";
import { confirmOverwriteStore } from "./file-dialog";

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

function timestampLabel(): string {
  return new Date().toLocaleTimeString("en-GB", { hour12: false });
}

function capEntries<T>(entries: T[], next: T, limit: number): T[] {
  return [next, ...entries].slice(0, limit);
}

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

function isExistingStoreError(message: string): boolean {
  return message.toLowerCase().includes("store root already exists:");
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

  return `${directory}${basename}.zarr`;
}

export class ViewerModel {
  readonly tauriRuntime: boolean;

  inputPath = $state("");
  outputStorePath = $state("");
  activeStorePath = $state("");
  dataset = $state<DatasetSummary | null>(null);
  preflight = $state<SurveyPreflightResponse | null>(null);
  axis = $state<SectionAxis>("inline");
  index = $state(0);
  section = $state<SectionView | null>(null);
  loading = $state(false);
  busyLabel = $state<string | null>(null);
  error = $state<string | null>(null);
  resetToken = $state("inline:0");
  displayTransform = $state({
    renderMode: "heatmap" as "heatmap" | "wiggle",
    colormap: "grayscale" as "grayscale" | "red-white-blue",
    gain: 1,
    polarity: "normal" as "normal" | "reversed"
  });
  lastProbe = $state<SectionProbeChanged | null>(null);
  lastViewport = $state<SectionViewportChanged | null>(null);
  lastInteraction = $state<SectionInteractionChanged | null>(null);
  diagnosticsStatus = $state<DiagnosticsStatus | null>(null);
  verboseDiagnostics = $state(false);
  backendEvents = $state<DiagnosticsEvent[]>([]);
  recentActivity = $state<ViewerActivity[]>([]);
  lastImportedInputPath = $state("");
  lastImportedStorePath = $state("");

  #activityCounter = 0;
  #diagnosticsUnlisten: (() => void) | null = null;
  #outputPathSource: "auto" | "manual" = "auto";

  constructor(options: ViewerModelOptions) {
    this.tauriRuntime = options.tauriRuntime;
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

  setOutputStorePath = (outputStorePath: string): void => {
    const normalizedPath = trimPath(outputStorePath);
    this.outputStorePath = normalizedPath;
    this.error = null;
    this.#outputPathSource = "manual";
    this.note("Selected runtime store output path.", "ui", "info", normalizedPath);
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

  setProbe = (event: SectionProbeChanged): void => {
    this.lastProbe = event;
  };

  setViewport = (event: SectionViewportChanged): void => {
    this.lastViewport = event;
  };

  setInteraction = (event: SectionInteractionChanged): void => {
    this.lastInteraction = event;
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
        `Preflight completed as ${preflight.classification}.`,
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

  importDataset = async (): Promise<void> => {
    const inputPath = this.inputPath.trim();
    const outputStorePath = this.outputStorePath.trim();
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
        response = await importDataset(inputPath, outputStorePath);
      } catch (error) {
        const message = errorMessage(error, "Unknown import error");
        if (!isExistingStoreError(message)) {
          throw error;
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
        response = await importDataset(inputPath, outputStorePath, true);
      }

      this.loading = false;
      this.busyLabel = null;
      this.dataset = response.dataset;
      this.activeStorePath = response.dataset.store_path;
      this.outputStorePath = response.dataset.store_path;
      this.#outputPathSource = "manual";
      this.lastImportedInputPath = inputPath;
      this.lastImportedStorePath = response.dataset.store_path;
      this.error = null;
      this.note(
        "Dataset import completed.",
        "backend",
        "info",
        `${response.dataset.descriptor.label} @ ${response.dataset.store_path}`
      );
      await this.load("inline", 0, response.dataset.store_path);
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
    this.loading = true;
    this.busyLabel = "Opening dataset";
    this.error = null;
    this.note("Opening runtime store.", "ui", "info", storePath || null);

    if (!storePath) {
      this.loading = false;
      this.busyLabel = null;
      this.error = "Store path is required.";
      this.note("Open-store blocked because no runtime store path was provided.", "ui", "error");
      return;
    }

    try {
      const response = await openDataset(storePath);
      this.loading = false;
      this.busyLabel = null;
      this.dataset = response.dataset;
      this.activeStorePath = response.dataset.store_path;
      this.outputStorePath = response.dataset.store_path;
      this.#outputPathSource = "manual";
      this.error = null;
      this.note(
        "Runtime store opened.",
        "backend",
        "info",
        `${response.dataset.descriptor.label} @ ${response.dataset.store_path}`
      );
      await this.load("inline", 0, response.dataset.store_path);
    } catch (error) {
      this.loading = false;
      this.busyLabel = null;
      this.error = error instanceof Error ? error.message : "Unknown open-store error";
      this.note(
        "Opening runtime store failed.",
        "backend",
        "error",
        error instanceof Error ? error.message : "Unknown open-store error"
      );
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
      this.note("Section load blocked because no active store is open.", "ui", "error");
      return;
    }

    try {
      const section = await fetchSectionView(activeStorePath, axis, index);
      this.axis = axis;
      this.index = index;
      this.section = section;
      this.loading = false;
      this.busyLabel = null;
      this.error = null;
      this.resetToken = `${axis}:${index}`;
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
