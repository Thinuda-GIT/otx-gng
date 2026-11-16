/** MockBackend: realistic fake data so the whole UI can be reviewed before any system code exists.
 *  It never touches the real system. */
import type {
  Backend,
  BoostStep,
  BoostSummary,
  ChangeRecord,
  Connection,
  ActionResult,
  CoreConnection,
  ProcessDetails,
  DnsResult,
  DnsServer,
  Game,
  GameProfile,
  Hop,
  Metrics,
  PriorityLevel,
  ProcessGroup,
  QualityResult,
  RamCleanResult,
  RamMethod,
  RamStatus,
  ShaperRule,
  TrafficApp,
  Tweak,
  TweakApplyResult,
} from "./types";
import {
  MOCK_ADAPTERS,
  MOCK_CHANGES,
  MOCK_COMMUNITY,
  MOCK_DNS,
  MOCK_DNS_LATENCY,
  MOCK_GAME_PROCESS,
  MOCK_GAMES,
  MOCK_NOT_RECOMMENDED,
  MOCK_PROCESSES,
  MOCK_RULES,
  MOCK_SYSTEM,
  MOCK_TUNE,
  MOCK_TWEAK_CHANGES,
  MOCK_TWEAKS,
  WINDOWS_DEFAULT_ON,
} from "./mockData";
import { clamp } from "../lib/format";

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const jitter = (v: number, pct: number) => v * (1 + (Math.random() * 2 - 1) * pct);
const walk = (v: number, step: number, lo: number, hi: number) => clamp(v + (Math.random() * 2 - 1) * step, lo, hi);
let idSeq = 100;
const nextId = (p: string) => `${p}-${++idSeq}`;

export class MockBackend implements Backend {
  readonly kind: "mock" | "tauri" = "mock";

  private gameRunning: Game | null = null;
  private tweakState = new Map<string, boolean>(
    MOCK_TWEAKS.map((t) => [t.id, WINDOWS_DEFAULT_ON.has(t.id) || t.id === "nic-power" || t.id === "game-dvr"]),
  );
  private changes: ChangeRecord[] = [...MOCK_CHANGES];
  private ramHistory: RamCleanResult[] = [];
  private usedMB = 11_880;
  private standbyMB = 2_900;
  private priorities = new Map<string, PriorityLevel>();
  private eco = new Set<string>();
  private ended = new Set<string>();
  private dns: DnsServer[] = [...MOCK_DNS];
  private activeDns: string | null = "cloudflare";
  private games: Game[] = MOCK_GAMES.map((g) => ({ ...g, profile: { ...g.profile } }));
  private rules: ShaperRule[] = [...MOCK_RULES];
  private trafficHistory = new Map<string, number[]>();

  /** Mock-only: simulates a game in the foreground (drives Glass auto-degrade and live HUD). */
  simulateGame(gameId: string | null): void {
    this.gameRunning = gameId ? (this.games.find((g) => g.id === gameId) ?? null) : null;
  }

  async getSystemInfo() {
    await sleep(120);
    return structuredClone(MOCK_SYSTEM);
  }

  async getCoreConnection(): Promise<CoreConnection> {
    return { link: "notRunning", message: "Browser preview: there is no Core service here.", status: null };
  }

