/** Thin wrapper around the Tauri window API. Every call is a safe no-op in the browser preview,
 *  so the same UI runs under `npm run dev` and inside the Tauri window. */
import type { Window as TauriWindow } from "@tauri-apps/api/window";

export const isTauri = (): boolean => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

let cached: TauriWindow | null = null;
async function win(): Promise<TauriWindow | null> {
  if (!isTauri()) return null;
  if (!cached) {
    const mod = await import("@tauri-apps/api/window");
    cached = mod.getCurrentWindow();
  }
  return cached;
}

export async function minimizeWindow(): Promise<void> {
  await (await win())?.minimize();
}

export async function toggleMaximizeWindow(): Promise<boolean> {
  const w = await win();
  if (!w) return false;
  await w.toggleMaximize();
  return w.isMaximized();
}

export async function closeWindow(): Promise<void> {
  await (await win())?.close();
}

export async function isMaximized(): Promise<boolean> {
  return (await win())?.isMaximized() ?? false;
}

/** Applies a "1440x810"-style size and re-centres the window. */
export async function applyWindowSize(size: string): Promise<void> {
  const w = await win();
  if (!w) return;
  const m = /^(\d+)x(\d+)$/.exec(size);
  if (!m) return;
  const { LogicalSize } = await import("@tauri-apps/api/dpi");
  await w.setSize(new LogicalSize(Number(m[1]), Number(m[2])));
  await w.center();
}

/** Subscribes to window resize so the rounded corners can be dropped when maximised. */
export async function onResized(cb: () => void): Promise<() => void> {
  const w = await win();
  if (!w) return () => undefined;
  return w.onResized(cb);
}
