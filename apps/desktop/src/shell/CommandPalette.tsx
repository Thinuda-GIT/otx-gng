/** Ctrl+K command palette: jump to any page or run a quick action. */
import { AnimatePresence, motion } from "motion/react";
import { CornerDownLeft, Eraser, Gauge, Rocket, RotateCcw, Search, Sparkles, Wifi } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { spring } from "../design-system";
import { cleanRamNow, restoreInternet, revertEverything } from "../lib/actions";
import { rank } from "../lib/search";
import { useSettings } from "../store/settings";
import { useUi, type PageId } from "../store/ui";
import { EXTRA_PAGES, NAV } from "./nav";

interface Cmd {
  id: string;
  label: string;
  group: "pages" | "actions";
  icon: ReactNode;
  keywords?: string[];
  run: () => void;
}

const PAGE_KEYWORDS: Partial<Record<PageId, string[]>> = {
  boost: ["tweaks", "ram", "dns", "cleaner"],
  usage: ["cpu", "gpu", "processes", "task manager"],
  network: ["traffic", "ping", "bufferbloat", "tune-up"],
  shaper: ["focus", "limit", "block", "bandwidth"],
  connections: ["route", "traceroute", "relay", "server"],
  settings: ["preferences", "glass", "language"],
  benchmark: ["fps", "frametime", "a/b"],
};

export function CommandPalette() {
  const { t } = useTranslation();
  const open = useUi((s) => s.paletteOpen);
  const setOpen = useUi((s) => s.setPalette);
  const go = useUi((s) => s.go);
  const setSetting = useSettings((s) => s.set);
  const [q, setQ] = useState("");
  const [idx, setIdx] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  const cmds = useMemo<Cmd[]>(() => {
    const pages: Cmd[] = [...NAV, ...EXTRA_PAGES].map((n) => {
      const Icon = n.icon;
      return { id: `page-${n.id}`, label: t(`nav.${n.id}`), group: "pages", icon: <Icon size={17} />, keywords: PAGE_KEYWORDS[n.id], run: () => go(n.id) };
    });
    const actions: Cmd[] = [
      { id: "a-boost", label: t("palette.boost"), group: "actions", icon: <Rocket size={17} />, keywords: ["game mode"], run: () => go("home") },
      { id: "a-ram", label: t("palette.cleanRam"), group: "actions", icon: <Eraser size={17} />, keywords: ["memory"], run: () => void cleanRamNow() },
      { id: "a-net", label: t("palette.restoreInternet"), group: "actions", icon: <Wifi size={17} />, keywords: ["panic", "unblock"], run: () => void restoreInternet() },
      { id: "a-revert", label: t("palette.revertAll"), group: "actions", icon: <RotateCcw size={17} />, keywords: ["undo", "defaults"], run: () => void revertEverything() },
      { id: "a-focus", label: t("palette.focus"), group: "actions", icon: <Gauge size={17} />, run: () => go("shaper") },
      { id: "a-glass-off", label: t("palette.glassOff"), group: "actions", icon: <Sparkles size={17} />, run: () => setSetting("glassQuality", "off") },
      { id: "a-glass-auto", label: t("palette.glassAuto"), group: "actions", icon: <Sparkles size={17} />, run: () => setSetting("glassQuality", "auto") },
    ];
    return [...pages, ...actions];
  }, [t, go, setSetting]);

  const results = useMemo(() => rank(cmds, q), [cmds, q]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen(!useUi.getState().paletteOpen);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setOpen]);

  useEffect(() => {
    if (open) {
      setQ("");
      setIdx(0);
      setTimeout(() => input.current?.focus(), 20);
    }
  }, [open]);

  useEffect(() => setIdx(0), [q]);

  const run = (c: Cmd | undefined) => {
    if (!c) return;
    setOpen(false);
    c.run();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIdx((i) => Math.min(results.length - 1, i + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIdx((i) => Math.max(0, i - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(results[idx]);
    } else if (e.key === "Escape") {
      setOpen(false);
    }
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div className="absolute inset-0 z-[85] flex justify-center pt-[12vh]" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
          <div className="absolute inset-0" style={{ background: "rgb(3 4 8 / .5)", backdropFilter: "blur(4px)" }} onClick={() => setOpen(false)} aria-hidden />
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label="Command palette"
            className="glass glass--thick relative h-fit w-[620px] max-w-[90%] overflow-hidden"
            style={{ ["--radius" as string]: "22px" }}
            initial={{ y: -16, scale: 0.97 }}
            animate={{ y: 0, scale: 1 }}
            exit={{ y: -10, scale: 0.98 }}
            transition={spring}
          >
            <div className="flex items-center gap-3 border-b border-white/[0.08] px-5">
              <Search size={18} className="text-t2" aria-hidden />
              <input
                ref={input}
                type="search"
                value={q}
                onChange={(e) => setQ(e.target.value)}
                onKeyDown={onKeyDown}
                placeholder={t("palette.placeholder")}
                aria-label={t("palette.placeholder")}
                aria-controls="palette-list"
                aria-activedescendant={results[idx] ? `cmd-${results[idx]!.id}` : undefined}
                className="h-14 flex-1 bg-transparent text-base text-t1 outline-none placeholder:text-t3"
              />
              <span className="kbd">Esc</span>
            </div>
            <ul id="palette-list" role="listbox" className="max-h-[360px] overflow-y-auto p-2">
              {results.length === 0 && <li className="p-4 text-t2">{t("palette.empty")}</li>}
              {results.map((c, i) => (
                <li
                  key={c.id}
                  id={`cmd-${c.id}`}
                  role="option"
                  aria-selected={i === idx}
                  onMouseEnter={() => setIdx(i)}
                  onClick={() => run(c)}
                  className={`flex h-11 cursor-pointer items-center gap-3 rounded-[12px] px-3 ${i === idx ? "bg-white/[0.09] text-t1" : "text-t2"}`}
                >
                  <span className="text-t2">{c.icon}</span>
                  <span className="flex-1">{c.label}</span>
                  <span className="text-[12px] text-t2">{c.group === "pages" ? t("palette.pages") : t("palette.actions")}</span>
                  {i === idx && <CornerDownLeft size={14} aria-hidden />}
                </li>
              ))}
            </ul>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
