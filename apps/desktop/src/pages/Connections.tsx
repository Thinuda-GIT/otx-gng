import { Route, Waypoints } from "lucide-react";
import { useEffect, useState } from "react";
import { backend } from "../backend";
import type { Game, Hop } from "../backend/types";
import { PageHeader, Panel, Stat } from "../components/ui";
import { Glass, GlassBadge, GlassButton, GlassSelect, LiveChart, TierBadge } from "../design-system";
import { useLive } from "../store/live";
import { useUi } from "../store/ui";

const REGIONS = ["EU West (Frankfurt)", "EU North (Stockholm)", "US East (Virginia)", "US West (Oregon)", "Asia (Singapore)", "Middle East (Bahrain)"];

const SEGMENT: Record<Hop["segment"], { label: string; rgb: string }> = {
  local: { label: "Your network", rgb: "30 242 160" },
  isp: { label: "Your ISP", rgb: "45 226 255" },
  transit: { label: "Transit", rgb: "139 108 255" },
  destination: { label: "Game network", rgb: "255 176 32" },
};

/** Plain-language diagnosis from hop data. Loss on a middle hop that does not continue to the
 *  destination is ICMP de-prioritisation by that router, not real loss. */
function diagnose(hops: Hop[]): string {
  const dest = hops[hops.length - 1];
  if (!dest) return "";
  const first = hops[0]!;
  if (first.avgMs > 10) return "Most of the delay starts inside your home network. Wi-Fi quality or a busy router is the likely cause: try Ethernet or the Tune-up tab.";
  const ispJump = hops.find((h) => h.segment === "isp" && h.avgMs - first.avgMs > 25);
  if (ispJump) return "Delay jumps at your ISP's first hops. That is outside this PC; a relay route or contacting your ISP are the honest options.";
  if (dest.lossPct > 1) return "Packets are lost all the way to the game server. The problem is outside this PC.";
  const fakeLoss = hops.find((h) => h.lossPct > 0 && h !== dest);
  return `Your path looks healthy: ${dest.avgMs.toFixed(0)} ms to the server.${fakeLoss ? ` Hop ${fakeLoss.n} shows ${fakeLoss.lossPct}% loss, but later hops do not, so that router is only deprioritising test replies. It is not real loss.` : ""}`;
}

