/** TauriBackend (Phase 2): system facts and Core status are REAL; everything else still comes
 *  from the mock until its phase lands. Each method moves from MockBackend to a real Tauri
 *  command as phases complete, so the UI never changes shape. */
import { invoke } from "@tauri-apps/api/core";
import { MockBackend } from "./mock";
import type { ActionResult, ChangeRecord, CoreConnection, Metrics, PriorityLevel, ProcessDetails, ProcessGroup, SystemMetrics, SystemSnapshot } from "./types";
import type { ChangeEntry } from "./generated/ChangeEntry";
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

  /** Real system metrics from the unprivileged monitor (Phase 3). */
  override onMetrics(cb: (m: Metrics) => void, rateHz: number): () => void {
    let alive = true;
    const tick = () =>
      void invoke<SystemMetrics>("monitor_system")
        .then((s) => {
          if (!alive) return;
          const g = s.gpu;
          cb({
            t: s.tMs,
            cpu: s.cpu,
            perCore: s.perCore,
            gpuAvailable: g !== null,
            gpu: { d3: g?.d3 ?? 0, compute: g?.compute ?? 0, copy: g?.copy ?? 0, videoDecode: g?.videoDecode ?? 0 },
            ram: {
              totalMB: s.ramTotalMb,
              usedMB: s.ramTotalMb - s.ramAvailableMb,
              availableMB: s.ramAvailableMb,
              commitMB: s.commitMb,
              commitLimitMB: s.commitLimitMb,
              standbyMB: s.standbyMb,
            },
            disk: { readMBs: s.diskReadMbs, writeMBs: s.diskWriteMbs, activePct: s.diskActivePct },
            net: { downKbps: s.netDownKbps, upKbps: s.netUpKbps },
            ping: null,
            fps: null,
            notes: s.notes,
          });
        })
        .catch(() => undefined);
    tick();
    const h = setInterval(tick, 1000 / rateHz);
    return () => {
      alive = false;
      clearInterval(h);
    };
  }

  override async getProcesses(): Promise<ProcessGroup[]> {
    return invoke<ProcessGroup[]>("monitor_processes");
  }

  override async getProcessDetails(g: ProcessGroup): Promise<ProcessDetails> {
    return invoke<ProcessDetails>("process_details", { path: g.path });
  }

  /** Runs one Core action per process of the app; reports partial failures honestly. */
  private async forEach(g: ProcessGroup, cmd: string, args: Record<string, unknown>): Promise<ActionResult> {
    const changes: string[] = [];
    const errors: string[] = [];
    let dryRun = false;
    for (const target of g.processes) {
      try {
        const r = await invoke<ActionResult>(cmd, { target, ...args });
        dryRun = r.dryRun;
        changes.push(...r.changes);
      } catch (e) {
        errors.push(String(e));
      }
    }
    if (changes.length === 0) throw new Error(errors[0] ?? "Nothing was changed.");
    if (errors.length) changes.push(`${errors.length} process(es) skipped: ${errors[0]}`);
    return { dryRun, changes, changeId: null };
  }

  override setPriority(g: ProcessGroup, level: PriorityLevel, dryRun: boolean) {
    return this.forEach(g, "core_set_priority", { level, dryRun });
  }
  override setEfficiencyMode(g: ProcessGroup, on: boolean, dryRun: boolean) {
    return this.forEach(g, "core_set_efficiency", { on, dryRun });
  }
  override endTask(g: ProcessGroup, dryRun: boolean) {
    return this.forEach(g, "core_end_process", { dryRun });
  }

  /** The real change journal from the Core (empty when the Core is not running). */
  override async getChanges(): Promise<ChangeRecord[]> {
    try {
      const list = await invoke<ChangeEntry[]>("core_list_changes");
      return list.map((c) => ({ id: c.id, at: c.createdUnix * 1000, title: c.title, detail: c.detail, kind: "priority", temporary: c.temporary }));
    } catch {
      return [];
    }
  }
  override async revertChange(id: string): Promise<void> {
    await invoke("core_revert", { id });
  }
  override async revertAll(): Promise<void> {
    await invoke("core_revert", { id: null });
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
