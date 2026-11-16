import { Activity, CheckCircle2, Info, Play, Wrench } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { backend } from "../backend";
import type { Connection, QualityResult, TrafficApp, TuneFinding } from "../backend/types";
import { PageHeader, Panel, Stat } from "../components/ui";
import { Glass, GlassBadge, GlassButton, GlassSegmented, GlassTable, Sparkline, type Column } from "../design-system";
import { formatRate } from "../lib/format";
import { useLive } from "../store/live";
import { useUi } from "../store/ui";

type Tab = "live" | "connections" | "quality" | "tuneup";

const PURPOSE: Record<TrafficApp["purpose"], { label: string; tone: "ok" | "info" | "warn" | "violet" | "neutral" | "danger" }> = {
  game: { label: "Game traffic", tone: "ok" },
  launcher: { label: "Launcher", tone: "info" },
  update: { label: "Update / download", tone: "warn" },
  "cloud-sync": { label: "Cloud sync", tone: "warn" },
  streaming: { label: "Streaming", tone: "violet" },
  browser: { label: "Browser", tone: "violet" },
  telemetry: { label: "Telemetry", tone: "neutral" },
  voice: { label: "Voice chat", tone: "info" },
  system: { label: "System", tone: "neutral" },
  unknown: { label: "Unknown", tone: "danger" },
};

export function NetworkAnalyzer() {
  const tab = (useUi((s) => s.subTab.network) as Tab | undefined) ?? "live";
  const setSubTab = useUi((s) => s.setSubTab);
  return (
    <div>
      <PageHeader title="Network Analyzer" subtitle="See which app uses your connection and for what, test its quality, and fix what can be fixed on this PC." tier={0} />
      <div className="mb-6">
        <GlassSegmented<Tab>
          role="tabs"
          label="Network Analyzer sections"
          value={tab}
          onChange={(v) => setSubTab("network", v)}
          options={[
            { value: "live", label: "Live Traffic" },
            { value: "connections", label: "Connections" },
            { value: "quality", label: "Quality Test" },
            { value: "tuneup", label: "Tune-up" },
          ]}
        />
      </div>
      {tab === "live" && <LiveTraffic />}
      {tab === "connections" && <Connections />}
      {tab === "quality" && <QualityTest />}
      {tab === "tuneup" && <TuneUp />}
    </div>
  );
}

function LiveTraffic() {
  const [apps, setApps] = useState<TrafficApp[]>([]);
  const paused = useLive((s) => s.paused);
  const latest = useLive((s) => s.latest);
  useEffect(() => {
    if (paused) return;
    const load = () => void backend.getTraffic().then(setApps);
    load();
    const h = setInterval(load, 1000);
    return () => clearInterval(h);
  }, [paused]);
  const sorted = [...apps].sort((a, b) => b.downKbps + b.upKbps - (a.downKbps + a.upKbps));
  const max = Math.max(1, ...sorted.map((a) => a.downKbps + a.upKbps));
  return (
    <div className="space-y-5">
      <Glass className="grid grid-cols-2 gap-5 p-5 md:grid-cols-4">
        <Stat label="Download (all apps)" value={latest ? formatRate(latest.net.downKbps) : "-"} color="var(--accent-cyan)" />
        <Stat label="Upload (all apps)" value={latest ? formatRate(latest.net.upKbps) : "-"} color="var(--accent-violet)" />
        <Stat label="Adapter" value="Wi-Fi" sub="866 Mbps link, 5 GHz" />
        <Stat label="Apps using the network" value={apps.length} />
      </Glass>
      <Panel title="Top talkers" help="Measured per process from Windows network events (ETW). The 'what is it for' label is a heuristic from the process, its publisher and where it connects.">
        <ul className="space-y-3">
          {sorted.map((a) => {
            const total = a.downKbps + a.upKbps;
            const p = PURPOSE[a.purpose];
            return (
              <li key={a.id} className="grid grid-cols-[minmax(160px,1.2fr)_140px_1fr_150px] items-center gap-4">
                <div className="min-w-0">
                  <div className="truncate text-t1">{a.name}</div>
                  <GlassBadge tone={p.tone}>{p.label}</GlassBadge>
                </div>
                <div className="h-9">
                  <Sparkline values={a.history} color={a.purpose === "game" ? "30 242 160" : "45 226 255"} height={36} />
                </div>
                <div className="otx-progress">
                  <span style={{ width: `${(total / max) * 100}%`, background: a.purpose === "game" ? undefined : a.purpose === "update" || a.purpose === "cloud-sync" ? "linear-gradient(90deg,#FFB020,#FF3D5A)" : undefined }} />
                </div>
                <div className="num text-right text-[13px]">
                  <div className="text-t1">↓ {formatRate(a.downKbps)}</div>
                  <div className="text-t2">↑ {formatRate(a.upKbps)}</div>
                </div>
              </li>
            );
          })}
        </ul>
      </Panel>
    </div>
  );
}

