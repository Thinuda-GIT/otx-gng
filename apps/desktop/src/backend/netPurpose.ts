/** Best-effort "what is it for" label for network traffic. A heuristic from the exe name and the
 *  offline category; the UI says so. */
import type { TrafficApp } from "./types";

const BY_EXE: Record<string, TrafficApp["purpose"]> = {
  "onedrive.exe": "cloud-sync",
  "dropbox.exe": "cloud-sync",
  "googledrivefs.exe": "cloud-sync",
  "spotify.exe": "streaming",
  "discord.exe": "voice",
  "teamspeak.exe": "voice",
  "steam.exe": "launcher",
  "steamwebhelper.exe": "launcher",
  "epicgameslauncher.exe": "launcher",
  "riotclientservices.exe": "launcher",
  "battle.net.exe": "launcher",
  "agent.exe": "update",
  "eadesktop.exe": "launcher",
  "upc.exe": "launcher",
  "svchost.exe": "system",
  "mousocoreworker.exe": "update",
  "msedge.exe": "browser",
  "chrome.exe": "browser",
  "firefox.exe": "browser",
  "brave.exe": "browser",
  "opera.exe": "browser",
  "msedgewebview2.exe": "browser",
  "compattelrunner.exe": "telemetry",
  "devicecensus.exe": "telemetry",
};

const GAME_EXE = /(-win64-shipping|cs2|r5apex|gta5|cod|league of legends|robloxplayerbeta|dota2|overwatch|rocketleague|tslgame)\.exe$/i;

export function purposeOf(exe: string, runningGameExe: string | null): TrafficApp["purpose"] {
  const e = exe.toLowerCase();
  if (runningGameExe && e === runningGameExe.toLowerCase()) return "game";
  if (GAME_EXE.test(e)) return "game";
  return BY_EXE[e] ?? "unknown";
}
