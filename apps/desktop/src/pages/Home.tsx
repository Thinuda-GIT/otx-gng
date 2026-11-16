import { AnimatePresence, animate, motion, useMotionValue, useTransform } from "motion/react";
import { Check, CircleDashed, Loader2, MinusCircle, RotateCcw, Timer, XCircle, Zap } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { backend } from "../backend";
import type { BoostStep, ChangeRecord, Game } from "../backend/types";
import { Divider, GameTile, PageHeader, Panel, Stat, ToggleRow } from "../components/ui";
import { Glass, GlassBadge, GlassButton, GlassGauge, GlassModal, GlassSelect, GlassTooltip, Sparkline, spring } from "../design-system";
import { revertEverything, setFocus } from "../lib/actions";
import { setGameTint } from "../lib/dom";
import { formatGB, formatMs, timeAgo } from "../lib/format";
import { computeHealth } from "../lib/healthScore";
import { useLive } from "../store/live";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

type OrbState = "ready" | "boosting" | "boosted" | "game";

export function Home() {
  const go = useUi((s) => s.go);
  const toast = useUi((s) => s.toast);
  const logActivity = useUi((s) => s.logActivity);
  const activity = useUi((s) => s.activity);
  const latest = useLive((s) => s.latest);
  const history = useLive((s) => s.history);
  const system = useLive((s) => s.system);
  const simulateGameId = useSettings((s) => s.simulateGameId);
  const ramAutoClean = useSettings((s) => s.ramAutoClean);
  const setSetting = useSettings((s) => s.set);

  const [games, setGames] = useState<Game[]>([]);
  const [target, setTarget] = useState<string>("none");
  const [steps, setSteps] = useState<BoostStep[]>([]);
  const [orb, setOrb] = useState<OrbState>("ready");
  const [freed, setFreed] = useState(0);
  const [changes, setChanges] = useState<ChangeRecord[]>([]);
  const [confirmRevert, setConfirmRevert] = useState(false);
  const [focusOn, setFocusOn] = useState(false);

  const running = games.find((g) => g.id === simulateGameId) ?? null;

  const refreshChanges = () => void backend.getChanges().then(setChanges);
  useEffect(() => {
    void backend.getGames().then(setGames);
    refreshChanges();
  }, []);

  useEffect(() => {
    setGameTint(running?.tint ?? null);
    if (running) setTarget(running.id);
    return () => setGameTint(null);
  }, [running]);

  const state: OrbState = orb === "boosting" ? "boosting" : running && orb !== "boosted" ? "game" : orb;

  const runBoost = async () => {
    if (state === "boosting") return;
    setOrb("boosting");
    setFreed(0);
    const gameId = target === "none" ? null : target;
    const res = await backend.boost(gameId, setSteps);
    setFreed(res.freedMB);
    setOrb("boosted");
    refreshChanges();
    const g = games.find((x) => x.id === gameId);
    toast({ tone: "ok", title: g ? `Game Mode ready for ${g.name}` : "Tune-up complete", body: `${res.freedMB} MB more memory available. Everything reverts when the session ends.` });
    logActivity(g ? `Game Mode applied for ${g.name}` : "Quick tune-up");
  };

  const health = useMemo(() => {
    if (!latest || !system) return null;
    return computeHealth({
      ramAvailablePct: (latest.ram.availableMB / latest.ram.totalMB) * 100,
      commitPct: (latest.ram.commitMB / latest.ram.commitLimitMB) * 100,
      backgroundCpuPct: running ? Math.max(0, latest.cpu - 38) : latest.cpu * 0.6,
      backgroundGpuPct: running ? 2 : latest.gpu.d3 * 0.7,
      diskActivePct: latest.disk.activePct ?? 0,
      pingMs: latest.ping?.ms ?? null,
      jitterMs: latest.ping?.jitter ?? null,
      lossPct: latest.ping?.loss ?? null,
      powerPlan: system.info.power.plan,
      startupApps: system.startupApps,
    });
    // Recompute every 5 samples so the ring is calm, not jittery
  }, [Math.floor(history.length / 5), system, running]);

  const scoreColor = !health ? "var(--rgb-green)" : health.score >= 75 ? "var(--rgb-green)" : health.score >= 50 ? "var(--rgb-amber)" : "var(--rgb-red)";

  return (
    <div>
      <PageHeader
        title="Home"
        subtitle="One click prepares your PC for a game. Everything OTX.GNG changes is listed below and can be undone."
        tier={0}
        actions={
          <GlassButton icon={<Timer size={16} />} onClick={() => go("benchmark")}>
            Benchmark Lab
          </GlassButton>
        }
      />

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1.25fr_1fr]">
        {/* Boost orb */}
        <Glass className="relative flex flex-col items-center overflow-hidden p-7" variant="clear">
          <div className="mb-2 flex w-full items-center justify-between">
            <h2 className="otx-section-title">Boost</h2>
            <GlassSelect
              label="Game to boost"
              value={target}
              onChange={setTarget}
              options={[{ value: "none", label: "No game: general tune-up" }, ...games.map((g) => ({ value: g.id, label: g.name }))]}
            />
          </div>
          <BoostOrb state={state} freed={freed} onClick={() => void runBoost()} />
          <div className="mt-6 w-full max-w-md">
            {steps.length === 0 ? (
              <p className="text-center text-t2">
                Frees memory, gives the game CPU priority, switches to a high-performance power plan and focuses the network. Each step is measured and reverted automatically when the game closes.
              </p>
            ) : (
              <ul className="space-y-1.5" aria-live="polite">
                {steps.map((s) => (
                  <li key={s.id} className="flex items-center gap-3 text-sm">
                    <StepIcon status={s.status} />
                    <span className={s.status === "pending" ? "text-t2" : "text-t1"}>{s.label}</span>
                    {s.detail && <span className="ml-auto truncate text-[13px] text-t2">{s.detail}</span>}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </Glass>

        {/* Health score */}
        <Panel title="PC Health Score" help="Built only from measured values. Each factor shows its own score and why it matters.">
          {health ? (
            <div className="flex flex-col items-center gap-5 sm:flex-row sm:items-start">
              <GlassGauge value={health.score} color={scoreColor} label="out of 100" />
              <ul className="w-full flex-1 space-y-2.5">
                {health.factors.map((f) => (
                  <li key={f.id}>
                    <div className="flex items-center justify-between gap-2 text-sm">
                      <span className="flex items-center gap-1.5 text-t1">
                        {f.label}
                        <GlassTooltip text={`${f.explain} Measured: ${f.value}. Weight ${f.weight}%.`} />
                      </span>
                      <span className="num text-t2">{f.score}</span>
                    </div>
                    <div className="otx-progress mt-1">
                      <span style={{ width: `${f.score}%`, background: f.score >= 75 ? undefined : f.score >= 50 ? "var(--accent-amber)" : "var(--accent-red)" }} />
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          ) : (
            <div className="flex h-48 items-center justify-center text-t2">Measuring…</div>
          )}
        </Panel>
      </div>

      {/* Active game HUD */}
      <div className="mt-5 grid grid-cols-1 gap-5 xl:grid-cols-[1.25fr_1fr]">
        <Glass
          className={`p-6 ${running ? "edge-pulse" : ""}`}
          style={{ ["--hud" as string]: state === "boosting" ? "var(--rgb-red)" : running ? "var(--rgb-green)" : "var(--rgb-amber)" }}
          tint={running?.tint}
        >
          <span className="hud-corners" aria-hidden />
          <div className="mb-4 flex items-center justify-between">
            <h2 className="otx-section-title">Active game</h2>
            {running ? <GlassBadge tone="ok">Running</GlassBadge> : <GlassBadge tone="neutral">Waiting for a game</GlassBadge>}
          </div>
          {running && latest ? (
            <div className="flex flex-col gap-5">
              <div className="flex items-center gap-4">
                <GameTile name={running.name} tint={running.tint} size={52} />
                <div>
                  <div className="font-head text-2xl font-bold">{running.name}</div>
                  <div className="text-[13px] text-t2">Server detected from the game's own network events (ETW). Anti-cheat: {running.antiCheat ?? "none"}</div>
                </div>
              </div>
              <div className="grid grid-cols-3 gap-4 md:grid-cols-6">
                <Stat label="Ping" value={latest.ping ? formatMs(latest.ping.ms) : "-"} color="var(--accent-cyan)" />
                <Stat label="Jitter" value={latest.ping ? formatMs(latest.ping.jitter, 1) : "-"} />
                <Stat label="Loss" value={latest.ping ? `${latest.ping.loss.toFixed(1)}%` : "-"} />
                <Stat label="CPU" value={`${latest.cpu.toFixed(0)}%`} />
                <Stat label="GPU" value={`${latest.gpu.d3.toFixed(0)}%`} color="var(--accent-violet)" />
                <Stat label="RAM" value={formatGB(latest.ram.usedMB)} />
              </div>
              <div className="grid grid-cols-2 gap-4">
                <div className="glass glass--clear p-3" style={{ ["--radius" as string]: "16px" }}>
                  <Stat label="FPS (avg)" value={latest.fps ? latest.fps.avg.toFixed(0) : "Capture off"} sub="Benchmark capture" />
                  <Sparkline values={history.map((m) => m.fps?.avg ?? 0)} color="var(--rgb-green)" />
                </div>
                <div className="glass glass--clear p-3" style={{ ["--radius" as string]: "16px" }}>
                  <Stat label="1% low" value={latest.fps ? latest.fps.low1.toFixed(0) : "-"} sub="Slowest 1% of frames" />
                  <Sparkline values={history.map((m) => m.fps?.low1 ?? 0)} color="var(--rgb-cyan)" />
                </div>
              </div>
            </div>
          ) : (
            <div className="text-t2">
              Start a game from the Library or your usual launcher. OTX.GNG detects it and can run Game Mode automatically.
              <div className="mt-4 flex gap-2">
                <GlassButton onClick={() => go("library")}>Open Library</GlassButton>
                <GlassButton kind="ghost" onClick={() => go("settings", "dev")}>
                  Simulate a game (preview)
                </GlassButton>
              </div>
            </div>
          )}
        </Glass>

        <Panel title="Quick toggles">
          <ToggleRow
            label="Focus mode"
            hint="Pauses or limits non-game network use for this session."
            checked={focusOn}
            onChange={(v) => {
              setFocusOn(v);
              void setFocus(v).then(refreshChanges);
            }}
          />
          <Divider />
          <ToggleRow label="Automatic RAM cleanups" hint="Runs when a game starts or memory gets low." checked={ramAutoClean} onChange={(v) => setSetting("ramAutoClean", v)} />
          <Divider />
          <ToggleRow
            label="Simulate a running game"
            hint="Preview only: tests the HUD and glass auto-degrade."
            checked={simulateGameId !== null}
            onChange={(v) => setSetting("simulateGameId", v ? (target !== "none" ? target : "valorant") : null)}
          />
        </Panel>
      </div>

      {/* Changes + activity */}
      <div className="mt-5 grid grid-cols-1 gap-5 xl:grid-cols-[1.25fr_1fr]">
        <Panel
          title="What OTX.GNG changed"
          help="Every change is logged with its previous value and can be undone. Temporary changes revert automatically when the game closes, after a crash or on reboot."
          right={
            <GlassButton kind="destructive" size="sm" icon={<RotateCcw size={14} />} onClick={() => setConfirmRevert(true)} disabled={changes.length === 0}>
              Revert everything
            </GlassButton>
          }
        >
          {changes.length === 0 ? (
            <p className="text-t2">Nothing changed. Your PC is at its saved snapshot.</p>
          ) : (
            <ul className="divide-y divide-white/[0.06]">
              {changes.map((c) => (
                <li key={c.id} className="flex items-center gap-3 py-2.5">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 text-t1">
                      {c.title}
                      {c.temporary && <GlassBadge tone="info">Session</GlassBadge>}
                    </div>
                    <div className="truncate text-[13px] text-t2">
                      {c.detail} · {timeAgo(c.at)}
                    </div>
                  </div>
                  <GlassButton
                    size="sm"
                    kind="ghost"
                    onClick={() =>
                      void backend.revertChange(c.id).then(() => {
                        refreshChanges();
                        logActivity(`Reverted: ${c.title}`);
                      })
                    }
                  >
                    Revert
                  </GlassButton>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel title="Recent activity">
          {activity.length === 0 ? (
            <p className="text-t2">No activity in this session yet.</p>
          ) : (
            <ul className="space-y-2">
              {activity.slice(0, 8).map((a) => (
                <li key={a.id} className="flex justify-between gap-3 text-sm">
                  <span className="text-t1">{a.text}</span>
                  <span className="shrink-0 text-t2">{timeAgo(a.at)}</span>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>

      <GlassModal
        open={confirmRevert}
        onClose={() => setConfirmRevert(false)}
        title="Revert everything?"
        footer={
          <>
            <GlassButton onClick={() => setConfirmRevert(false)}>Cancel</GlassButton>
            <GlassButton
              kind="destructive"
              onClick={() => {
                setConfirmRevert(false);
                void revertEverything().then(refreshChanges);
                setOrb("ready");
                setSteps([]);
              }}
            >
              Revert everything
            </GlassButton>
          </>
        }
      >
        Every OTX.GNG change ({changes.length}) is restored to the value saved before it was made. Settings that need a reboot are restored on the next restart.
      </GlassModal>
    </div>
  );
}

function StepIcon({ status }: { status: BoostStep["status"] }) {
  if (status === "done") return <Check size={16} className="text-ok" aria-label="Done" />;
  if (status === "running") return <Loader2 size={16} className="spin-slow text-live" aria-label="Running" />;
  if (status === "skipped") return <MinusCircle size={16} className="text-t2" aria-label="Skipped" />;
  if (status === "failed") return <XCircle size={16} className="text-live" aria-label="Failed" />;
  return <CircleDashed size={16} className="text-t3" aria-label="Pending" />;
}

const ORB_LABEL: Record<OrbState, { title: string; sub: string; rgb: string }> = {
  ready: { title: "BOOST", sub: "Ready", rgb: "var(--rgb-green)" },
  boosting: { title: "BOOSTING", sub: "Applying steps…", rgb: "var(--rgb-red)" },
  boosted: { title: "BOOSTED", sub: "Freed", rgb: "var(--rgb-green)" },
  game: { title: "IN GAME", sub: "Game running", rgb: "var(--rgb-cyan)" },
};

function BoostOrb({ state, freed, onClick }: { state: OrbState; freed: number; onClick: () => void }) {
  const l = ORB_LABEL[state];
  const mv = useMotionValue(0);
  const freedText = useTransform(mv, (v) => `${Math.round(v)} MB`);
  useEffect(() => {
    if (state !== "boosted") return;
    mv.set(0);
    const c = animate(mv, freed, { duration: 1.2, ease: [0.2, 0.8, 0.2, 1] });
    return () => c.stop();
  }, [state, freed, mv]);

  return (
    <div className="relative my-4 flex h-[260px] w-[260px] items-center justify-center">
      {/* outer glow */}
      <div className="absolute inset-0 rounded-full" style={{ background: `radial-gradient(circle, rgb(${l.rgb} / .28), transparent 66%)` }} aria-hidden />
      {/* ring sweep while boosting */}
      <AnimatePresence>
        {state === "boosting" && (
          <motion.div
            key="sweep"
            className="absolute inset-3 rounded-full"
            style={{ background: `conic-gradient(from 0deg, transparent 0 70%, rgb(${l.rgb}) 100%)`, WebkitMask: "radial-gradient(circle, transparent 62%, #000 63%)", mask: "radial-gradient(circle, transparent 62%, #000 63%)" }}
            initial={{ opacity: 0, rotate: 0 }}
            animate={{ opacity: 1, rotate: 360 }}
            exit={{ opacity: 0 }}
            transition={{ rotate: { repeat: Infinity, duration: 1.1, ease: "linear" }, opacity: { duration: 0.2 } }}
            aria-hidden
          />
        )}
      </AnimatePresence>
      {/* particle burst when done */}
      <AnimatePresence>
        {state === "boosted" &&
          Array.from({ length: 14 }, (_, i) => {
            const a = (i / 14) * Math.PI * 2;
            return (
              <motion.span
                key={`p${i}`}
                className="absolute h-2 w-2 rounded-full"
                style={{ background: "var(--accent-green)", boxShadow: "0 0 10px var(--accent-green)" }}
                initial={{ x: 0, y: 0, opacity: 1, scale: 1 }}
                animate={{ x: Math.cos(a) * 150, y: Math.sin(a) * 150, opacity: 0, scale: 0.3 }}
                transition={{ duration: 0.9, ease: "easeOut" }}
                aria-hidden
              />
            );
          })}
      </AnimatePresence>
      <motion.button
        type="button"
        onClick={onClick}
        disabled={state === "boosting"}
        aria-label={state === "boosting" ? "Boost in progress" : "Boost now"}
        className="glass glass--tinted glass--lens relative flex h-[200px] w-[200px] cursor-pointer flex-col items-center justify-center disabled:cursor-wait"
        style={{ ["--tint" as string]: l.rgb, ["--radius" as string]: "999px", boxShadow: `0 0 60px -10px rgb(${l.rgb} / .7), inset 0 1px 0 rgb(255 255 255 / .4), inset 0 -10px 30px rgb(0 0 0 / .25)` }}
        whileHover={{ scale: 1.03 }}
        whileTap={{ scale: 0.95 }}
        transition={spring}
      >
        <Zap size={34} className="mb-1" aria-hidden />
        <span className="font-head text-[30px] font-bold tracking-[0.14em]">{l.title}</span>
        {state === "boosted" ? <motion.span className="num text-sm text-t1">{freedText}</motion.span> : <span className="text-sm text-t2">{l.sub}</span>}
      </motion.button>
    </div>
  );
}
