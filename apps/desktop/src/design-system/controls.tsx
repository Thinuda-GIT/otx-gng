/** Smaller primitives: Slider, Select, Tooltip ("?" bubble), Badge, Chip, and the SVG lens filter. */
import { useId, useState, type ReactNode } from "react";
import { HelpCircle, Sparkle } from "lucide-react";

/* ---------- GlassSlider ---------- */
export interface GlassSliderProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
  label: string;
  format?: (v: number) => string;
  disabled?: boolean;
}

export function GlassSlider({ value, min, max, step = 1, onChange, label, format, disabled }: GlassSliderProps) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="flex items-center gap-3">
      <input
        type="range"
        className="otx-range"
        aria-label={label}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ ["--pct" as string]: `${pct}%` }}
      />
      <span className="num w-16 text-right text-t2">{format ? format(value) : value}</span>
    </div>
  );
}

/* ---------- GlassSelect (native select for full keyboard + screen reader support) ---------- */
export interface GlassSelectProps<T extends string> {
  value: T;
  options: { value: T; label: string; disabled?: boolean }[];
  onChange: (v: T) => void;
  label: string;
  disabled?: boolean;
  className?: string;
}

export function GlassSelect<T extends string>({ value, options, onChange, label, disabled, className = "" }: GlassSelectProps<T>) {
  return (
    <div className={`glass relative inline-flex ${className}`} style={{ ["--radius" as string]: "14px" }}>
      <select
        aria-label={label}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value as T)}
        className="otx-select h-10 w-full cursor-pointer appearance-none bg-transparent pl-4 pr-10 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-45"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value} disabled={o.disabled}>
            {o.label}
          </option>
        ))}
      </select>
      <span aria-hidden className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-t2">
        ▾
      </span>
    </div>
  );
}

/* ---------- GlassTooltip: the "?" help bubble ---------- */
export function GlassTooltip({ text, children }: { text: string; children?: ReactNode }) {
  const id = useId();
  const [open, setOpen] = useState(false);
  return (
    <span className="relative inline-flex" onMouseEnter={() => setOpen(true)} onMouseLeave={() => setOpen(false)}>
      {children ?? (
        <button
          type="button"
          aria-describedby={open ? id : undefined}
          aria-label="More info"
          onFocus={() => setOpen(true)}
          onBlur={() => setOpen(false)}
          onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
          className="inline-flex h-5 w-5 items-center justify-center rounded-full text-t2 hover:text-t1"
        >
          <HelpCircle size={15} aria-hidden />
        </button>
      )}
      {open && (
        <span
          role="tooltip"
          id={id}
          className="glass glass--thick absolute bottom-[calc(100%+8px)] left-1/2 z-50 w-64 -translate-x-1/2 px-3 py-2 text-[13px] font-normal leading-snug text-t1"
          style={{ ["--radius" as string]: "12px" }}
        >
          {text}
        </span>
      )}
    </span>
  );
}

/* ---------- GlassBadge ---------- */
export type BadgeTone = "beta" | "recommended" | "new" | "ok" | "warn" | "danger" | "info" | "neutral" | "violet";

const BADGE: Record<BadgeTone, { rgb: string; text: string }> = {
  beta: { rgb: "70 140 255", text: "#9cc2ff" },
  recommended: { rgb: "200 205 220", text: "rgb(255 255 255 / .78)" },
  new: { rgb: "var(--rgb-green)", text: "var(--accent-green)" },
  ok: { rgb: "var(--rgb-green)", text: "var(--accent-green)" },
  warn: { rgb: "var(--rgb-amber)", text: "var(--accent-amber)" },
  danger: { rgb: "var(--rgb-red)", text: "#ff8a9b" },
  info: { rgb: "var(--rgb-cyan)", text: "var(--accent-cyan)" },
  violet: { rgb: "var(--rgb-violet)", text: "#b7a5ff" },
  neutral: { rgb: "255 255 255", text: "rgb(255 255 255 / .7)" },
};

export function GlassBadge({ tone = "neutral", children, title }: { tone?: BadgeTone; children: ReactNode; title?: string }) {
  const b = BADGE[tone];
  return (
    <span
      title={title}
      className="inline-flex h-[22px] shrink-0 items-center gap-1 whitespace-nowrap rounded-full px-2 text-[12px] font-semibold"
      style={{
        color: b.text,
        background: `rgb(${b.rgb} / 0.14)`,
        boxShadow: `inset 0 0 0 1px rgb(${b.rgb} / 0.28)`,
      }}
    >
      {tone === "new" && <Sparkle size={11} aria-hidden />}
      {children}
    </span>
  );
}

/* ---------- Tier badge (every feature is labeled by tier) ---------- */
export function TierBadge({ tier }: { tier: 0 | 1 | 2 }) {
  const map = {
    0: { tone: "ok" as const, text: "Tier 0 · Local", title: "Runs entirely on your PC. Costs nothing to operate." },
    1: { tone: "info" as const, text: "Tier 1 · Community lists", title: "Uses signed static lists published on GitHub; tested from your PC." },
    2: { tone: "violet" as const, text: "Tier 2 · Needs a relay", title: "Needs a community-run OTX Relay. Optional." },
  };
  const m = map[tier];
  return (
    <GlassBadge tone={m.tone} title={m.title}>
      {m.text}
    </GlassBadge>
  );
}

/* ---------- SVG lens filter used by backdrop-filter: url(#otx-lens) ---------- */
export function LensFilterDefs() {
  return (
    <svg width="0" height="0" aria-hidden style={{ position: "absolute" }}>
      <filter id="otx-lens" x="0" y="0" width="100%" height="100%" colorInterpolationFilters="sRGB">
        <feTurbulence type="fractalNoise" baseFrequency="0.008 0.012" numOctaves="1" seed="7" result="noise" />
        <feGaussianBlur in="noise" stdDeviation="2" result="soft" />
        <feDisplacementMap in="SourceGraphic" in2="soft" scale="18" xChannelSelector="R" yChannelSelector="G" />
      </filter>
    </svg>
  );
}
