/** TauriBackend (Phase 2): system facts and Core status are REAL; everything else still comes
 *  from the mock until its phase lands. Each method moves from MockBackend to a real Tauri
 *  command as phases complete, so the UI never changes shape. */
import { invoke } from "@tauri-apps/api/core";
import { MockBackend } from "./mock";
import type { CoreConnection, SystemSnapshot } from "./types";
import type { SystemSnapshot as WireSnapshot } from "./generated/SystemSnapshot";

export class TauriBackend extends MockBackend {
  override readonly kind: "mock" | "tauri" = "tauri";

  override async getSystemInfo(): Promise<SystemSnapshot> {
    const s = await invoke<WireSnapshot>("get_system_info");
    return { source: s.source, info: s.info, startupApps: s.startupApps };
  }

  override async getCoreConnection(): Promise<CoreConnection> {
    return invoke<CoreConnection>("core_connection");
  }
}

/** Desktop-only integrations. All are no-ops outside Tauri. */
export async function setCloseToTray(enabled: boolean): Promise<void> {
  await invoke("set_close_to_tray", { enabled });
}

/** Returns accelerators that could not be registered (already taken by another app). */
export async function setShortcuts(bindings: [string, string][]): Promise<string[]> {
  return invoke<string[]>("set_shortcuts", { bindings });
}

export async function setAutostart(enabled: boolean): Promise<boolean> {
  const a = await import("@tauri-apps/plugin-autostart");
  if (enabled) await a.enable();
  else await a.disable();
  return a.isEnabled();
}

export async function onDesktopEvent(name: "tray-action" | "shortcut", cb: (payload: string) => void): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<string>(name, (e) => cb(e.payload));
}
