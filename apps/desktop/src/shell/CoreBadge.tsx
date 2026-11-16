import type { CoreConnection } from "../backend/types";
import { GlassBadge } from "../design-system";

/** One-line Core service state, shared by Settings, the wizard and the top bar. */
export function coreLabel(core: CoreConnection | null): { text: string; tone: "ok" | "warn" | "danger" | "neutral" } {
  if (!core) return { text: "Checking…", tone: "neutral" };
  switch (core.link) {
    case "connected": {
      const st = core.status;
      if (st?.safeMode) return { text: "Connected · Safe Mode", tone: "warn" };
      if (st?.mode === "console") return { text: "Connected · developer console", tone: "warn" };
      return { text: "Connected", tone: "ok" };
    }
    case "notRunning":
      return { text: "Not running", tone: "neutral" };
    case "untrusted":
      return { text: "Untrusted pipe: not connected", tone: "danger" };
    case "rejected":
      return { text: "Refused by the Core", tone: "danger" };
    default:
      return { text: "Error", tone: "danger" };
  }
}

export function CoreBadge({ core }: { core: CoreConnection | null }) {
  const l = coreLabel(core);
  return (
    <span title={core?.message ?? undefined}>
      <GlassBadge tone={l.tone}>{l.text}</GlassBadge>
    </span>
  );
}
