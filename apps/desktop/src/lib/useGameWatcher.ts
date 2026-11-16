/** Watches for library/known games starting and exiting (desktop app only, every 3 s, by exe
 *  name; no process is opened). On start it runs Game Mode when "Start Game Mode automatically"
 *  is on; on exit the Core's own watchdog has already reverted the session. */
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { backend } from "../backend";
import { startSession, type RunningGame } from "../backend/gameMode";
import { isTauri } from "./tauri";
import { useLive } from "../store/live";
import { useProfiles } from "../store/profiles";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

export function sameGame(a: RunningGame | null, b: RunningGame | null): boolean {
  return a?.gameId === b?.gameId && a?.process.pid === b?.process.pid && a?.process.createTime === b?.process.createTime;
}

export function useGameWatcher(): void {
  useEffect(() => {
    if (!isTauri()) return;
    let alive = true;
    void backend.getGames().catch(() => undefined); // fills the Rust-side library cache
    const tick = async () => {
      const list = await invoke<RunningGame[]>("games_running").catch(() => [] as RunningGame[]);
      if (!alive) return;
      const live = useLive.getState();
      const prev = live.runningGame;
      const now = list[0] ?? null;
      if (sameGame(prev, now)) return;
      live.setRunningGame(now);
      const ui = useUi.getState();
      if (prev && !now) {
        ui.toast({ tone: "info", title: `${prev.exe} closed`, body: "Game Mode changes were reverted automatically." });
        ui.logActivity(`${prev.exe} closed: session reverted`);
      }
      if (now) {
        ui.logActivity(`${now.exe} started`);
        const core = live.core?.link === "connected";
        if (useSettings.getState().autoGameMode && core) {
          try {
            const r = await startSession(now.process, useProfiles.getState().get(now.gameId));
            const failed = r.steps.filter((s) => s.status === "failed");
            ui.toast({
              tone: failed.length ? "warn" : "ok",
              title: `Game Mode on for ${now.exe}`,
              body: failed.length ? failed.map((f) => `${f.label}: ${f.detail}`).join("; ") : `${Math.max(0, r.freedMb)} MB freed. Everything reverts when the game closes.`,
            });
          } catch (e) {
            ui.toast({ tone: "error", title: "Game Mode could not start", body: String(e) });
          }
        }
      }
    };
    void tick();
    const h = setInterval(() => void tick(), 3000);
    return () => {
      alive = false;
      clearInterval(h);
    };
  }, []);
}
