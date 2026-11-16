/** The Backend contract between the UI and the system layer.
 *  Phase 1: MockBackend (realistic fake data). Phase 2+: TauriBackend talks to the Core service.
 *  From Phase 2 on these types are generated from the Rust `otx-protocol` crate (ts-rs). */

import type { CoreConnection } from "./generated/CoreConnection";
import type { SystemInfo } from "./generated/SystemInfo";

export type Tier = 0 | 1 | 2;

/** Generated from Rust (crates/otx-protocol) by ts-rs. Never edit the generated files by hand. */
export type { SystemInfo, CoreConnection };
export type { CoreStatus } from "./generated/CoreStatus";
export type { GpuInfo } from "./generated/GpuInfo";
export type { PowerPlan } from "./generated/PowerPlan";

/** System facts plus where they came from. "mock" only in the browser preview. */
export interface SystemSnapshot {
  source: "core" | "local" | "mock";
  info: SystemInfo;
  /** Machine-wide plus current-user startup entries. */
  startupApps: number;
}

export interface Metrics {
  t: number;
  cpu: number;
  perCore: number[];
  gpu: { d3: number; compute: number; copy: number; videoDecode: number };
  ram: { totalMB: number; usedMB: number; availableMB: number; commitMB: number; commitLimitMB: number; standbyMB: number };
  disk: { readMBs: number; writeMBs: number; activePct: number };
  net: { downKbps: number; upKbps: number };
  ping: { ms: number; jitter: number; loss: number } | null;
  fps: { avg: number; low1: number } | null;
}

export type ProcessCategory = "game" | "launcher" | "browser" | "system" | "background" | "unknown";
export type PriorityLevel = "idle" | "below" | "normal" | "above" | "high";

export interface ProcessGroup {
  id: string;
  name: string;
  exe: string;
  category: ProcessCategory;
  pids: number[];
  cpu: number;
  gpu: number;
  gpuEngine: string;
  wsMB: number;
  privateMB: number;
  commitMB: number;
  diskReadKBs: number;
  diskWriteKBs: number;
  netDownKbps: number;
  netUpKbps: number;
  protected: boolean;
  hog: boolean;
  publisher: string;
  signed: boolean;
  description: string;
  path: string;
  startup: boolean;
  safeToClose: "yes" | "no" | "caution";
  priority: PriorityLevel;
  eco: boolean;
}

export type Risk = "safe" | "moderate" | "advanced";
export type Evidence = "documented" | "community" | "experimental";

export interface NotRecommendedRule {
  label: string;
  /** Evaluated against THIS machine by the backend. */
  matches: boolean;
}

export interface Tweak {
  id: string;
  group: "system" | "input" | "network" | "power" | "game";
  title: string;
  description: string;
  icon: string; // lucide icon name, resolved in the UI
  risk: Risk;
  evidence: Evidence;
  beta: boolean;
  requiresReboot: boolean;
  appliesTo: ("win10" | "win11")[];
  inHighPerformance: boolean;
  enabled: boolean;
  supported: boolean;
  unsupportedReason?: string;
  notRecommendedIf: NotRecommendedRule[];
  conflictNote?: string;
}

export type TweakPreset = "default" | "high" | "custom";

export interface TweakApplyResult {
  ok: boolean;
  dryRun: boolean;
  changes: string[]; // human-readable description of each value that would change / changed
  error?: string;
}

export interface RamStatus {
  totalMB: number;
  usedMB: number;
  availableMB: number;
  standbyMB: number;
  modifiedMB: number;
  commitPct: number;
  status: "healthy" | "needs-cleaning";
  reason: string;
}

export type RamMethod = "memoryPriority" | "trimWorkingSets" | "purgeStandby" | "flushModified" | "trimFileCache";

export interface RamCleanResult {
  id: string;
  at: number;
  trigger: "manual" | "game-start" | "status" | "schedule" | "boost";
  methods: RamMethod[];
  availableBeforeMB: number;
  availableAfterMB: number;
  freedMB: number;
  skipped: string[];
}

export interface DnsServer {
  id: string;
  name: string;
  v4: string[];
  v6: string[];
  filtering: boolean;
  isp: boolean;
  custom: boolean;
}

export interface DnsResult {
  serverId: string;
  medianMs: number;
  jitterMs: number;
  failures: number;
  samples: number;
}

export interface Game {
  id: string;
  name: string;
  launcher: "steam" | "epic" | "riot" | "battlenet" | "xbox" | "ea" | "ubisoft" | "manual";
  exe: string;
  installPath: string;
  sizeGB: number;
  lastPlayed: number | null;
  tint: string; // "r g b" triplet extracted locally from the game's own icon (mocked here)
  antiCheat: string | null;
  engine: "unreal" | "unity" | "source" | "other";
  profile: GameProfile;
}

export interface GameProfile {
  tweakPreset: TweakPreset;
  ramCleanOnLaunch: boolean;
  cpuPriority: "normal" | "above" | "high";
  cpuSets: "auto" | "performance-cores" | "off";
  ioPriority: "normal" | "high";
  gpuPreference: "default" | "power-saving" | "high-performance";
  disableFullscreenOptimizations: boolean;
  networkProfile: "none" | "focus" | "esports" | "streaming";
  backgroundApps: string[];
}

