/** Canvas line/area chart (no chart library). Redraws only when data changes, inside rAF,
 *  and never while the UI is paused (game in foreground). DPR-aware. */
import { useEffect, useRef } from "react";
import { useLive } from "../store/live";

/** Canvas cannot read CSS variables, so "var(--rgb-x)" is resolved from the root element. */
function resolveRgb(color: string): string {
  const m = /^var\((--[\w-]+)\)$/.exec(color.trim());
  if (!m) return color.trim();
  const v = getComputedStyle(document.documentElement).getPropertyValue(m[1]!).trim();
  // Tokens may themselves reference another var (e.g. --rgb-accent: var(--rgb-green))
  return v.startsWith("var(") ? resolveRgb(v) : v || "255 255 255";
}

export interface Series {
  values: number[];
  color: string; // "r g b"
  fill?: boolean;
  label?: string;
}

export interface LiveChartProps {
  series: Series[];
  max?: number; // fixed y max; auto when omitted
  height?: number;
  grid?: boolean;
  ariaLabel: string;
}

export function LiveChart({ series, max, height = 120, grid = true, ariaLabel }: LiveChartProps) {
  const ref = useRef<HTMLCanvasElement>(null);
  const paused = useLive((s) => s.paused);
  const frame = useRef(0);

  useEffect(() => {
    if (paused) return;
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => {
      const cv = ref.current;
      if (!cv) return;
      const dpr = window.devicePixelRatio || 1;
      const w = cv.clientWidth;
      const h = cv.clientHeight;
      if (w === 0 || h === 0) return;
      if (cv.width !== Math.round(w * dpr) || cv.height !== Math.round(h * dpr)) {
        cv.width = Math.round(w * dpr);
        cv.height = Math.round(h * dpr);
      }
      const ctx = cv.getContext("2d");
      if (!ctx) return;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      if (grid) {
        ctx.strokeStyle = "rgba(255,255,255,0.06)";
        ctx.lineWidth = 1;
        for (let i = 1; i < 4; i++) {
          const y = Math.round((h * i) / 4) + 0.5;
          ctx.beginPath();
          ctx.moveTo(0, y);
          ctx.lineTo(w, y);
          ctx.stroke();
        }
      }
      const yMax = max ?? Math.max(1, ...series.flatMap((s) => s.values)) * 1.15;
      for (const s of series) {
        const n = s.values.length;
        if (n < 2) continue;
        const pts = s.values.map((v, i) => [(i / (n - 1)) * w, h - (Math.min(v, yMax) / yMax) * (h - 4) - 2] as const);
        ctx.beginPath();
        pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)));
        const rgb = resolveRgb(s.color).split(/\s+/).join(",");
        ctx.strokeStyle = `rgba(${rgb},0.95)`;
        ctx.lineWidth = 2;
        ctx.lineJoin = "round";
        ctx.shadowColor = `rgba(${rgb},0.6)`;
        ctx.shadowBlur = 8;
        ctx.stroke();
        ctx.shadowBlur = 0;
        if (s.fill) {
          ctx.lineTo(w, h);
          ctx.lineTo(0, h);
          ctx.closePath();
          const g = ctx.createLinearGradient(0, 0, 0, h);
          g.addColorStop(0, `rgba(${rgb},0.28)`);
          g.addColorStop(1, `rgba(${rgb},0)`);
          ctx.fillStyle = g;
          ctx.fill();
        }
      }
    });
    return () => cancelAnimationFrame(frame.current);
  }, [series, max, grid, paused]);

  return <canvas ref={ref} role="img" aria-label={ariaLabel} className="block w-full" style={{ height }} />;
}

/** Tiny sparkline, used as a faint "heartbeat" behind key stats. */
export function Sparkline({ values, color, height = 36 }: { values: number[]; color: string; height?: number }) {
  return <LiveChart series={[{ values, color, fill: true }]} height={height} grid={false} ariaLabel="Trend" />;
}