  onMetrics(cb: (m: Metrics) => void, rateHz: number): () => void {
    const threads = MOCK_SYSTEM.info.cpu.threads;
    let cpu = 14;
    let gpu = 6;
    let ping = 31;
    const cores = Array.from({ length: threads }, () => 10);
    const tick = () => {
      const g = this.gameRunning;
      cpu = walk(cpu, 4, g ? 38 : 6, g ? 72 : 28);
      gpu = walk(gpu, 5, g ? 78 : 2, g ? 98 : 18);
      ping = walk(ping, 2.5, 24, 48);
      for (let i = 0; i < threads; i++) cores[i] = walk(cores[i]!, 9, 1, i < 2 && g ? 98 : 88);
      this.usedMB = walk(this.usedMB, 40, 8_000, MOCK_SYSTEM.info.ramMb - 900);
      const total = MOCK_SYSTEM.info.ramMb;
      cb({
        t: Date.now(),
        cpu,
        perCore: [...cores],
        gpuAvailable: true,
        gpu: { d3: gpu, compute: gpu * 0.08, copy: gpu * 0.12, videoDecode: g ? 0 : 3 + Math.random() * 4 },
        ram: {
          totalMB: total,
          usedMB: this.usedMB,
          availableMB: total - this.usedMB,
          commitMB: this.usedMB * 1.32,
          commitLimitMB: total + 4_096,
          standbyMB: this.standbyMB,
        },
        disk: { readMBs: jitter(g ? 42 : 6, 0.6), writeMBs: jitter(3, 0.8), activePct: clamp(jitter(g ? 28 : 7, 0.7), 0, 100) },
        net: { downKbps: jitter(g ? 160 : 21_000, 0.3), upKbps: jitter(g ? 90 : 2_700, 0.4) },
        ping: { ms: ping, jitter: Math.abs(jitter(3, 0.8)), loss: Math.random() < 0.08 ? 1 : 0 },
        fps: g ? { avg: jitter(212, 0.06), low1: jitter(141, 0.1) } : null,
        notes: [],
      });
    };
    tick();
    const h = setInterval(tick, 1000 / rateHz);
    return () => clearInterval(h);
  }

  async getProcesses(): Promise<ProcessGroup[]> {
    const list = this.gameRunning ? [MOCK_GAME_PROCESS(this.gameRunning), ...MOCK_PROCESSES] : MOCK_PROCESSES;
    return list
      .filter((pr) => !this.ended.has(pr.id))
      .map(({ base, ...pr }) => {
        const cpu = Math.max(0, jitter(base.cpu, 0.5));
        const gpu = Math.max(0, jitter(base.gpu, 0.5));
        const io = Math.max(0, jitter(base.disk, 0.6));
        return {
          id: pr.id,
          name: pr.name,
          exe: pr.exe,
          path: pr.path || null,
          category: pr.category,
          processes: pr.pids.map((pid) => ({ pid, createTime: "133795871234567890" })),
          cpu,
          gpu,
          gpuEngine: base.gpu > 0 ? pr.gpuEngine : null,
          memoryMb: pr.privateMB,
          workingSetMb: pr.wsMB,
          commitMb: pr.commitMB,
          ioReadKbs: io,
          ioWriteKbs: io * 0.3,
          priority: this.priorities.get(pr.id) ?? pr.priority,
          sessionId: pr.category === "system" ? 0 : 1,
          protected: pr.protected,
          hog: pr.category !== "game" && (cpu > 4 || pr.wsMB > 1400 || io > 2000),
          description: pr.description || null,
          safeToClose: pr.safeToClose,
        };
      });
  }

  async getProcessDetails(g: ProcessGroup): Promise<ProcessDetails> {
    const pr = MOCK_PROCESSES.find((x) => x.id === g.id);
    return {
      path: g.path,
      signer: pr?.signed ? pr.publisher : null,
      signature: pr?.signed ? "valid" : "noEmbedded",
      startup: pr?.startup ?? false,
      efficiencyMode: this.eco.has(g.id),
    };
  }

  async setPriority(g: ProcessGroup, level: PriorityLevel, dryRun: boolean): Promise<ActionResult> {
    await sleep(150);
    this.guardProtected(g);
    const desc = `${g.exe}: priority → ${level}`;
    if (!dryRun) {
      this.priorities.set(g.id, level);
      this.log({ title: desc, detail: `${g.processes.length} process(es)`, kind: "priority", temporary: true });
    }
    return { dryRun, changes: [desc], changeId: null };
  }

  async setEfficiencyMode(g: ProcessGroup, on: boolean, dryRun: boolean): Promise<ActionResult> {
    await sleep(150);
    this.guardProtected(g);
    if (!dryRun) {
      if (on) this.eco.add(g.id);
      else this.eco.delete(g.id);
    }
    return { dryRun, changes: [`${g.exe}: efficiency mode ${on ? "on" : "off"}`], changeId: null };
  }

  async endTask(g: ProcessGroup, dryRun: boolean): Promise<ActionResult> {
    await sleep(250);
    this.guardProtected(g);
    if (!dryRun) this.ended.add(g.id);
    return { dryRun, changes: [`${g.exe} ended`], changeId: null };
  }

  private guardProtected(g: ProcessGroup) {
    if (g.protected) throw new Error("This process is protected and cannot be changed.");
  }