export interface ChangeRecord {
  id: string;
  at: number;
  title: string;
  detail: string;
  kind: "tweak" | "priority" | "firewall" | "qos" | "power" | "dns" | "ram";
  temporary: boolean;
}

export interface BoostStep {
  id: string;
  label: string;
  status: "pending" | "running" | "done" | "skipped" | "failed";
  detail?: string;
}

export interface BoostSummary {
  freedMB: number;
  steps: BoostStep[];
  durationMs: number;
}

export interface TrafficApp {
  id: string;
  name: string;
  downKbps: number;
  upKbps: number;
  purpose: "game" | "launcher" | "update" | "cloud-sync" | "streaming" | "browser" | "telemetry" | "voice" | "system" | "unknown";
  history: number[]; // last 60 s, total kbps
}

export interface Connection {
  id: string;
  app: string;
  protocol: "TCP" | "UDP";
  local: string;
  remote: string;
  state: string;
  rdns: string;
  country: string;
  asn: string;
}

export type RuleAction = "allow" | "limit" | "block";

export interface ShaperRule {
  id: string;
  app: string;
  action: RuleAction;
  upKbps?: number;
  downKbps?: number;
  priority: "high" | "normal" | "low";
  onlyWhileGaming: boolean;
  essential: boolean;
}

export interface Adapter {
  id: string;
  name: string;
  kind: "ethernet" | "wifi" | "tether";
  connected: boolean;
  pingMs: number | null;
  lossPct: number | null;
  metric: number;
  preferred: boolean;
}

export interface QualityResult {
  target: string;
  method: "ICMP" | "TCP" | "UDP";
  pingMs: number;
  jitterMs: number;
  lossPct: number;
  idleMs: number;
  loadedDownMs: number;
  loadedUpMs: number;
  dataUsedMB: number;
}

export interface Hop {
  n: number;
  host: string;
  ip: string;
  asn: string;
  avgMs: number;
  lossPct: number;
  segment: "local" | "isp" | "transit" | "destination";
}

export interface CommunityEntry {
  id: string;
  kind: "dns" | "relay";
  name: string;
  address: string;
  region: string;
  pingMs: number | null;
  favorite: boolean;
  signatureOk: boolean;
  source: string;
}

export interface TuneFinding {
  id: string;
  title: string;
  detail: string;
  severity: "ok" | "info" | "warn";
  evidence: Evidence;
  fixLabel?: string;
}

export interface Backend {
  readonly kind: "mock" | "tauri";
  getSystemInfo(): Promise<SystemSnapshot>;
  getCoreConnection(): Promise<CoreConnection>;
  onMetrics(cb: (m: Metrics) => void, rateHz: number): () => void;
  getProcesses(): Promise<ProcessGroup[]>;
  setPriority(id: string, level: PriorityLevel): Promise<void>;
  setEfficiencyMode(id: string, on: boolean): Promise<void>;
  endTask(id: string): Promise<void>;

  getTweaks(): Promise<Tweak[]>;
  setTweak(id: string, enabled: boolean, dryRun: boolean): Promise<TweakApplyResult>;
  applyPreset(preset: Exclude<TweakPreset, "custom">, dryRun: boolean): Promise<TweakApplyResult>;

  getRamStatus(): Promise<RamStatus>;
  cleanRam(methods: RamMethod[], trigger: RamCleanResult["trigger"]): Promise<RamCleanResult>;
  getRamHistory(): Promise<RamCleanResult[]>;

  getDnsServers(): Promise<DnsServer[]>;
  benchmarkDns(onProgress: (done: number, total: number) => void): Promise<DnsResult[]>;
  applyDns(serverId: string): Promise<void>;
  resetDns(): Promise<void>;
  getActiveDns(): Promise<string | null>;
  addDnsServer(name: string, v4: string[]): Promise<DnsServer>;

  getGames(): Promise<Game[]>;
  updateGameProfile(id: string, profile: GameProfile): Promise<void>;

  getChanges(): Promise<ChangeRecord[]>;
  revertChange(id: string): Promise<void>;
  revertAll(): Promise<void>;
  boost(gameId: string | null, onStep: (steps: BoostStep[]) => void): Promise<BoostSummary>;

  getTraffic(): Promise<TrafficApp[]>;
  getConnections(): Promise<Connection[]>;
  getRules(): Promise<ShaperRule[]>;
  saveRule(rule: ShaperRule): Promise<void>;
  deleteRule(id: string): Promise<void>;
  setFocusMode(on: boolean): Promise<void>;
  restoreInternet(): Promise<void>;

  getAdapters(): Promise<Adapter[]>;
  runQualityTest(target: string, onProgress: (pct: number) => void): Promise<QualityResult>;
  traceroute(target: string): Promise<Hop[]>;
  getTuneFindings(): Promise<TuneFinding[]>;
  getCommunity(): Promise<CommunityEntry[]>;
  pingCommunity(id: string): Promise<number | null>;
}
