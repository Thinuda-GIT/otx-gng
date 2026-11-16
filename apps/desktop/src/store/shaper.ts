/** Traffic Shaper rules, stored locally. The Core enforces them only while Focus mode is on. */
import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import type { ShaperRule } from "../backend/types";

export const DEFAULT_RULES: ShaperRule[] = [
  { id: "r-discord", app: "Discord.exe", action: "allow", priority: "high", onlyWhileGaming: false, essential: true },
  { id: "r-onedrive", app: "OneDrive.exe", action: "block", priority: "low", onlyWhileGaming: true, essential: false },
  { id: "r-steam", app: "steam.exe", action: "limit", upKbps: 500, priority: "low", onlyWhileGaming: true, essential: false },
];

interface ShaperStore {
  rules: ShaperRule[];
  focus: boolean;
  setRules: (r: ShaperRule[]) => void;
  setFocus: (on: boolean) => void;
}

export const useShaper = create<ShaperStore>()(
  persist(
    (set) => ({
      rules: DEFAULT_RULES,
      focus: false,
      setRules: (rules) => set({ rules }),
      setFocus: (focus) => set({ focus }),
    }),
    { name: "otx-gng.shaper", version: 1, storage: createJSONStorage(() => localStorage), partialize: (s) => ({ rules: s.rules }) },
  ),
);

/** Rules the Core should enforce now (allow rules need no action). */
export function enforced(rules: ShaperRule[]): { exe: string; action: "block" | "limit"; upKbps: number | null }[] {
  return rules
    .filter((r) => r.action !== "allow" && !r.essential)
    .map((r) => ({ exe: r.app, action: r.action === "block" ? "block" : "limit", upKbps: r.action === "limit" ? (r.upKbps ?? 1000) : null }));
}
