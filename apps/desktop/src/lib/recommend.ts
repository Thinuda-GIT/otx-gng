/** First-run recommendations from real hardware facts. Pure and unit-tested. */
import type { SystemInfo } from "../backend/types";

export interface Recommendation {
  preset: "default" | "high";
  reasons: string[];
  cautions: string[];
}

export function recommend(info: SystemInfo): Recommendation {
  const reasons: string[] = [];
  const cautions: string[] = [];
  const laptop = info.power.formFactor === "laptop";
  const ramGb = info.ramMb / 1024;

  if (ramGb > 0 && ramGb <= 8.5) reasons.push(`${ramGb.toFixed(0)} GB of RAM: cleaning memory before games is likely to help.`);
  else if (ramGb > 8.5) reasons.push(`${ramGb.toFixed(0)} GB of RAM: memory cleaning will rarely matter, so Smart mode stays gentle.`);

  if (info.cpu.hybrid) reasons.push("Hybrid CPU: games will prefer performance cores through CPU Sets (no hard affinity).");
  if (!info.gpus.some((g) => !g.integrated) && info.gpus.length > 0) cautions.push("Integrated graphics only: glass effects use the lighter mode automatically.");
  if (info.disk?.kind === "hdd") cautions.push("Windows is on a hard disk: SysMain and Search indexing stay on, because turning them off can slow an HDD.");
  if (laptop) cautions.push("Laptop: high-performance power settings raise heat and battery use; they apply only while a game runs.");
  if (info.power.onBattery) cautions.push("Running on battery right now: boosting is most useful when plugged in.");
  if (info.network?.kind === "wifi") cautions.push("Wi-Fi connection: a cable usually lowers jitter more than any software tweak.");
  if (!info.os.supported) cautions.push(`Windows build ${info.os.build} is older than supported (needs Windows 10 22H2 or Windows 11). Some features will be disabled.`);

  const preset: Recommendation["preset"] = info.power.onBattery || !info.os.supported ? "default" : "high";
  return { preset, reasons, cautions };
}
