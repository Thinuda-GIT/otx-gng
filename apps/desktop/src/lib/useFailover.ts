/** Automatic failover (desktop app): every 15 s, if the connection in use fails three checks in a
 *  row and another connection works, prefer the working one. */
import { useEffect } from "react";
import { backend } from "../backend";
import type { Adapter } from "../backend/types";
import { isTauri } from "./tauri";
import { useLive } from "../store/live";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

/** Pure decision: returns the adapter to switch to, or null. */
export function failoverTarget(adapters: Adapter[], failedChecks: number): Adapter | null {
  const live = adapters.filter((a) => a.connected && a.kind !== "vpn");
  const current = live.find((a) => a.preferred);
  if (!current || failedChecks < 3) return null;
  return live.filter((a) => a.id !== current.id && a.lossPct !== null && a.lossPct < 100).sort((a, b) => (a.pingMs ?? 9999) - (b.pingMs ?? 9999))[0] ?? null;
}

export function useFailover(): void {
  const enabled = useSettings((s) => s.multiFailover);
  useEffect(() => {
    if (!isTauri() || !enabled) return;
    let failed = 0;
    const h = setInterval(() => {
      if (useLive.getState().core?.link !== "connected") return;
      void backend.getAdapters().then(async (adapters) => {
        const current = adapters.find((a) => a.preferred);
        failed = current && current.lossPct === 100 ? failed + 1 : 0;
        const to = failoverTarget(adapters, failed);
        if (!to) return;
        failed = 0;
        try {
          await backend.preferAdapter(to.id);
          useUi.getState().toast({ tone: "warn", title: `Switched to ${to.name}`, body: `${current?.name ?? "The previous connection"} stopped reaching the internet.` });
          useUi.getState().logActivity(`Failover to ${to.name}`);
        } catch {
          /* the Multi Internet page shows the reason when opened */
        }
      });
    }, 15_000);
    return () => clearInterval(h);
  }, [enabled]);
}
