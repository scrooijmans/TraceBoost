import type {
  DatasetSummary,
  SectionAxis,
  SectionInteractionChanged,
  SectionProbeChanged,
  SectionView,
  SectionViewportChanged,
  SurveyPreflightResponse
} from "@traceboost/seis-contracts";
import { writable } from "svelte/store";
import { fetchSectionView, importDataset, openDataset, preflightImport } from "./bridge";

export interface TraceBoostViewerState {
  inputPath: string;
  outputStorePath: string;
  activeStorePath: string;
  dataset: DatasetSummary | null;
  preflight: SurveyPreflightResponse | null;
  axis: SectionAxis;
  index: number;
  section: SectionView | null;
  loading: boolean;
  busyLabel: string | null;
  error: string | null;
  resetToken: string;
  displayTransform: {
    renderMode: "heatmap" | "wiggle";
    colormap: "grayscale" | "red-white-blue";
    gain: number;
    polarity: "normal" | "reversed";
  };
  lastProbe: SectionProbeChanged | null;
  lastViewport: SectionViewportChanged | null;
  lastInteraction: SectionInteractionChanged | null;
}

const initialState: TraceBoostViewerState = {
  inputPath: "",
  outputStorePath: "",
  activeStorePath: "",
  dataset: null,
  preflight: null,
  axis: "inline",
  index: 0,
  section: null,
  loading: false,
  busyLabel: null,
  error: null,
  resetToken: "inline:0",
  displayTransform: {
    renderMode: "heatmap",
    colormap: "grayscale",
    gain: 1,
    polarity: "normal"
  },
  lastProbe: null,
  lastViewport: null,
  lastInteraction: null
};

function createViewerStore() {
  const { subscribe, update } = writable(initialState);

  return {
    subscribe,
    setInputPath(inputPath: string) {
      update((state) => ({ ...state, inputPath }));
    },
    setOutputStorePath(outputStorePath: string) {
      update((state) => ({ ...state, outputStorePath }));
    },
    async runPreflight() {
      let inputPath = "";
      update((state) => {
        inputPath = state.inputPath.trim();
        return {
          ...state,
          loading: true,
          busyLabel: "Preflighting survey",
          error: null
        };
      });

      if (!inputPath) {
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          error: "Input SEG-Y path is required."
        }));
        return;
      }

      try {
        const preflight = await preflightImport(inputPath);
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          preflight,
          error: null
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          error: error instanceof Error ? error.message : "Unknown preflight error"
        }));
      }
    },
    async importDataset() {
      let inputPath = "";
      let outputStorePath = "";
      update((state) => {
        inputPath = state.inputPath.trim();
        outputStorePath = state.outputStorePath.trim();
        return {
          ...state,
          loading: true,
          busyLabel: "Importing survey",
          error: null
        };
      });

      if (!inputPath || !outputStorePath) {
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          error: "Both input SEG-Y path and output store path are required."
        }));
        return;
      }

      try {
        const response = await importDataset(inputPath, outputStorePath);
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          dataset: response.dataset,
          activeStorePath: response.dataset.store_path,
          outputStorePath: response.dataset.store_path,
          error: null
        }));
        await this.load("inline", 0, response.dataset.store_path);
      } catch (error) {
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          error: error instanceof Error ? error.message : "Unknown import error"
        }));
      }
    },
    async openDataset() {
      let storePath = "";
      update((state) => {
        storePath = state.outputStorePath.trim() || state.activeStorePath.trim();
        return {
          ...state,
          loading: true,
          busyLabel: "Opening dataset",
          error: null
        };
      });

      if (!storePath) {
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          error: "Store path is required."
        }));
        return;
      }

      try {
        const response = await openDataset(storePath);
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          dataset: response.dataset,
          activeStorePath: response.dataset.store_path,
          outputStorePath: response.dataset.store_path,
          error: null
        }));
        await this.load("inline", 0, response.dataset.store_path);
      } catch (error) {
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          error: error instanceof Error ? error.message : "Unknown open-store error"
        }));
      }
    },
    async load(axis: SectionAxis, index: number, storePathOverride?: string) {
      let activeStorePath = "";
      update((state) => ({
        ...state,
        activeStorePath: storePathOverride ?? state.activeStorePath,
        axis,
        index,
        loading: true,
        busyLabel: "Loading section",
        error: null
      }));
      update((state) => {
        activeStorePath = (storePathOverride ?? state.activeStorePath).trim();
        return state;
      });

      if (!activeStorePath) {
        update((state) => ({
          ...state,
          loading: false,
          busyLabel: null,
          error: "Open or import a dataset before loading sections."
        }));
        return;
      }

      try {
        const section = await fetchSectionView(activeStorePath, axis, index);
        update((state) => ({
          ...state,
          axis,
          index,
          section,
          loading: false,
          busyLabel: null,
          error: null,
          resetToken: `${axis}:${index}`
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          axis,
          index,
          loading: false,
          busyLabel: null,
          error: error instanceof Error ? error.message : "Unknown section load error"
        }));
      }
    },
    setRenderMode(renderMode: TraceBoostViewerState["displayTransform"]["renderMode"]) {
      update((state) => ({
        ...state,
        displayTransform: {
          ...state.displayTransform,
          renderMode
        }
      }));
    },
    setColormap(colormap: TraceBoostViewerState["displayTransform"]["colormap"]) {
      update((state) => ({
        ...state,
        displayTransform: {
          ...state.displayTransform,
          colormap
        }
      }));
    },
    setProbe(event: SectionProbeChanged) {
      update((state) => ({ ...state, lastProbe: event }));
    },
    setViewport(event: SectionViewportChanged) {
      update((state) => ({ ...state, lastViewport: event }));
    },
    setInteraction(event: SectionInteractionChanged) {
      update((state) => ({ ...state, lastInteraction: event }));
    }
  };
}

export const viewerStore = createViewerStore();
