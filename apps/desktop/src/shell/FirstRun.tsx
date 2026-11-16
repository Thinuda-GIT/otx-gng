/** First-run wizard: real hardware, what OTX.GNG will and will not touch, Core status, and the two
 *  questions the brief says to ASK (launch on startup, usage statistics). Nothing is changed on
 *  the PC by finishing the wizard; the only effect is the autostart entry if the user opts in. */
import { AnimatePresence, motion } from "motion/react";
import { Check, ChevronLeft, ChevronRight, ShieldCheck, X } from "lucide-react";
import { useState, type ReactNode } from "react";
import { Glass, GlassBadge, GlassButton, GlassToggle, spring } from "../design-system";
import { formatGB } from "../lib/format";
import { recommend } from "../lib/recommend";
import { useLive } from "../store/live";
import { useSettings } from "../store/settings";
import { CoreBadge } from "./CoreBadge";
import type { LinkKind } from "../backend/generated/LinkKind";

const LINK_LABEL: Record<LinkKind, string> = { ethernet: "Ethernet", wifi: "Wi-Fi", mobile: "Mobile broadband", ppp: "PPP (dial-up dongle or PPPoE)", other: "Other" };

const STEPS = ["Welcome", "Your PC", "Safety", "Your choices"] as const;

export function FirstRun({ open }: { open: boolean }) {
  const [step, setStep] = useState(0);
  const system = useLive((s) => s.system);
  const core = useLive((s) => s.core);
  const s = useSettings();

  const finish = () => s.set("firstRunDone", true);
  const rec = system ? recommend(system.info) : null;

  return (
    <AnimatePresence>
      {open && (
        <motion.div className="absolute inset-0 z-[95] flex items-center justify-center p-6" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
          <div className="absolute inset-0" style={{ background: "rgb(3 4 8 / .6)", backdropFilter: "blur(8px)" }} aria-hidden />
          <motion.div
            role="dialog"
            aria-modal="true"
            aria-label="Welcome to OTX.GNG"
            className="glass glass--thick relative flex max-h-[88%] w-full max-w-[760px] flex-col"
            style={{ ["--radius" as string]: "26px" }}
            initial={{ y: 30, scale: 0.97 }}
            animate={{ y: 0, scale: 1 }}
            transition={spring}
          >
            <div className="flex items-center gap-3 px-7 pt-6">
              <img src="/logo.svg" alt="" className="h-9 w-9" />
              <div className="flex-1">
                <div className="font-head text-xl font-bold">OTX.GNG setup</div>
                <ol className="mt-1 flex gap-3 text-[12px]" aria-label="Steps">
                  {STEPS.map((name, i) => (
                    <li key={name} className={i === step ? "text-t1" : "text-t2"} aria-current={i === step ? "step" : undefined}>
                      {i + 1}. {name}
                    </li>
                  ))}
                </ol>
              </div>
              <button type="button" onClick={finish} aria-label="Skip setup" className="inline-flex h-8 w-8 items-center justify-center rounded-full text-t2 hover:bg-white/10 hover:text-t1">
                <X size={18} aria-hidden />
              </button>
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto px-7 py-5">
              {step === 0 && (
                <div className="space-y-4 text-t2">
                  <p className="text-base text-t1">Free, honest game boosting. Everything OTX.GNG changes is measured, logged and can be undone in one click.</p>
                  <div className="grid gap-4 sm:grid-cols-2">
                    <List title="It may change (only when you ask)" items={["Process priority and efficiency mode of background apps", "Memory lists (standby, working sets)", "Power plan while a game runs", "Firewall and QoS rules tagged OTX.GNG", "DNS and network adapter settings", "Selected Windows settings you switch on"]} tone="ok" />
                    <List title="It never touches" items={["Your games' memory or files (except the opt-in Unreal config tuner, with backup)", "Anti-cheat software", "Defender, firewall on/off, UAC, Memory Integrity, Secure Boot", "Windows Update (only paused during a session if you choose)", "Your personal files"]} tone="danger" />
                  </div>
                </div>
              )}

              {step === 1 && (
                <div>
                  {!system ? (
                    <p className="text-t2">Detecting your hardware…</p>
                  ) : (
                    <>
                      <div className="mb-3 flex items-center gap-2">
                        <GlassBadge tone={system.source === "mock" ? "warn" : "ok"}>
                          {system.source === "core" ? "Detected by the Core service" : system.source === "local" ? "Detected by the app" : "Preview data (browser)"}
                        </GlassBadge>
                      </div>
                      <dl className="grid grid-cols-[150px_1fr] gap-x-4 gap-y-2.5 text-sm">
                        <Fact k="Windows" v={`${system.info.os.name} ${system.info.os.displayVersion ?? ""} (build ${system.info.os.build}.${system.info.os.ubr})${system.info.os.edition ? ` · ${system.info.os.edition}` : ""}`} />
                        <Fact k="CPU" v={`${system.info.cpu.model} · ${system.info.cpu.cores} cores / ${system.info.cpu.threads} threads${system.info.cpu.hybrid ? ` · ${system.info.cpu.performanceCores}P + ${system.info.cpu.efficiencyCores}E` : ""}`} />
                        <Fact k="Graphics" v={system.info.gpus.length ? system.info.gpus.map((g) => `${g.name}${g.integrated ? " (integrated)" : ` (${formatGB(g.dedicatedVramMb, 0)})`}`).join(" · ") : "Not detected"} />
                        <Fact k="Memory" v={system.info.ramMb ? formatGB(system.info.ramMb) : "Not detected"} />
                        <Fact k="System disk" v={system.info.disk ? `${system.info.disk.model} · ${system.info.disk.kind.toUpperCase()} (${system.info.disk.systemDrive})` : "Not detected"} />
                        <Fact k="Device" v={`${system.info.power.formFactor === "laptop" ? "Laptop" : "Desktop"}${system.info.power.batteryPercent !== null ? ` · battery ${system.info.power.batteryPercent}%${system.info.power.onBattery ? " (on battery)" : " (plugged in)"}` : ""}`} />
                        <Fact k="Network" v={system.info.network ? `${LINK_LABEL[system.info.network.kind]} · ${system.info.network.description} · ${system.info.network.linkMbps} Mbps link` : "No connected adapter with internet"} />
                        <Fact k="Anti-cheat" v={system.info.antiCheats.length ? system.info.antiCheats.map((a) => `${a.name}${a.running ? " (running)" : ""}`).join(" · ") : "None detected"} />
                      </dl>
                      {system.info.unavailable.length > 0 && (
                        <p className="mt-3 text-[13px] text-warn">Could not read: {system.info.unavailable.join("; ")}</p>
                      )}
                    </>
                  )}
                </div>
              )}

              {step === 2 && (
                <div className="space-y-4 text-t2">
                  <div className="flex items-center gap-2">
                    <span className="text-t1">Core service:</span> <CoreBadge core={core} />
                  </div>
                  <p>
                    The Core is a small Windows service that makes the changes you approve. It accepts only a fixed list of typed requests from this app, never commands or file paths. It writes every change to a journal before applying it, and it undoes temporary changes automatically after a crash or reboot.
                  </p>
                  <p className="flex items-start gap-2">
                    <ShieldCheck size={18} className="mt-0.5 shrink-0 text-ok" aria-hidden />
                    Before the first system change, OTX.GNG saves a snapshot of every value it will touch and asks Windows for a System Restore point. Restore points can be turned off on your PC; the snapshot works either way.
                  </p>
                  <p className="text-[13px]">Game scanning arrives with the Library (Phase 5) and the baseline network test with the Quality Test (Phase 7).</p>
                </div>
              )}

              {step === 3 && (
                <div className="space-y-5">
                  <Choice label="Start OTX.GNG with Windows" hint="Starts minimized to the tray so it can react when a game launches. You can change this any time." checked={s.launchOnStartup} onChange={(v) => s.set("launchOnStartup", v)} />
                  <Choice label="Share anonymous usage statistics" hint="Off by default. This version has no statistics server, so nothing is sent either way." checked={s.telemetry} onChange={(v) => s.set("telemetry", v)} />
                  {rec && (
                    <Glass className="p-4" variant="clear" radius={16}>
                      <div className="mb-2 font-medium text-t1">
                        Suggested tweak preset: <span className="text-ok">{rec.preset === "high" ? "High performance" : "Windows defaults"}</span>
                      </div>
                      <ul className="space-y-1 text-[13px] text-t2">
                        {rec.reasons.map((r) => (
                          <li key={r}>• {r}</li>
                        ))}
                        {rec.cautions.map((r) => (
                          <li key={r} className="text-warn">
                            • {r}
                          </li>
                        ))}
                      </ul>
                      <p className="mt-2 text-[13px] text-t2">Nothing is applied now. You choose the preset in PC Boost.</p>
                    </Glass>
                  )}
                </div>
              )}
            </div>

            <div className="flex items-center justify-between border-t border-white/[0.07] px-7 py-4">
              <GlassButton kind="ghost" icon={<ChevronLeft size={16} />} disabled={step === 0} onClick={() => setStep((x) => x - 1)}>
                Back
              </GlassButton>
              {step < STEPS.length - 1 ? (
                <GlassButton kind="primary" onClick={() => setStep((x) => x + 1)}>
                  Next <ChevronRight size={16} aria-hidden />
                </GlassButton>
              ) : (
                <GlassButton kind="primary" icon={<Check size={16} />} onClick={finish}>
                  Finish
                </GlassButton>
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function Fact({ k, v }: { k: string; v: ReactNode }) {
  return (
    <>
      <dt className="text-t2">{k}</dt>
      <dd className="m-0 text-t1">{v}</dd>
    </>
  );
}

function List({ title, items, tone }: { title: string; items: string[]; tone: "ok" | "danger" }) {
  return (
    <Glass className="p-4" variant="clear" radius={16}>
      <div className={`mb-2 font-medium ${tone === "ok" ? "text-ok" : "text-live"}`}>{title}</div>
      <ul className="space-y-1 text-[13px]">
        {items.map((i) => (
          <li key={i}>• {i}</li>
        ))}
      </ul>
    </Glass>
  );
}

function Choice({ label, hint, checked, onChange }: { label: string; hint: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="flex items-start justify-between gap-6">
      <div>
        <div className="font-medium text-t1">{label}</div>
        <div className="text-[13px] text-t2">{hint}</div>
      </div>
      <GlassToggle label={label} checked={checked} onChange={onChange} />
    </div>
  );
}
