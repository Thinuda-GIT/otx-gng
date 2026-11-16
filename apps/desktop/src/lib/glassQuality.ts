/** Decides the effective glass tier. Pure function so it can be unit-tested.
 *  Rule from the brief: Auto = Full when no game is running and the GPU is capable;
 *  Lite on integrated GPUs or battery saver; Off whenever a game is running. */

export type GlassQuality = "auto" | "full" | "lite" | "off";
export type EffectiveGlass = "full" | "lite" | "off";
export type GpuTier = "discrete" | "integrated" | "software" | "unknown";

export interface GlassInputs {
  setting: GlassQuality;
  gameRunning: boolean;
  gpuTier: GpuTier;
  batterySaver: boolean;
  reduceTransparency: boolean;
}

export function effectiveGlass(i: GlassInputs): EffectiveGlass {
  if (i.reduceTransparency) return "off";
  if (i.setting !== "auto") return i.setting;
  if (i.gameRunning) return "off";
  if (i.gpuTier === "software") return "off";
  if (i.batterySaver || i.gpuTier === "integrated") return "lite";
  return "full";
}

/** Best-effort GPU tier from the WebGL renderer string. This is a browser-side signal only;
 *  the Core service reports the real adapter list from Phase 2 on. */
export function classifyRenderer(renderer: string | null | undefined): GpuTier {
  if (!renderer) return "unknown";
  const r = renderer.toLowerCase();
  if (/swiftshader|llvmpipe|basic render/.test(r)) return "software";
  if (/nvidia|geforce|rtx|gtx|quadro|radeon rx|radeon pro|arc\(tm\) a\d|arc a\d/.test(r)) return "discrete";
  if (/intel|uhd|iris|radeon\(tm\) graphics|radeon graphics|vega \d+/.test(r)) return "integrated";
  return "unknown";
}

/** GPU tier from the real adapter list (Core or local DXGI detection). */
export function tierFromGpus(gpus: readonly { integrated: boolean }[]): GpuTier {
  if (gpus.length === 0) return "software";
  return gpus.some((g) => !g.integrated) ? "discrete" : "integrated";
}

export function detectGpuTier(): GpuTier {
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl");
    if (!gl) return "software";
    const ext = gl.getExtension("WEBGL_debug_renderer_info");
    const renderer = ext ? (gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) as string) : null;
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return classifyRenderer(renderer);
  } catch {
    return "unknown";
  }
}

/** True when CSS backdrop-filter accepts an SVG filter reference (Chromium / WebView2). */
export function supportsLens(): boolean {
  try {
    return typeof CSS !== "undefined" && CSS.supports("backdrop-filter", "url(#otx-lens)");
  } catch {
    return false;
  }
}