  async getTweaks(): Promise<Tweak[]> {
    await sleep(100);
    return MOCK_TWEAKS.map((t) => ({
      ...t,
      enabled: this.tweakState.get(t.id) ?? false,
      notRecommendedIf: (MOCK_NOT_RECOMMENDED[t.id] ?? []).map((r) => ({ label: r.label, matches: r.when(MOCK_SYSTEM.info) })),
    }));
  }

  async setTweak(id: string, enabled: boolean, dryRun: boolean): Promise<TweakApplyResult> {
    await sleep(dryRun ? 120 : 380);
    const t = MOCK_TWEAKS.find((x) => x.id === id);
    if (!t) return { ok: false, dryRun, changes: [], error: "Unknown tweak id" };
    if (!t.supported) return { ok: false, dryRun, changes: [], error: t.unsupportedReason };
    const changes = (MOCK_TWEAK_CHANGES[id] ?? []).map((c) => (enabled ? c : `Revert: ${c}`));
    if (!dryRun) {
      this.tweakState.set(id, enabled);
      if (enabled) this.log({ title: t.title, detail: changes.join("; "), kind: "tweak", temporary: false });
      else this.changes = this.changes.filter((c) => c.title !== t.title);
    }
    return { ok: true, dryRun, changes };
  }

  async applyPreset(preset: "default" | "high", dryRun: boolean): Promise<TweakApplyResult> {
    const changes: string[] = [];
    for (const t of MOCK_TWEAKS) {
      if (!t.supported) continue;
      const want = preset === "default" ? WINDOWS_DEFAULT_ON.has(t.id) : t.inHighPerformance;
      if ((this.tweakState.get(t.id) ?? false) !== want) {
        const r = await this.setTweak(t.id, want, dryRun);
        changes.push(...r.changes);
      }
    }
    return { ok: true, dryRun, changes };
  }

  async getRamStatus(): Promise<RamStatus> {
    const total = MOCK_SYSTEM.info.ramMb;
    const available = total - this.usedMB;
    const availPct = (available / total) * 100;
    const commitPct = ((this.usedMB * 1.32) / (total + 4_096)) * 100;
    const needs = availPct < 25 || commitPct > 85;
    return {
      totalMB: total,
      usedMB: this.usedMB,
      availableMB: available,
      standbyMB: this.standbyMB,
      modifiedMB: 180,
      commitPct,
      status: needs ? "needs-cleaning" : "healthy",
      reason: needs
        ? availPct < 25
          ? `Available memory is ${availPct.toFixed(0)}%, below the 25% threshold.`
          : `Commit charge is ${commitPct.toFixed(0)}% of the limit, above 85%.`
        : `Available memory is ${availPct.toFixed(0)}% and commit is ${commitPct.toFixed(0)}%: no cleaning needed.`,
    };
  }

  async cleanRam(methods: RamMethod[], trigger: RamCleanResult["trigger"]): Promise<RamCleanResult> {
    await sleep(900);
    const before = MOCK_SYSTEM.info.ramMb - this.usedMB;
    let freed = 0;
    if (methods.includes("trimWorkingSets")) freed += jitter(900, 0.3);
    if (methods.includes("memoryPriority")) freed += jitter(120, 0.3);
    if (methods.includes("flushModified")) freed += 140;
    if (methods.includes("trimFileCache")) freed += jitter(220, 0.3);
    // Standby purge raises "free" memory but standby pages already count as available,
    // so it does not change available memory. The mock reflects that honestly.
    if (methods.includes("purgeStandby")) this.standbyMB = 300;
    freed = Math.round(freed);
    this.usedMB = Math.max(6_000, this.usedMB - freed);
    const res: RamCleanResult = {
      id: nextId("ram"),
      at: Date.now(),
      trigger,
      methods,
      availableBeforeMB: before,
      availableAfterMB: before + freed,
      freedMB: freed,
      skipped: ["Game process", "Protected system processes", "Anti-cheat services", "Audio services", "Allow-listed apps"],
    };
    this.ramHistory.unshift(res);
    return res;
  }

  async getRamHistory() {
    return [...this.ramHistory];
  }

  async getDnsServers() {
    return [...this.dns];
  }

