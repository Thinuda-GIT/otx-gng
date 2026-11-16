/** Per-game profiles, stored locally. */
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import type { GameProfile } from "../backend/types";

export const DEFAULT_PROFILE: GameProfile = {
  tweakPreset: "high",
  ramCleanOnLaunch: true,
  cpuPriority: "high",
  cpuSets: "auto",
  ioPriority: "normal",
  gpuPreference: "default",
  disableFullscreenOptimizations: false,
  networkProfile: "none",
  backgroundApps: ["OneDrive.exe", "Spotify.exe", "ms-teams.exe", "msedge.exe"],
};

interface ProfilesStore {
  profiles: Record<string, GameProfile>;
  get: (gameId: string) => GameProfile;
  put: (gameId: string, p: GameProfile) => void;
}

export const useProfiles = create<ProfilesStore>()(
  persist(
    (set, get) => ({
      profiles: {},
      get: (id) => ({ ...DEFAULT_PROFILE, ...(get().profiles[id] ?? {}) }),
      put: (id, p) => set((s) => ({ profiles: { ...s.profiles, [id]: p } })),
    }),
    { name: "otx-gng.profiles", version: 1, storage: createJSONStorage(() => localStorage), partialize: (s) => ({ profiles: s.profiles }) },
  ),
);
