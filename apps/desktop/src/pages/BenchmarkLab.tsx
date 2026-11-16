import { Download, FlaskConical } from "lucide-react";
import { useEffect, useState } from "react";
import { backend } from "../backend";
import type { Game } from "../backend/types";
import { PageHeader, Panel, Row } from "../components/ui";
import { GlassBadge, GlassButton, GlassSelect, GlassSlider } from "../design-system";
import { compare, summarizeFrametimes, type Comparison, type RunSummary } from "../lib/benchStats";
import { downloadText } from "../lib/dom";
import { invoke } from "@tauri-apps/api/core";
import { endSession, startSession } from "../backend/gameMode";
import { isTauri } from "../lib/tauri";
import { useLive } from "../store/live";
import { useProfiles } from "../store/profiles";
import { useUi } from "../store/ui";

/** Preview-only frame-time generator. Real captures use ETW Present events (PresentMon approach). */
function fakeCapture(baseFps: number, stutter: number, seconds: number): number[] {
  const n = Math.round(baseFps * seconds);
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    let ft = 1000 / baseFps + (Math.random() - 0.5) * 0.9;
    if (Math.random() < stutter) ft *= 2 + Math.random() * 2.5;
    out.push(ft);
  }
  return out;
}

const LABEL: Record<Comparison["metric"], string> = { avgFps: "Average FPS", low1: "1% low FPS", low01: "0.1% low FPS", frametimeStdMs: "Frame-time variation (ms)" };