  async benchmarkDns(onProgress: (done: number, total: number) => void): Promise<DnsResult[]> {
    const samples = 10;
    const total = this.dns.length * samples;
    let done = 0;
    const out: DnsResult[] = [];
    for (const s of this.dns) {
      const lat = MOCK_DNS_LATENCY[s.id] ?? { ms: 20 + Math.random() * 30, fail: 0 };
      const xs: number[] = [];
      for (let i = 0; i < samples; i++) {
        await sleep(35);
        if (i >= lat.fail) xs.push(jitter(lat.ms, 0.25));
        onProgress(++done, total);
      }
      xs.sort((a, b) => a - b);
      const med = xs.length ? xs[Math.floor(xs.length / 2)]! : Number.NaN;
      const jit = xs.length > 1 ? xs[xs.length - 1]! - xs[0]! : 0;
      out.push({ serverId: s.id, medianMs: med, jitterMs: jit, failures: lat.fail, samples });
    }
    return out;
  }

  async applyDns(serverId: string) {
    await sleep(500);
    const s = this.dns.find((d) => d.id === serverId);
    if (!s) throw new Error("Unknown DNS server");
    const prev = this.activeDns ? this.dns.find((d) => d.id === this.activeDns)?.name : "automatic";
    this.activeDns = serverId;
    this.changes = this.changes.filter((c) => c.kind !== "dns");
    this.log({ title: `DNS set to ${s.name}`, detail: `Wi-Fi IPv4 ${s.v4.join(", ")} (previous: ${prev}); cache flushed; verified`, kind: "dns", temporary: false });
  }

  async resetDns() {
    await sleep(400);
    this.activeDns = null;
    this.changes = this.changes.filter((c) => c.kind !== "dns");
  }

  async getActiveDns() {
    return this.activeDns;
  }

  async addDnsServer(name: string, v4: string[]) {
    const s: DnsServer = { id: nextId("dns"), name, v4, v6: [], filtering: false, isp: false, custom: true };
    this.dns.push(s);
    return s;
  }

  async getGames() {
    await sleep(150);
    return this.games.map((g) => ({ ...g, profile: { ...g.profile } }));
  }

  async updateGameProfile(id: string, profile: GameProfile) {
    const g = this.games.find((x) => x.id === id);
    if (g) g.profile = { ...profile };
  }

  async launchGame(id: string) {
    await sleep(300);
    this.simulateGame(id);
  }

  async getChanges() {
    return [...this.changes].sort((a, b) => b.at - a.at);
  }

  async revertChange(id: string) {
    await sleep(300);
    const c = this.changes.find((x) => x.id === id);
    if (!c) return;
    const t = MOCK_TWEAKS.find((x) => x.title === c.title);
    if (t) this.tweakState.set(t.id, WINDOWS_DEFAULT_ON.has(t.id));
    if (c.kind === "dns") this.activeDns = null;
    this.changes = this.changes.filter((x) => x.id !== id);
  }

  async revertAll() {
    await sleep(700);
    for (const t of MOCK_TWEAKS) this.tweakState.set(t.id, WINDOWS_DEFAULT_ON.has(t.id));
    this.changes = [];
    this.activeDns = null;
    this.priorities.clear();
    this.eco.clear();
  }

  async boost(gameId: string | null, onStep: (steps: BoostStep[]) => void): Promise<BoostSummary> {
    const t0 = Date.now();
    const steps: BoostStep[] = [
      { id: "snapshot", label: "Snapshot current state", status: "pending" },
      { id: "ram", label: "Free memory", status: "pending" },
      { id: "cpu", label: gameId ? "Prioritise the game, calm background apps" : "Calm background apps", status: "pending" },
      { id: "power", label: "High performance power plan", status: "pending" },
      { id: "network", label: "Focus network on the game", status: "pending" },
      { id: "overlay", label: "External overlay", status: "pending" },
      { id: "watchdog", label: "Arm the auto-revert watchdog", status: "pending" },
    ];
    let freed = 0;
    for (const s of steps) {
      s.status = "running";
      onStep(structuredClone(steps));
      await sleep(380 + Math.random() * 300);
      if (s.id === "ram") {
        const r = await this.cleanRam(["memoryPriority", "trimWorkingSets"], "boost");
        freed = r.freedMB;
        s.detail = `${r.freedMB} MB more available`;
      }
      if (s.id === "cpu") s.detail = "7 background apps set to Below Normal + efficiency mode";
      if (s.id === "power") s.detail = "Balanced → High performance (temporary)";
      if (s.id === "network") s.detail = gameId ? "2 apps paused, 2 limited" : "Skipped: no game selected";
      if (s.id === "overlay") {
        s.status = "skipped";
        s.detail = "Off in settings";
      } else if (s.id === "network" && !gameId) {
        s.status = "skipped";
      } else s.status = "done";
      onStep(structuredClone(steps));
    }
    this.log({ title: gameId ? "Game Mode session started" : "Quick tune-up", detail: `${freed} MB freed`, kind: "power", temporary: true });
    return { freedMB: freed, steps, durationMs: Date.now() - t0 };
  }

