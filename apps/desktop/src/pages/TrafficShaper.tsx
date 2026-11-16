import { LifeBuoy, Plus, ShieldCheck, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { backend } from "../backend";
import type { RuleAction, ShaperRule } from "../backend/types";
import { PageHeader, Panel, Row, ToggleRow } from "../components/ui";
import { Glass, GlassBadge, GlassButton, GlassModal, GlassSegmented, GlassSelect, GlassToggle } from "../design-system";
import { restoreInternet, setFocus } from "../lib/actions";

type Template = "esports" | "streaming" | "downloads";

export function TrafficShaper() {
  const [rules, setRules] = useState<ShaperRule[]>([]);
  const [focus, setFocusState] = useState(false);
  const [template, setTemplate] = useState<Template>("esports");
  const [edit, setEdit] = useState<ShaperRule | null>(null);
  const [busy, setBusy] = useState(false);

  const load = () => void backend.getRules().then(setRules);
  useEffect(load, []);

  const toggleFocus = async (on: boolean) => {
    setBusy(true);
    await setFocus(on);
    setFocusState(on);
    setBusy(false);
  };

  return (
    <div>
      <PageHeader
        title="Traffic Shaper"
        subtitle="Keep your connection for the game. Blocking uses Windows Firewall rules; limits use Windows Policy-based QoS. No driver is needed."
        tier={0}
        actions={
          <GlassButton kind="destructive" icon={<LifeBuoy size={16} />} onClick={() => void restoreInternet().then(() => setFocusState(false))}>
            Restore normal internet
          </GlassButton>
        }
      />

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1fr_1.4fr]">
        <Glass className={`p-6 ${focus ? "edge-pulse" : ""}`} variant={focus ? "tinted" : "regular"} tint="30 242 160" style={{ ["--hud" as string]: "var(--rgb-green)" }}>
          <div className="flex items-start justify-between gap-4">
            <div>
              <h2 className="font-head text-3xl font-bold">Focus mode</h2>
              <p className="mt-1 text-t2">Only the game, its launcher and anti-cheat, voice chat and essential Windows services keep full network. Everything else is limited or paused for the session.</p>
            </div>
            <GlassToggle label="Focus mode" checked={focus} loading={busy} onChange={(v) => void toggleFocus(v)} />
          </div>
          <div className="mt-5">
            <div className="mb-2 text-[13px] text-t2">Template</div>
            <GlassSegmented<Template>
              label="Focus template"
              value={template}
              onChange={setTemplate}
              options={[
                { value: "esports", label: "Esports" },
                { value: "streaming", label: "Streaming" },
                { value: "downloads", label: "Downloads-friendly" },
              ]}
            />
            <p className="mt-2 text-[13px] text-t2">
              {template === "esports" && "Strictest: cloud sync and updates paused, browsers capped."}
              {template === "streaming" && "Keeps upload bandwidth for your streaming app; caps everything else."}
              {template === "downloads" && "Lets launcher downloads continue at a capped rate so ping stays stable."}
            </p>
          </div>
          <div className="mt-5 rounded-[14px] bg-white/[0.04] p-4 text-[13px] text-t2">
            <div className="mb-1 flex items-center gap-2 font-medium text-t1">
              <ShieldCheck size={15} aria-hidden /> Safety
            </div>
            DNS, DHCP, time sync, Windows security services and anti-cheat updaters are never blocked, and svchost.exe is never blocked as a whole. Rules are removed when the game exits, after a crash and at boot.
          </div>
        </Glass>

        <Panel
          title="Rules"
          help="What is blocked or limited right now. Rules marked 'while gaming' only apply during a game session."
          right={
            <GlassButton size="sm" icon={<Plus size={14} />} onClick={() => setEdit({ id: `r-${Date.now()}`, app: "", action: "limit", upKbps: 1000, downKbps: 5000, priority: "normal", onlyWhileGaming: true, essential: false })}>
              Add rule
            </GlassButton>
          }
        >
          <ul className="divide-y divide-white/[0.06]">
            {rules.map((r) => (
              <li key={r.id} className="flex items-center gap-3 py-3">
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="num text-t1">{r.app}</span>
                    <GlassBadge tone={r.action === "block" ? "danger" : r.action === "limit" ? "warn" : "ok"}>{r.action}</GlassBadge>
                    {r.essential && <GlassBadge tone="info">Essential</GlassBadge>}
                    {r.onlyWhileGaming && <GlassBadge tone="neutral">While gaming</GlassBadge>}
                  </div>
                  <div className="mt-0.5 text-[13px] text-t2">
                    {r.action === "limit" && `${r.upKbps ? `Upload ≤ ${r.upKbps} kbps` : ""}${r.downKbps ? ` · Download ≤ ${r.downKbps} kbps (needs the optional inbound limiter)` : ""}`}
                    {r.action === "block" && "Blocked by a Windows Firewall rule (group OTX.GNG)"}
                    {r.action === "allow" && "Full network"} · Priority {r.priority}
                  </div>
                </div>
                <GlassButton size="sm" kind="ghost" onClick={() => setEdit(r)}>
                  Edit
                </GlassButton>
                {!r.essential && (
                  <button type="button" aria-label={`Delete rule for ${r.app}`} className="inline-flex h-8 w-8 items-center justify-center rounded-full text-t2 hover:bg-white/10 hover:text-t1" onClick={() => void backend.deleteRule(r.id).then(load)}>
                    <Trash2 size={15} aria-hidden />
                  </button>
                )}
              </li>
            ))}
          </ul>
          <p className="mt-4 text-[13px] text-t2">
            Windows can only throttle outgoing traffic without a driver. Download limits need the optional inbound limiter (WinDivert), which is off by default and switched off automatically while anti-cheat games run.
          </p>
        </Panel>
      </div>

      <GlassModal
        open={edit !== null}
        onClose={() => setEdit(null)}
        title={edit?.app ? `Rule for ${edit.app}` : "New rule"}
        footer={
          <>
            <GlassButton onClick={() => setEdit(null)}>Cancel</GlassButton>
            <GlassButton
              kind="primary"
              disabled={!edit || !/^[\w .()-]+\.exe$/i.test(edit.app)}
              onClick={() => {
                if (!edit) return;
                void backend.saveRule(edit).then(() => {
                  setEdit(null);
                  load();
                });
              }}
            >
              Save
            </GlassButton>
          </>
        }
      >
        {edit && (
          <div>
            <Row label="App (exe name)">
              <input className="otx-input num w-60" value={edit.app} onChange={(e) => setEdit({ ...edit, app: e.target.value })} aria-label="App exe name" placeholder="example.exe" />
            </Row>
            <Row label="Action">
              <GlassSegmented<RuleAction> label="Rule action" size="sm" value={edit.action} onChange={(v) => setEdit({ ...edit, action: v })} options={[{ value: "allow", label: "Allow" }, { value: "limit", label: "Limit" }, { value: "block", label: "Block" }]} />
            </Row>
            {edit.action === "limit" && (
              <>
                <Row label="Upload limit (kbps)">
                  <input className="otx-input num w-32" type="number" min={64} value={edit.upKbps ?? 0} onChange={(e) => setEdit({ ...edit, upKbps: Math.max(0, Number(e.target.value)) })} aria-label="Upload limit" />
                </Row>
                <Row label="Download limit (kbps)" hint="Needs the optional inbound limiter.">
                  <input className="otx-input num w-32" type="number" min={0} value={edit.downKbps ?? 0} onChange={(e) => setEdit({ ...edit, downKbps: Math.max(0, Number(e.target.value)) })} aria-label="Download limit" />
                </Row>
              </>
            )}
            <Row label="Priority" hint="Marks outgoing packets with DSCP. Only helps if your router honours it (Wi-Fi WMM usually does).">
              <GlassSelect label="Priority" value={edit.priority} onChange={(v) => setEdit({ ...edit, priority: v })} options={[{ value: "high", label: "High" }, { value: "normal", label: "Normal" }, { value: "low", label: "Low" }]} />
            </Row>
            <ToggleRow label="Only while a game runs" checked={edit.onlyWhileGaming} onChange={(v) => setEdit({ ...edit, onlyWhileGaming: v })} />
          </div>
        )}
      </GlassModal>
    </div>
  );
}
