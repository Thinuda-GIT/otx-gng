import { Ban, Feather, Flame, Leaf, Lock, ShieldCheck, ShieldQuestion, Stethoscope, Timer } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { backend } from "../backend";
import type { PriorityLevel, ProcessCategory, ProcessGroup } from "../backend/types";
import { PageHeader, Panel, Stat } from "../components/ui";
import { Glass, GlassBadge, GlassButton, GlassModal, GlassSegmented, GlassTable, LiveChart, type Column } from "../design-system";
import { analyze, type AnalyzerSample, type Finding } from "../lib/analyzer";
import { formatGB, formatMB, formatRate } from "../lib/format";
import { useLive } from "../store/live";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

const CAT_TONE: Record<ProcessCategory, "ok" | "info" | "violet" | "neutral" | "warn" | "danger"> = {
  game: "ok",
  launcher: "info",
  browser: "violet",
  system: "neutral",
  background: "neutral",
  unknown: "warn",
};

type ChartTab = "cpu" | "gpu" | "ram" | "disk" | "net";

export function UsageMonitor() {
  const history = useLive((s) => s.history);
  const latest = useLive((s) => s.latest);
  const paused = useLive((s) => s.paused);
  const hwMon = useSettings((s) => s.hardwareMonitoring);
  const go = useUi((s) => s.go);
  const toast = useUi((s) => s.toast);
  const logActivity = useUi((s) => s.logActivity);
  const [procs, setProcs] = useState<ProcessGroup[]>([]);
  const [sel, setSel] = useState<string | null>(null);
  const [chart, setChart] = useState<ChartTab>("cpu");
  const [confirmEnd, setConfirmEnd] = useState<ProcessGroup | null>(null);
  const [findings, setFindings] = useState<Finding[] | null>(null);

  useEffect(() => {
    if (paused) return;
    const load = () => void backend.getProcesses().then(setProcs);
    load();
    const h = setInterval(load, 2000);
    return () => clearInterval(h);
  }, [paused]);

  const selected = procs.find((p) => p.id === sel) ?? null;
  const last = (f: (m: (typeof history)[number]) => number) => history.map(f);

  const columns = useMemo<Column<ProcessGroup>[]>(
    () => [
      {
        key: "name",
        header: "App",
        width: "minmax(200px,2.2fr)",
        sortValue: (p) => p.name,
        render: (p) => (
          <span className="flex items-center gap-2">
            {p.protected && <Lock size={13} className="shrink-0 text-t2" aria-label="Protected" />}
            <span className="truncate text-t1">{p.name}</span>
            {p.pids.length > 1 && <span className="num text-[12px] text-t2">×{p.pids.length}</span>}
            {p.hog && (
              <span title="Resource hog: high CPU, memory or network for a background app">
                <Flame size={14} className="text-warn" aria-label="Resource hog" />
              </span>
            )}
          </span>
        ),
      },
      { key: "cat", header: "Category", width: "110px", sortValue: (p) => p.category, render: (p) => <GlassBadge tone={CAT_TONE[p.category]}>{p.category}</GlassBadge> },
      { key: "cpu", header: "CPU", width: "70px", align: "right", sortValue: (p) => p.cpu, render: (p) => `${p.cpu.toFixed(1)}%` },
      { key: "gpu", header: "GPU", width: "90px", align: "right", sortValue: (p) => p.gpu, render: (p) => (p.gpu > 0.05 ? `${p.gpu.toFixed(1)}% ${p.gpuEngine}` : "-") },
      { key: "ws", header: "Memory", width: "90px", align: "right", sortValue: (p) => p.wsMB, render: (p) => formatMB(p.wsMB) },
      { key: "disk", header: "Disk", width: "90px", align: "right", sortValue: (p) => p.diskReadKBs + p.diskWriteKBs, render: (p) => `${((p.diskReadKBs + p.diskWriteKBs) / 1024).toFixed(1)} MB/s` },
      { key: "net", header: "Network", width: "110px", align: "right", sortValue: (p) => p.netDownKbps + p.netUpKbps, render: (p) => formatRate(p.netDownKbps + p.netUpKbps) },
    ],
    [],
  );

  const act = async (fn: () => Promise<void>, msg: string) => {
    try {
      await fn();
      toast({ tone: "ok", title: msg, body: "Undone automatically when the session ends, or from Home." });
      logActivity(msg);
      setProcs(await backend.getProcesses());
    } catch (e) {
      toast({ tone: "error", title: "Not changed", body: (e as Error).message });
    }
  };

  const runAnalyzer = () => {
    const samples: AnalyzerSample[] = history.slice(-60).map((m) => ({
      cpuTotal: m.cpu,
      maxCoreLoad: Math.max(...m.perCore),
      gpu3d: m.gpu.d3,
      ramAvailablePct: (m.ram.availableMB / m.ram.totalMB) * 100,
      diskActivePct: m.disk.activePct,
      cpuTempC: null,
      gpuTempC: null,
      pingMs: m.ping?.ms ?? null,
      jitterMs: m.ping?.jitter ?? null,
      lossPct: m.ping?.loss ?? null,
    }));
    setFindings(analyze(samples));
  };

  return (
    <div>
      <PageHeader
        title="Usage Monitor"
        subtitle="What every app is using right now, grouped by app, and what it is doing."
        tier={0}
        actions={
          <>
            <GlassButton icon={<Stethoscope size={16} />} kind="primary" onClick={runAnalyzer}>
              Why is my game slow?
            </GlassButton>
            <GlassButton icon={<Timer size={16} />} onClick={() => go("benchmark")}>
              Benchmark Lab
            </GlassButton>
          </>
        }
      />

      <Glass className="mb-5 p-5">
        <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
          <GlassSegmented<ChartTab>
            role="tabs"
            size="sm"
            label="Chart"
            value={chart}
            onChange={setChart}
            options={[
              { value: "cpu", label: "CPU" },
              { value: "gpu", label: "GPU" },
              { value: "ram", label: "RAM" },
              { value: "disk", label: "Disk" },
              { value: "net", label: "Network" },
            ]}
          />
          {latest && (
            <div className="flex flex-wrap gap-6">
              {chart === "cpu" && <Stat label="Total" value={`${latest.cpu.toFixed(0)}%`} />}
              {chart === "gpu" && (
                <>
                  <Stat label="3D" value={`${latest.gpu.d3.toFixed(0)}%`} color="var(--accent-violet)" />
                  <Stat label="Compute" value={`${latest.gpu.compute.toFixed(0)}%`} />
                  <Stat label="Copy" value={`${latest.gpu.copy.toFixed(0)}%`} />
                  <Stat label="Video decode" value={`${latest.gpu.videoDecode.toFixed(0)}%`} />
                </>
              )}
              {chart === "ram" && (
                <>
                  <Stat label="Used" value={formatGB(latest.ram.usedMB)} />
                  <Stat label="Available" value={formatGB(latest.ram.availableMB)} color="var(--accent-green)" />
                  <Stat label="Commit" value={`${formatGB(latest.ram.commitMB)} / ${formatGB(latest.ram.commitLimitMB)}`} />
                  <Stat label="Standby" value={formatGB(latest.ram.standbyMB)} />
                </>
              )}
              {chart === "disk" && (
                <>
                  <Stat label="Read" value={`${latest.disk.readMBs.toFixed(1)} MB/s`} />
                  <Stat label="Write" value={`${latest.disk.writeMBs.toFixed(1)} MB/s`} />
                  <Stat label="Active" value={`${latest.disk.activePct.toFixed(0)}%`} />
                </>
              )}
              {chart === "net" && (
                <>
                  <Stat label="Down" value={formatRate(latest.net.downKbps)} color="var(--accent-cyan)" />
                  <Stat label="Up" value={formatRate(latest.net.upKbps)} />
                </>
              )}
            </div>
          )}
        </div>
        {chart === "cpu" && (
          <>
            <LiveChart ariaLabel="CPU usage over time" max={100} height={140} series={[{ values: last((m) => m.cpu), color: "30 242 160", fill: true }]} />
            {latest && (
              <div className="mt-4 grid gap-1.5" style={{ gridTemplateColumns: `repeat(${Math.min(20, latest.perCore.length)}, minmax(0,1fr))` }} aria-label="Per-core load">
                {latest.perCore.map((v, i) => (
                  <div key={i} className="flex flex-col items-center gap-1">
                    <div className="relative h-14 w-full overflow-hidden rounded-[6px] bg-white/[0.07]">
                      <div className="absolute inset-x-0 bottom-0 rounded-[6px]" style={{ height: `${v}%`, background: v > 90 ? "var(--accent-red)" : "linear-gradient(0deg,#1EF2A0,#2DE2FF)", transition: "height .4s" }} />
                    </div>
                    <span className="num text-[11px] text-t2">{i}</span>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
        {chart === "gpu" && (
          <LiveChart
            ariaLabel="GPU engine usage over time"
            max={100}
            height={180}
            series={[
              { values: last((m) => m.gpu.d3), color: "139 108 255", fill: true },
              { values: last((m) => m.gpu.copy), color: "45 226 255" },
              { values: last((m) => m.gpu.videoDecode), color: "255 176 32" },
            ]}
          />
        )}
        {chart === "ram" && (
          <LiveChart
            ariaLabel="Memory over time"
            height={180}
            series={[
              { values: last((m) => m.ram.usedMB), color: "30 242 160", fill: true },
              { values: last((m) => m.ram.standbyMB), color: "45 226 255" },
            ]}
          />
        )}
        {chart === "disk" && (
          <LiveChart
            ariaLabel="Disk throughput over time"
            height={180}
            series={[
              { values: last((m) => m.disk.readMBs), color: "45 226 255", fill: true },
              { values: last((m) => m.disk.writeMBs), color: "255 61 90" },
            ]}
          />
        )}
        {chart === "net" && (
          <LiveChart
            ariaLabel="Network throughput over time"
            height={180}
            series={[
              { values: last((m) => m.net.downKbps), color: "45 226 255", fill: true },
              { values: last((m) => m.net.upKbps), color: "139 108 255" },
            ]}
          />
        )}
        {!hwMon && <p className="mt-3 text-[13px] text-t2">Temperatures, clocks and fans appear when Hardware data monitoring is on (General Settings).</p>}
      </Glass>

      <div className="grid grid-cols-1 gap-5 2xl:grid-cols-[1fr_380px]">
        <Glass className="overflow-hidden">
          <GlassTable
            label="Processes grouped by app"
            rows={procs}
            columns={columns}
            rowKey={(p) => p.id}
            height={460}
            selectedKey={sel}
            onSelect={(p) => setSel(p.id)}
            initialSort={{ key: "cpu", dir: "desc" }}
          />
        </Glass>

        <Panel title="What is this process?">
          {!selected ? (
            <p className="text-t2">Select an app in the table to see what it is and what you can safely do with it.</p>
          ) : (
            <div className="space-y-4">
              <div>
                <div className="font-head text-2xl font-bold">{selected.name}</div>
                <div className="num break-all text-[12px] text-t2">{selected.path}</div>
              </div>
              <div className="flex flex-wrap gap-1.5">
                {selected.signed ? (
                  <GlassBadge tone="ok">
                    <ShieldCheck size={12} aria-hidden /> {selected.publisher}
                  </GlassBadge>
                ) : (
                  <GlassBadge tone="warn">
                    <ShieldQuestion size={12} aria-hidden /> Unsigned
                  </GlassBadge>
                )}
                {selected.startup && <GlassBadge tone="info">Starts with Windows</GlassBadge>}
                {selected.protected && (
                  <GlassBadge tone="neutral">
                    <Lock size={12} aria-hidden /> Protected
                  </GlassBadge>
                )}
                <GlassBadge tone={selected.safeToClose === "yes" ? "ok" : selected.safeToClose === "no" ? "danger" : "warn"}>
                  {selected.safeToClose === "yes" ? "Safe to close" : selected.safeToClose === "no" ? "Do not close" : "Close with care"}
                </GlassBadge>
              </div>
              <p className="text-t1">{selected.description}</p>
              <div className="grid grid-cols-3 gap-3 text-sm">
                <Stat label="Working set" value={formatMB(selected.wsMB)} />
                <Stat label="Private" value={formatMB(selected.privateMB)} />
                <Stat label="Commit" value={formatMB(selected.commitMB)} />
              </div>
              <div className="text-[13px] text-t2">
                Priority: <span className="text-t1">{selected.priority}</span> · Efficiency mode: <span className="text-t1">{selected.eco ? "on" : "off"}</span>
              </div>
              {selected.protected ? (
                <p className="text-[13px] text-t2">Protected processes can be viewed but never changed by OTX.GNG.</p>
              ) : (
                <div className="flex flex-wrap gap-2">
                  <GlassButton size="sm" icon={<Feather size={14} />} disabled={selected.category === "game"} onClick={() => void act(() => backend.setPriority(selected.id, "below" as PriorityLevel), `${selected.name}: priority lowered`)}>
                    Lower priority
                  </GlassButton>
                  <GlassButton size="sm" icon={<Leaf size={14} />} disabled={selected.category === "game"} onClick={() => void act(() => backend.setEfficiencyMode(selected.id, !selected.eco), `${selected.name}: efficiency mode ${selected.eco ? "off" : "on"}`)}>
                    {selected.eco ? "Efficiency mode off" : "Efficiency mode"}
                  </GlassButton>
                  <GlassButton size="sm" kind="destructive" icon={<Ban size={14} />} onClick={() => setConfirmEnd(selected)}>
                    End task
                  </GlassButton>
                </div>
              )}
              <a className="inline-block text-[13px] text-t2 underline underline-offset-2 hover:text-t1" href={`https://www.bing.com/search?q=${encodeURIComponent(selected.exe)}`} target="_blank" rel="noreferrer noopener">
                Look up “{selected.exe}” online (opens your browser; only the file name is sent)
              </a>
            </div>
          )}
        </Panel>
      </div>

      <GlassModal
        open={confirmEnd !== null}
        onClose={() => setConfirmEnd(null)}
        title={`End ${confirmEnd?.name ?? ""}?`}
        footer={
          <>
            <GlassButton onClick={() => setConfirmEnd(null)}>Cancel</GlassButton>
            <GlassButton
              kind="destructive"
              onClick={() => {
                const p = confirmEnd!;
                setConfirmEnd(null);
                setSel(null);
                void act(() => backend.endTask(p.id), `${p.name} ended`);
              }}
            >
              End task
            </GlassButton>
          </>
        }
      >
        Unsaved work in {confirmEnd?.name} will be lost. This closes {confirmEnd?.pids.length} process{confirmEnd && confirmEnd.pids.length > 1 ? "es" : ""}.
      </GlassModal>

      <GlassModal open={findings !== null} onClose={() => setFindings(null)} title="Why is my game slow?" width={640}>
        <p className="mb-4 text-[13px]">Based on the last {Math.min(60, history.length)} seconds of measured data. Rules, not guesses: when the data is not clear, it says so.</p>
        <ul className="space-y-3">
          {findings?.map((f) => (
            <li key={f.id} className="glass glass--clear p-4" style={{ ["--radius" as string]: "16px" }}>
              <div className="flex items-center justify-between gap-2">
                <span className="font-semibold text-t1">{f.title}</span>
                <GlassBadge tone={f.confidence === "high" ? "ok" : f.confidence === "medium" ? "warn" : "neutral"}>{f.confidence} confidence</GlassBadge>
              </div>
              <p className="mt-1 text-[13px]">{f.detail}</p>
              {f.fixes.length > 0 && (
                <div className="mt-3 flex flex-wrap gap-2">
                  {f.fixes.map((x) => (
                    <GlassButton
                      key={x.id}
                      size="sm"
                      onClick={() => {
                        setFindings(null);
                        if (x.id === "ram-clean") go("boost", "ram");
                        else if (x.id === "route-lab") go("connections");
                        else if (x.id === "focus-mode") go("shaper");
                        else if (x.id === "benchmark") go("benchmark");
                        else go("boost", "tweaks");
                      }}
                    >
                      {x.label}
                    </GlassButton>
                  ))}
                </div>
              )}
            </li>
          ))}
        </ul>
      </GlassModal>
    </div>
  );
}
