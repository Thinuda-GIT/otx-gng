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
import type { BoostStep, BoostSummary, Game, GameProfile } from "./types";
import { startSession, stepsFrom, tintFor } from "./gameMode";
import { useProfiles } from "../store/profiles";
import type { Adapter, Connection, Hop, QualityResult, ShaperRule, TrafficApp, TuneFinding } from "./types";

interface ProbeStats {
  sent: number;
  received: number;
  medianMs: number | null;
  jitterMs: number;
  lossPct: number;
}
import { purposeOf } from "./netPurpose";
import { enforced, useShaper } from "../store/shaper";

interface DetectedGame {
  id: string;
  name: string;
  launcher: string;
  installPath: string;
  exe: string | null;
  sizeGb: number | null;
  launch: string | null;
  antiCheat: string | null;
}
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
    // Live ping: one ICMP echo per tick to the game's server (or 1.1.1.1), with jitter as the
    // change from the previous reply and loss over the last 20 probes.
    let pingTarget = "1.1.1.1";
    let targetAt = 0;
    let lastRtt: number | null = null;
    const recent: boolean[] = [];
    const pingOnce = async (): Promise<Metrics["ping"]> => {
      if (Date.now() - targetAt > 10_000) {
        targetAt = Date.now();
        pingTarget = await this.resolveTarget("auto").catch(() => "1.1.1.1");
      }
      const s = await invoke<ProbeStats>("net_probe", { target: pingTarget, count: 1, tcpPort: null }).catch(() => null);
      const ok = !!s && s.received > 0 && s.medianMs !== null;
      recent.push(ok);
      while (recent.length > 20) recent.shift();
      const loss = (100 * recent.filter((x) => !x).length) / recent.length;
      if (!ok) return lastRtt === null ? null : { ms: lastRtt, jitter: 0, loss };
      const ms = s!.medianMs!;
      const jitter = lastRtt === null ? 0 : Math.abs(ms - lastRtt);
      lastRtt = ms;
      return { ms, jitter, loss };
    };
    const tick = () =>
      void Promise.all([invoke<SystemMetrics>("monitor_system"), pingOnce()])
        .then(([s, ping]) => {
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
            ping,
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

  /* ---------------- Network (Phase 6) ---------------- */

  private liveTrafficHistory = new Map<string, number[]>();

  override async getTraffic(): Promise<TrafficApp[]> {
    const rows = await invoke<{ id: string; exe: string; downKbps: number; upKbps: number; topRemote: string | null }[]>("net_traffic");
    const game = useLive.getState().runningGame?.exe ?? null;
    return rows.map((r) => {
      const h = this.liveTrafficHistory.get(r.id) ?? [];
      h.push(r.downKbps + r.upKbps);
      while (h.length > 60) h.shift();
      this.liveTrafficHistory.set(r.id, h);
      return { id: r.id, name: r.exe, downKbps: r.downKbps, upKbps: r.upKbps, purpose: purposeOf(r.exe, game), history: [...h] };
    });
  }

  override async getConnections(): Promise<Connection[]> {
    const rows = await invoke<{ app: string; pid: number; protocol: "TCP" | "UDP"; local: string; remote: string; state: string }[]>("net_connections");
    // Country/network names need the offline DB-IP database, which is not bundled yet.
    return rows.map((r, i) => ({ id: `c${i}`, app: r.app, protocol: r.protocol, local: r.local, remote: r.remote, state: r.state, rdns: "", country: "-", asn: "-" }));
  }

  override async getRules(): Promise<ShaperRule[]> {
    return [...useShaper.getState().rules];
  }

  override async saveRule(rule: ShaperRule): Promise<void> {
    const rules = useShaper.getState().rules;
    useShaper.getState().setRules(rules.some((r) => r.id === rule.id) ? rules.map((r) => (r.id === rule.id ? rule : r)) : [...rules, rule]);
    if (useShaper.getState().focus) await this.setFocusMode(true);
  }

  override async deleteRule(id: string): Promise<void> {
    useShaper.getState().setRules(useShaper.getState().rules.filter((r) => r.id !== id || r.essential));
    if (useShaper.getState().focus) await this.setFocusMode(true);
  }

  override async setFocusMode(on: boolean): Promise<void> {
    if (on) {
      const r = await invoke<ActionResult>("shaper_apply", { rules: enforced(useShaper.getState().rules), dryRun: useSettings.getState().dryRun });
      useShaper.getState().setFocus(!r.dryRun);
    } else {
      await invoke<ActionResult>("net_restore");
      useShaper.getState().setFocus(false);
    }
  }

  override async restoreInternet(): Promise<void> {
    await invoke<ActionResult>("net_restore");
    useShaper.getState().setFocus(false);
  }

  /* ---------------- Quality, Route Lab, Multi Internet (Phase 7) ---------------- */

  /** "auto" targets resolve to the running game's busiest server, else 1.1.1.1. */
  private async resolveTarget(target: string): Promise<string> {
    if (!/auto/i.test(target)) return target.trim();
    const game = useLive.getState().runningGame;
    if (game) {
      const rows = await invoke<{ exe: string; topRemote: string | null }[]>("net_traffic").catch(() => []);
      const remote = rows.find((r) => r.exe.toLowerCase() === game.exe.toLowerCase())?.topRemote;
      if (remote) return remote.replace(/:\d+$/, "");
    }
    return "1.1.1.1";
  }

  private async probe(target: string, count: number): Promise<{ stats: ProbeStats; method: "ICMP" | "TCP" }> {
    const icmp = await invoke<ProbeStats>("net_probe", { target, count, tcpPort: null });
    if (icmp.received > 0) return { stats: icmp, method: "ICMP" };
    return { stats: await invoke<ProbeStats>("net_probe", { target, count, tcpPort: 443 }), method: "TCP" };
  }

  override async runQualityTest(target: string, onProgress: (pct: number) => void): Promise<QualityResult> {
    const host = await this.resolveTarget(target);
    onProgress(5);
    const idle = await this.probe(host, 12);
    onProgress(35);
    // Bufferbloat: latency while the link is busy. Uses Cloudflare's public speed-test endpoints
    // (about 25 MB down, 10 MB up). If they are unreachable the loaded values stay empty.
    const loaded = async (load: () => Promise<unknown>) => {
      const [r] = await Promise.all([this.probe(host, 8).catch(() => null), load().catch(() => null)]);
      return r?.stats.medianMs ?? Number.NaN;
    };
    const down = await loaded(async () => {
      const res = await fetch("https://speed.cloudflare.com/__down?bytes=25000000", { cache: "no-store" });
      await res.arrayBuffer();
    });
    onProgress(70);
    const up = await loaded(() => fetch("https://speed.cloudflare.com/__up", { method: "POST", body: new Uint8Array(10_000_000) }));
    onProgress(100);
    return {
      target: host,
      method: idle.method,
      pingMs: idle.stats.medianMs ?? Number.NaN,
      jitterMs: idle.stats.jitterMs,
      lossPct: idle.stats.lossPct,
      idleMs: idle.stats.medianMs ?? Number.NaN,
      loadedDownMs: down,
      loadedUpMs: up,
      dataUsedMB: 35,
    };
  }

  override async traceroute(target: string): Promise<Hop[]> {
    const host = await this.resolveTarget(target);
    const hops = await invoke<{ n: number; ip: string | null; avgMs: number | null; lossPct: number; segment: Hop["segment"] }[]>("net_traceroute", { target: host });
    // Network names need the offline DB-IP database, which is not bundled yet.
    return hops.map((h) => ({ n: h.n, host: h.ip ?? "*", ip: h.ip ?? "*", asn: "-", avgMs: h.avgMs ?? 0, lossPct: h.lossPct, segment: h.segment }));
  }

  override async getAdapters(): Promise<Adapter[]> {
    const rows = await invoke<{ luid: string; name: string; description: string; kind: string; up: boolean; hasGateway: boolean; metric: number; pingMs: number | null; lossPct: number | null }[]>("net_adapters");
    const live = rows.filter((r) => r.up && r.hasGateway);
    const best = live.length ? Math.min(...live.map((r) => r.metric)) : -1;
    const kinds: Adapter["kind"][] = ["ethernet", "wifi", "ppp", "vpn", "other"];
    return rows.map((r) => ({
      id: r.luid,
      name: r.description && r.description !== r.name ? `${r.name} (${r.description})` : r.name,
      kind: r.kind === "mobile" ? "tether" : kinds.includes(r.kind as Adapter["kind"]) ? (r.kind as Adapter["kind"]) : "other",
      connected: r.up && r.hasGateway,
      pingMs: r.pingMs,
      lossPct: r.lossPct,
      metric: r.metric,
      preferred: r.up && r.hasGateway && r.metric === best,
    }));
  }

  override async preferAdapter(id: string | null): Promise<ActionResult> {
    return invoke<ActionResult>("net_prefer", { luid: id, dryRun: useSettings.getState().dryRun });
  }

  override async getTuneFindings(): Promise<TuneFinding[]> {
    const out: TuneFinding[] = [];
    const sys = useLive.getState().system?.info;
    const vpn = await invoke<string | null>("net_vpn").catch(() => null);
    if (vpn) {
      out.push({ id: "vpn", title: `${vpn} is carrying all your traffic`, detail: `Every route, ping and DNS result reflects ${vpn}, not your ISP directly. To compare, pause it and run the Quality Test again.`, severity: "warn", evidence: "documented" });
    }
    const kind = sys?.network?.kind;
    if (kind === "wifi") out.push({ id: "link", title: "You are on Wi-Fi", detail: "Wi-Fi adds jitter compared with a cable. Use Ethernet for ranked games when you can.", severity: "info", evidence: "documented" });
    else if (kind === "ppp" || kind === "mobile") out.push({ id: "link", title: "You are on a mobile or PPP connection", detail: "Mobile links vary with signal and tower load; expect more jitter than fixed broadband.", severity: "info", evidence: "documented" });
    else if (kind === "ethernet") out.push({ id: "link", title: "You are on Ethernet", detail: "The most stable link type for games.", severity: "ok", evidence: "documented" });

    const mtu = await invoke<number | null>("net_mtu", { target: "1.1.1.1" }).catch(() => null);
    if (mtu === null) out.push({ id: "mtu", title: "Path MTU could not be measured", detail: "The test target did not answer ICMP.", severity: "info", evidence: "documented" });
    else if (mtu >= 1400) out.push({ id: "mtu", title: `Path MTU ${mtu} works end to end`, detail: "No fragmentation problems to the test target.", severity: "ok", evidence: "documented" });
    else out.push({ id: "mtu", title: `Path MTU is ${mtu}`, detail: "Lower than the usual 1500, typical of VPN tunnels and some mobile networks. Windows adapts automatically; no change needed unless games disconnect.", severity: "info", evidence: "documented" });

    const dns = await invoke<string[]>("dns_current").catch(() => [] as string[]);
    if (dns.some((d) => d.startsWith("127.") || d === "::1")) out.push({ id: "dns", title: "DNS is handled by a local app", detail: "A VPN or DNS app answers lookups on this PC. DNS only affects how fast connections start, not in-game ping.", severity: "info", evidence: "documented" });
    else out.push({ id: "dns", title: "DNS", detail: "Compare resolvers in PC Boost → DNS Optimizer. DNS only affects how fast connections start, not in-game ping.", severity: "info", evidence: "documented" });

    const tweaks = await invoke<{ id: string; state: string; reason: string | null }[]>("tweaks_status").catch(() => []);
    const nic = tweaks.find((t) => t.id === "nic-power");
    if (nic?.state === "applied") out.push({ id: "power", title: "Adapter power saving is off", detail: "The adapter will not sleep between packets.", severity: "ok", evidence: "documented" });
    else if (nic?.state === "unsupported") out.push({ id: "power", title: "Adapter power saving", detail: nic.reason ?? "Not available for this connection.", severity: "info", evidence: "documented" });
    else if (nic) out.push({ id: "power", title: "Adapter power saving is on", detail: "Windows may let the adapter sleep between packets, which can add small lag spikes.", severity: "warn", evidence: "documented", fixLabel: "Turn off power saving", fixTweak: "nic-power" });

    out.push({ id: "bloat", title: "Bufferbloat", detail: "Run the Quality Test: it measures latency while downloading and uploading. A big rise means upload or download is saturating the link; capping background apps in Traffic Shaper usually fixes it.", severity: "info", evidence: "documented" });
    out.push({ id: "nagle", title: "TCP acknowledgement tweaks", detail: "Most shooters use UDP, so Nagle/ACK changes are not expected to help them.", severity: "info", evidence: "experimental" });
    return out;
  }

  /* ---------------- Library and Game Mode (Phase 5) ---------------- */

  override async getGames(): Promise<Game[]> {
    const found = await invoke<DetectedGame[]>("games_scan");
    const profiles = useProfiles.getState();
    const known: Game["launcher"][] = ["steam", "epic", "riot", "battlenet", "xbox", "ea", "ubisoft", "manual"];
    return found.map((g) => ({
      id: g.id,
      name: g.name,
      launcher: known.includes(g.launcher as Game["launcher"]) ? (g.launcher as Game["launcher"]) : "manual",
      exe: g.exe ?? "",
      installPath: g.installPath,
      sizeGB: g.sizeGb ?? 0,
      lastPlayed: null,
      tint: tintFor(g.name),
      antiCheat: g.antiCheat,
      engine: g.exe && /-shipping\.exe$/i.test(g.exe) ? "unreal" : "other",
      profile: profiles.get(g.id),
    }));
  }

  override async updateGameProfile(id: string, profile: GameProfile): Promise<void> {
    const before = useProfiles.getState().get(id);
    useProfiles.getState().put(id, profile);
    if (before.gpuPreference !== profile.gpuPreference || before.disableFullscreenOptimizations !== profile.disableFullscreenOptimizations) {
      const gpu = profile.gpuPreference === "high-performance" ? 2 : profile.gpuPreference === "power-saving" ? 1 : 0;
      await invoke<string[]>("game_settings", { id, gpu, fsoOff: profile.disableFullscreenOptimizations });
    }
  }

  override async launchGame(id: string): Promise<void> {
    await invoke("game_launch", { id });
  }

  override async boost(gameId: string | null, onStep: (steps: BoostStep[]) => void): Promise<BoostSummary> {
    const t0 = Date.now();
    const running = useLive.getState().runningGame;
    const target = running && (gameId === null || running.gameId === gameId) ? running : null;
    const profile = useProfiles.getState().get(target?.gameId ?? gameId ?? "default");
    onStep([{ id: "start", label: "Applying Game Mode…", status: "running" }]);
    const r = await startSession(target?.process ?? null, profile);
    const steps = stepsFrom(r);
    onStep(steps);
    return { freedMB: Math.max(0, r.freedMb), steps, durationMs: Date.now() - t0 };
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
