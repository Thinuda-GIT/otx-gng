/** Game detection and automatic Game Mode run in the app's Rust side (so they keep working while
 *  the window is closed to the tray). This hook only mirrors their events into the UI and pushes
 *  the current settings and profiles down. */
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { sessionProfileFrom, type RunningGame } from "../backend/gameMode";
import type { SessionReport } from "../backend/generated/SessionReport";
import { onDesktopEvent } from "../backend/tauri";
import { isTauri } from "./tauri";
import { useLive } from "../store/live";
import { DEFAULT_PROFILE, useProfiles } from "../store/profiles";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

export function sameGame(a: RunningGame | null, b: RunningGame | null): boolean {
  return a?.gameId === b?.gameId && a?.process.pid === b?.process.pid && a?.process.createTime === b?.process.createTime;
}

export function useGameWatcher(): void {
  const autoGameMode = useSettings((s) => s.autoGameMode);
  const allow = useSettings((s) => s.ramAllowList);
  const profiles = useProfiles((s) => s.profiles);

  // Push automation settings to Rust whenever they change.
  useEffect(() => {
    if (!isTauri()) return;
    const map: Record<string, ReturnType<typeof sessionProfileFrom>> = { default: sessionProfileFrom(DEFAULT_PROFILE) };
    for (const [id, p] of Object.entries(profiles)) map[id] = sessionProfileFrom({ ...DEFAULT_PROFILE, ...p });
    void invoke("set_automation", { config: { autoGameMode, profiles: map } }).catch(() => undefined);
  }, [autoGameMode, allow, profiles]);

  // Mirror events into the UI.
  useEffect(() => {
    if (!isTauri()) return;
    void invoke<RunningGame | null>("running_game").then((g) => useLive.getState().setRunningGame(g)).catch(() => undefined);
    const ui = () => useUi.getState();
    const offs = [
      onDesktopEvent<RunningGame>("game-started", (g) => {
        if (!sameGame(useLive.getState().runningGame, g)) useLive.getState().setRunningGame(g);
        ui().logActivity(`${g.exe} started`);
      }),
      onDesktopEvent<RunningGame>("game-closed", (g) => {
        useLive.getState().setRunningGame(null);
        ui().toast({ tone: "info", title: `${g.exe} closed`, body: "Game Mode changes were reverted automatically." });
        ui().logActivity(`${g.exe} closed: session reverted`);
      }),
      onDesktopEvent<[string, SessionReport]>("game-mode-started", ([exe, r]) => {
        const failed = r.steps.filter((s) => s.status === "failed");
        ui().toast({
          tone: failed.length ? "warn" : "ok",
          title: `Game Mode on for ${exe}`,
          body: failed.length ? failed.map((f) => `${f.label}: ${f.detail}`).join("; ") : `${Math.max(0, r.freedMb)} MB freed. Everything reverts when the game closes.`,
        });
      }),
      onDesktopEvent<[string, string]>("game-mode-failed", ([exe, msg]) => ui().toast({ tone: "error", title: `Game Mode could not start for ${exe}`, body: msg })),
    ];
    return () => offs.forEach((p) => void p.then((off) => off()));
  }, []);
}
