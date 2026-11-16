/** Benchmark Lab statistics: frame-time summaries and an honest A/B verdict. */
import { median } from "./format";

export interface RunSummary {
  avgFps: number;
  low1: number; // 1% low FPS
  low01: number; // 0.1% low FPS
  frametimeStdMs: number;
}

/** Summarise one capture from its frame times in milliseconds. */
export function summarizeFrametimes(ft: readonly number[]): RunSummary {
  if (ft.length === 0) return { avgFps: 0, low1: 0, low01: 0, frametimeStdMs: 0 };
  const mean = ft.reduce((a, b) => a + b, 0) / ft.length;
  const sorted = [...ft].sort((a, b) => b - a); // slowest first
  // 1% low = average FPS of the slowest 1% of frames (a common definition; CapFrameX-style)
  const pctAvg = (p: number): number => {
    const n = Math.max(1, Math.floor(sorted.length * p));
    const slice = sorted.slice(0, n);
    return 1000 / (slice.reduce((a, b) => a + b, 0) / n);
  };
  const variance = ft.reduce((a, b) => a + (b - mean) ** 2, 0) / ft.length;
  return {
    avgFps: 1000 / mean,
    low1: pctAvg(0.01),
    low01: pctAvg(0.001),
    frametimeStdMs: Math.sqrt(variance),
  };
}

export interface Comparison {
  metric: keyof RunSummary;
  baselineMedian: number;
  boostedMedian: number;
  deltaPct: number;
  spreadPct: number; // the larger relative run-to-run spread of the two groups
  verdict: "better" | "worse" | "within-noise";
}

/** Relative spread of a group: (max - min) / median. */
function spread(xs: number[]): number {
  const m = median(xs);
  if (!Number.isFinite(m) || m === 0) return 0;
  return (Math.max(...xs) - Math.min(...xs)) / m;
}

/** A change only counts when it is larger than the run-to-run spread of both groups.
 *  For frame-time std, lower is better. Requires at least 3 runs per side. */
export function compare(baseline: RunSummary[], boosted: RunSummary[]): Comparison[] | null {
  if (baseline.length < 3 || boosted.length < 3) return null;
  const metrics: (keyof RunSummary)[] = ["avgFps", "low1", "low01", "frametimeStdMs"];
  return metrics.map((metric) => {
    const a = baseline.map((r) => r[metric]);
    const b = boosted.map((r) => r[metric]);
    const am = median(a);
    const bm = median(b);
    const deltaPct = am === 0 ? 0 : ((bm - am) / am) * 100;
    const spreadPct = Math.max(spread(a), spread(b)) * 100;
    const lowerIsBetter = metric === "frametimeStdMs";
    let verdict: Comparison["verdict"] = "within-noise";
    if (Math.abs(deltaPct) > spreadPct && Math.abs(deltaPct) >= 1) {
      const improved = lowerIsBetter ? deltaPct < 0 : deltaPct > 0;
      verdict = improved ? "better" : "worse";
    }
    return { metric, baselineMedian: am, boostedMedian: bm, deltaPct, spreadPct, verdict };
  });
}
