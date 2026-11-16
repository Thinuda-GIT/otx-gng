import { Cable, Globe, Shield, Smartphone, Wifi } from "lucide-react";
import { useEffect, useState } from "react";
import { backend } from "../backend";
import type { Adapter } from "../backend/types";
import { PageHeader, Panel, Stat, ToggleRow } from "../components/ui";
import { Glass, GlassBadge, GlassButton, TierBadge } from "../design-system";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

const ICON: Record<Adapter["kind"], typeof Cable> = { ethernet: Cable, wifi: Wifi, tether: Smartphone, ppp: Smartphone, vpn: Shield, other: Globe };
const KIND_LABEL: Record<Adapter["kind"], string> = { ethernet: "Ethernet", wifi: "Wi-Fi", tether: "Mobile", ppp: "PPP / dial-up", vpn: "VPN", other: "Other" };

export function MultiInternet() {
  const [adapters, setAdapters] = useState<Adapter[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const failover = useSettings((s) => s.multiFailover);
  const setSetting = useSettings((s) => s.set);
  const toast = useUi((s) => s.toast);
  const logActivity = useUi((s) => s.logActivity);

  const load = () => void backend.getAdapters().then(setAdapters).catch(() => undefined);
  useEffect(() => {
    load();
    const h = setInterval(load, 10_000);
    return () => clearInterval(h);
  }, []);

  const prefer = async (id: string | null) => {
    setBusy(id ?? "auto");
    try {
      const r = await backend.preferAdapter(id);
      toast({ tone: "ok", title: r.dryRun ? "Dry run: nothing changed" : id ? "Connection preference set" : "Back to automatic", body: r.changes.join("; ") });
      if (!r.dryRun) logActivity(r.changes[0] ?? "Connection preference changed");
      load();
    } catch (e) {
      toast({ tone: "error", title: "Not changed", body: String(e) });
    } finally {
      setBusy(null);
    }
  };

  const live = adapters.filter((a) => a.connected);

  return (
    <div>
      <PageHeader title="Multi Internet" subtitle="Every connection, measured separately (each health check is sent from that connection's own address). Prefer one, and fail over when it drops." tier={0} />
      <div className="mb-5 grid grid-cols-1 gap-4 lg:grid-cols-3">
        {adapters.map((a) => {
          const Icon = ICON[a.kind];
          return (
            <Glass key={a.id} className={`p-5 ${a.connected ? "" : "opacity-70"}`} tint={a.connected ? "30 242 160" : undefined}>
              <div className="flex items-center gap-3">
                <div className="glass glass--clear flex h-11 w-11 shrink-0 items-center justify-center" style={{ ["--radius" as string]: "14px" }}>
                  <Icon size={20} aria-hidden />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-t1" title={a.name}>
                    {a.name}
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    <GlassBadge tone="neutral">{KIND_LABEL[a.kind]}</GlassBadge>
                    <GlassBadge tone={a.connected ? "ok" : "neutral"}>{a.connected ? "Internet" : "No internet"}</GlassBadge>
                    {a.preferred && <GlassBadge tone="info">In use</GlassBadge>}
                  </div>
                </div>
              </div>
              <div className="mt-4 grid grid-cols-3 gap-3">
                <Stat label="Ping" value={a.pingMs !== null ? `${a.pingMs.toFixed(0)} ms` : "-"} />
                <Stat label="Loss" value={a.lossPct !== null ? `${a.lossPct.toFixed(0)}%` : "-"} />
                <Stat label="Metric" value={a.metric} sub="lower wins" />
              </div>
              {a.connected && !a.preferred && a.kind !== "vpn" && (
                <GlassButton className="mt-4" size="sm" loading={busy === a.id} onClick={() => void prefer(a.id)}>
                  Prefer this connection
                </GlassButton>
              )}
            </Glass>
          );
        })}
      </div>

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <Panel title="Failover">
          <ToggleRow
            label="Automatic failover"
            hint="When the connection in use stops reaching the internet three checks in a row and another connection works, OTX.GNG prefers the working one and tells you. Needs the Core service."
            checked={failover}
            onChange={(v) => setSetting("multiFailover", v)}
          />
          <p className="mt-2 text-[13px] text-t2">
            {live.length < 2 ? "Only one connection reaches the internet right now, so there is nothing to fail over to." : `${live.length} connections reach the internet.`}
          </p>
          <GlassButton className="mt-3" size="sm" kind="ghost" loading={busy === "auto"} onClick={() => void prefer(null)}>
            Back to automatic (Windows decides)
          </GlassButton>
        </Panel>
        <Panel title={<span className="flex items-center gap-2">Dual-path stability <TierBadge tier={2} /></span>}>
          <p className="text-t2">
            Windows cannot merge two internet connections into one faster connection on its own. What is possible with a relay: send each game packet over two connections to the same relay, which keeps the first copy and drops the duplicate. That hides packet loss on one path; it does not add speed.
          </p>
          <p className="mt-3 text-[13px] text-t2">Needs a relay from Community Servers and a second connected adapter.</p>
        </Panel>
      </div>
    </div>
  );
}
