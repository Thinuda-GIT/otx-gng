/** PC Health Score: a transparent 0-100 score built from measured factors.
 *  Each factor reports its own score, weight, the measured value and a plain explanation. */

export interface HealthInputs {
  ramAvailablePct: number; // available / total * 100
  commitPct: number; // commit / commit limit * 100
  backgroundCpuPct: number; // CPU used by non-game, non-system processes
  backgroundGpuPct: number;
  diskActivePct: number;
  pingMs: number | null;
  jitterMs: number | null;
  lossPct: number | null;
  powerPlan: "ultimate" | "high" | "balanced" | "saver" | "unknown";
  startupApps: number;
}

export interface HealthFactor {
  id: string;
  label: string;
  score: number; // 0-100
  weight: number;
  value: string;
  explain: string;
}

export interface HealthResult {
  score: number;
  factors: HealthFactor[];
}

/** 100 at `good`, 0 at `bad`, linear between. Works whether higher or lower is better. */
export function lerpScore(v: number, good: number, bad: number): number {
  const t = (v - good) / (bad - good);
  return Math.round(100 * Math.min(1, Math.max(0, 1 - t)));
}

const PLAN_SCORE: Record<HealthInputs["powerPlan"], number> = {
  ultimate: 100,
  high: 95,
  balanced: 70,
  saver: 20,
  unknown: 60,
};

export function computeHealth(i: HealthInputs): HealthResult {
  const netScore =
    i.pingMs === null
      ? 50
      : Math.round(
          0.5 * lerpScore(i.pingMs, 25, 150) +
            0.25 * lerpScore(i.jitterMs ?? 0, 3, 30) +
            0.25 * lerpScore(i.lossPct ?? 0, 0, 5),
        );

  const factors: HealthFactor[] = [
    {
      id: "memory",
      label: "Memory pressure",
      weight: 22,
      score: Math.min(lerpScore(i.ramAvailablePct, 35, 8), lerpScore(i.commitPct, 70, 97)),
      value: `${i.ramAvailablePct.toFixed(0)}% available, commit ${i.commitPct.toFixed(0)}%`,
      explain: "Low available memory or a nearly full commit limit makes Windows page to disk, which causes stutter.",
    },
    {
      id: "bgcpu",
      label: "Background CPU",
      weight: 20,
      score: lerpScore(i.backgroundCpuPct, 5, 45),
      value: `${i.backgroundCpuPct.toFixed(0)}% used by background apps`,
      explain: "CPU time used by apps other than your game competes for cores and can lower 1% lows.",
    },
    {
      id: "bggpu",
      label: "Background GPU",
      weight: 14,
      score: lerpScore(i.backgroundGpuPct, 3, 30),
      value: `${i.backgroundGpuPct.toFixed(0)}% used by background apps`,
      explain: "Browsers, video and overlays that use the GPU take frame time away from the game.",
    },
    {
      id: "disk",
      label: "Disk activity",
      weight: 10,
      score: lerpScore(i.diskActivePct, 15, 90),
      value: `${i.diskActivePct.toFixed(0)}% active time`,
      explain: "A busy disk slows asset streaming. This matters most on hard drives.",
    },
    {
      id: "network",
      label: "Network quality",
      weight: 18,
      score: netScore,
      value:
        i.pingMs === null
          ? "Not measured yet"
          : `${i.pingMs.toFixed(0)} ms ping, ${(i.jitterMs ?? 0).toFixed(0)} ms jitter, ${(i.lossPct ?? 0).toFixed(1)}% loss`,
      explain: "Latency, jitter and packet loss to your last tested server. Unmeasured counts as neutral (50).",
    },
    {
      id: "power",
      label: "Power plan",
      weight: 8,
      score: PLAN_SCORE[i.powerPlan],
      value: i.powerPlan === "unknown" ? "Unknown" : i.powerPlan[0]!.toUpperCase() + i.powerPlan.slice(1),
      explain: "Power saver caps CPU clocks. Balanced is fine on most desktops; High performance helps some laptops when plugged in.",
    },
    {
      id: "startup",
      label: "Startup load",
      weight: 8,
      score: lerpScore(i.startupApps, 4, 20),
      value: `${i.startupApps} apps start with Windows`,
      explain: "Apps that start with Windows keep running in the background and use memory and CPU.",
    },
  ];
  const total = factors.reduce((a, f) => a + f.weight, 0);
  const score = Math.round(factors.reduce((a, f) => a + f.score * f.weight, 0) / total);
  return { score, factors };
}
