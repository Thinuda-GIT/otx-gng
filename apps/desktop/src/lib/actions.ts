/** App-wide actions shared by the tray (later), the command palette, hotkeys and pages. */
import { backend } from "../backend";
import type { RamMethod } from "../backend/types";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";

const ui = () => useUi.getState();

export function smartRamMethods(): RamMethod[] {
  return useSettings.getState().ramSmartMode
    ? ["memoryPriority", "trimWorkingSets"]
    : ["memoryPriority", "trimWorkingSets", "purgeStandby", "flushModified"];
}

export async function cleanRamNow(trigger: "manual" | "boost" | "status" | "schedule" = "manual") {
  const r = await backend.cleanRam(smartRamMethods(), trigger);
  ui().toast({
    tone: r.freedMB > 0 ? "ok" : "info",
    title: r.freedMB > 0 ? `${r.freedMB} MB more memory available` : "No measurable change",
    body: `Available: ${(r.availableBeforeMB / 1024).toFixed(1)} GB → ${(r.availableAfterMB / 1024).toFixed(1)} GB (measured before and after)`,
  });
  ui().logActivity(`RAM cleaned (${trigger}): ${r.freedMB} MB`);
  return r;
}

export async function restoreInternet() {
  await backend.restoreInternet();
  ui().toast({ tone: "ok", title: "Normal internet restored", body: "All OTX.GNG firewall rules and QoS policies were removed." });
  ui().logActivity("Restored normal internet");
}

export async function revertEverything() {
  await backend.revertAll();
  ui().toast({ tone: "ok", title: "Everything reverted", body: "All OTX.GNG changes were undone from the saved snapshot." });
  ui().logActivity("Reverted everything to the snapshot");
}

export async function setFocus(on: boolean) {
  await backend.setFocusMode(on);
  ui().toast({ tone: on ? "ok" : "info", title: on ? "Focus mode on" : "Focus mode off" });
  ui().logActivity(on ? "Focus mode on" : "Focus mode off");
}
