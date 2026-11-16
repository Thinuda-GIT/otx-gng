/** Formatting helpers. All numbers shown in the UI go through these so units stay consistent. */

export function formatGB(mb: number, digits = 1): string {
  return `${(mb / 1024).toFixed(digits)} GB`;
}

export function formatMB(mb: number): string {
  return mb >= 1024 ? formatGB(mb) : `${Math.round(mb)} MB`;
}

export function formatRate(kbps: number): string {
  if (kbps >= 1000) return `${(kbps / 1000).toFixed(1)} Mbps`;
  return `${Math.round(kbps)} kbps`;
}

export function formatPct(v: number, digits = 0): string {
  return `${v.toFixed(digits)}%`;
}

export function formatMs(v: number, digits = 0): string {
  return `${v.toFixed(digits)} ms`;
}

/** "just now", "3 min ago", "2 h ago", "4 d ago" */
export function timeAgo(ts: number, now: number = Date.now()): string {
  const s = Math.max(0, Math.round((now - ts) / 1000));
  if (s < 45) return "just now";
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 48) return `${h} h ago`;
  return `${Math.round(h / 24)} d ago`;
}

export function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

/** Strict dotted-quad IPv4 validation (no leading zeros, each octet 0-255). */
export function isValidIPv4(s: string): boolean {
  const parts = s.trim().split(".");
  if (parts.length !== 4) return false;
  return parts.every((p) => /^(0|[1-9]\d{0,2})$/.test(p) && Number(p) <= 255);
}

/** Median of an array (NaN when empty). */
export function median(xs: readonly number[]): number {
  if (xs.length === 0) return Number.NaN;
  const s = [...xs].sort((a, b) => a - b);
  const mid = Math.floor(s.length / 2);
  return s.length % 2 ? s[mid]! : (s[mid - 1]! + s[mid]!) / 2;
}
