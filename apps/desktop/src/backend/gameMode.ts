/** Game Mode helpers shared by the Tauri backend and the game watcher. */
import { invoke } from "@tauri-apps/api/core";
import type { BoostStep, Game, GameProfile } from "./types";
import type { ProcessRef } from "./generated/ProcessRef";
import type { SessionProfile } from "./generated/SessionProfile";
import type { SessionReport } from "./generated/SessionReport";
import { useSettings } from "../store/settings";

export interface RunningGame {
  gameId: string;
  exe: string;
  process: ProcessRef;
}

/** Profile → what the Core needs. Priority is never above High (Realtime does not exist). */
export function sessionProfileFrom(p: GameProfile): SessionProfile {
  return {
    gamePriority: p.cpuPriority === "high" ? "high" : p.cpuPriority === "above" ? "aboveNormal" : "normal",
    background: p.backgroundApps.filter((x) => /^[\w .()-]+\.exe$/i.test(x)),
    cleanMemory: p.ramCleanOnLaunch,
    allow: useSettings.getState().ramAllowList,
    highPerformancePower: p.tweakPreset === "high",
  };
}

export function stepsFrom(r: SessionReport): BoostStep[] {
  return r.steps.map((s) => ({ id: s.id, label: s.label, status: s.status, detail: s.detail ?? undefined }));
}

export async function startSession(game: ProcessRef | null, profile: GameProfile): Promise<SessionReport> {
  return invoke<SessionReport>("session_start", { game, profile: sessionProfileFrom(profile), dryRun: useSettings.getState().dryRun });
}

export async function endSession(): Promise<SessionReport> {
  return invoke<SessionReport>("session_end");
}

/** Deterministic colour for games without extracted icons ("r g b"). */
export function tintFor(name: string): string {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  const hue = h % 360;
  // HSL(hue, 75%, 60%) → RGB
  const f = (n: number) => {
    const k = (n + hue / 30) % 12;
    const a = 0.75 * Math.min(0.6, 1 - 0.6);
    return Math.round(255 * (0.6 - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))));
  };
  return `${f(0)} ${f(8)} ${f(4)}`;
}

export type LauncherId = Game["launcher"];
