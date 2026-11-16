/** iOS-style toggle: the thumb is a glass lens that stretches while pressed; neon-green glow when on. */
import { motion } from "motion/react";
import { useState } from "react";
import { spring } from "./motion";

export interface GlassToggleProps {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
  loading?: boolean;
  label: string; // accessible name (visually hidden when the toggle sits next to its own text)
  id?: string;
}

export function GlassToggle({ checked, onChange, disabled, loading, label, id }: GlassToggleProps) {
  const [pressed, setPressed] = useState(false);
  const off = disabled || loading;
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      aria-busy={loading || undefined}
      disabled={off}
      onClick={() => onChange(!checked)}
      onPointerDown={() => setPressed(true)}
      onPointerUp={() => setPressed(false)}
      onPointerLeave={() => setPressed(false)}
      onKeyDown={(e) => (e.key === " " || e.key === "Enter") && setPressed(true)}
      onKeyUp={() => setPressed(false)}
      className={[
        "relative inline-flex h-[30px] w-[52px] shrink-0 items-center rounded-full p-[3px] transition-[background,box-shadow] duration-300",
        off ? "cursor-not-allowed opacity-45" : "cursor-pointer",
      ].join(" ")}
      style={{
        background: checked ? "rgb(var(--rgb-green) / 0.85)" : "rgb(255 255 255 / 0.12)",
        boxShadow: checked
          ? "inset 0 1px 0 rgb(255 255 255 / .35), 0 0 18px -2px rgb(var(--rgb-green) / .65)"
          : "inset 0 1px 2px rgb(0 0 0 / .4), inset 0 0 0 1px rgb(255 255 255 / .1)",
      }}
    >
      <motion.span
        aria-hidden
        className="glass glass--lens block h-6 rounded-full"
        style={{
          ["--radius" as string]: "999px",
          ["--glass-fill-top" as string]: "rgb(255 255 255 / .95)",
          ["--glass-fill-bottom" as string]: "rgb(235 240 255 / .85)",
          boxShadow: "0 2px 6px rgb(0 0 0 / .35), inset 0 1px 0 #fff",
        }}
        initial={false}
        animate={{ x: checked ? (pressed ? 16 : 22) : 0, width: pressed ? 30 : 24 }}
        transition={spring}
      />
    </button>
  );
}
