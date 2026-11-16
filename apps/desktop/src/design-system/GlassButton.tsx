import { motion, type HTMLMotionProps } from "motion/react";
import { Loader2 } from "lucide-react";
import type { ReactNode } from "react";
import { pressScale, spring } from "./motion";

export type ButtonKind = "primary" | "secondary" | "ghost" | "destructive";

export interface GlassButtonProps extends Omit<HTMLMotionProps<"button">, "children"> {
  kind?: ButtonKind;
  size?: "sm" | "md" | "lg";
  loading?: boolean;
  icon?: ReactNode;
  children?: ReactNode;
}

const TINT: Record<ButtonKind, string | null> = {
  primary: "var(--rgb-accent)",
  destructive: "var(--rgb-red)",
  secondary: null,
  ghost: null,
};

const SIZE = {
  sm: "h-8 px-3 text-[13px] gap-1.5",
  md: "h-10 px-4 text-sm gap-2",
  lg: "h-12 px-6 text-base gap-2.5",
};

export function GlassButton({
  kind = "secondary",
  size = "md",
  loading = false,
  disabled,
  icon,
  children,
  className = "",
  style,
  ...rest
}: GlassButtonProps) {
  const tint = TINT[kind];
  const isGlass = kind !== "ghost";
  const cls = [
    "relative inline-flex select-none items-center justify-center whitespace-nowrap font-semibold",
    "transition-[filter,opacity] duration-150",
    isGlass ? "glass" : "rounded-[14px] hover:bg-white/[0.07]",
    tint ? "glass--tinted" : "",
    SIZE[size],
    disabled || loading ? "cursor-not-allowed opacity-45" : "cursor-pointer hover:brightness-125",
    className,
  ].join(" ");
  return (
    <motion.button
      type="button"
      className={cls}
      style={{ ...(tint ? { ["--tint" as string]: tint } : {}), ["--radius" as string]: "14px", ...style }}
      whileTap={disabled || loading ? undefined : { scale: pressScale }}
      transition={spring}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? <Loader2 size={16} className="spin-slow" aria-hidden /> : icon}
      {children}
    </motion.button>
  );
}
