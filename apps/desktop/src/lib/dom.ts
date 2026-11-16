/** Small DOM helpers for root-level CSS variables and attributes (no React re-render needed). */

const root = () => document.documentElement;

export function setRootVar(name: string, value: string | null): void {
  if (value === null) root().style.removeProperty(name);
  else root().style.setProperty(name, value);
}

export function setRootAttr(name: string, value: string | null): void {
  if (value === null) root().removeAttribute(name);
  else root().setAttribute(name, value);
}

/** Per-game backdrop tint ("r g b"), or null to clear. */
export function setGameTint(rgb: string | null): void {
  setRootVar("--game-tint", rgb);
  setRootVar("--game-tint-o", rgb ? "0.32" : "0");
}

export function downloadText(filename: string, text: string, type = "application/json"): void {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export function mediaMatches(q: string): boolean {
  try {
    return window.matchMedia(q).matches;
  } catch {
    return false;
  }
}
