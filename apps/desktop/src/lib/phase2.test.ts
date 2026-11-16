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
