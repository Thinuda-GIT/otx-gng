/** "Automate cleanups" routine (desktop app only): cleans when status turns "Needs cleaning"
 *  and/or every N hours, at most once every 10 minutes, with Smart-mode methods. */
import { useEffect } from "react";
import { backend } from "../backend";
import { cleanRamNow } from "./actions";
import { isTauri } from "./tauri";
import { useSettings } from "../store/settings";

const MIN_GAP_MS = 10 * 60_000;

export function shouldClean(opts: {
  enabled: boolean;
  onStatus: boolean;
  scheduleOn: boolean;
  everyHours: number;
  needsCleaning: boolean;
  now: number;
  lastClean: number;
}): "status" | "schedule" | null {
  if (!opts.enabled || opts.now - opts.lastClean < MIN_GAP_MS) return null;
  if (opts.onStatus && opts.needsCleaning) return "status";
  if (opts.scheduleOn && opts.now - opts.lastClean >= opts.everyHours * 3_600_000) return "schedule";
  return null;
}

export function useRamRoutine(): void {
  const enabled = useSettings((s) => s.ramAutoClean);
  useEffect(() => {
    if (!isTauri() || !enabled) return;
    let last = Date.now(); // never clean immediately at startup
    const h = setInterval(() => {
      void backend.getRamStatus().then((st) => {
        const s = useSettings.getState();
        const why = shouldClean({
          enabled: s.ramAutoClean,
          onStatus: s.ramOnStatus,
          scheduleOn: s.ramScheduleOn,
          everyHours: s.ramEveryHours,
          needsCleaning: st.status === "needs-cleaning",
          now: Date.now(),
          lastClean: last,
        });
        if (why) {
          last = Date.now();
          void cleanRamNow(why).catch(() => undefined);
        }
      });
    }, 60_000);
    return () => clearInterval(h);
  }, [enabled]);
}
