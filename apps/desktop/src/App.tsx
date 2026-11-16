import { AnimatePresence, MotionConfig, motion } from "motion/react";
import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { backend, mockBackend } from "./backend";
import { LensFilterDefs, spring, Toasts } from "./design-system";
import { applyWindowSize, isTauri } from "./lib/tauri";
import { detectGpuTier, effectiveGlass, supportsLens } from "./lib/glassQuality";
import { mediaMatches, setRootAttr, setRootVar } from "./lib/dom";
import { useSpecular } from "./lib/useSpecular";
import { useLive } from "./store/live";
import { useSettings } from "./store/settings";
import { useUi, type PageId } from "./store/ui";
import { Backdrop } from "./shell/Backdrop";
import { CommandPalette } from "./shell/CommandPalette";
import { Sidebar, SIDEBAR_W } from "./shell/Sidebar";
import { Splash } from "./shell/Splash";
import { DragStrip } from "./shell/TitleBar";
import { TopBar } from "./shell/TopBar";
import { Home } from "./pages/Home";

// Pages other than Home are split into their own chunks to keep startup light.
const pages: Record<PageId, React.LazyExoticComponent<() => React.JSX.Element> | (() => React.JSX.Element)> = {
  home: Home,
  connections: lazy(() => import("./pages/Connections").then((m) => ({ default: m.Connections }))),
  library: lazy(() => import("./pages/Library").then((m) => ({ default: m.Library }))),
  community: lazy(() => import("./pages/CommunityServers").then((m) => ({ default: m.CommunityServers }))),
  network: lazy(() => import("./pages/NetworkAnalyzer").then((m) => ({ default: m.NetworkAnalyzer }))),
  usage: lazy(() => import("./pages/UsageMonitor").then((m) => ({ default: m.UsageMonitor }))),
  boost: lazy(() => import("./pages/PcBoost").then((m) => ({ default: m.PcBoost }))),
  multi: lazy(() => import("./pages/MultiInternet").then((m) => ({ default: m.MultiInternet }))),
  shaper: lazy(() => import("./pages/TrafficShaper").then((m) => ({ default: m.TrafficShaper }))),
  settings: lazy(() => import("./pages/Settings").then((m) => ({ default: m.Settings }))),
  benchmark: lazy(() => import("./pages/BenchmarkLab").then((m) => ({ default: m.BenchmarkLab }))),
};

export default function App() {
  const s = useSettings();
  const page = useUi((x) => x.page);
  const collapsed = useUi((x) => x.sidebarCollapsed);
  const gpuTier = useLive((x) => x.gpuTier);
  // Rounded so App re-renders only when the backdrop load visibly changes, not on every sample.
  const load = useLive((x) => (x.latest ? Math.round(Math.max(x.latest.cpu, x.latest.gpu.d3)) / 100 : 0));
  const live = useLive.getState();
  const [splash, setSplash] = useState(true);
  const [focused, setFocused] = useState(() => document.hasFocus());
  const [hidden, setHidden] = useState(() => document.hidden);

  const sysReduceMotion = mediaMatches("(prefers-reduced-motion: reduce)");
  const sysReduceTransparency = mediaMatches("(prefers-reduced-transparency: reduce)");
  const reduceMotion = s.reduceMotion === "on" || (s.reduceMotion === "system" && sysReduceMotion);
  const reduceTransparency = s.reduceTransparency === "on" || (s.reduceTransparency === "system" && sysReduceTransparency);
  const gameRunning = s.simulateGameId !== null;

  const glass = effectiveGlass({
    setting: s.glassQuality,
    gameRunning,
    gpuTier,
    batterySaver: false, // reported by the Core service from Phase 2
    reduceTransparency,
  });
  // "While a game is in the foreground": a game runs and our window is not focused, or the window is hidden.
  const paused = hidden || (gameRunning && !focused);

  // One-time detection and data subscriptions
  useEffect(() => {
    live.setGpuTier(detectGpuTier());
    setRootAttr("data-lens", supportsLens() ? "on" : null);
    if (isTauri()) setRootAttr("data-tauri", "");
    void backend.getSystemInfo().then(live.setSystem);
    const onFocus = () => setFocused(true);
    const onBlur = () => setFocused(false);
    const onVis = () => setHidden(document.hidden);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    document.addEventListener("visibilitychange", onVis);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, []);

  // Metrics: 1 Hz normally; stopped entirely while paused (the Core keeps sampling at 1 Hz itself).
  useEffect(() => {
    if (paused) return;
    return backend.onMetrics(live.push, 1);
  }, [paused]);

  // Mock-only: simulated foreground game
  useEffect(() => mockBackend.simulateGame(s.simulateGameId), [s.simulateGameId]);

  // Root attributes that drive the CSS design system
  useEffect(() => {
    setRootAttr("data-glass", glass);
    live.setGlass(glass);
  }, [glass, live]);
  useEffect(() => {
    setRootAttr("data-paused", paused ? "" : null);
    live.setPaused(paused);
  }, [paused, live]);
  useEffect(() => setRootAttr("data-transparency", reduceTransparency ? "reduced" : null), [reduceTransparency]);
  useEffect(() => setRootAttr("data-motion", reduceMotion ? "reduced" : null), [reduceMotion]);
  useEffect(() => setRootAttr("data-accent", s.accent === "green" ? null : s.accent), [s.accent]);
  useEffect(() => setRootVar("--ui-scale", String(s.uiScale)), [s.uiScale]);
  useEffect(() => void applyWindowSize(s.windowSize), [s.windowSize]);

  // Data-reactive backdrop: load = max(CPU, GPU); the CSS transition on the orbs smooths it
  useEffect(() => setRootVar("--load", load.toFixed(2)), [load]);

  useSpecular(glass === "full" && !paused);

  const doneSplash = useCallback(() => setSplash(false), []);
  const sidebarW = collapsed ? SIDEBAR_W.collapsed : SIDEBAR_W.open;
  const Page = pages[page];

  return (
    <MotionConfig reducedMotion={s.reduceMotion === "on" ? "always" : s.reduceMotion === "off" ? "never" : "user"}>
      <div className="otx-window" style={{ ["--sidebar-w" as string]: `${sidebarW}px` }}>
        <LensFilterDefs />
        {glass !== "off" && <Backdrop />}
        <DragStrip />

        <main className="otx-scroll" id="main" tabIndex={-1}>
          <AnimatePresence mode="wait">
            <motion.div
              key={page}
              initial={{ opacity: 0, y: 14 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={spring}
              className="mx-auto max-w-[1480px]"
            >
              <Suspense fallback={<div className="p-10 text-t2">Loading…</div>}>
                <Page />
              </Suspense>
            </motion.div>
          </AnimatePresence>
          <footer className="mx-auto mt-10 max-w-[1480px] pb-2 text-center text-[12px] text-t2">OTX.GNG — Built and performed by Thinuda</footer>
        </main>

        <Sidebar />
        <TopBar left={sidebarW + 28} />
        <CommandPalette />
        <Toasts />
        <AnimatePresence>{splash && <Splash onDone={doneSplash} reduced={reduceMotion} />}</AnimatePresence>
      </div>
    </MotionConfig>
  );
}
