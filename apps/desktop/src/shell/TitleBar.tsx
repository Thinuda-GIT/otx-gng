/** Frameless-window chrome. A thin drag strip runs along the top edge (above the floating bars),
 *  and the window controls sit at the right end of the floating top bar. Both the top bar
 *  background and the sidebar logo are drag regions too. */
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { closeWindow, isMaximized, isTauri, minimizeWindow, onResized, toggleMaximizeWindow } from "../lib/tauri";

export function DragStrip() {
  return <div className="absolute inset-x-0 top-0 z-[40] h-4" data-tauri-drag-region aria-hidden />;
}

export function WindowControls() {
  const { t } = useTranslation();
  const [max, setMax] = useState(false);

  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    const sync = () =>
      void isMaximized().then((m) => {
        if (!alive) return;
        setMax(m);
        if (m) document.documentElement.setAttribute("data-maximized", "");
        else document.documentElement.removeAttribute("data-maximized");
      });
    sync();
    void onResized(sync).then((u) => {
      if (alive) off = u;
      else u();
    });
    return () => {
      alive = false;
      off?.();
    };
  }, []);

  if (!isTauri()) return null;
  const btn = "inline-flex h-10 w-10 items-center justify-center rounded-[12px] text-t2 transition-colors hover:bg-white/10 hover:text-t1";
  return (
    <div className="ml-1 flex items-center gap-0.5 border-l border-white/[0.08] pl-2">
      <button type="button" className={btn} aria-label={t("window.minimize")} onClick={() => void minimizeWindow()}>
        <Minus size={17} aria-hidden />
      </button>
      <button type="button" className={btn} aria-label={max ? t("window.restore") : t("window.maximize")} onClick={() => void toggleMaximizeWindow().then(setMax)}>
        {max ? <Copy size={14} aria-hidden /> : <Square size={14} aria-hidden />}
      </button>
      <button type="button" className={`${btn} hover:!bg-[#e81123] hover:!text-white`} aria-label={t("window.close")} onClick={() => void closeWindow()}>
        <X size={17} aria-hidden />
      </button>
    </div>
  );
}