  async getTraffic(): Promise<TrafficApp[]> {
    const base: Omit<TrafficApp, "history">[] = [
      { id: "steam", name: "Steam", downKbps: jitter(18_500, 0.2), upKbps: jitter(120, 0.3), purpose: "update" },
      { id: "chrome", name: "Google Chrome", downKbps: jitter(920, 0.6), upKbps: jitter(40, 0.5), purpose: "streaming" },
      { id: "onedrive", name: "OneDrive", downKbps: jitter(40, 0.5), upKbps: jitter(2_600, 0.3), purpose: "cloud-sync" },
      { id: "wu", name: "Windows Update (Delivery Optimization)", downKbps: jitter(2_400, 0.4), upKbps: jitter(40, 0.5), purpose: "update" },
      { id: "spotify", name: "Spotify", downKbps: jitter(320, 0.3), upKbps: 4, purpose: "streaming" },
      { id: "discord", name: "Discord", downKbps: jitter(64, 0.3), upKbps: jitter(48, 0.3), purpose: "voice" },
      { id: "diag", name: "Connected User Experiences (DiagTrack)", downKbps: jitter(3, 0.8), upKbps: jitter(12, 0.8), purpose: "telemetry" },
      { id: "updater", name: "AppUpdateHelper.exe", downKbps: jitter(600, 0.5), upKbps: 10, purpose: "unknown" },
    ];
    if (this.gameRunning) base.unshift({ id: "game", name: this.gameRunning.name, downKbps: jitter(95, 0.2), upKbps: jitter(60, 0.2), purpose: "game" });
    return base.map((a) => {
      const h = this.trafficHistory.get(a.id) ?? Array.from({ length: 60 }, () => (a.downKbps + a.upKbps) * jitter(1, 0.3));
      h.push(a.downKbps + a.upKbps);
      while (h.length > 60) h.shift();
      this.trafficHistory.set(a.id, h);
      return { ...a, history: [...h] };
    });
  }

  async getConnections(): Promise<Connection[]> {
    const rows: [string, Connection["protocol"], string, string, string, string, string][] = [
      ["steam.exe", "TCP", "23.62.%d.%d:443", "ESTABLISHED", "a23-62-1-1.deploy.static.akamaitechnologies.com", "NL", "AS20940 Akamai"],
      ["chrome.exe", "TCP", "142.250.%d.%d:443", "ESTABLISHED", "fra16s56-in-f14.1e100.net", "DE", "AS15169 Google"],
      ["chrome.exe", "UDP", "142.250.%d.%d:443", "-", "QUIC", "DE", "AS15169 Google"],
      ["Discord.exe", "UDP", "66.22.%d.%d:50004", "-", "Voice server", "DE", "AS49544 i3D.net"],
      ["Discord.exe", "TCP", "162.159.%d.%d:443", "ESTABLISHED", "gateway.discord.gg", "US", "AS13335 Cloudflare"],
      ["OneDrive.exe", "TCP", "13.107.%d.%d:443", "ESTABLISHED", "onedrive.live.com", "IE", "AS8068 Microsoft"],
      ["svchost.exe", "TCP", "13.89.%d.%d:443", "ESTABLISHED", "Delivery Optimization", "US", "AS8075 Microsoft"],
      ["Spotify.exe", "TCP", "35.186.%d.%d:443", "ESTABLISHED", "spclient.wg.spotify.com", "BE", "AS15169 Google"],
      ["RiotClientServices.exe", "TCP", "104.18.%d.%d:443", "ESTABLISHED", "auth.riotgames.com", "US", "AS13335 Cloudflare"],
    ];
    if (this.gameRunning) rows.unshift([this.gameRunning.exe, "UDP", "162.249.%d.%d:7086", "-", "Game server (detected via ETW)", "DE", "AS6507 Riot Games"]);
    const out: Connection[] = [];
    rows.forEach((r, i) => {
      for (let k = 0; k < (i < 3 ? 3 : 1); k++) {
        const remote = r[2].replace("%d", String(1 + ((i * 7 + k) % 250))).replace("%d", String(10 + k * 3));
        out.push({
          id: `conn-${i}-${k}`,
          app: r[0],
          protocol: r[1],
          local: `192.168.1.24:${49_152 + i * 31 + k}`,
          remote,
          state: r[3],
          rdns: r[4],
          country: r[5],
          asn: r[6],
        });
      }
    });
    return out;
  }

