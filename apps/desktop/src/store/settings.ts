/** Persisted user settings (General Settings page). Stored locally; nothing leaves the PC. */
import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { GlassQuality } from "../lib/glassQuality";

export type Accent = "green" | "cyan" | "violet" | "red" | "amber";

export interface Settings {
  language: "en-US";
  windowSize: "1280x720" | "1440x810" | "1600x900" | "1920x1080";
  // Application behavior
  launchOnStartup: boolean;
  minimizeToTray: boolean;
  // Notifications
  alertOnSessionStart: boolean;
  serverUnavailableAlerts: boolean;
  // Optimization
  realtimeOptimization: boolean;
  smartOptimization: boolean;
  dualRoutes: boolean;
  accuracy: "low" | "medium" | "high";
  // Advanced
  hardwareMonitoring: boolean;
  restartShortcut: string;
  redirectionMethod: "auto" | "firewall-qos" | "wireguard" | "windivert";
  // Appearance
  glassQuality: GlassQuality;
  reduceTransparency: "system" | "on" | "off";
  reduceMotion: "system" | "on" | "off";
  accent: Accent;
  uiScale: number;
  uiSounds: boolean;
  // Privacy
  telemetry: boolean;
  crashReports: boolean;
  // Updates
  updateChannel: "stable" | "beta";
  // Tweaks
  dryRun: boolean;
  // RAM routine
  ramAutoClean: boolean;
  ramOnGameStart: boolean;
  ramOnStatus: boolean;
  ramEveryHours: number;
  ramScheduleOn: boolean;
  ramSmartMode: boolean;
  ramAllowList: string[];
  // Onboarding
  firstRunDone: boolean;
  // Developer (Phase 1 only): simulates a game in the foreground to test glass auto-degrade
  simulateGameId: string | null;
}

export const DEFAULT_SETTINGS: Settings = {
  language: "en-US",
  windowSize: "1440x810",
  launchOnStartup: false, // asked in the first-run wizard, never assumed
  minimizeToTray: true,
  alertOnSessionStart: true,
  serverUnavailableAlerts: true,
  realtimeOptimization: false,
  smartOptimization: true,
  dualRoutes: false, // needs a relay (Tier 2); disabled until one is configured
  accuracy: "medium",
  hardwareMonitoring: false,
  restartShortcut: "Shift+Ctrl+Q",
  redirectionMethod: "auto",
  glassQuality: "auto",
  reduceTransparency: "system",
  reduceMotion: "system",
  accent: "green",
  uiScale: 1,
  uiSounds: false,
  telemetry: false,
  crashReports: false,
  updateChannel: "stable",
  dryRun: false,
  ramAutoClean: false,
  ramOnGameStart: true,
  ramOnStatus: true,
  ramEveryHours: 12,
  ramScheduleOn: false,
  ramSmartMode: true,
  ramAllowList: ["Discord.exe", "obs64.exe"],
  firstRunDone: false,
  simulateGameId: null,
};

interface SettingsStore extends Settings {
  set: <K extends keyof Settings>(key: K, value: Settings[K]) => void;
  reset: () => void;
  importJson: (json: string) => { ok: boolean; applied: number; error?: string };
}

/** Accept only known keys whose value type matches the default's type. */
export function sanitizeImport(raw: unknown): Partial<Settings> {
  if (!raw || typeof raw !== "object") return {};
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (!(k in DEFAULT_SETTINGS)) continue;
    const def = (DEFAULT_SETTINGS as unknown as Record<string, unknown>)[k];
    if (Array.isArray(def)) {
      if (Array.isArray(v) && v.every((x) => typeof x === "string")) out[k] = v;
    } else if (def === null) {
      if (v === null || typeof v === "string") out[k] = v;
    } else if (typeof v === typeof def) {
      out[k] = v;
    }
  }
  return out as Partial<Settings>;
}

export const useSettings = create<SettingsStore>()(
  persist(
    (set) => ({
      ...DEFAULT_SETTINGS,
      set: (key, value) => set({ [key]: value } as Partial<SettingsStore>),
      reset: () => set({ ...DEFAULT_SETTINGS, firstRunDone: true }),
      importJson: (json) => {
        try {
          const raw = JSON.parse(json) as unknown;
          // Accept our export envelope ({ app, version, settings }) or a flat object.
          const body =
            raw && typeof raw === "object" && "settings" in raw ? (raw as { settings: unknown }).settings : raw;
          const parsed = sanitizeImport(body);
          const n = Object.keys(parsed).length;
          if (n === 0) return { ok: false, applied: 0, error: "No valid OTX.GNG settings found in that file." };
          set(parsed);
          return { ok: true, applied: n };
        } catch {
          return { ok: false, applied: 0, error: "That file is not valid JSON." };
        }
      },
    }),
    {
      name: "otx-gng.settings",
      version: 1,
      storage: createJSONStorage(() => localStorage),
      partialize: (s) => {
        const { set: _s, reset: _r, importJson: _i, ...rest } = s;
        void _s;
        void _r;
        void _i;
        return rest;
      },
    },
  ),
);

export function exportSettings(): string {
  const { set: _s, reset: _r, importJson: _i, ...rest } = useSettings.getState();
  void _s;
  void _r;
  void _i;
  return JSON.stringify({ app: "OTX.GNG", version: 1, settings: rest }, null, 2);
}
