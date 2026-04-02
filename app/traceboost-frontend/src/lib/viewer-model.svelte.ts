import { createContext } from "svelte";
import type { SeismicChartInteractionState, SeismicChartTool } from "@geoviz/svelte";
import type {
  DatasetRegistryEntry,
  DatasetSummary,
  ImportDatasetResponse,
  SectionAxis,
  SectionInteractionChanged,
  SectionProbeChanged,
  SectionView,
  SectionViewportChanged,
  SurveyPreflightResponse,
  WorkspacePipelineEntry,
  WorkspaceSession
} from "@traceboost/seis-contracts";
import type { DiagnosticsEvent, DiagnosticsStatus } from "./bridge";
import {
  defaultImportStorePath,
  fetchSectionView,
  getDiagnosticsStatus,
  importDataset,
  loadWorkspaceState,
  listenToDiagnosticsEvents,
  openDataset,
  preflightImport,
  removeDatasetEntry,
  saveWorkspaceSession,
  setActiveDatasetEntry,
  upsertDatasetEntry,
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

interface OpenDatasetOptions {
  entryId?: string | null;
  sourcePath?: string | null;
  sessionPipelines?: WorkspacePipelineEntry[] | null;
  activeSessionPipelineId?: string | null;
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

  return `${directory}${basename}.tbvol`;
}

function fileExtension(filePath: string): string {
  const normalized = trimPath(filePath);
  const separatorIndex = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  const filename = separatorIndex >= 0 ? normalized.slice(separatorIndex + 1) : normalized;
  const extensionIndex = filename.lastIndexOf(".");
  return extensionIndex >= 0 ? filename.slice(extensionIndex).toLowerCase() : "";
}

function sortWorkspaceEntries(entries: DatasetRegistryEntry[]): DatasetRegistryEntry[] {
  return [...entries].sort((left, right) => right.updated_at_unix_s - left.updated_at_unix_s);
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
  return entries ? entries.map((entry) => ({ ...entry })) : null;
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
  axis = $state<SectionAxis>("inline");
  index = $state(0);
  section = $state<SectionView | null>(null);
  backgroundSection = $state<SectionView | null>(null);
  loading = $state(false);
  backgroundLoading = $state(false);
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
        displayName: entry.display_name,
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
      primaryLabel: primary?.descriptor.label ?? null,
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
    await this.openDatasetAt(nextCandidate.storePath, this.axis, this.index);
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

  #applyWorkspaceSession = (session: WorkspaceSession): void => {
    this.activeEntryId = session.active_entry_id;
    this.selectedPresetId = session.selected_preset_id;
    this.axis = session.active_axis;
    this.index = session.active_index;
  };

  #applyWorkspaceEntry = (entry: DatasetRegistryEntry | null): void => {
    if (!entry) {
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
  };

  #clearLoadedDataset = (): void => {
    this.activeStorePath = "";
    this.dataset = null;
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
  };

  #syncWorkspaceState = (entries: DatasetRegistryEntry[], session: WorkspaceSession): void => {
    this.workspaceEntries = sortWorkspaceEntries(entries);
    this.#applyWorkspaceSession(session);
    this.#applyWorkspaceEntry(
      this.workspaceEntries.find((entry) => entry.entry_id === session.active_entry_id) ?? null
    );
    this.refreshCompareSelection();
    this.workspaceReady = true;
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
        selected_preset_id: this.selectedPresetId
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

  openVolumePath = async (volumePath: string): Promise<void> => {
    const normalizedPath = trimPath(volumePath);
    if (!normalizedPath) {
      this.error = "Volume path is required.";
      this.note("Open-volume blocked because no usable path was provided.", "ui", "error");
      return;
    }

    const extension = fileExtension(normalizedPath);
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
        activeSessionPipelineId: matchingEntry?.active_session_pipeline_id ?? null
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
        activeSessionPipelineId: matchingEntry?.active_session_pipeline_id ?? null
      });
      return;
    }

    this.loading = true;
    this.busyLabel = "Inspecting volume";
    this.error = null;
    this.preflight = null;
    this.note("Started one-shot volume import.", "ui", "info", normalizedPath);

    try {
      const preflight = await preflightImport(normalizedPath);
      this.preflight = preflight;

      if (preflight.suggested_action !== "direct_dense_ingest") {
        throw new Error(
          `This SEG-Y survey cannot be opened automatically yet. Classification: ${preflight.classification}. Suggested action: ${preflight.suggested_action}.`
        );
      }

      const outputStorePath =
        trimPath(matchingEntry?.imported_store_path ?? matchingEntry?.preferred_store_path ?? "") ||
        (await defaultImportStorePath(normalizedPath));
      this.loading = false;
      this.busyLabel = null;
      this.inputPath = normalizedPath;
      this.outputStorePath = outputStorePath;
      this.#outputPathSource = "manual";
      await this.importDataset();
    } catch (error) {
      this.loading = false;
      this.busyLabel = null;
      this.error = errorMessage(error, "Failed to open the selected volume.");
      this.note("One-shot volume open failed.", "backend", "error", this.error);
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
    const nextEntryId: string | null = hasOwnOption("entryId")
      ? options.entryId ?? null
      : this.activeEntryId;
    const nextSourcePath: string = hasOwnOption("sourcePath")
      ? options.sourcePath ?? ""
      : this.inputPath;
    const nextSessionPipelines: WorkspacePipelineEntry[] | null = hasOwnOption("sessionPipelines")
      ? cloneSessionPipelines(options.sessionPipelines)
      : this.activeDatasetEntry?.session_pipelines ?? null;
    const nextActiveSessionPipelineId: string | null = hasOwnOption("activeSessionPipelineId")
      ? options.activeSessionPipelineId ?? null
      : this.activeDatasetEntry?.active_session_pipeline_id ?? null;

    try {
      const response = await openDataset(normalizedStorePath);
      this.dataset = response.dataset;
      this.activeStorePath = response.dataset.store_path;
      this.outputStorePath = response.dataset.store_path;
      this.#outputPathSource = "manual";
      this.inputPath = trimPath(nextSourcePath);
      this.error = null;

      const workspaceResponse = await upsertDatasetEntry({
        schema_version: 1,
        entry_id: nextEntryId,
        display_name: response.dataset.descriptor.label,
        source_path: trimPath(nextSourcePath) || null,
        preferred_store_path: response.dataset.store_path,
        imported_store_path: response.dataset.store_path,
        dataset: response.dataset,
        session_pipelines: nextSessionPipelines,
        active_session_pipeline_id: nextActiveSessionPipelineId,
        make_active: true
      });
      this.activeEntryId = workspaceResponse.entry.entry_id;
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, workspaceResponse.entry);
      this.#applyWorkspaceSession(workspaceResponse.session);
      this.refreshCompareSelection();

      this.note(
        "Runtime store opened.",
        "backend",
        "info",
        `${response.dataset.descriptor.label} @ ${response.dataset.store_path}`
      );
      await this.load(axis, index, response.dataset.store_path);
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
      const workspaceResponse = await upsertDatasetEntry({
        schema_version: 1,
        entry_id: this.activeEntryId,
        display_name: response.dataset.descriptor.label,
        source_path: inputPath,
        preferred_store_path: response.dataset.store_path,
        imported_store_path: response.dataset.store_path,
        dataset: response.dataset,
        session_pipelines: this.activeDatasetEntry?.session_pipelines ?? null,
        active_session_pipeline_id: this.activeDatasetEntry?.active_session_pipeline_id ?? null,
        make_active: true
      });
      this.activeEntryId = workspaceResponse.entry.entry_id;
      this.workspaceEntries = mergeWorkspaceEntry(this.workspaceEntries, workspaceResponse.entry);
      this.#applyWorkspaceSession(workspaceResponse.session);
      this.refreshCompareSelection();
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
      await this.persistWorkspaceSession();
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
