/** "Why is my game slow?" rule-based analyzer.
 *  Works only from measured samples. Says "can't tell" when the data is insufficient. */

export interface AnalyzerSample {
  cpuTotal: number; // %
  maxCoreLoad: number; // % of the busiest logical core
  gpu3d: number; // % of the busiest GPU 3D engine
  ramAvailablePct: number;
  diskActivePct: number;
  cpuTempC: number | null; // only when Hardware data monitoring is on
  gpuTempC: number | null;
  pingMs: number | null;
  jitterMs: number | null;
  lossPct: number | null;
}

export type Confidence = "high" | "medium" | "low";

export interface Finding {
  id: "cpu-bound" | "gpu-bound" | "memory" | "disk" | "thermal" | "network" | "cant-tell";
  title: string;
  detail: string;
  confidence: Confidence;
  fixes: { id: string; label: string }[];
}

const avg = (xs: number[]): number => xs.reduce((a, b) => a + b, 0) / xs.length;
const share = (xs: number[], pred: (v: number) => boolean): number => xs.filter(pred).length / xs.length;

/** Needs at least 10 samples (about 10 s at 1 Hz) to say anything. */
export const MIN_SAMPLES = 10;

export function analyze(samples: AnalyzerSample[]): Finding[] {
  if (samples.length < MIN_SAMPLES) {
    return [
      {
        id: "cant-tell",
        title: "Not enough data yet",
        detail: `Play for at least ${MIN_SAMPLES} seconds with the game in focus, then run the analysis again.`,
        confidence: "low",
        fixes: [],
      },
    ];
  }

  const out: Finding[] = [];
  const gpuHigh = share(samples.map((s) => s.gpu3d), (v) => v >= 95);
  const coreHigh = share(samples.map((s) => s.maxCoreLoad), (v) => v >= 92);
  const gpuAvg = avg(samples.map((s) => s.gpu3d));

  if (gpuHigh >= 0.7) {
    out.push({
      id: "gpu-bound",
      title: "Your GPU is the limit",
      detail: `The GPU 3D engine was at 95% or more in ${Math.round(gpuHigh * 100)}% of samples. Lowering resolution, upscaling or shadow quality helps more than any system tweak.`,
      confidence: gpuHigh >= 0.85 ? "high" : "medium",
      fixes: [{ id: "gpu-pref", label: "Check GPU preference for this game" }],
    });
  } else if (coreHigh >= 0.6 && gpuAvg < 85) {
    out.push({
      id: "cpu-bound",
      title: "Your CPU is the limit",
      detail: `At least one CPU core was at 92% or more in ${Math.round(coreHigh * 100)}% of samples while the GPU averaged ${gpuAvg.toFixed(0)}%. Background apps and power settings matter here.`,
      confidence: coreHigh >= 0.8 ? "high" : "medium",
      fixes: [
        { id: "bg-priority", label: "Lower background app priority" },
        { id: "power-plan", label: "Use High performance power plan" },
      ],
    });
  }

  const ramLow = share(samples.map((s) => s.ramAvailablePct), (v) => v < 12);
  if (ramLow >= 0.3) {
    out.push({
      id: "memory",
      title: "Memory is running low",
      detail: `Available memory was under 12% in ${Math.round(ramLow * 100)}% of samples, so Windows is likely paging.`,
      confidence: ramLow >= 0.6 ? "high" : "medium",
      fixes: [
        { id: "ram-clean", label: "Free up RAM" },
        { id: "close-hogs", label: "Review memory-heavy apps" },
      ],
    });
  }

  const diskBusy = share(samples.map((s) => s.diskActivePct), (v) => v >= 85);
  if (diskBusy >= 0.3) {
    out.push({
      id: "disk",
      title: "The disk is very busy",
      detail: `Disk active time was 85% or more in ${Math.round(diskBusy * 100)}% of samples. Updates, antivirus scans or downloads may be competing with the game.`,
      confidence: "medium",
      fixes: [{ id: "pause-updates", label: "Pause updates for this session" }],
    });
  }

  const temps = samples.map((s) => Math.max(s.cpuTempC ?? 0, s.gpuTempC ?? 0));
  if (samples.some((s) => s.cpuTempC !== null || s.gpuTempC !== null)) {
    const hot = share(temps, (v) => v >= 95);
    if (hot >= 0.2) {
      out.push({
        id: "thermal",
        title: "Hardware is running hot",
        detail: "Temperatures reached 95 °C or more, which usually means clocks are being throttled. Clean the vents and check airflow.",
        confidence: "medium",
        fixes: [],
      });
    }
  }

  const pings = samples.filter((s) => s.pingMs !== null);
  if (pings.length >= MIN_SAMPLES) {
    const jitter = avg(pings.map((s) => s.jitterMs ?? 0));
    const loss = avg(pings.map((s) => s.lossPct ?? 0));
    const ping = avg(pings.map((s) => s.pingMs!));
    if (jitter > 15 || loss > 1 || ping > 120) {
      out.push({
        id: "network",
        title: "The connection is unstable",
        detail: `Average ping ${ping.toFixed(0)} ms, jitter ${jitter.toFixed(0)} ms, loss ${loss.toFixed(1)}%. Run Route Lab to see whether the problem is your Wi-Fi, your ISP or further away.`,
        confidence: "medium",
        fixes: [
          { id: "route-lab", label: "Open Route Lab" },
          { id: "focus-mode", label: "Turn on Focus mode" },
        ],
      });
    }
  }

  if (out.length === 0) {
    out.push({
      id: "cant-tell",
      title: "No clear bottleneck",
      detail: "Nothing in the measured data stands out. If the game still feels slow, run a Benchmark Lab capture to look at frame times.",
      confidence: "low",
      fixes: [{ id: "benchmark", label: "Open Benchmark Lab" }],
    });
  }
  return out;
}
