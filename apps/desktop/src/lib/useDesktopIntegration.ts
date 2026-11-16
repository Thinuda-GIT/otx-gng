/** Wires settings to the desktop shell (tray, close-to-tray, global hotkeys, autostart) and
 *  reacts to tray menu and hotkey events. Does nothing in the browser preview. */
import { useEffect } from "react";
import { onDesktopEvent, setAutostart, setCloseToTray, setShortcuts } from "../backend/tauri";
import { restoreInternet, revertEverything } from "./actions";
import { isTauri } from "./tauri";
import { useSettings } from "../store/settings";
import { useUi } from "../store/ui";
import { useRamRoutine } from "./useRamRoutine";

export function useDesktopIntegration(): void {
  useRamRoutine();
  const minimizeToTray = useSettings((s) => s.minimizeToTray);
  const restart = useSettings((s) => s.restartShortcut);
  const restore = useSettings((s) => s.restoreShortcut);
  const launchOnStartup = useSettings((s) => s.launchOnStartup);
  const firstRunDone = useSettings((s) => s.firstRunDone);

  useEffect(() => {
    if (isTauri()) void setCloseToTray(minimizeToTray);
  }, [minimizeToTray]);

  useEffect(() => {
    if (!isTauri()) return;
    void setShortcuts([
      ["restartConnections", restart],
      ["restoreInternet", restore],
    ]).then((failed) => {
      if (failed.length)
        useUi.getState().toast({ tone: "warn", title: "Shortcut unavailable", body: `${failed.join(", ")} is already used by another app or is not valid. Pick another in General Settings.` });
    });
  }, [restart, restore]);

  // Autostart is only touched after the first-run wizard asked the question.
  useEffect(() => {
    if (!isTauri() || !firstRunDone) return;
    setAutostart(launchOnStartup).catch((e: unknown) =>
      useUi.getState().toast({ tone: "error", title: "Could not change startup setting", body: String(e) }),
    );
  }, [launchOnStartup, firstRunDone]);

  useEffect(() => {
    if (!isTauri()) return;
    const offs: Promise<() => void>[] = [
      onDesktopEvent("tray-action", (a) => {
        const ui = useUi.getState();
        if (a === "boost") {
          ui.go("home");
          ui.toast({ tone: "info", title: "Ready to boost", body: "Select BOOST on Home. One-click boosting from the tray arrives with Game Mode (Phase 5)." });
        } else if (a === "restoreInternet") void restoreInternet();
        else if (a === "revertAll") void revertEverything();
      }),
      onDesktopEvent("shortcut", (a) => {
        if (a === "restoreInternet") void restoreInternet();
        else if (a === "restartConnections")
          useUi.getState().toast({ tone: "info", title: "Restart connections", body: "Relay connections arrive in Phase 9. Nothing to restart yet." });
      }),
    ];
    return () => offs.forEach((p) => void p.then((off) => off()));
  }, []);
}
