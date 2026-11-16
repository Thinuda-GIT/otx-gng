/** Segmented control: a sliding glass lens morphs between options. Arrow keys move the selection. */
import { motion } from "motion/react";
import { useId, useRef, type ReactNode } from "react";
import { spring } from "./motion";

export interface SegOption<T extends string> {
  value: T;
  label: string;
  icon?: ReactNode;
  disabled?: boolean;
}

export interface GlassSegmentedProps<T extends string> {
  options: SegOption<T>[];
  value: T;
  onChange: (v: T) => void;
  label: string;
  size?: "sm" | "md";
  /** "tabs" gives tab semantics (for page sub-navigation), "radio" for settings choices */
  role?: "tabs" | "radio";
}

export function GlassSegmented<T extends string>({
  options,
  value,
  onChange,
  label,
  size = "md",
  role = "radio",
}: GlassSegmentedProps<T>) {
  const lensId = useId();
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const enabled = options.filter((o) => !o.disabled);

  const onKey = (e: React.KeyboardEvent, idx: number) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const cur = enabled.findIndex((o) => o.value === options[idx]!.value);
    const next = enabled[(cur + (e.key === "ArrowRight" ? 1 : enabled.length - 1)) % enabled.length]!;
    onChange(next.value);
    refs.current[options.indexOf(next)]?.focus();
  };

  return (
    <div
      role={role === "tabs" ? "tablist" : "radiogroup"}
      aria-label={label}
      className="glass glass--clear inline-flex gap-1 p-1"
      style={{ ["--radius" as string]: "16px" }}
    >
      {options.map((o, i) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            ref={(el) => {
              refs.current[i] = el;
            }}
            type="button"
            role={role === "tabs" ? "tab" : "radio"}
            aria-selected={role === "tabs" ? active : undefined}
            aria-checked={role === "radio" ? active : undefined}
            tabIndex={active ? 0 : -1}
            disabled={o.disabled}
            onClick={() => onChange(o.value)}
            onKeyDown={(e) => onKey(e, i)}
            className={[
              "relative flex items-center gap-2 rounded-[12px] font-semibold transition-colors",
              size === "sm" ? "h-8 px-3 text-[13px]" : "h-10 px-4 text-sm",
              active ? "text-t1" : "text-t2 hover:text-t1",
              o.disabled ? "cursor-not-allowed opacity-40" : "cursor-pointer",
            ].join(" ")}
          >
            {active && (
              <motion.span
                layoutId={lensId}
                transition={spring}
                className="glass glass--tinted glass--lens absolute inset-0"
                style={{ ["--radius" as string]: "12px" }}
                aria-hidden
              />
            )}
            <span className="relative z-[1] flex items-center gap-2">
              {o.icon}
              {o.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