  async getRules() {
    return [...this.rules];
  }

  async saveRule(rule: ShaperRule) {
    const i = this.rules.findIndex((r) => r.id === rule.id);
    if (i >= 0) this.rules[i] = rule;
    else this.rules.push(rule);
  }

  async deleteRule(id: string) {
    this.rules = this.rules.filter((r) => r.id !== id || r.essential);
  }

  async setFocusMode(on: boolean) {
    await sleep(400);
    if (on) this.log({ title: "Focus mode on", detail: "Firewall rules (group OTX.GNG) + QoS throttles applied", kind: "firewall", temporary: true });
    else this.changes = this.changes.filter((c) => c.kind !== "firewall");
  }

  async restoreInternet() {
    await sleep(500);
    this.changes = this.changes.filter((c) => c.kind !== "firewall" && c.kind !== "qos");
  }

  async getAdapters() {
    return MOCK_ADAPTERS.map((a) => ({ ...a, pingMs: a.pingMs === null ? null : jitter(a.pingMs, 0.15) }));
  }

  async runQualityTest(target: string, onProgress: (pct: number) => void): Promise<QualityResult> {
    for (let i = 1; i <= 20; i++) {
      await sleep(120);
      onProgress(i * 5);
    }
    return { target, method: "UDP", pingMs: jitter(31, 0.1), jitterMs: jitter(4, 0.3), lossPct: 0.4, idleMs: 31, loadedDownMs: jitter(68, 0.2), loadedUpMs: jitter(142, 0.15), dataUsedMB: 48 };
  }

  async traceroute(target: string): Promise<Hop[]> {
    await sleep(1600);
    return [
      { n: 1, host: "router.home", ip: "192.168.1.1", asn: "Private", avgMs: 3.1, lossPct: 0, segment: "local" },
      { n: 2, host: "-", ip: "100.72.0.1", asn: "AS64512 Your ISP", avgMs: 9.8, lossPct: 0, segment: "isp" },
      { n: 3, host: "core1.isp.example", ip: "203.0.113.9", asn: "AS64512 Your ISP", avgMs: 11.2, lossPct: 0, segment: "isp" },
      { n: 4, host: "ix-ae5.transit.example", ip: "198.51.100.17", asn: "AS64600 Transit", avgMs: 18.9, lossPct: 0, segment: "transit" },
      { n: 5, host: "be2.fra.transit.example", ip: "198.51.100.88", asn: "AS64600 Transit", avgMs: 27.4, lossPct: 40, segment: "transit" },
      { n: 6, host: "edge.game.example", ip: "192.0.2.10", asn: "AS64700 Game network", avgMs: 29.6, lossPct: 0, segment: "transit" },
      { n: 7, host: target, ip: "192.0.2.77", asn: "AS64700 Game network", avgMs: 30.8, lossPct: 0, segment: "destination" },
    ];
  }

  async getTuneFindings() {
    return [...MOCK_TUNE];
  }

  async getCommunity() {
    return MOCK_COMMUNITY.map((c) => ({ ...c }));
  }

  async pingCommunity(id: string) {
    await sleep(600);
    const c = MOCK_COMMUNITY.find((x) => x.id === id);
    if (!c || !c.signatureOk || c.kind === "dns") return null;
    return c.region === "Asia" ? jitter(190, 0.1) : jitter(24, 0.15);
  }

  private log(c: Omit<ChangeRecord, "id" | "at">) {
    this.changes.unshift({ ...c, id: nextId("chg"), at: Date.now() });
  }
}
