/** TauriBackend (Phase 2): system facts and Core status are REAL; everything else still comes
 *  from the mock until its phase lands. Each method moves from MockBackend to a real Tauri
 *  command as phases complete, so the UI never changes shape. */
import { invoke } from "@tauri-apps/api/core";
import { MockBackend } from "./mock";
import type { ActionResult, ChangeRecord, CoreConnection, Metrics, PriorityLevel, ProcessDetails, ProcessGroup, SystemMetrics, SystemSnapshot } from "./types";
import type { ChangeEntry } from "./generated/ChangeEntry";
import type { DnsResult, DnsServer, RamCleanResult, RamMethod, RamStatus, Tweak, TweakApplyResult } from "./types";
import type { RamCleanReport } from "./generated/RamCleanReport";
import type { TweakStatus } from "./generated/TweakStatus";
import { MOCK_NOT_RECOMMENDED } from "./mockData";
import { useLive } from "../store/live";
import { useSettings } from "../store/settings";
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

  /** Real changes: the Core's journal plus per-user tweaks applied by the app. */
  override async getChanges(): Promise<ChangeRecord[]> {
    const toRecord = (c: ChangeEntry): ChangeRecord => ({ id: c.id, at: c.createdUnix * 1000, title: c.title, detail: c.detail, kind: "tweak", temporary: c.temporary });
    const core = await invoke<ChangeEntry[]>("core_list_changes").catch(() => [] as ChangeEntry[]);
    const user = await invoke<ChangeEntry[]>("user_changes").catch(() => [] as ChangeEntry[]);
    return [...core, ...user].map(toRecord).sort((a, b) => b.at - a.at);
  }
  override async revertChange(id: string): Promise<void> {
    if (id.startsWith("user:")) await invoke("user_revert", { id: id.slice(5) });
    else await invoke("core_revert", { id });
  }
  override async revertAll(): Promise<void> {
    const errors: string[] = [];
    await invoke("core_revert", { id: null }).catch((e: unknown) => errors.push(String(e)));
    await invoke("user_revert", { id: null }).catch((e: unknown) => errors.push(String(e)));
    if (errors.length) throw new Error(errors.join("; "));
  }

  /* ---------------- PC Boost (Phase 4) ---------------- */

  override async getTweaks(): Promise<Tweak[]> {
    const meta = await super.getTweaks(); // titles, descriptions, risk/evidence labels
    const status = await invoke<TweakStatus[]>("tweaks_status");
    const sys = useLive.getState().system;
    return meta.map((t) => {
      const st = status.find((s) => s.id === t.id);
      const rules = MOCK_NOT_RECOMMENDED[t.id] ?? [];
      return {
        ...t,
        enabled: st?.state === "applied",
        supported: !!st && st.state !== "unsupported",
        unsupportedReason: st?.reason ?? undefined,
        notRecommendedIf: rules.map((r) => ({ label: r.label, matches: sys ? r.when(sys.info) : false })),
      };
    });
  }

  override async setTweak(id: string, enabled: boolean, dryRun: boolean): Promise<TweakApplyResult> {
    try {
      const r = await invoke<ActionResult>("tweak_set", { id, on: enabled, dryRun });
      return { ok: true, dryRun: r.dryRun, changes: r.changes };
    } catch (e) {
      return { ok: false, dryRun, changes: [], error: String(e) };
    }
  }

  override async applyPreset(preset: "default" | "high", dryRun: boolean): Promise<TweakApplyResult> {
    const tweaks = await this.getTweaks();
    const changes: string[] = [];
    const errors: string[] = [];
    for (const t of tweaks.filter((x) => x.supported)) {
      // "Default" reverts only what OTX.GNG changed; it never guesses Windows defaults.
      const want = preset === "high" ? t.inHighPerformance : false;
      if (preset === "high" && (!want || t.enabled)) continue;
      if (preset === "default" && !t.enabled) continue;
      const r = await this.setTweak(t.id, want, dryRun);
      if (r.ok) changes.push(...r.changes);
      else if (preset === "high") errors.push(`${t.title}: ${r.error}`);
    }
    return { ok: errors.length === 0, dryRun, changes, error: errors.join("; ") || undefined };
  }

  private ramLog: RamCleanResult[] = [];

  override async getRamStatus(): Promise<RamStatus> {
    const s = await invoke<SystemMetrics>("monitor_system");
    return ramStatusFrom(s.ramTotalMb, s.ramAvailableMb, s.commitMb, s.commitLimitMb, s.standbyMb ?? 0);
  }

  override async cleanRam(methods: RamMethod[], trigger: RamCleanResult["trigger"]): Promise<RamCleanResult> {
    const allow = useSettings.getState().ramAllowList;
    const r = await invoke<RamCleanReport>("ram_clean", { methods, allow, skip: [], dryRun: useSettings.getState().dryRun });
    const res: RamCleanResult = {
      id: `ram-${Date.now()}`,
      at: Date.now(),
      trigger,
      methods,
      availableBeforeMB: r.availableBeforeMb,
      availableAfterMB: r.availableAfterMb,
      freedMB: r.freedMb,
      skipped: r.skipped,
    };
    this.ramLog.unshift(res);
    return res;
  }

  override async getRamHistory(): Promise<RamCleanResult[]> {
    return [...this.ramLog];
  }

  private dnsList: DnsServer[] | null = null;

  override async getDnsServers(): Promise<DnsServer[]> {
    if (!this.dnsList) {
      const base = await super.getDnsServers();
      const current = await invoke<string[]>("dns_current").catch(() => [] as string[]);
      const local = current.some((ip) => ip.startsWith("127.") || ip === "::1");
      this.dnsList = base.map((s) =>
        s.isp
          ? { ...s, name: local ? "Current (set by a local app such as a VPN)" : "Current (your network)", v4: current.filter((i) => !i.includes(":")), v6: current.filter((i) => i.includes(":")) }
          : s,
      );
    }
    return [...this.dnsList];
  }

  override async benchmarkDns(onProgress: (done: number, total: number) => void): Promise<DnsResult[]> {
    const servers = await this.getDnsServers();
    const pairs = servers.filter((s) => s.v4[0] ?? s.v6[0]).map((s) => [s.id, s.v4[0] ?? s.v6[0]] as [string, string]);
    onProgress(0, 1);
    const res = await invoke<{ serverId: string; medianMs: number | null; jitterMs: number; failures: number; samples: number }[]>("dns_benchmark", { servers: pairs, samples: 8 });
    onProgress(1, 1);
    return res.map((r) => ({ serverId: r.serverId, medianMs: r.medianMs ?? Number.NaN, jitterMs: r.jitterMs, failures: r.failures, samples: r.samples }));
  }

  override async applyDns(serverId: string): Promise<void> {
    const s = (await this.getDnsServers()).find((x) => x.id === serverId);
    if (!s) throw new Error("Unknown DNS server");
    const r = await invoke<ActionResult>("dns_set", { servers: [...s.v4, ...s.v6], dryRun: useSettings.getState().dryRun });
    if (r.dryRun) throw new Error(`Dry run: ${r.changes.join("; ")}`);
  }

  override async resetDns(): Promise<void> {
    await invoke<ActionResult>("dns_set", { servers: [], dryRun: useSettings.getState().dryRun });
  }

  override async getActiveDns(): Promise<string | null> {
    const current = await invoke<string[]>("dns_current").catch(() => [] as string[]);
    const servers = await this.getDnsServers();
    return servers.find((s) => !s.isp && s.v4.length > 0 && s.v4.every((ip) => current.includes(ip)))?.id ?? null;
  }

  override async addDnsServer(name: string, v4: string[]): Promise<DnsServer> {
    const s: DnsServer = { id: `custom-${Date.now()}`, name, v4, v6: [], filtering: false, isp: false, custom: true };
    this.dnsList = [...(await this.getDnsServers()), s];
    return s;
  }
}

/** The RAM status rule, shared with the mock: needs cleaning below 25% available or above 85% commit. */
export function ramStatusFrom(totalMB: number, availableMB: number, commitMB: number, commitLimitMB: number, standbyMB: number): RamStatus {
  const availPct = (availableMB / totalMB) * 100;
  const commitPct = (commitMB / commitLimitMB) * 100;
  const needs = availPct < 25 || commitPct > 85;
  return {
    totalMB,
    usedMB: totalMB - availableMB,
    availableMB,
    standbyMB,
    modifiedMB: 0,
    commitPct,
    status: needs ? "needs-cleaning" : "healthy",
    reason: needs
      ? availPct < 25
        ? `Available memory is ${availPct.toFixed(0)}%, below the 25% threshold.`
        : `Commit charge is ${commitPct.toFixed(0)}% of the limit, above 85%.`
      : `Available memory is ${availPct.toFixed(0)}% and commit is ${commitPct.toFixed(0)}%: no cleaning needed.`,
  };
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
