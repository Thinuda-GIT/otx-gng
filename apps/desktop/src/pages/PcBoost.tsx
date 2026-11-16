import {
  AlertTriangle,
  ArrowDownWideNarrow,
  Cpu,
  Eraser,
  FileCog,
  Gamepad2,
  Gauge,
  Globe,
  HardDrive,
  Keyboard,
  Layers,
  MemoryStick,
  Mouse,
  MousePointerClick,
  Network,
  Plus,
  RefreshCw,
  Rocket,
  Search,
  SlidersHorizontal,
  Timer,
  Trash2,
  Trophy,
  Video,
  Wifi,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { backend } from "../backend";
import type { DnsResult, DnsServer, ProcessGroup, RamCleanResult, RamMethod, RamStatus, Tweak, TweakPreset } from "../backend/types";
import { Divider, Empty, PageHeader, Panel, Row, Stat, ToggleRow } from "../components/ui";
import { Glass, GlassBadge, GlassButton, GlassGauge, GlassModal, GlassSegmented, GlassSlider, GlassToggle, GlassTooltip } from "../design-system";
import { formatGB, isValidIPv4, timeAgo } from "../lib/format";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

const ICONS: Record<string, LucideIcon> = {
  FileCog, Cpu, MousePointerClick, Keyboard, Gauge, Rocket, ArrowDownWideNarrow, Wifi, Video, Layers,
  HardDrive, MemoryStick, Search, Zap, Gamepad2, Mouse, Network, SlidersHorizontal, Timer,
};

type Tab = "tweaks" | "ram" | "dns";

export function PcBoost() {
  const tab = (useUi((s) => s.subTab.boost) as Tab | undefined) ?? "tweaks";
  const setSubTab = useUi((s) => s.setSubTab);
  return (
    <div>
      <PageHeader title="PC Boost" subtitle="System tweaks, memory cleaning and DNS, each one explained, measured and reversible." tier={0} />
      <div className="mb-6">
        <GlassSegmented<Tab>
          role="tabs"
          label="PC Boost sections"
          value={tab}
          onChange={(v) => setSubTab("boost", v)}
          options={[
            { value: "tweaks", label: "System Tweaks", icon: <SlidersHorizontal size={16} /> },
            { value: "ram", label: "RAM Cleaner", icon: <MemoryStick size={16} /> },
            { value: "dns", label: "DNS Optimizer", icon: <Globe size={16} /> },
          ]}
        />
      </div>
      {tab === "tweaks" && <SystemTweaks />}
      {tab === "ram" && <RamCleaner />}
      {tab === "dns" && <DnsOptimizer />}
    </div>
  );
}

/* ======================= System Tweaks ======================= */

const RISK_TONE = { safe: "ok", moderate: "warn", advanced: "danger" } as const;
const RISK_LABEL = { safe: "Safe", moderate: "Moderate", advanced: "Advanced" };
const EVIDENCE_LABEL = { documented: "Documented", community: "Community-reported", experimental: "Experimental" };
const EVIDENCE_HELP = {
  documented: "Microsoft documents this setting and what it does.",
  community: "Widely used and reported to help by the community, but not formally documented as a performance setting.",
  experimental: "The effect is uncertain or depends on the game. Measure it in Benchmark Lab.",
};

function SystemTweaks() {
  const [tweaks, setTweaks] = useState<Tweak[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [preset, setPreset] = useState<TweakPreset>("custom");
  const [sheet, setSheet] = useState<Tweak | null>(null);
  const [preview, setPreview] = useState<{ title: string; changes: string[] } | null>(null);
  const dryRun = useSettings((s) => s.dryRun);
  const setSetting = useSettings((s) => s.set);
  const toast = useUi((s) => s.toast);
  const logActivity = useUi((s) => s.logActivity);

  const load = () => void backend.getTweaks().then(setTweaks);
  useEffect(load, []);

  const toggle = async (t: Tweak, on: boolean) => {
    setBusy(t.id);
    const r = await backend.setTweak(t.id, on, dryRun);
    setBusy(null);
    if (!r.ok) {
      toast({ tone: "error", title: `Could not change “${t.title}”`, body: r.error });
      return;
    }
    if (dryRun) {
      setPreview({ title: `Dry run: ${on ? "apply" : "revert"} “${t.title}”`, changes: r.changes });
      return;
    }
    setPreset("custom");
    logActivity(`${on ? "Applied" : "Reverted"}: ${t.title}`);
    if (t.requiresReboot) toast({ tone: "info", title: "Reboot required", body: `“${t.title}” takes effect after a restart.` });
    load();
  };

  const applyPreset = async (p: TweakPreset) => {
    setPreset(p);
    if (p === "custom") return;
    setBusy("preset");
    const r = await backend.applyPreset(p, dryRun);
    setBusy(null);
    if (dryRun) {
      setPreview({ title: `Dry run: ${p === "default" ? "Windows defaults" : "High performance"} preset`, changes: r.changes });
      return;
    }
    toast({ tone: "ok", title: p === "default" ? "Windows defaults restored" : "High performance preset applied", body: `${r.changes.length} values changed. Each one is in the change log.` });
    logActivity(`Preset applied: ${p}`);
    load();
  };

  const grouped = useMemo(() => {
    const order: Tweak["group"][] = ["game", "system", "input", "power", "network"];
    const names = { game: "Games", system: "System", input: "Input", power: "Power", network: "Network" };
    return order.map((g) => ({ id: g, name: names[g], items: tweaks.filter((t) => t.group === g) })).filter((g) => g.items.length);
  }, [tweaks]);

  return (
    <div>
      <Glass className="mb-5 flex flex-wrap items-center justify-between gap-4 p-4" variant="clear">
        <div className="flex flex-wrap items-center gap-3">
          <span className="font-medium text-t1">Preset</span>
          <GlassSegmented<TweakPreset>
            label="Tweak preset"
            value={preset}
            onChange={(v) => void applyPreset(v)}
            options={[
              { value: "default", label: "Default" },
              { value: "high", label: "High performance" },
              { value: "custom", label: "Custom" },
            ]}
          />
          <GlassTooltip text="Default restores Windows' own values. High performance applies a curated set of safe, explainable tweaks. Custom is whatever you choose below." />
        </div>
        <label className="flex items-center gap-3">
          <span className="text-t1">Dry run</span>
          <GlassTooltip text="Shows exactly what would change without changing anything." />
          <GlassToggle label="Dry run" checked={dryRun} onChange={(v) => setSetting("dryRun", v)} />
        </label>
      </Glass>

      {grouped.map((g) => (
        <section key={g.id} className="mb-6">
          <h2 className="otx-section-title mb-3">{g.name}</h2>
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2 2xl:grid-cols-3">
            {g.items.map((t) => {
              const Icon = ICONS[t.icon] ?? SlidersHorizontal;
              const warn = t.notRecommendedIf.some((r) => r.matches);
              return (
                <Glass key={t.id} className={`flex flex-col p-5 ${t.supported ? "" : "opacity-70"}`}>
                  <div className="flex items-start gap-3">
                    <div className="glass glass--clear flex h-11 w-11 shrink-0 items-center justify-center" style={{ ["--radius" as string]: "14px" }}>
                      <Icon size={20} className="text-t1" aria-hidden />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <h3 className="font-body text-[15px] font-semibold tracking-normal text-t1">{t.title}</h3>
                        {t.beta && <GlassBadge tone="beta">Beta</GlassBadge>}
                        {t.inHighPerformance && <GlassBadge tone="recommended">Recommended</GlassBadge>}
                      </div>
                      <p className="mt-1 line-clamp-2 text-[13px] text-t2">{t.description}</p>
                    </div>
                    <GlassToggle
                      label={t.title}
                      checked={t.enabled}
                      loading={busy === t.id || busy === "preset"}
                      disabled={!t.supported}
                      onChange={(v) => void toggle(t, v)}
                    />
                  </div>
                  <div className="mt-4 flex flex-wrap items-center gap-1.5">
                    <GlassBadge tone={RISK_TONE[t.risk]}>{RISK_LABEL[t.risk]}</GlassBadge>
                    <GlassBadge tone={t.evidence === "documented" ? "info" : t.evidence === "community" ? "neutral" : "violet"} title={EVIDENCE_HELP[t.evidence]}>
                      {EVIDENCE_LABEL[t.evidence]}
                    </GlassBadge>
                    {t.requiresReboot && <GlassBadge tone="warn">Reboot required</GlassBadge>}
                    <span className="flex-1" />
                    {(t.notRecommendedIf.length > 0 || !t.supported || t.conflictNote) && (
                      <button type="button" onClick={() => setSheet(t)} className={`inline-flex items-center gap-1 text-[13px] underline-offset-2 hover:underline ${warn ? "text-warn" : "text-t2"}`}>
                        {warn && <AlertTriangle size={13} aria-hidden />}
                        {t.supported ? "Not recommended if…" : "Why unavailable?"}
                      </button>
                    )}
                  </div>
                </Glass>
              );
            })}
          </div>
        </section>
      ))}

      <p className="text-[13px] text-t2">
        Never included: turning off Defender, the firewall, UAC, Memory Integrity/VBS, Secure Boot, or Windows Update for good. Many games and anti-cheats need them.
      </p>

      <GlassModal open={sheet !== null} onClose={() => setSheet(null)} title={sheet?.title ?? ""}>
        {sheet && (
          <div className="space-y-4">
            {!sheet.supported && <p className="text-warn">{sheet.unsupportedReason}</p>}
            {sheet.notRecommendedIf.length > 0 && (
              <div>
                <div className="mb-2 font-medium text-t1">Not recommended if… (checked against this PC)</div>
                <ul className="space-y-2">
                  {sheet.notRecommendedIf.map((r) => (
                    <li key={r.label} className="flex items-center gap-2">
                      <GlassBadge tone={r.matches ? "warn" : "ok"}>{r.matches ? "Applies to this PC" : "Not this PC"}</GlassBadge>
                      <span className="text-t1">{r.label}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}
            {sheet.conflictNote && (
              <div>
                <div className="mb-1 font-medium text-t1">Known conflict and how it is handled</div>
                <p>{sheet.conflictNote}</p>
              </div>
            )}
            <p className="text-[13px]">Evidence: {EVIDENCE_LABEL[sheet.evidence]}. {EVIDENCE_HELP[sheet.evidence]}</p>
          </div>
        )}
      </GlassModal>

      <GlassModal open={preview !== null} onClose={() => setPreview(null)} title={preview?.title ?? ""} footer={<GlassButton onClick={() => setPreview(null)}>Close</GlassButton>}>
        {preview && (preview.changes.length === 0 ? <p>Nothing would change.</p> : <ul className="num list-disc space-y-1.5 pl-5 text-[13px] text-t1">{preview.changes.map((c, i) => <li key={i}>{c}</li>)}</ul>)}
        <p className="mt-4 text-[13px]">Nothing was changed. Turn off Dry run to apply.</p>
      </GlassModal>
    </div>
  );
}

/* ======================= RAM Cleaner ======================= */

const METHODS: { id: RamMethod; label: string; help: string; smart: boolean }[] = [
  { id: "memoryPriority", label: "Lower memory priority of background apps", help: "Windows trims low-priority pages first when the game needs memory. Gentle and safe.", smart: true },
  { id: "trimWorkingSets", label: "Trim working sets of allowed background apps", help: "Moves idle pages of background apps out of RAM. They page back in if the app needs them.", smart: true },
  { id: "purgeStandby", label: "Purge the standby list", help: "Empties cached file data. Raises 'free' memory but can cause extra disk reads later. Only used under real pressure in Smart mode.", smart: false },
  { id: "flushModified", label: "Flush the modified page list", help: "Writes changed pages to disk so they can be reused.", smart: false },
  { id: "trimFileCache", label: "Trim the system file cache", help: "Shrinks the cache Windows keeps for recently used files.", smart: false },
];

function RamCleaner() {
  const s = useSettings();
  const toast = useUi((x) => x.toast);
  const logActivity = useUi((x) => x.logActivity);
  const [status, setStatus] = useState<RamStatus | null>(null);
  const [history, setHistory] = useState<RamCleanResult[]>([]);
  const [manual, setManual] = useState<RamMethod[]>(["memoryPriority", "trimWorkingSets"]);
  const [cleaning, setCleaning] = useState(false);
  const [picker, setPicker] = useState(false);

  const refresh = () => {
    void backend.getRamStatus().then(setStatus);
    void backend.getRamHistory().then(setHistory);
  };
  useEffect(() => {
    refresh();
    const h = setInterval(() => void backend.getRamStatus().then(setStatus), 2000);
    return () => clearInterval(h);
  }, []);

  const methods = s.ramSmartMode ? METHODS.filter((m) => m.smart).map((m) => m.id) : manual;

  const clean = async () => {
    setCleaning(true);
    const r = await backend.cleanRam(methods, "manual");
    setCleaning(false);
    toast({
      tone: r.freedMB > 0 ? "ok" : "info",
      title: r.freedMB > 0 ? `${r.freedMB} MB more memory available` : "No measurable change",
      body: `Measured available memory: ${formatGB(r.availableBeforeMB)} → ${formatGB(r.availableAfterMB)}`,
    });
    logActivity(`RAM cleaned: ${r.freedMB} MB`);
    refresh();
  };

  const usedPct = status ? (status.usedMB / status.totalMB) * 100 : 0;
  const needs = status?.status === "needs-cleaning";

  return (
    <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1fr_1.1fr]">
      <div className="space-y-5">
        <Panel title="Memory" help="Used = in use by apps and Windows. Free here means available: memory Windows can give to a program right away, including the standby cache.">
          {status ? (
            <div className="flex flex-col items-center gap-6 sm:flex-row">
              <GlassGauge value={usedPct} color={needs ? "var(--rgb-amber)" : "var(--rgb-green)"} label="% used" />
              <div className="grid flex-1 grid-cols-3 gap-4">
                <Stat label="Used" value={formatGB(status.usedMB)} />
                <Stat label="Free" value={formatGB(status.availableMB)} color="var(--accent-green)" />
                <Stat label="Total" value={formatGB(status.totalMB, 0)} />
                <div className="col-span-3">
                  <div className="flex items-center gap-2">
                    <GlassBadge tone={needs ? "warn" : "ok"}>{needs ? "Needs cleaning" : "Healthy"}</GlassBadge>
                    <span className="text-[13px] text-t2">{status.reason}</span>
                  </div>
                  <div className="mt-3">
                    <GlassButton kind="primary" size="lg" icon={<Eraser size={18} />} loading={cleaning} onClick={() => void clean()}>
                      Clean
                    </GlassButton>
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <div className="h-40 text-t2">Measuring…</div>
          )}
          <p className="mt-4 text-[13px] text-t2">
            Honest note: when plenty of memory is free, cleaning gains little. It helps most on 4 to 8 GB PCs right before a game loads. Smart mode only does more when pressure is real.
          </p>
        </Panel>

        <Panel title="Cleaning methods">
          <ToggleRow label="Smart mode" badge={<GlassBadge tone="recommended">Recommended</GlassBadge>} hint="Gentle methods by default; standby purge only under real pressure." checked={s.ramSmartMode} onChange={(v) => s.set("ramSmartMode", v)} />
          <Divider />
          <ul className="mt-3 space-y-2.5">
            {METHODS.map((m) => {
              const checked = s.ramSmartMode ? m.smart : manual.includes(m.id);
              return (
                <li key={m.id} className="flex items-center gap-3">
                  <input
                    id={`m-${m.id}`}
                    type="checkbox"
                    className="otx-check"
                    checked={checked}
                    disabled={s.ramSmartMode}
                    onChange={(e) => setManual((xs) => (e.target.checked ? [...xs, m.id] : xs.filter((x) => x !== m.id)))}
                  />
                  <label htmlFor={`m-${m.id}`} className="flex-1 text-t1">
                    {m.label}
                  </label>
                  <GlassTooltip text={m.help} />
                </li>
              );
            })}
          </ul>
          <p className="mt-4 text-[13px] text-t2">Never touched: the game, protected system processes, anti-cheat services, audio services and apps on your allow list.</p>
        </Panel>
      </div>

      <div className="space-y-5">
        <Panel title="Routine">
          <ToggleRow label="Automate cleanups" checked={s.ramAutoClean} onChange={(v) => s.set("ramAutoClean", v)} />
          <div className={`space-y-1 pl-1 ${s.ramAutoClean ? "" : "pointer-events-none opacity-50"}`} aria-disabled={!s.ramAutoClean}>
            <CheckRow label="When a game connection starts or a game is detected" checked={s.ramOnGameStart} onChange={(v) => s.set("ramOnGameStart", v)} />
            <CheckRow label="When status changes to “Needs cleaning”" checked={s.ramOnStatus} onChange={(v) => s.set("ramOnStatus", v)} />
            <div className="flex items-center gap-3 py-1.5">
              <input id="ram-sched" type="checkbox" className="otx-check" checked={s.ramScheduleOn} onChange={(e) => s.set("ramScheduleOn", e.target.checked)} />
              <label htmlFor="ram-sched" className="text-t1">
                Every
              </label>
              <div className="w-56">
                <GlassSlider label="Hours between cleanups" min={1} max={24} value={s.ramEveryHours} onChange={(v) => s.set("ramEveryHours", v)} format={(v) => `${v} h`} disabled={!s.ramScheduleOn} />
              </div>
            </div>
          </div>
          <p className="mt-3 text-[13px] text-t2">
            Before-launch cleaning is guaranteed when you start a game with Play in the Library or through a watched launcher (Steam, Epic, Riot, Battle.net, EA, Ubisoft). Other launches are cleaned within a few seconds of the game starting.
          </p>
        </Panel>

        <Panel
          title="Keep your essential apps untouched"
          right={
            <GlassButton size="sm" icon={<Plus size={14} />} onClick={() => setPicker(true)}>
              Add app
            </GlassButton>
          }
        >
          {s.ramAllowList.length === 0 ? (
            <Empty>No apps on the allow list.</Empty>
          ) : (
            <ul className="flex flex-wrap gap-2">
              {s.ramAllowList.map((a) => (
                <li key={a} className="glass glass--clear flex h-9 items-center gap-2 pl-3 pr-1.5" style={{ ["--radius" as string]: "999px" }}>
                  <span className="num text-[13px]">{a}</span>
                  <button type="button" aria-label={`Remove ${a}`} onClick={() => s.set("ramAllowList", s.ramAllowList.filter((x) => x !== a))} className="inline-flex h-6 w-6 items-center justify-center rounded-full text-t2 hover:bg-white/10 hover:text-t1">
                    <Trash2 size={13} aria-hidden />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </Panel>

        <Panel title="History">
          {history.length === 0 ? (
            <Empty>No cleanups yet.</Empty>
          ) : (
            <ul className="divide-y divide-white/[0.06]">
              {history.slice(0, 8).map((h) => (
                <li key={h.id} className="flex items-center justify-between py-2 text-sm">
                  <span className="text-t1">{h.freedMB} MB freed</span>
                  <span className="text-t2">
                    {h.trigger} · {timeAgo(h.at)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Panel>
      </div>

      <ProcessPicker
        open={picker}
        onClose={() => setPicker(false)}
        onPick={(exe) => {
          if (!s.ramAllowList.includes(exe)) s.set("ramAllowList", [...s.ramAllowList, exe]);
          setPicker(false);
        }}
      />
    </div>
  );
}

function CheckRow({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  const id = `cr-${label.replace(/\W+/g, "-")}`;
  return (
    <div className="flex items-center gap-3 py-1.5">
      <input id={id} type="checkbox" className="otx-check" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <label htmlFor={id} className="text-t1">
        {label}
      </label>
    </div>
  );
}

export function ProcessPicker({ open, onClose, onPick }: { open: boolean; onClose: () => void; onPick: (exe: string) => void }) {
  const [procs, setProcs] = useState<ProcessGroup[]>([]);
  const [q, setQ] = useState("");
  useEffect(() => {
    if (open) void backend.getProcesses().then(setProcs);
  }, [open]);
  const list = procs.filter((p) => !p.protected && (p.name + p.exe).toLowerCase().includes(q.toLowerCase()));
  return (
    <GlassModal open={open} onClose={onClose} title="Add an app">
      <input className="otx-input mb-3" type="search" placeholder="Filter running apps" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Filter running apps" />
      <ul className="max-h-72 space-y-1 overflow-y-auto">
        {list.map((p) => (
          <li key={p.id}>
            <button type="button" onClick={() => onPick(p.exe)} className="flex w-full items-center justify-between rounded-[12px] px-3 py-2 text-left hover:bg-white/[0.07]">
              <span className="text-t1">{p.name}</span>
              <span className="num text-[13px] text-t2">{p.exe}</span>
            </button>
          </li>
        ))}
      </ul>
      <p className="mt-3 text-[13px]">A file picker for apps that are not running arrives with the Core service (Phase 2).</p>
    </GlassModal>
  );
}

/* ======================= DNS Optimizer ======================= */

function DnsOptimizer() {
  const toast = useUi((x) => x.toast);
  const logActivity = useUi((x) => x.logActivity);
  const [servers, setServers] = useState<DnsServer[]>([]);
  const [results, setResults] = useState<DnsResult[]>([]);
  const [progress, setProgress] = useState<number | null>(null);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [active, setActive] = useState<string | null>(null);
  const [applying, setApplying] = useState<string | null>(null);
  const [addOpen, setAddOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newIp, setNewIp] = useState("");

  useEffect(() => {
    void backend.getDnsServers().then(setServers);
    void backend.getActiveDns().then(setActive);
  }, []);

  const run = async () => {
    setProgress(0);
    const r = await backend.benchmarkDns((d, t) => setProgress(d / t));
    setResults(r);
    setUpdatedAt(Date.now());
    setProgress(null);
  };

  const ranked = useMemo(() => {
    const byId = new Map(results.map((r) => [r.serverId, r]));
    return servers
      .map((s) => ({ s, r: byId.get(s.id) }))
      .sort((a, b) => {
        const am = a.r && a.r.failures < a.r.samples ? a.r.medianMs : Infinity;
        const bm = b.r && b.r.failures < b.r.samples ? b.r.medianMs : Infinity;
        return am - bm;
      });
  }, [servers, results]);

  const fastest = ranked[0]?.r && ranked[0].r.failures < ranked[0].r.samples ? ranked[0].s.id : null;

  const choose = async (s: DnsServer) => {
    setApplying(s.id);
    await backend.applyDns(s.id);
    setApplying(null);
    setActive(s.id);
    toast({ tone: "ok", title: `DNS set to ${s.name}`, body: "Applied to the active adapter (IPv4 and IPv6), cache flushed and verified. The previous setting was backed up." });
    logActivity(`DNS set to ${s.name}`);
  };

  const ipValid = isValidIPv4(newIp);

  return (
    <div className="space-y-5">
      <Glass className="flex flex-wrap items-center justify-between gap-4 p-4" variant="clear">
        <div className="text-t2">
          Real DNS queries for uncached names, 10 samples per server. Median, jitter and failures are reported.
          {updatedAt && <span className="ml-2 text-t1">Updated {timeAgo(updatedAt)}</span>}
        </div>
        <div className="flex gap-2">
          <GlassButton size="sm" icon={<Plus size={14} />} onClick={() => setAddOpen(true)}>
            Add server
          </GlassButton>
          <GlassButton
            size="sm"
            kind="ghost"
            onClick={() =>
              void backend.resetDns().then(() => {
                setActive(null);
                toast({ tone: "ok", title: "Windows default DNS restored" });
              })
            }
          >
            Reset Windows default
          </GlassButton>
          <GlassButton kind="primary" size="sm" icon={<RefreshCw size={14} />} loading={progress !== null} onClick={() => void run()}>
            Run benchmark
          </GlassButton>
        </div>
      </Glass>

      {progress !== null && (
        <div className="otx-progress" role="progressbar" aria-valuenow={Math.round(progress * 100)} aria-valuemin={0} aria-valuemax={100}>
          <span style={{ width: `${progress * 100}%` }} />
        </div>
      )}

      <Glass className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-white/[0.08] text-left text-[12px] uppercase tracking-wider text-t2">
              <th className="px-5 py-3 font-semibold">#</th>
              <th className="py-3 font-semibold">Server</th>
              <th className="py-3 font-semibold">Addresses</th>
              <th className="py-3 text-right font-semibold">Median</th>
              <th className="py-3 text-right font-semibold">Jitter</th>
              <th className="py-3 text-right font-semibold">Failures</th>
              <th className="px-5 py-3" />
            </tr>
          </thead>
          <tbody>
            {ranked.map(({ s, r }, i) => {
              const failed = r && r.failures >= r.samples;
              return (
                <tr key={s.id} className="border-b border-white/[0.05] last:border-0 hover:bg-white/[0.035]">
                  <td className="num px-5 py-3 text-t2">{r ? i + 1 : "-"}</td>
                  <td className="py-3">
                    <div className="flex flex-wrap items-center gap-1.5 text-t1">
                      {s.name}
                      {s.id === fastest && (
                        <GlassBadge tone="ok">
                          <Trophy size={11} aria-hidden /> Fastest
                        </GlassBadge>
                      )}
                      {s.filtering && <GlassBadge tone="info" title="Blocks ad and tracker domains. Some sites or game services may behave differently.">Filtering</GlassBadge>}
                      {s.custom && <GlassBadge tone="neutral">Custom</GlassBadge>}
                      {active === s.id && <GlassBadge tone="ok">In use</GlassBadge>}
                    </div>
                  </td>
                  <td className="num py-3 text-[13px] text-t2">{s.v4.join(", ")}</td>
                  <td className="num py-3 text-right text-t1">{r ? (failed ? "-" : `${r.medianMs.toFixed(0)} ms`) : "-"}</td>
                  <td className="num py-3 text-right text-t2">{r && !failed ? `${r.jitterMs.toFixed(0)} ms` : "-"}</td>
                  <td className={`num py-3 text-right ${r && r.failures ? "text-warn" : "text-t2"}`}>{r ? `${r.failures}/${r.samples}` : "-"}</td>
                  <td className="px-5 py-3 text-right">
                    {!s.isp && (
                      <GlassButton size="sm" kind={active === s.id ? "ghost" : "secondary"} disabled={active === s.id || failed} loading={applying === s.id} onClick={() => void choose(s)}>
                        {active === s.id ? "Chosen" : "Choose"}
                      </GlassButton>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </Glass>

      <p className="text-[13px] text-t2">
        DNS speed affects how fast connections start, not your in-game ping. DNS-over-HTTPS is offered only where Windows supports it (Windows 11). Verisign's public resolver availability is unverified; if it fails every query it is shown as failed rather than hidden.
      </p>

      <GlassModal
        open={addOpen}
        onClose={() => setAddOpen(false)}
        title="Add a DNS server"
        footer={
          <>
            <GlassButton onClick={() => setAddOpen(false)}>Cancel</GlassButton>
            <GlassButton
              kind="primary"
              disabled={!ipValid || !newName.trim()}
              onClick={() =>
                void backend.addDnsServer(newName.trim(), [newIp.trim()]).then((srv) => {
                  setServers((xs) => [...xs, srv]);
                  setAddOpen(false);
                  setNewName("");
                  setNewIp("");
                })
              }
            >
              Add
            </GlassButton>
          </>
        }
      >
        <div className="space-y-3">
          <Row label="Name">
            <input className="otx-input w-64" value={newName} onChange={(e) => setNewName(e.target.value)} aria-label="Server name" />
          </Row>
          <Row label="IPv4 address" hint={newIp && !ipValid ? "Enter an address like 1.1.1.1" : undefined}>
            <input className="otx-input num w-64" value={newIp} onChange={(e) => setNewIp(e.target.value)} aria-label="IPv4 address" aria-invalid={newIp !== "" && !ipValid} />
          </Row>
        </div>
      </GlassModal>
    </div>
  );
}
