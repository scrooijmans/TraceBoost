import type {
  SectionAxis,
  SectionInteractionChanged,
  SectionProbeChanged,
  SectionView,
  SectionViewportChanged
} from "@traceboost/seis-contracts";
import { writable } from "svelte/store";
import { fetchSectionView } from "./section-service";

export interface TraceBoostViewerState {
  axis: SectionAxis;
  index: number;
  section: SectionView | null;
  loading: boolean;
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
  axis: "inline",
  index: 0,
  section: null,
  loading: false,
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
    async load(axis: SectionAxis, index: number) {
      update((state) => ({
        ...state,
        axis,
        index,
        loading: true,
        error: null
      }));
      try {
        const section = await fetchSectionView(axis, index);
        update((state) => ({
          ...state,
          axis,
          index,
          section,
          loading: false,
          error: null,
          resetToken: `${axis}:${index}`
        }));
      } catch (error) {
        update((state) => ({
          ...state,
          axis,
          index,
          loading: false,
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
