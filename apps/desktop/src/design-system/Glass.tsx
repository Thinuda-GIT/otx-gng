/** The one Glass primitive. Every surface in the app is built from it. */
import { forwardRef, type CSSProperties, type HTMLAttributes } from "react";

export type GlassVariant = "regular" | "clear" | "tinted" | "thick";

export interface GlassProps extends HTMLAttributes<HTMLDivElement> {
  variant?: GlassVariant;
  /** rgb triplet like "30 242 160" for tinted variant and shadow tint */
  tint?: string;
  radius?: number;
  /** Hero refraction lens (Full quality + Chromium only; falls back to blur) */
  lens?: boolean;
}

export const Glass = forwardRef<HTMLDivElement, GlassProps>(function Glass(
  { variant = "regular", tint, radius, lens, className = "", style, children, ...rest },
  ref,
) {
  const cls = [
    "glass",
    variant !== "regular" ? `glass--${variant}` : "",
    lens ? "glass--lens" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");
  const st: CSSProperties & Record<string, string | number | undefined> = {
    ...style,
    ...(tint ? { "--tint": tint, "--shadow-tint": tint } : {}),
    ...(radius !== undefined ? { "--radius": `${radius}px` } : {}),
  };
  return (
    <div ref={ref} className={cls} style={st} {...rest}>
      {children}
    </div>
  );
});
