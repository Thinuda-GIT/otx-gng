import { describe, expect, it } from "vitest";
import { MOCK_SYSTEM } from "../backend/mockData";
import type { SystemInfo } from "../backend/types";
import { coreLabel } from "../shell/CoreBadge";
import { tierFromGpus } from "./glassQuality";
import { recommend } from "./recommend";

const base: SystemInfo = structuredClone(MOCK_SYSTEM.info);
const withInfo = (patch: (i: SystemInfo) => void): SystemInfo => {
  const i = structuredClone(base);
  patch(i);
  return i;
};

describe("GPU tier from real adapters", () => {
  it("prefers any discrete adapter", () => expect(tierFromGpus([{ integrated: true }, { integrated: false }])).toBe("discrete"));
  it("integrated only", () => expect(tierFromGpus([{ integrated: true }])).toBe("integrated"));
  it("no hardware adapter means software rendering", () => expect(tierFromGpus([])).toBe("software"));
});

describe("first-run recommendation", () => {
  it("recommends High performance on a plugged-in, supported PC", () => {
    const r = recommend(base);
    expect(r.preset).toBe("high");
    expect(r.reasons.some((x) => x.includes("Hybrid CPU"))).toBe(true);
    expect(r.cautions.some((x) => x.startsWith("Laptop"))).toBe(true);
    expect(r.cautions.some((x) => x.includes("Wi-Fi"))).toBe(true);
  });
  it("stays on Windows defaults on battery", () => {
    expect(recommend(withInfo((i) => (i.power.onBattery = true))).preset).toBe("default");
  });
  it("stays on defaults on unsupported Windows builds", () => {
    const r = recommend(withInfo((i) => ((i.os.build = 19044), (i.os.supported = false))));
    expect(r.preset).toBe("default");
    expect(r.cautions.some((x) => x.includes("19044"))).toBe(true);
  });
  it("warns about HDDs and integrated graphics", () => {
    const r = recommend(withInfo((i) => {
      i.disk = { model: "x", kind: "hdd", systemDrive: "C:" };
      i.gpus = [{ name: "Intel UHD", vendor: "intel", dedicatedVramMb: 128, integrated: true }];
    }));
    expect(r.cautions.some((x) => x.includes("hard disk"))).toBe(true);
    expect(r.cautions.some((x) => x.includes("Integrated graphics"))).toBe(true);
  });
  it("talks about RAM honestly", () => {
    expect(recommend(withInfo((i) => (i.ramMb = 8192))).reasons[0]).toContain("likely to help");
    expect(recommend(withInfo((i) => (i.ramMb = 32768))).reasons[0]).toContain("rarely matter");
  });
});

describe("generated protocol types", () => {
  it("never use bigint (JSON numbers arrive as plain numbers)", async () => {
    const { readdirSync, readFileSync } = await import("node:fs");
    const dir = new URL("../backend/generated/", import.meta.url);
    const files = readdirSync(dir).filter((f) => f.endsWith(".ts"));
    expect(files.length).toBeGreaterThan(20);
    for (const f of files) expect(readFileSync(new URL(f, dir), "utf8"), f).not.toContain("bigint");
  });
});

describe("Core badge", () => {
  it("labels every link state", () => {
    expect(coreLabel(null).text).toBe("Checking…");
    expect(coreLabel({ link: "notRunning", message: null, status: null }).tone).toBe("neutral");
    expect(coreLabel({ link: "untrusted", message: "x", status: null }).tone).toBe("danger");
    const st = { coreVersion: "0.2.0", protocol: 1, mode: "service" as const, elevated: true, safeMode: false, dryRun: false, recoveredChanges: 0, uptimeSecs: 1 };
    expect(coreLabel({ link: "connected", message: null, status: st })).toEqual({ text: "Connected", tone: "ok" });
    expect(coreLabel({ link: "connected", message: null, status: { ...st, safeMode: true } }).tone).toBe("warn");
  });
});

