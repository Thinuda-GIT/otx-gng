import { describe, expect, it } from "vitest";
import { analyze, MIN_SAMPLES, type AnalyzerSample } from "./analyzer";
import { compare, summarizeFrametimes, type RunSummary } from "./benchStats";
import { formatRate, isValidIPv4, median, timeAgo } from "./format";
import { classifyRenderer, effectiveGlass } from "./glassQuality";
import { computeHealth, lerpScore } from "./healthScore";
import { rank } from "./search";
import { sanitizeImport, DEFAULT_SETTINGS } from "../store/settings";
import { shortcutFromEvent } from "../pages/Settings";
import { isValidRelayAddress } from "../pages/CommunityServers";

describe("glass quality", () => {
  const base = { setting: "auto" as const, gameRunning: false, gpuTier: "discrete" as const, batterySaver: false, reduceTransparency: false };
  it("is Full on a capable GPU with no game", () => expect(effectiveGlass(base)).toBe("full"));
  it("turns Off whenever a game runs in Auto", () => expect(effectiveGlass({ ...base, gameRunning: true })).toBe("off"));
  it("uses Lite on integrated GPUs and battery saver", () => {
    expect(effectiveGlass({ ...base, gpuTier: "integrated" })).toBe("lite");
    expect(effectiveGlass({ ...base, batterySaver: true })).toBe("lite");
  });
  it("respects a manual setting even with a game running", () => expect(effectiveGlass({ ...base, setting: "full", gameRunning: true })).toBe("full"));
  it("reduce transparency always wins", () => expect(effectiveGlass({ ...base, setting: "full", reduceTransparency: true })).toBe("off"));
  it("classifies renderer strings", () => {
    expect(classifyRenderer("ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Laptop GPU Direct3D11 vs_5_0 ps_5_0, D3D11)")).toBe("discrete");
    expect(classifyRenderer("ANGLE (Intel, Intel(R) Iris(R) Xe Graphics Direct3D11 vs_5_0 ps_5_0, D3D11)")).toBe("integrated");
    expect(classifyRenderer("ANGLE (AMD, AMD Radeon(TM) Graphics Direct3D11)")).toBe("integrated");
    expect(classifyRenderer("ANGLE (AMD, AMD Radeon RX 6700 XT Direct3D11)")).toBe("discrete");
    expect(classifyRenderer("Google SwiftShader")).toBe("software");
    expect(classifyRenderer(null)).toBe("unknown");
  });
});

describe("health score", () => {
  it("lerpScore is 100 at good, 0 at bad, clamped", () => {
    expect(lerpScore(5, 5, 45)).toBe(100);
    expect(lerpScore(45, 5, 45)).toBe(0);
    expect(lerpScore(25, 5, 45)).toBe(50);
    expect(lerpScore(100, 5, 45)).toBe(0);
    expect(lerpScore(40, 35, 8)).toBe(100); // higher-is-better direction
  });
  it("a healthy PC scores high and a stressed PC scores low", () => {
    const good = computeHealth({ ramAvailablePct: 50, commitPct: 40, backgroundCpuPct: 2, backgroundGpuPct: 1, diskActivePct: 5, pingMs: 20, jitterMs: 2, lossPct: 0, powerPlan: "high", startupApps: 3 });
    const bad = computeHealth({ ramAvailablePct: 6, commitPct: 98, backgroundCpuPct: 60, backgroundGpuPct: 40, diskActivePct: 95, pingMs: 200, jitterMs: 40, lossPct: 6, powerPlan: "saver", startupApps: 25 });
    expect(good.score).toBeGreaterThanOrEqual(95);
    expect(bad.score).toBeLessThanOrEqual(10);
    expect(good.factors).toHaveLength(7);
  });
  it("unmeasured network counts as neutral 50", () => {
    const r = computeHealth({ ramAvailablePct: 50, commitPct: 40, backgroundCpuPct: 2, backgroundGpuPct: 1, diskActivePct: 5, pingMs: null, jitterMs: null, lossPct: null, powerPlan: "high", startupApps: 3 });
    expect(r.factors.find((f) => f.id === "network")!.score).toBe(50);
  });
});

describe("analyzer", () => {
  const s = (o: Partial<AnalyzerSample>): AnalyzerSample => ({ cpuTotal: 30, maxCoreLoad: 50, gpu3d: 50, ramAvailablePct: 40, diskActivePct: 10, cpuTempC: null, gpuTempC: null, pingMs: 30, jitterMs: 2, lossPct: 0, ...o });
  const many = (o: Partial<AnalyzerSample>) => Array.from({ length: 30 }, () => s(o));
  it("says can't tell with too few samples", () => expect(analyze([s({})])[0]!.id).toBe("cant-tell"));
  it("detects GPU-bound", () => expect(analyze(many({ gpu3d: 99 })).map((f) => f.id)).toContain("gpu-bound"));
  it("detects CPU-bound only when the GPU is not saturated", () => {
    expect(analyze(many({ maxCoreLoad: 97, gpu3d: 60 })).map((f) => f.id)).toContain("cpu-bound");
    expect(analyze(many({ maxCoreLoad: 97, gpu3d: 99 })).map((f) => f.id)).not.toContain("cpu-bound");
  });
  it("detects memory pressure and network trouble", () => {
    const ids = analyze(many({ ramAvailablePct: 5, jitterMs: 30 })).map((f) => f.id);
    expect(ids).toContain("memory");
    expect(ids).toContain("network");
  });
  it("ignores thermals when sensors are off", () => expect(analyze(many({})).map((f) => f.id)).toEqual(["cant-tell"]));
  it("flags thermals when sensors report heat", () => expect(analyze(many({ cpuTempC: 99 })).map((f) => f.id)).toContain("thermal"));
  it("MIN_SAMPLES is enforced", () => expect(analyze(Array.from({ length: MIN_SAMPLES - 1 }, () => s({ gpu3d: 99 })))[0]!.id).toBe("cant-tell"));
});

