import { Play, Plus, ShieldAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { backend } from "../backend";
import type { Game, GameProfile } from "../backend/types";
import { Divider, GameTile, PageHeader, Row, ToggleRow } from "../components/ui";
import { Glass, GlassBadge, GlassButton, GlassModal, GlassSelect } from "../design-system";
import { setGameTint } from "../lib/dom";
import { timeAgo } from "../lib/format";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

const LAUNCHER: Record<Game["launcher"], string> = {
  steam: "Steam",
  epic: "Epic Games",
  riot: "Riot Client",
  battlenet: "Battle.net",
  xbox: "Xbox / Microsoft Store",
  ea: "EA app",
  ubisoft: "Ubisoft Connect",
  manual: "Added manually",
};

export function Library() {
  const [games, setGames] = useState<Game[]>([]);
  const [open, setOpen] = useState<Game | null>(null);
  const [launching, setLaunching] = useState<string | null>(null);
  const setSetting = useSettings((s) => s.set);
  const toast = useUi((s) => s.toast);
  const logActivity = useUi((s) => s.logActivity);
  const go = useUi((s) => s.go);

  useEffect(() => void backend.getGames().then(setGames), []);
  useEffect(() => () => setGameTint(null), []);

  const play = async (g: Game) => {
    setLaunching(g.id);
    await backend.boost(g.id, () => undefined);
    setLaunching(null);
    setSetting("simulateGameId", g.id); // preview: the "game" is now running
    toast({ tone: "ok", title: `${g.name} launched with its profile`, body: "Memory was cleaned before launch. In the real app the game starts through its launcher." });
    logActivity(`Played ${g.name} from Library`);
    go("home");
  };

  const saveProfile = async (g: Game, p: GameProfile) => {
    await backend.updateGameProfile(g.id, p);
    setGames((xs) => xs.map((x) => (x.id === g.id ? { ...x, profile: p } : x)));
    setOpen({ ...g, profile: p });
  };

  return (
    <div>
      <PageHeader
        title="Library"
        subtitle="Games found by reading launcher data (read-only). Icons are extracted from the games on your PC; OTX.GNG ships no game art."
        tier={0}
        actions={
          <GlassButton icon={<Plus size={16} />} onClick={() => toast({ tone: "info", title: "Manual add arrives with the Core service", body: "Phase 5 adds a file picker for any game exe." })}>
            Add game
          </GlassButton>
        }
      />
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2 xl:grid-cols-3">
        {games.map((g) => (
          <Glass
            key={g.id}
            className="group flex cursor-pointer flex-col gap-4 p-5 transition-transform hover:-translate-y-0.5"
            tint={g.tint}
            onMouseEnter={() => setGameTint(g.tint)}
            onFocus={() => setGameTint(g.tint)}
            onClick={() => setOpen(g)}
            onKeyDown={(e) => e.key === "Enter" && setOpen(g)}
            tabIndex={0}
            role="button"
            aria-label={`${g.name} profile`}
          >
            <div className="flex items-center gap-4">
              <GameTile name={g.name} tint={g.tint} />
              <div className="min-w-0 flex-1">
                <div className="truncate font-head text-xl font-bold">{g.name}</div>
                <div className="text-[13px] text-t2">
                  {LAUNCHER[g.launcher]} · {g.sizeGB.toFixed(1)} GB
                </div>
              </div>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {g.antiCheat && (
                <GlassBadge tone="neutral" title="Driver-based OTX.GNG modules are switched off automatically while this game runs.">
                  <ShieldAlert size={12} aria-hidden /> {g.antiCheat}
                </GlassBadge>
              )}
              {g.engine === "unreal" && <GlassBadge tone="beta">UE config tuner</GlassBadge>}
              <GlassBadge tone="info">Preset: {g.profile.tweakPreset === "high" ? "High performance" : g.profile.tweakPreset}</GlassBadge>
            </div>
            <div className="mt-auto flex items-center justify-between">
              <span className="text-[13px] text-t2">{g.lastPlayed ? `Played ${timeAgo(g.lastPlayed)}` : "Never played"}</span>
              <GlassButton
                kind="primary"
                size="sm"
                icon={<Play size={14} />}
                loading={launching === g.id}
                onClick={(e) => {
                  e.stopPropagation();
                  void play(g);
                }}
              >
                Play
              </GlassButton>
            </div>
          </Glass>
        ))}
      </div>

      <GlassModal open={open !== null} onClose={() => setOpen(null)} title={open ? `${open.name} profile` : ""} width={640}>
        {open && <ProfileEditor game={open} onChange={(p) => void saveProfile(open, p)} />}
      </GlassModal>
    </div>
  );
}

function ProfileEditor({ game, onChange }: { game: Game; onChange: (p: GameProfile) => void }) {
  const p = game.profile;
  const set = <K extends keyof GameProfile>(k: K, v: GameProfile[K]) => onChange({ ...p, [k]: v });
  return (
    <div>
      <p className="mb-2 num text-[12px] text-t2">{game.installPath}\{game.exe}</p>
      <div>
        <h3 className="otx-section-title mt-2">Applied before launch</h3>
        <Row label="Tweak preset">
          <GlassSelect label="Tweak preset" value={p.tweakPreset} onChange={(v) => set("tweakPreset", v)} options={[{ value: "default", label: "Windows defaults" }, { value: "high", label: "High performance" }, { value: "custom", label: "Custom" }]} />
        </Row>
        <Divider />
        <ToggleRow label="Clean RAM on launch" checked={p.ramCleanOnLaunch} onChange={(v) => set("ramCleanOnLaunch", v)} />
        <Divider />
        <Row label="CPU priority" hint="High at most. Realtime is never used: it can freeze input and audio.">
          <GlassSelect label="CPU priority" value={p.cpuPriority} onChange={(v) => set("cpuPriority", v)} options={[{ value: "normal", label: "Normal" }, { value: "above", label: "Above normal" }, { value: "high", label: "High" }]} />
        </Row>
        <Divider />
        <Row label="CPU cores" hint="CPU Sets prefer performance cores on hybrid CPUs. Hard affinity is only used when a benchmark shows it helps.">
          <GlassSelect label="CPU cores" value={p.cpuSets} onChange={(v) => set("cpuSets", v)} options={[{ value: "auto", label: "Automatic" }, { value: "performance-cores", label: "Prefer performance cores" }, { value: "off", label: "Windows decides" }]} />
        </Row>
        <Divider />
        <Row label="I/O priority">
          <GlassSelect label="I/O priority" value={p.ioPriority} onChange={(v) => set("ioPriority", v)} options={[{ value: "normal", label: "Normal" }, { value: "high", label: "High" }]} />
        </Row>
        <Divider />
        <Row label="GPU preference" badge={<GlassBadge tone="beta">Beta</GlassBadge>}>
          <GlassSelect label="GPU preference" value={p.gpuPreference} onChange={(v) => set("gpuPreference", v)} options={[{ value: "default", label: "Let Windows decide" }, { value: "power-saving", label: "Power saving" }, { value: "high-performance", label: "High performance" }]} />
        </Row>
        <Divider />
        <ToggleRow label="Disable fullscreen optimizations" hint="Per-game Windows compatibility flag. Helps some older games; most modern games are fine either way." checked={p.disableFullscreenOptimizations} onChange={(v) => set("disableFullscreenOptimizations", v)} />
        <Divider />
        <Row label="Network profile">
          <GlassSelect label="Network profile" value={p.networkProfile} onChange={(v) => set("networkProfile", v)} options={[{ value: "none", label: "None" }, { value: "focus", label: "Focus mode" }, { value: "esports", label: "Esports" }, { value: "streaming", label: "Streaming" }]} />
        </Row>
        <Divider />
        <Row label="Background apps calmed during play" hint={p.backgroundApps.join(", ") || "None"}>
          <span />
        </Row>
      </div>
      {game.antiCheat && <p className="mt-3 text-[13px]">{game.antiCheat} detected: OTX.GNG never touches the game's memory or files, and optional driver modules stay off while it runs.</p>}
    </div>
  );
}