function Connections() {
  const [rows, setRows] = useState<Connection[]>([]);
  const [q, setQ] = useState("");
  useEffect(() => void backend.getConnections().then(setRows), []);
  const filtered = rows.filter((r) => (r.app + r.remote + r.rdns + r.asn).toLowerCase().includes(q.toLowerCase()));
  const columns = useMemo<Column<Connection>[]>(
    () => [
      { key: "app", header: "App", width: "minmax(150px,1.2fr)", sortValue: (r) => r.app, render: (r) => <span className="text-t1">{r.app}</span> },
      { key: "proto", header: "Proto", width: "60px", sortValue: (r) => r.protocol, render: (r) => r.protocol },
      { key: "remote", header: "Remote", width: "minmax(160px,1fr)", sortValue: (r) => r.remote, render: (r) => <span className="num">{r.remote}</span> },
      { key: "rdns", header: "Reverse DNS", width: "minmax(180px,1.4fr)", sortValue: (r) => r.rdns, render: (r) => <span className="text-t2">{r.rdns}</span> },
      { key: "asn", header: "Network", width: "minmax(140px,1fr)", sortValue: (r) => r.asn, render: (r) => r.asn },
      { key: "cc", header: "Country", width: "70px", sortValue: (r) => r.country, render: (r) => r.country },
      { key: "state", header: "State", width: "110px", sortValue: (r) => r.state, render: (r) => <span className="text-t2">{r.state}</span> },
    ],
    [],
  );
  return (
    <Glass className="overflow-hidden">
      <div className="flex items-center justify-between gap-3 p-4">
        <input className="otx-input max-w-sm" type="search" placeholder="Filter by app, address or network" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Filter connections" />
        <span className="text-[13px] text-t2">Country and network names come from an offline database (DB-IP Lite, CC BY 4.0); nothing is looked up online.</span>
      </div>
      <GlassTable label="Connections" rows={filtered} columns={columns} rowKey={(r) => r.id} height={480} initialSort={{ key: "app", dir: "asc" }} />
    </Glass>
  );
}

