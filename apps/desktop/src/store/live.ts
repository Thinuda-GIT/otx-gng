/** Live data: system info, a rolling metrics history and the effective glass tier. */
import { create } from "zustand";
import type { CoreConnection, Metrics, SystemSnapshot } from "../backend/types";
import type { EffectiveGlass, GpuTier } from "../lib/glassQuality";

export const HISTORY = 120; // samples kept for charts

interface LiveStore {
  system: SystemSnapshot | null;
  core: CoreConnection | null;
  latest: Metrics | null;
  history: Metrics[];
  gpuTier: GpuTier;
  glass: EffectiveGlass;
  paused: boolean;
  setSystem: (s: SystemSnapshot) => void;
  setCore: (c: CoreConnection) => void;
  push: (m: Metrics) => void;
  setGpuTier: (t: GpuTier) => void;
  setGlass: (g: EffectiveGlass) => void;
  setPaused: (p: boolean) => void;
}

export const useLive = create<LiveStore>()((set) => ({
  system: null,
  core: null,
  latest: null,
  history: [],
  gpuTier: "unknown",
  glass: "full",
  paused: false,
  setSystem: (system) => set({ system }),
  setCore: (core) => set({ core }),
  push: (m) => set((s) => ({ latest: m, history: [...s.history.slice(-(HISTORY - 1)), m] })),
  setGpuTier: (gpuTier) => set({ gpuTier }),
  setGlass: (glass) => set({ glass }),
  setPaused: (paused) => set({ paused }),
}));