describe("RAM status rule and routine", async () => {
  const { ramStatusFrom } = await import("../backend/tauri");
  const { shouldClean } = await import("./useRamRoutine");
  it("flags low available memory and high commit", () => {
    expect(ramStatusFrom(16000, 900, 20000, 36000, 800).status).toBe("needs-cleaning");
    expect(ramStatusFrom(16000, 9000, 33000, 36000, 800).status).toBe("needs-cleaning");
    expect(ramStatusFrom(16000, 9000, 10000, 36000, 800).status).toBe("healthy");
  });
  it("routine respects the switches and the 10-minute gap", () => {
    const base = { enabled: true, onStatus: true, scheduleOn: false, everyHours: 12, needsCleaning: true, now: 10_000_000, lastClean: 0 };
    expect(shouldClean(base)).toBe("status");
    expect(shouldClean({ ...base, enabled: false })).toBeNull();
    expect(shouldClean({ ...base, lastClean: base.now - 60_000 })).toBeNull();
    expect(shouldClean({ ...base, needsCleaning: false })).toBeNull();
    expect(shouldClean({ ...base, needsCleaning: false, scheduleOn: true, everyHours: 1, now: 4 * 3_600_000, lastClean: 0 })).toBe("schedule");
  });
});

describe("Game Mode mapping", async () => {
  const { sessionProfileFrom, tintFor } = await import("../backend/gameMode");
  const { DEFAULT_PROFILE } = await import("../store/profiles");
  const { sameGame } = await import("./useGameWatcher");
  it("maps profiles safely (no Realtime, plain exe names only)", () => {
    const p = sessionProfileFrom({ ...DEFAULT_PROFILE, cpuPriority: "high", backgroundApps: ["Spotify.exe", "C:/evil/x.exe", "rm -rf"], tweakPreset: "default" });
    expect(p.gamePriority).toBe("high");
    expect(p.background).toEqual(["Spotify.exe"]);
    expect(p.highPerformancePower).toBe(false);
    expect(sessionProfileFrom({ ...DEFAULT_PROFILE, cpuPriority: "above" }).gamePriority).toBe("aboveNormal");
  });
  it("tints are stable rgb triplets", () => {
    expect(tintFor("Valorant")).toBe(tintFor("Valorant"));
    expect(tintFor("Valorant")).toMatch(/^\d{1,3} \d{1,3} \d{1,3}$/);
  });
  it("detects the same running game by pid and creation time", () => {
    const g = { gameId: "steam:730", exe: "cs2.exe", process: { pid: 4, createTime: "1" } };
    expect(sameGame(g, { ...g })).toBe(true);
    expect(sameGame(g, { ...g, process: { pid: 4, createTime: "2" } })).toBe(false);
    expect(sameGame(null, null)).toBe(true);
  });
});

describe("Traffic Shaper and purpose labels", async () => {
  const { enforced, DEFAULT_RULES } = await import("../store/shaper");
  const { purposeOf } = await import("../backend/netPurpose");
  it("never sends allow or essential rules to the Core", () => {
    const e = enforced(DEFAULT_RULES);
    expect(e.map((r) => r.exe)).toEqual(["OneDrive.exe", "steam.exe"]);
    expect(e[1]).toEqual({ exe: "steam.exe", action: "limit", upKbps: 500 });
  });
  it("labels traffic by app", () => {
    expect(purposeOf("OneDrive.exe", null)).toBe("cloud-sync");
    expect(purposeOf("mygame.exe", "MyGame.exe")).toBe("game");
    expect(purposeOf("VALORANT-Win64-Shipping.exe", null)).toBe("game");
    expect(purposeOf("random.exe", null)).toBe("unknown");
  });
});

describe("Failover decision", async () => {
  const { failoverTarget } = await import("./useFailover");
  const a = (id: string, o: Partial<import("../backend/types").Adapter>) => ({ id, name: id, kind: "ethernet" as const, connected: true, pingMs: 20, lossPct: 0, metric: 25, preferred: false, ...o });
  it("switches only after three failed checks to a working connection", () => {
    const list = [a("eth", { preferred: true, lossPct: 100 }), a("wifi", { kind: "wifi", lossPct: 0, pingMs: 30 }), a("warp", { kind: "vpn" })];
    expect(failoverTarget(list, 2)).toBeNull();
    expect(failoverTarget(list, 3)?.id).toBe("wifi");
    expect(failoverTarget([list[0]!, a("wifi", { kind: "wifi", lossPct: 100 })], 5)).toBeNull();
  });
});