function QualityTest() {
  const [target, setTarget] = useState("game server (auto-detected)");
  const [pct, setPct] = useState<number | null>(null);
  const [res, setRes] = useState<QualityResult | null>(null);
  const run = async () => {
    setRes(null);
    setPct(0);
    setRes(await backend.runQualityTest(target, setPct));
    setPct(null);
  };
  const grade = res ? (res.loadedUpMs - res.idleMs < 30 ? "A" : res.loadedUpMs - res.idleMs < 60 ? "B" : res.loadedUpMs - res.idleMs < 120 ? "C" : "D") : null;
  return (
    <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
      <Panel title="Run a test">
        <label className="mb-1 block text-[13px] text-t2" htmlFor="qt-target">
          Target
        </label>
        <input id="qt-target" className="otx-input mb-4" value={target} onChange={(e) => setTarget(e.target.value)} />
        <p className="mb-4 text-[13px] text-t2">
          Ping, jitter and loss use ICMP, TCP and UDP probes (many game servers ignore ICMP). The bufferbloat part downloads and uploads for about 20 seconds and uses about 50 MB of data.
        </p>
        <GlassButton kind="primary" icon={<Play size={16} />} loading={pct !== null} onClick={() => void run()}>
          Start test
        </GlassButton>
        {pct !== null && (
          <div className="otx-progress mt-4" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
            <span style={{ width: `${pct}%` }} />
          </div>
        )}
      </Panel>
      <Panel title="Results">
        {!res ? (
          <p className="text-t2">No results yet.</p>
        ) : (
          <div className="space-y-5">
            <div className="grid grid-cols-3 gap-4">
              <Stat label="Ping" value={`${res.pingMs.toFixed(0)} ms`} sub={`${res.method} probe`} color="var(--accent-cyan)" />
              <Stat label="Jitter" value={`${res.jitterMs.toFixed(1)} ms`} />
              <Stat label="Loss" value={`${res.lossPct.toFixed(1)}%`} />
            </div>
            <div>
              <div className="mb-2 flex items-center gap-2">
                <span className="font-medium text-t1">Bufferbloat</span>
                <GlassBadge tone={grade === "A" || grade === "B" ? "ok" : "warn"}>Grade {grade}</GlassBadge>
              </div>
              <div className="grid grid-cols-3 gap-4">
                <Stat label="Idle" value={`${res.idleMs.toFixed(0)} ms`} />
                <Stat label="While downloading" value={`${res.loadedDownMs.toFixed(0)} ms`} />
                <Stat label="While uploading" value={`${res.loadedUpMs.toFixed(0)} ms`} color="var(--accent-amber)" />
              </div>
            </div>
            <p className="text-[13px] text-t2">Data used: {res.dataUsedMB} MB.</p>
          </div>
        )}
      </Panel>
    </div>
  );
}

function TuneUp() {
  const [items, setItems] = useState<TuneFinding[]>([]);
  const toast = useUi((s) => s.toast);
  useEffect(() => void backend.getTuneFindings().then(setItems), []);
  const icon = (s: TuneFinding["severity"]) =>
    s === "ok" ? <CheckCircle2 size={18} className="text-ok" aria-hidden /> : s === "warn" ? <Activity size={18} className="text-warn" aria-hidden /> : <Info size={18} className="text-net" aria-hidden />;
  return (
    <div className="space-y-5">
      <Panel
        title="Diagnosis"
        help="Diagnose first, then fix. Each fix is reversible and labelled with how well the evidence supports it."
        right={
          <GlassButton size="sm" kind="ghost" onClick={() => toast({ tone: "ok", title: "Network settings reset to defaults", body: "Preview only: nothing was changed." })}>
            Reset all to default
          </GlassButton>
        }
      >
        <ul className="divide-y divide-white/[0.06]">
          {items.map((f) => (
            <li key={f.id} className="flex items-start gap-3 py-3.5">
              <span className="mt-0.5">{icon(f.severity)}</span>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2 text-t1">
                  {f.title}
                  <GlassBadge tone={f.evidence === "documented" ? "info" : f.evidence === "community" ? "neutral" : "violet"}>
                    {f.evidence === "documented" ? "Documented" : f.evidence === "community" ? "Community-reported" : "Experimental"}
                  </GlassBadge>
                </div>
                <p className="mt-0.5 text-[13px] text-t2">{f.detail}</p>
              </div>
              {f.fixLabel && (
                <GlassButton size="sm" icon={<Wrench size={14} />} onClick={() => toast({ tone: "ok", title: f.fixLabel!, body: "Preview only: the Core service applies this from Phase 7." })}>
                  {f.fixLabel}
                </GlassButton>
              )}
            </li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}
