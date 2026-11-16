/** Page-level building blocks shared by all pages. */
import type { ReactNode } from "react";
import { Glass, GlassToggle, GlassTooltip, TierBadge } from "../design-system";

export function PageHeader({ title, subtitle, tier, actions }: { title: string; subtitle?: string; tier?: 0 | 1 | 2; actions?: ReactNode }) {
  return (
    <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0">
        <div className="flex items-center gap-3">
          <h1 className="text-[34px] leading-none text-t1">{title}</h1>
          {tier !== undefined && <TierBadge tier={tier} />}
        </div>
        {subtitle && <p className="mt-2 max-w-3xl text-t2">{subtitle}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

export function Panel({
  title,
  help,
  right,
  children,
  className = "",
  pad = true,
  tint,
}: {
  title?: ReactNode;
  help?: string;
  right?: ReactNode;
  children: ReactNode;
  className?: string;
  pad?: boolean;
  tint?: string;
}) {
  return (
    <Glass className={`${pad ? "p-5" : ""} ${className}`} tint={tint}>
      {(title || right) && (
        <div className={`mb-4 flex items-center justify-between gap-3 ${pad ? "" : "px-5 pt-5"}`}>
          <div className="flex items-center gap-2">
            {title && <h2 className="otx-section-title">{title}</h2>}
            {help && <GlassTooltip text={help} />}
          </div>
          {right}
        </div>
      )}
      {children}
    </Glass>
  );
}

export function Stat({ label, value, sub, color }: { label: string; value: ReactNode; sub?: ReactNode; color?: string }) {
  return (
    <div className="min-w-0">
      <div className="text-[13px] text-t2">{label}</div>
      <div className="num mt-0.5 whitespace-nowrap text-xl font-semibold" style={color ? { color } : undefined}>
        {value}
      </div>
      {sub && <div className="mt-0.5 text-[12px] text-t2">{sub}</div>}
    </div>
  );
}

/** A settings-style row: label + description on the left, control on the right. */
export function Row({
  label,
  hint,
  help,
  badge,
  children,
  disabled,
}: {
  label: string;
  hint?: string;
  help?: string;
  badge?: ReactNode;
  children: ReactNode;
  disabled?: boolean;
}) {
  return (
    <div className={`flex items-center justify-between gap-6 py-3.5 ${disabled ? "opacity-60" : ""}`}>
      <div className="min-w-0">
        <div className="flex flex-wrap items-center gap-2 font-medium text-t1">
          {label}
          {help && <GlassTooltip text={help} />}
          {badge}
        </div>
        {hint && <div className="mt-0.5 text-[13px] text-t2">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function ToggleRow(props: {
  label: string;
  hint?: string;
  help?: string;
  badge?: ReactNode;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <Row label={props.label} hint={props.hint} help={props.help} badge={props.badge} disabled={props.disabled}>
      <GlassToggle label={props.label} checked={props.checked} onChange={props.onChange} disabled={props.disabled} />
    </Row>
  );
}

export function Divider() {
  return <div className="h-px bg-white/[0.07]" />;
}

export function Empty({ children }: { children: ReactNode }) {
  return <div className="rounded-[16px] border border-dashed border-white/15 p-6 text-center text-t2">{children}</div>;
}

/** Monogram tile for games: we never bundle game logos or art. The real app extracts the icon
 *  locally from the installed game's exe; the mock uses the game's tint. */
export function GameTile({ name, tint, size = 56 }: { name: string; tint: string; size?: number }) {
  const letters = name
    .split(/\s+/)
    .filter((w) => /^[A-Za-z0-9]/.test(w))
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join("");
  return (
    <div
      className="glass glass--tinted flex shrink-0 items-center justify-center font-head font-bold text-white"
      style={{ width: size, height: size, ["--tint" as string]: tint, ["--radius" as string]: `${Math.round(size / 3.5)}px`, fontSize: size * 0.38 }}
      aria-hidden
    >
      {letters}
    </div>
  );
}
