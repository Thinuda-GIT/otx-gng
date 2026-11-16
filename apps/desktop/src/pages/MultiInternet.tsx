import { Cable, Smartphone, Wifi } from "lucide-react";
import { useEffect, useState } from "react";
import { backend } from "../backend";
import type { Adapter } from "../backend/types";
import { PageHeader, Panel, Stat, ToggleRow } from "../components/ui";
import { Glass, GlassBadge, TierBadge } from "../design-system";

const ICON = { ethernet: Cable, wifi: Wifi, tether: Smartphone };

export function MultiInternet() {
  const [adapters, setAdapters] = useState<Adapter[]>([]);
  const [failover, setFailover] = useState(true);
  const [preferEth, setPreferEth] = useState(true);

  useEffect(() => {
    const load = () => void backend.getAdapters().then(setAdapters);
    load();
    const h = setInterval(load, 3000);
    return () => clearInterval(h);
  }, []);

  return (
    <div>
      <PageHeader title="Multi Internet" subtitle="Watch every connection, prefer the best one, and switch automatically when one fails." tier={0} />
      <div className="mb-5 grid grid-cols-1 gap-4 lg:grid-cols-3">
        {adapters.map((a) => {
          const Icon = ICON[a.kind];
          return (
            <Glass key={a.id} className={`p-5 ${a.connected ? "" : "opacity-70"}`} tint={a.connected ? "30 242 160" : undefined}>
              <div className="flex items-center gap-3">
                <div className="glass glass--clear flex h-11 w-11 items-center justify-center" style={{ ["--radius" as string]: "14px" }}>
                  <Icon size={20} aria-hidden />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-t1">{a.name}</div>
                  <div className="flex gap-1.5">
                    <GlassBadge tone={a.connected ? "ok" : "neutral"}>{a.connected ? "Connected" : "Not connected"}</GlassBadge>
                    {a.preferred && <GlassBadge tone="info">Preferred</GlassBadge>}
                  </div>
                </div>
              </div>
              <div className="mt-4 grid grid-cols-3 gap-3">
                <Stat label="Ping" value={a.pingMs !== null ? `${a.pingMs.toFixed(0)} ms` : "-"} />
                <Stat label="Loss" value={a.lossPct !== null ? `${a.lossPct.toFixed(1)}%` : "-"} />
                <Stat label="Metric" value={a.metric} sub="lower wins" />
              </div>
            </Glass>
          );
        })}
      </div>

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <Panel title="Failover">
          <ToggleRow label="Automatic failover" hint="If the active connection loses its internet path, OTX.GNG lowers the interface metric of the next healthy one and notifies you." checked={failover} onChange={setFailover} />
          <ToggleRow label="Prefer Ethernet when plugged in" hint="Ethernet has less jitter than Wi-Fi." checked={preferEth} onChange={setPreferEth} />
          <p className="mt-2 text-[13px] text-t2">Interface metrics are restored when you turn this off or uninstall.</p>
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
