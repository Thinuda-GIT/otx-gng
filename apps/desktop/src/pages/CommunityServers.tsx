import { BookOpen, Radar, ShieldCheck, ShieldX, Star } from "lucide-react";
import { useEffect, useState } from "react";
import { backend } from "../backend";
import type { CommunityEntry } from "../backend/types";
import { PageHeader, Panel, Row } from "../components/ui";
import { GlassBadge, GlassButton, TierBadge } from "../design-system";
import { useUi } from "../store/ui";

/** host:port, where host is a DNS name or IPv4 address and port is 1-65535. */
export function isValidRelayAddress(s: string): boolean {
  const m = /^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?(\.[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?)*):(\d{1,5})$/i.exec(s.trim());
  if (!m) return false;
  const port = Number(m[5]);
  return port >= 1 && port <= 65535;
}

export function CommunityServers() {
  const [items, setItems] = useState<CommunityEntry[]>([]);
  const [pinging, setPinging] = useState<string | null>(null);
  const [addr, setAddr] = useState("");
  const [token, setToken] = useState("");
  const toast = useUi((s) => s.toast);

  useEffect(() => void backend.getCommunity().then(setItems), []);

  const ping = async (id: string) => {
    setPinging(id);
    const ms = await backend.pingCommunity(id);
    setPinging(null);
    setItems((xs) => xs.map((x) => (x.id === id ? { ...x, pingMs: ms } : x)));
  };

  const addrOk = isValidRelayAddress(addr);
  const tokenOk = token.trim().length >= 16;

  return (
    <div>
      <PageHeader
        title="Community Servers"
        subtitle="DNS lists and relays published by the community as signed files on GitHub. Every entry is tested from your PC; lists with a bad signature are rejected."
        tier={1}
      />
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-[1.4fr_1fr]">
        <Panel title="Lists and relays" help="Ratings come only from your own measurements. There are no accounts and no cloud votes.">
          <ul className="divide-y divide-white/[0.06]">
            {items.map((c) => (
              <li key={c.id} className={`flex items-center gap-3 py-3 ${c.signatureOk ? "" : "opacity-60"}`}>
                <button
                  type="button"
                  aria-label={c.favorite ? `Remove ${c.name} from favorites` : `Add ${c.name} to favorites`}
                  aria-pressed={c.favorite}
                  disabled={!c.signatureOk}
                  onClick={() => setItems((xs) => xs.map((x) => (x.id === c.id ? { ...x, favorite: !x.favorite } : x)))}
                  className="text-t2 hover:text-warn disabled:cursor-not-allowed"
                >
                  <Star size={18} fill={c.favorite ? "currentColor" : "none"} className={c.favorite ? "text-warn" : ""} aria-hidden />
                </button>
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2 text-t1">
                    {c.name}
                    {c.kind === "relay" ? <TierBadge tier={2} /> : <TierBadge tier={1} />}
                    {c.signatureOk ? (
                      <GlassBadge tone="ok">
                        <ShieldCheck size={12} aria-hidden /> Signature OK
                      </GlassBadge>
                    ) : (
                      <GlassBadge tone="danger">
                        <ShieldX size={12} aria-hidden /> Rejected
                      </GlassBadge>
                    )}
                  </div>
                  <div className="num truncate text-[12px] text-t2">
                    {c.address} · {c.region} · {c.source}
                  </div>
                </div>
                <span className="num w-16 text-right text-t1">{c.pingMs ? `${c.pingMs.toFixed(0)} ms` : "-"}</span>
                {c.kind === "relay" && (
                  <GlassButton size="sm" icon={<Radar size={14} />} disabled={!c.signatureOk} loading={pinging === c.id} onClick={() => void ping(c.id)}>
                    Test
                  </GlassButton>
                )}
              </li>
            ))}
          </ul>
          <p className="mt-4 text-[13px] text-t2">Phase 1 shows example entries. The real lists are published and signed in Phase 9.</p>
        </Panel>

        <div className="space-y-5">
          <Panel title="Add your own relay">
            <Row label="Address" hint={addr && !addrOk ? "Use host:port, for example relay.example.org:51820" : undefined}>
              <input className="otx-input num w-56" value={addr} onChange={(e) => setAddr(e.target.value)} placeholder="relay.example.org:51820" aria-label="Relay address" aria-invalid={addr !== "" && !addrOk} />
            </Row>
            <Row label="Access token" hint={token && !tokenOk ? "Tokens are at least 16 characters." : "Given to you by the relay's host."}>
              <input className="otx-input num w-56" type="password" autoComplete="off" value={token} onChange={(e) => setToken(e.target.value)} aria-label="Relay access token" aria-invalid={token !== "" && !tokenOk} />
            </Row>
            <div className="mt-3 flex justify-end">
              <GlassButton
                kind="primary"
                disabled={!addrOk || !tokenOk}
                onClick={() => {
                  toast({ tone: "info", title: "Relay saved for later", body: "Relay connections arrive in Phase 9. The token is stored only on this PC." });
                  setAddr("");
                  setToken("");
                }}
              >
                Add relay
              </GlassButton>
            </div>
          </Panel>
          <Panel title="Host a relay">
            <p className="text-t2">
              Anyone can run the open-source OTX Relay on a free-tier server or a home PC with one command. Relays only forward game traffic to allow-listed game server ranges, need an access token, are rate-limited and never log traffic content.
            </p>
            <div className="mt-3">
              <GlassButton icon={<BookOpen size={16} />} onClick={() => toast({ tone: "info", title: "Guide: docs/RELAY_HOSTING.md", body: "Published with Phase 9." })}>
                Read the hosting guide
              </GlassButton>
            </div>
          </Panel>
        </div>
      </div>
    </div>
  );
}
