/** SVG ring gauge with glow. `value` is 0-100. The number ticks smoothly via motion. */
import { animate, motion, useMotionValue, useTransform } from "motion/react";
import { useEffect, useId, type ReactNode } from "react";

export interface GlassGaugeProps {
  value: number;
  size?: number;
  stroke?: number;
  color?: string; // rgb triplet or css var
  label?: string;
  sublabel?: ReactNode;
  display?: string; // overrides the number shown in the middle
}

export function GlassGauge({ value, size = 168, stroke = 12, color = "var(--rgb-green)", label, sublabel, display }: GlassGaugeProps) {
  const gid = useId().replace(/:/g, "");
  const r = (size - stroke) / 2 - 6;
  const c = 2 * Math.PI * r;
  const mv = useMotionValue(0);
  const dash = useTransform(mv, (v) => `${(Math.max(0, Math.min(100, v)) / 100) * c} ${c}`);
  const text = useTransform(mv, (v) => Math.round(v).toString());

  useEffect(() => {
    const ctl = animate(mv, value, { type: "spring", stiffness: 120, damping: 22 });
    return () => ctl.stop();
  }, [value, mv]);

  return (
    <div className="relative inline-flex items-center justify-center" style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden>
        <defs>
          <filter id={`glow-${gid}`} x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="4" result="b" />
            <feMerge>
              <feMergeNode in="b" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="rgb(255 255 255 / 0.08)" strokeWidth={stroke} />
        <motion.circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke={`rgb(${color})`}
          strokeWidth={stroke}
          strokeLinecap="round"
          style={{ strokeDasharray: dash }}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
          filter={`url(#glow-${gid})`}
        />
      </svg>
      <div className="absolute inset-0 flex flex-col items-center justify-center text-center" role="img" aria-label={`${label ?? "Value"}: ${display ?? Math.round(value)}`}>
        {display ? (
          <span className="num text-2xl font-semibold text-t1">{display}</span>
        ) : (
          <motion.span className="num text-4xl font-semibold text-t1">{text}</motion.span>
        )}
        {label && <span className="mt-0.5 text-[13px] text-t2">{label}</span>}
        {sublabel && <span className="mt-1">{sublabel}</span>}
      </div>
    </div>
  );
}
