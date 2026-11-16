import { Download, Keyboard, RotateCcw, Upload } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Divider, PageHeader, Panel, Row, ToggleRow } from "../components/ui";
import { GlassBadge, GlassButton, GlassModal, GlassSegmented, GlassSelect, GlassSlider, GlassToggle } from "../design-system";
import { revertEverything } from "../lib/actions";
import { downloadText } from "../lib/dom";
import { useLive } from "../store/live";
import { exportSettings, useSettings, type Accent } from "../store/settings";
import { useUi } from "../store/ui";
import { APP_VERSION } from "../shell/Sidebar";
import { CoreBadge } from "../shell/CoreBadge";
import type { CoreConnection, SystemSnapshot } from "../backend/types";

/** Builds "Shift+Ctrl+Q"-style strings. Requires at least one modifier so plain typing is never stolen. */
export function shortcutFromEvent(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "shiftKey" | "altKey" | "metaKey">): string | null {
  const k = e.key;
  if (["Control", "Shift", "Alt", "Meta"].includes(k)) return null;
  const mods = [e.shiftKey && "Shift", e.ctrlKey && "Ctrl", e.altKey && "Alt"].filter(Boolean) as string[];
  if (mods.length === 0) return null;
  const key = k.length === 1 ? k.toUpperCase() : /^F\d{1,2}$/.test(k) ? k : null;
  if (!key) return null;
  return [...mods, key].join("+");
}

const ACCENTS: { value: Accent; rgb: string }[] = [
  { value: "green", rgb: "30 242 160" },
  { value: "cyan", rgb: "45 226 255" },
  { value: "violet", rgb: "139 108 255" },
  { value: "red", rgb: "255 61 90" },
  { value: "amber", rgb: "255 176 32" },
];