describe("benchmark stats", () => {
  it("summarizes constant frame times exactly", () => {
    const r = summarizeFrametimes(Array.from({ length: 1000 }, () => 5));
    expect(r.avgFps).toBeCloseTo(200);
    expect(r.low1).toBeCloseTo(200);
    expect(r.frametimeStdMs).toBeCloseTo(0);
  });
  it("1% low reflects the slowest frames", () => {
    const ft = [...Array.from({ length: 990 }, () => 5), ...Array.from({ length: 10 }, () => 20)];
    expect(summarizeFrametimes(ft).low1).toBeCloseTo(50);
  });
  const run = (avgFps: number): RunSummary => ({ avgFps, low1: avgFps * 0.7, low01: avgFps * 0.5, frametimeStdMs: 1 });
  it("needs 3 runs per side", () => expect(compare([run(100)], [run(110)])).toBeNull());
  it("reports within noise when the change is smaller than the spread", () => {
    const r = compare([run(100), run(95), run(105)], [run(101), run(97), run(104)])!;
    expect(r.find((x) => x.metric === "avgFps")!.verdict).toBe("within-noise");
  });
  it("reports better when the change beats the spread", () => {
    const r = compare([run(100), run(101), run(99)], [run(120), run(121), run(119)])!;
    expect(r.find((x) => x.metric === "avgFps")!.verdict).toBe("better");
  });
});

describe("format and validation", () => {
  it("validates IPv4 strictly", () => {
    expect(isValidIPv4("1.1.1.1")).toBe(true);
    expect(isValidIPv4("255.255.255.255")).toBe(true);
    expect(isValidIPv4("256.1.1.1")).toBe(false);
    expect(isValidIPv4("01.1.1.1")).toBe(false);
    expect(isValidIPv4("1.1.1")).toBe(false);
    expect(isValidIPv4("a.b.c.d")).toBe(false);
  });
  it("validates relay addresses", () => {
    expect(isValidRelayAddress("relay.example.org:51820")).toBe(true);
    expect(isValidRelayAddress("10.0.0.5:443")).toBe(true);
    expect(isValidRelayAddress("relay.example.org")).toBe(false);
    expect(isValidRelayAddress("relay.example.org:70000")).toBe(false);
    expect(isValidRelayAddress("http://x:1")).toBe(false);
  });
  it("median and rates", () => {
    expect(median([3, 1, 2])).toBe(2);
    expect(median([4, 1, 2, 3])).toBe(2.5);
    expect(Number.isNaN(median([]))).toBe(true);
    expect(formatRate(950)).toBe("950 kbps");
    expect(formatRate(18500)).toBe("18.5 Mbps");
  });
  it("timeAgo buckets", () => {
    const now = 1_000_000_000;
    expect(timeAgo(now - 10_000, now)).toBe("just now");
    expect(timeAgo(now - 5 * 60_000, now)).toBe("5 min ago");
    expect(timeAgo(now - 3 * 3_600_000, now)).toBe("3 h ago");
  });
});

describe("command palette ranking", () => {
  const items = [
    { id: "a", label: "PC Boost", keywords: ["ram", "dns"] },
    { id: "b", label: "Usage Monitor", keywords: ["cpu"] },
    { id: "c", label: "Clean RAM" },
  ];
  it("ranks prefix matches first and finds keywords", () => {
    expect(rank(items, "clean")[0]!.id).toBe("c");
    expect(rank(items, "cpu")[0]!.id).toBe("b");
    // An exact keyword hit ("ram" on PC Boost) outranks a word-start label hit ("Clean RAM").
    expect(rank(items, "ram").map((i) => i.id)).toEqual(["a", "c"]);
    expect(rank(items, "zzz")).toEqual([]);
    expect(rank(items, "")).toHaveLength(3);
  });
});

describe("settings import", () => {
  it("keeps only known keys with matching types", () => {
    const r = sanitizeImport({ glassQuality: "lite", uiScale: "big", unknownKey: 1, ramAllowList: ["a.exe", 5], simulateGameId: null });
    expect(r).toEqual({ glassQuality: "lite", simulateGameId: null });
  });
  it("rejects non-objects", () => expect(sanitizeImport("x")).toEqual({}));
  it("defaults never opt into telemetry or startup", () => {
    expect(DEFAULT_SETTINGS.telemetry).toBe(false);
    expect(DEFAULT_SETTINGS.launchOnStartup).toBe(false);
    expect(DEFAULT_SETTINGS.hardwareMonitoring).toBe(false);
  });
});

describe("shortcut capture", () => {
  const ev = (key: string, m: Partial<{ ctrlKey: boolean; shiftKey: boolean; altKey: boolean }> = {}) => ({ key, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, ...m });
  it("builds Shift+Ctrl+Q", () => expect(shortcutFromEvent(ev("q", { ctrlKey: true, shiftKey: true }))).toBe("Shift+Ctrl+Q"));
  it("requires a modifier", () => expect(shortcutFromEvent(ev("q"))).toBeNull());
  it("ignores lone modifiers", () => expect(shortcutFromEvent(ev("Control", { ctrlKey: true }))).toBeNull());
  it("accepts function keys", () => expect(shortcutFromEvent(ev("F9", { altKey: true }))).toBe("Alt+F9"));
});