export function BenchmarkLab() {
  const [games, setGames] = useState<Game[]>([]);
  const [game, setGame] = useState("cs2");
  const [duration, setDuration] = useState(90);
  const [runs, setRuns] = useState(3);
  const [progress, setProgress] = useState<string | null>(null);
  const [base, setBase] = useState<RunSummary[]>([]);
  const [boosted, setBoosted] = useState<RunSummary[]>([]);

  useEffect(() => void backend.getGames().then(setGames), []);
  const runningGame = useLive((s) => s.runningGame);
  const real = isTauri();

  /** Real A/B: alternate baseline (no session) and boosted (Game Mode on) captures of the running game. */
  const runReal = async () => {
    if (!runningGame) {
      useUi.getState().toast({ tone: "warn", title: "Start the game first", body: "Benchmark Lab measures the game that is running now. Play the same scene for every run." });
      return;
    }
    const b: RunSummary[] = [];
    const o: RunSummary[] = [];
    const wait = (s: number, label: string) =>
      new Promise<void>((resolve) => {
        let left = s;
        setProgress(`${label}: ${left} s left`);
        const h = setInterval(() => {
          left -= 1;
          setProgress(`${label}: ${left} s left`);
          if (left <= 0) {
            clearInterval(h);
            resolve();
          }
        }, 1000);
      });
    try {
      for (let i = 0; i < runs; i++) {
        await endSession().catch(() => undefined);
        await invoke("capture_start", { target: runningGame.process });
        await wait(duration, `Baseline run ${i + 1} of ${runs}`);
        b.push(summarizeFrametimes(await invoke<number[]>("capture_stop")));
        await startSession(runningGame.process, useProfiles.getState().get(runningGame.gameId));
        await invoke("capture_start", { target: runningGame.process });
        await wait(duration, `Boosted run ${i + 1} of ${runs}`);
        o.push(summarizeFrametimes(await invoke<number[]>("capture_stop")));
      }
      setBase(b);
      setBoosted(o);
    } catch (e) {
      await invoke("capture_stop").catch(() => undefined);
      useUi.getState().toast({ tone: "error", title: "Benchmark stopped", body: String(e) });
    } finally {
      setProgress(null);
    }
  };

  const run = async () => {
    if (real) return runReal();
    const b: RunSummary[] = [];
    const o: RunSummary[] = [];
    for (let i = 0; i < runs; i++) {
      setProgress(`Baseline run ${i + 1} of ${runs}`);
      await new Promise((r) => setTimeout(r, 350));
      b.push(summarizeFrametimes(fakeCapture(200 + Math.random() * 8, 0.004, 10)));
      setProgress(`Boosted run ${i + 1} of ${runs}`);
      await new Promise((r) => setTimeout(r, 350));
      o.push(summarizeFrametimes(fakeCapture(202 + Math.random() * 8, 0.0025, 10)));
    }
    setBase(b);
    setBoosted(o);
    setProgress(null);
  };

  const result = compare(base, boosted);
  const g = games.find((x) => x.id === game);

  const exportReport = () => {
    // No personal data: no user name, paths, hardware serials or IPs.
    const report = {
      app: "OTX.GNG",
      kind: "benchmark-report",
      previewData: !real,
      game: g?.name,
      durationSecPerRun: duration,
      runsPerSide: runs,
      baseline: base,
      boosted,
      comparison: result,
      createdAt: new Date().toISOString(),
    };
    downloadText(`otx-benchmark-${game}.json`, JSON.stringify(report, null, 2));
  };

  return (
    <div>
      <PageHeader title="Benchmark Lab" subtitle="Honest A/B tests: baseline against boosted, several runs each, with the spread shown. 'Within noise' is a valid answer." tier={0} />
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1fr_1.4fr]">
        <Panel title="Setup">
          <Row label="Game" hint={real ? "The game that is running now is measured." : undefined}>
            {real ? (
              <GlassBadge tone={runningGame ? "ok" : "neutral"}>{runningGame ? runningGame.exe : "No game running"}</GlassBadge>
            ) : (
              <GlassSelect label="Game" value={game} onChange={setGame} options={games.map((x) => ({ value: x.id, label: x.name }))} />
            )}
          </Row>
          <Row label="Seconds per run" hint="60 to 180 s. Play the same scene each time.">
            <div className="w-56">
              <GlassSlider label="Seconds per run" min={60} max={180} step={10} value={duration} onChange={setDuration} format={(v) => `${v} s`} />
            </div>
          </Row>
          <Row label="Runs per side" hint="At least 3, so noise can be measured.">
            <div className="w-56">
              <GlassSlider label="Runs per side" min={3} max={7} value={runs} onChange={setRuns} />
            </div>
          </Row>
          <p className="my-3 text-[13px] text-t2">
            Frame times come from Windows' own DXGI Present events (the approach PresentMon uses): no injection, no hooks, nothing touches the game. They are intervals between Present calls, not display flips, and games that do not use DXGI (DirectX 9, OpenGL, Vulkan) produce none. In the desktop app this measures the running game; the browser preview uses generated data.
          </p>
          <div className="flex gap-2">
            <GlassButton kind="primary" icon={<FlaskConical size={16} />} loading={progress !== null} onClick={() => void run()}>
              Start A/B test
            </GlassButton>
            <GlassButton icon={<Download size={16} />} disabled={!result} onClick={exportReport}>
              Export report
            </GlassButton>
          </div>
          {progress && <p className="mt-3 text-t1">{progress}…</p>}
        </Panel>

        <Panel title="Results" help="Medians of each side. A change counts only if it is bigger than the run-to-run spread.">
          {!result ? (
            <p className="text-t2">Run a test to see results.</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-[12px] uppercase tracking-wider text-t2">
                  <th className="pb-2 font-semibold">Metric</th>
                  <th className="pb-2 text-right font-semibold">Baseline</th>
                  <th className="pb-2 text-right font-semibold">Boosted</th>
                  <th className="pb-2 text-right font-semibold">Change</th>
                  <th className="pb-2 text-right font-semibold">Spread</th>
                  <th className="pb-2 text-right font-semibold">Verdict</th>
                </tr>
              </thead>
              <tbody>
                {result.map((r) => (
                  <tr key={r.metric} className="border-t border-white/[0.06]">
                    <td className="py-2.5 text-t1">{LABEL[r.metric]}</td>
                    <td className="num py-2.5 text-right">{r.baselineMedian.toFixed(1)}</td>
                    <td className="num py-2.5 text-right">{r.boostedMedian.toFixed(1)}</td>
                    <td className="num py-2.5 text-right">
                      {r.deltaPct >= 0 ? "+" : ""}
                      {r.deltaPct.toFixed(1)}%
                    </td>
                    <td className="num py-2.5 text-right text-t2">±{r.spreadPct.toFixed(1)}%</td>
                    <td className="py-2.5 text-right">
                      <GlassBadge tone={r.verdict === "better" ? "ok" : r.verdict === "worse" ? "danger" : "neutral"}>{r.verdict === "within-noise" ? "Within noise" : r.verdict === "better" ? "Better" : "Worse"}</GlassBadge>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Panel>
      </div>
    </div>
  );
}