export function Settings() {
  const { t } = useTranslation();
  const s = useSettings();
  const subTab = useUi((x) => x.subTab.settings);
  const toast = useUi((x) => x.toast);
  const glass = useLive((x) => x.glass);
  const gpuTier = useLive((x) => x.gpuTier);
  const system = useLive((x) => x.system);
  const core = useLive((x) => x.core);
  const [capturing, setCapturing] = useState<null | "restartShortcut" | "restoreShortcut">(null);
  const [resetOpen, setResetOpen] = useState(false);
  const [revertOpen, setRevertOpen] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (subTab) document.getElementById(`settings-${subTab}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, [subTab]);

  useEffect(() => {
    if (!capturing) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") return setCapturing(null);
      const sc = shortcutFromEvent(e);
      if (sc) {
        s.set(capturing, sc);
        setCapturing(null);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [capturing, s]);

  const onImport = async (f: File | undefined) => {
    if (!f) return;
    const r = s.importJson(await f.text());
    toast(r.ok ? { tone: "ok", title: `Imported ${r.applied} settings` } : { tone: "error", title: "Import failed", body: r.error });
  };

  return (
    <div>
      <PageHeader
        title={t("settings.title")}
        actions={
          <>
            <GlassSelect label={t("settings.language")} value={s.language} onChange={(v) => s.set("language", v)} options={[{ value: "en-US", label: "English (EN-US)" }]} />
            <GlassSelect
              label={t("settings.windowSize")}
              value={s.windowSize}
              onChange={(v) => s.set("windowSize", v)}
              options={[
                { value: "1280x720", label: "1280 × 720" },
                { value: "1440x810", label: "1440 × 810" },
                { value: "1600x900", label: "1600 × 900" },
                { value: "1920x1080", label: "1920 × 1080" },
              ]}
            />
          </>
        }
      />

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        <div className="space-y-5">
          <Panel title={t("settings.appBehavior")}>
            <ToggleRow label={t("settings.launchOnStartup")} hint={t("settings.launchOnStartupHint")} checked={s.launchOnStartup} onChange={(v) => s.set("launchOnStartup", v)} />
            <Divider />
            <ToggleRow label={t("settings.minimizeToTray")} hint={t("settings.minimizeToTrayHint")} checked={s.minimizeToTray} onChange={(v) => s.set("minimizeToTray", v)} />
          </Panel>

          <Panel title={t("settings.notifications")}>
            <ToggleRow label={t("settings.alertOnStart")} checked={s.alertOnSessionStart} onChange={(v) => s.set("alertOnSessionStart", v)} />
            <Divider />
            <ToggleRow label={t("settings.serverUnavailable")} badge={<GlassBadge tone="recommended">{t("common.recommended")}</GlassBadge>} checked={s.serverUnavailableAlerts} onChange={(v) => s.set("serverUnavailableAlerts", v)} />
          </Panel>

          <Panel title={t("settings.optimization")}>
            <ToggleRow label="Start Game Mode automatically" hint="When a game from your Library (or a known game) starts, apply its profile; everything reverts when it closes. Needs the Core service." checked={s.autoGameMode} onChange={(v) => s.set("autoGameMode", v)} />
            <Divider />
            <ToggleRow label={t("settings.realtime")} help={t("settings.realtimeHint")} badge={<GlassBadge tone="beta">{t("common.beta")}</GlassBadge>} checked={s.realtimeOptimization} onChange={(v) => s.set("realtimeOptimization", v)} />
            <Divider />
            <ToggleRow label={t("settings.smart")} help={t("settings.smartHint")} badge={<GlassBadge tone="recommended">{t("common.recommended")}</GlassBadge>} checked={s.smartOptimization} onChange={(v) => s.set("smartOptimization", v)} />
            <Divider />
            <ToggleRow
              label={t("settings.dualRoutes")}
              help={t("settings.dualRoutesHint")}
              hint={t("settings.dualRoutesDisabled")}
              badge={<GlassBadge tone="recommended">{t("common.recommended")}</GlassBadge>}
              checked={s.dualRoutes}
              onChange={(v) => s.set("dualRoutes", v)}
              disabled
            />
            <Divider />
            <Row label={t("settings.accuracy")} help={t("settings.accuracyHint")}>
              <GlassSegmented label={t("settings.accuracy")} size="sm" value={s.accuracy} onChange={(v) => s.set("accuracy", v)} options={[{ value: "low", label: "Low" }, { value: "medium", label: "Medium" }, { value: "high", label: "High" }]} />
            </Row>
          </Panel>

          <Panel title={t("settings.advanced")}>
            <ToggleRow label={t("settings.hardware")} help={t("settings.hardwareHint")} checked={s.hardwareMonitoring} onChange={(v) => s.set("hardwareMonitoring", v)} />
            <Divider />
            <Row label={t("settings.shortcut")} hint={capturing === "restartShortcut" ? t("settings.shortcutHint") : undefined}>
              <GlassButton size="sm" icon={<Keyboard size={14} />} onClick={() => setCapturing("restartShortcut")} aria-live="polite">
                {capturing === "restartShortcut" ? "Press keys…" : s.restartShortcut}
              </GlassButton>
            </Row>
            <Divider />
            <Row label={t("settings.restoreShortcut")} hint={capturing === "restoreShortcut" ? t("settings.shortcutHint") : t("settings.restoreShortcutHint")}>
              <GlassButton size="sm" icon={<Keyboard size={14} />} onClick={() => setCapturing("restoreShortcut")} aria-live="polite">
                {capturing === "restoreShortcut" ? "Press keys…" : s.restoreShortcut}
              </GlassButton>
            </Row>
            <Divider />
            <Row label={t("settings.redirection")} help={t("settings.redirectionHint")}>
              <GlassSelect
                label={t("settings.redirection")}
                value={s.redirectionMethod}
                onChange={(v) => s.set("redirectionMethod", v)}
                options={[
                  { value: "auto", label: "Automatic" },
                  { value: "firewall-qos", label: "Firewall + QoS only (no driver)" },
                  { value: "wireguard", label: "WireGuard tunnel (Tier 2)" },
                  { value: "windivert", label: "Packet redirect (optional driver)" },
                ]}
              />
            </Row>
            <Divider />
            <Row label={t("settings.winNet")} hint={t("settings.winNetHint")}>
              <GlassButton size="sm" kind="destructive" onClick={() => setResetOpen(true)}>
                {t("settings.resetNet")}
              </GlassButton>
            </Row>
          </Panel>
        </div>

        <div className="space-y-5">
          <Panel title={t("settings.appearance")}>
            <Row label={t("settings.glassQuality")} help={t("settings.glassQualityHint")} hint={`Now: ${glass}${s.glassQuality === "auto" ? ` (GPU: ${gpuTier})` : ""}`}>
              <GlassSegmented label={t("settings.glassQuality")} size="sm" value={s.glassQuality} onChange={(v) => s.set("glassQuality", v)} options={[{ value: "auto", label: "Auto" }, { value: "full", label: "Full" }, { value: "lite", label: "Lite" }, { value: "off", label: "Off" }]} />
            </Row>
            <Divider />
            <Row label={t("settings.reduceTransparency")} hint="System follows the Windows Transparency effects setting.">
              <GlassSegmented label={t("settings.reduceTransparency")} size="sm" value={s.reduceTransparency} onChange={(v) => s.set("reduceTransparency", v)} options={[{ value: "system", label: "System" }, { value: "on", label: "On" }, { value: "off", label: "Off" }]} />
            </Row>
            <Divider />
            <Row label={t("settings.reduceMotion")} hint="System follows the Windows Animation effects setting.">
              <GlassSegmented label={t("settings.reduceMotion")} size="sm" value={s.reduceMotion} onChange={(v) => s.set("reduceMotion", v)} options={[{ value: "system", label: "System" }, { value: "on", label: "On" }, { value: "off", label: "Off" }]} />
            </Row>
            <Divider />
            <Row label={t("settings.accent")}>
              <div className="flex gap-2" role="radiogroup" aria-label={t("settings.accent")}>
                {ACCENTS.map((a) => (
                  <button
                    key={a.value}
                    type="button"
                    role="radio"
                    aria-checked={s.accent === a.value}
                    aria-label={a.value}
                    onClick={() => s.set("accent", a.value)}
                    className="h-7 w-7 rounded-full transition-transform hover:scale-110"
                    style={{ background: `rgb(${a.rgb})`, boxShadow: s.accent === a.value ? `0 0 0 2px #05060a, 0 0 0 4px rgb(${a.rgb}), 0 0 16px rgb(${a.rgb})` : "inset 0 0 0 1px rgb(255 255 255 / .3)" }}
                  />
                ))}
              </div>
            </Row>
            <Divider />
            <Row label={t("settings.uiScale")}>
              <div className="w-56">
                <GlassSlider label={t("settings.uiScale")} min={1} max={2} step={0.1} value={s.uiScale} onChange={(v) => s.set("uiScale", Math.round(v * 10) / 10)} format={(v) => `${Math.round(v * 100)}%`} />
              </div>
            </Row>
            <Divider />
            <ToggleRow label={t("settings.sounds")} hint="Off by default." checked={s.uiSounds} onChange={(v) => s.set("uiSounds", v)} />
          </Panel>

          <Panel title={t("settings.core")} help={t("settings.coreHelp")} right={<CoreBadge core={core} />}>
            <CoreDetails core={core} system={system} />
          </Panel>

          <Panel title={t("settings.compat")}>
            <div id="settings-compat" className="scroll-mt-28" />
            {system ? (
              <ul className="space-y-2">
                {system.info.antiCheats.map((a) => (
                  <li key={a.service} className="flex items-center justify-between gap-3">
                    <span className="text-t1">
                      {a.name} <span className="num text-[12px] text-t2">({a.service})</span>
                    </span>
                    <GlassBadge tone={a.running ? "warn" : "neutral"}>{a.running ? "Running: driver modules off" : "Installed, not running"}</GlassBadge>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-t2">Detecting…</p>
            )}
            <p className="mt-3 text-[13px] text-t2">
              OTX.GNG never injects into, reads or writes game memory. Optional driver modules (inbound limiter, sensors, relay tunnel) are switched off while a game with a kernel anti-cheat runs. No tool can guarantee how an anti-cheat will behave.
            </p>
          </Panel>

          <Panel title={t("settings.privacy")}>
            <ToggleRow label={t("settings.telemetry")} hint={t("settings.telemetryHint")} checked={s.telemetry} onChange={(v) => s.set("telemetry", v)} />
            <Divider />
            <ToggleRow label={t("settings.crash")} hint={t("settings.crashHint")} checked={s.crashReports} onChange={(v) => s.set("crashReports", v)} />
          </Panel>

          <Panel title={t("settings.backups")}>
            <div className="flex flex-wrap gap-2">
              <GlassButton size="sm" icon={<Download size={14} />} onClick={() => downloadText("otx-gng-settings.json", exportSettings())}>
                {t("settings.export")}
              </GlassButton>
              <GlassButton size="sm" icon={<Upload size={14} />} onClick={() => fileRef.current?.click()}>
                {t("settings.import")}
              </GlassButton>
              <input ref={fileRef} type="file" accept="application/json,.json" hidden onChange={(e) => void onImport(e.target.files?.[0]).finally(() => (e.target.value = ""))} />
              <GlassButton size="sm" kind="destructive" icon={<RotateCcw size={14} />} onClick={() => setRevertOpen(true)}>
                {t("settings.revertAll")}
              </GlassButton>
            </div>
            <p className="mt-3 text-[13px] text-t2">The audit log and system snapshots are kept by the Core service (Phase 2+). Settings export never includes personal data.</p>
          </Panel>

          <Panel title={t("settings.updates")}>
            <Row label={t("settings.channel")}>
              <GlassSegmented label={t("settings.channel")} size="sm" value={s.updateChannel} onChange={(v) => s.set("updateChannel", v)} options={[{ value: "stable", label: "Stable" }, { value: "beta", label: "Beta" }]} />
            </Row>
            <GlassButton size="sm" onClick={() => toast({ tone: "info", title: "You are on the development build", body: "Signed updates from GitHub Releases arrive with the first release." })}>
              {t("settings.checkUpdates")}
            </GlassButton>
          </Panel>

          <Panel title={t("settings.about")}>
            <div id="settings-about" className="scroll-mt-28" />
            <div className="flex items-center gap-4">
              <img src="/logo.svg" alt="" className="h-14 w-14" />
              <div>
                <div className="font-head text-2xl font-bold">OTX.GNG</div>
                <div className="text-t2">{t("brand.line")}</div>
                <div className="num text-[13px] text-t2">Version {APP_VERSION} · GPL-3.0-or-later · Free forever</div>
              </div>
            </div>
            <p className="mt-3 text-[13px] text-t2">Open-source licenses for every bundled component are listed in THIRD_PARTY_LICENSES.md.</p>
          </Panel>

          <Panel title={t("settings.dev")}>
            <div id="settings-dev" className="scroll-mt-28" />
            <Row label={t("settings.simulateGame")} hint={t("settings.simulateGameHint")}>
              <GlassToggle label={t("settings.simulateGame")} checked={s.simulateGameId !== null} onChange={(v) => s.set("simulateGameId", v ? "valorant" : null)} />
            </Row>
          </Panel>
        </div>
      </div>

      <GlassModal
        open={resetOpen}
        onClose={() => setResetOpen(false)}
        title="Reset Windows network settings?"
        footer={
          <>
            <GlassButton onClick={() => setResetOpen(false)}>Cancel</GlassButton>
            <GlassButton
              kind="destructive"
              onClick={() => {
                setResetOpen(false);
                toast({ tone: "info", title: "Preview only", body: "The Core service performs the reset from Phase 7. Nothing was changed." });
              }}
            >
              Reset
            </GlassButton>
          </>
        }
      >
        This resets the TCP/IP stack, Winsock catalog and DNS cache. Your connection drops for a few seconds, VPN adapters may need reinstalling, and Windows asks for a restart.
      </GlassModal>

      <GlassModal
        open={revertOpen}
        onClose={() => setRevertOpen(false)}
        title="Revert everything?"
        footer={
          <>
            <GlassButton onClick={() => setRevertOpen(false)}>Cancel</GlassButton>
            <GlassButton
              kind="destructive"
              onClick={() => {
                setRevertOpen(false);
                void revertEverything();
              }}
            >
              Revert everything
            </GlassButton>
          </>
        }
      >
        Every change OTX.GNG made is restored to the value saved before it. Your OTX.GNG preferences stay as they are.
      </GlassModal>
    </div>
  );
}

function CoreDetails({ core, system }: { core: CoreConnection | null; system: SystemSnapshot | null }) {
  const st = core?.status;
  return (
    <div className="space-y-2 text-[13px] text-t2">
      {st ? (
        <ul className="space-y-1">
          <li>
            Version <span className="num text-t1">{st.coreVersion}</span> · protocol {st.protocol} · {st.mode === "service" ? "Windows service" : "developer console"}
            {st.elevated ? "" : " (not elevated)"}
          </li>
          <li>Safe Mode: {st.safeMode ? "on (privileged modules disabled)" : "off"} · Dry run: {st.dryRun ? "on" : "off"}</li>
          {st.recoveredChanges > 0 && <li className="text-warn">The watchdog undid {st.recoveredChanges} temporary change(s) left from the last session.</li>}
        </ul>
      ) : (
        <p>
          {core?.link === "notRunning"
            ? "The Core service is not installed or not running. Read-only features still work; changes to Windows need the Core. Reinstalling OTX.GNG installs it."
            : core?.message ?? "Checking…"}
        </p>
      )}
      {system && (
        <p>
          Hardware facts: {system.source === "core" ? "from the Core service" : system.source === "local" ? "detected by the app (Core not reachable)" : "preview data"}.
        </p>
      )}
    </div>
  );
}
