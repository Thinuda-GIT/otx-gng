/** Pointer-following specular sheen. One window listener; positions are written as CSS variables
 *  on the hovered .glass element inside requestAnimationFrame (style writes only, no layout thrash
 *  beyond a single rect read per frame). Disabled unless glass is Full. */
import { useEffect } from "react";

export function useSpecular(enabled: boolean): void {
  useEffect(() => {
    if (!enabled) return;
    let raf = 0;
    let last: PointerEvent | null = null;
    let current: HTMLElement | null = null;

    const frame = () => {
      raf = 0;
      if (!last) return;
      const el = (last.target as Element | null)?.closest?.(".glass") as HTMLElement | null;
      if (current && current !== el) {
        current.style.removeProperty("--mx");
        current.style.removeProperty("--my");
        current.style.removeProperty("--sheen");
      }
      current = el;
      if (!el) return;
      const r = el.getBoundingClientRect();
      el.style.setProperty("--mx", `${last.clientX - r.left}px`);
      el.style.setProperty("--my", `${last.clientY - r.top}px`);
      el.style.setProperty("--sheen", "0.14");
    };

    const onMove = (e: PointerEvent) => {
      last = e;
      if (!raf) raf = requestAnimationFrame(frame);
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    return () => {
      window.removeEventListener("pointermove", onMove);
      cancelAnimationFrame(raf);
      current?.style.removeProperty("--mx");
      current?.style.removeProperty("--my");
      current?.style.removeProperty("--sheen");
    };
  }, [enabled]);
}