export function Connections() {
  const [games, setGames] = useState<Game[]>([]);
  const [game, setGame] = useState("valorant");
  const [region, setRegion] = useState(REGIONS[0]!);
  const [target, setTarget] = useState("auto");
  const [hops, setHops] = useState<Hop[] | null>(null);
  const [tracing, setTracing] = useState(false);
  const history = useLive((s) => s.history);
  const latest = useLive((s) => s.latest);
  const go = useUi((s) => s.go);

  useEffect(() => void backend.getGames().then(setGames), []);

  const trace = async () => {
    setTracing(true);
    try {
      setHops(await backend.traceroute(target));
    } catch (e) {
      useUi.getState().toast({ tone: "error", title: "Route analysis failed", body: String(e) });
    }
    setTracing(false);
  };

  const maxMs = Math.max(1, ...(hops ?? []).map((h) => h.avgMs));

  return (
    <div>
      <PageHeader title="Connections" subtitle="Route Lab: measure your path to a game server, find where the delay comes from, and fix what can be fixed on this PC." tier={0} />
      <Glass className="mb-5 flex flex-wrap items-end gap-4 p-5" variant="clear">
        <div>
          <div className="mb-1 text-[13px] text-t2">Game</div>
          <GlassSelect label="Game" value={game} onChange={setGame} options={games.map((g) => ({ value: g.id, label: g.name }))} />
        </div>
        <div>
          <div className="mb-1 text-[13px] text-t2">Server ("auto" = the running game's server, else 1.1.1.1)</div>
          <input className="otx-input mb-3 w-72" value={target} onChange={(e) => setTarget(e.target.value)} aria-label="Server to analyze" />
          <div className="mb-1 text-[13px] text-t2">Region (for community relays)</div>
          <GlassSelect label="Region" value={region} onChange={setRegion} options={REGIONS.map((r) => ({ value: r, label: r }))} />
        </div>
        <GlassButton kind="primary" icon={<Route size={16} />} loading={tracing} onClick={() => void trace()}>
          Analyze route
        </GlassButton>
      </Glass>

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1fr_1.3fr]">
        <Panel title="Live quality" help="Ping, jitter and loss to the detected or profile-listed server, measured once per second.">
          <div className="mb-4 grid grid-cols-3 gap-4">
            <Stat label="Ping" value={latest?.ping ? `${latest.ping.ms.toFixed(0)} ms` : "-"} color="var(--accent-cyan)" />
            <Stat label="Jitter" value={latest?.ping ? `${latest.ping.jitter.toFixed(1)} ms` : "-"} />
            <Stat label="Loss" value={latest?.ping ? `${latest.ping.loss.toFixed(1)}%` : "-"} />
          </div>
          <LiveChart ariaLabel="Ping over time" height={150} series={[{ values: history.map((m) => m.ping?.ms ?? 0), color: "45 226 255", fill: true }]} />
        </Panel>

        <Panel title="Route analysis" help="Traceroute with network names from an offline database. The slowest step is highlighted.">
          {!hops ? (
            <p className="text-t2">Choose a game and region, then select Analyze route.</p>
          ) : (
            <>
              <ol className="space-y-2">
                {hops.map((h, i) => {
                  const prev = i ? hops[i - 1]!.avgMs : 0;
                  const delta = h.avgMs - prev;
                  const worst = delta === Math.max(...hops.map((x, j) => x.avgMs - (j ? hops[j - 1]!.avgMs : 0)));
                  const seg = SEGMENT[h.segment];
                  return (
                    <li key={h.n} className={`grid grid-cols-[28px_minmax(140px,1fr)_minmax(120px,1fr)_70px] items-center gap-3 rounded-[12px] px-2 py-1.5 ${worst ? "bg-[rgb(255_176_32/0.1)] shadow-[inset_0_0_0_1px_rgb(255_176_32/0.35)]" : ""}`}>
                      <span className="num text-t2">{h.n}</span>
                      <div className="min-w-0">
                        <div className="num truncate text-[13px] text-t1">{h.host !== "-" ? h.host : h.ip}</div>
                        <div className="truncate text-[12px] text-t2">
                          {h.asn} · <span style={{ color: `rgb(${seg.rgb})` }}>{seg.label}</span>
                        </div>
                      </div>
                      <div className="otx-progress">
                        <span style={{ width: `${(h.avgMs / maxMs) * 100}%`, background: `rgb(${seg.rgb})` }} />
                      </div>
                      <span className="num text-right text-[13px]">
                        {h.avgMs.toFixed(0)} ms{h.lossPct > 0 && <span className="block text-warn">{h.lossPct}% loss</span>}
                      </span>
                    </li>
                  );
                })}
              </ol>
              <p className="mt-4 rounded-[14px] bg-white/[0.04] p-4 text-t1">{diagnose(hops)}</p>
            </>
          )}
        </Panel>
      </div>

      <Glass className="mt-5 p-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-center gap-3">
            <Waypoints size={22} className="text-gpu" aria-hidden />
            <h2 className="otx-section-title">Relay routes</h2>
            <TierBadge tier={2} />
          </div>
          <GlassButton onClick={() => go("community")}>Add a relay</GlassButton>
        </div>
        <p className="mt-3 max-w-3xl text-t2">
          When the delay is outside your PC, the only real fix is a different path. Community-run OTX Relays can carry your game traffic over a better route, and dual routes can send each packet over two paths to hide packet loss. No relay is configured yet, so automatic best route and before/after graphs are unavailable.
        </p>
        <div className="mt-3 flex gap-2">
          <GlassBadge tone="neutral">Automatic best route: needs a relay</GlassBadge>
          <GlassBadge tone="neutral">Dual routes: needs a relay</GlassBadge>
        </div>
      </Glass>
    </div>
  );
}
