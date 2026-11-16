/** Command palette ranking: prefix > word-start > substring > subsequence. */

export interface Searchable {
  id: string;
  label: string;
  keywords?: string[];
}

function scoreText(text: string, q: string): number {
  const t = text.toLowerCase();
  if (t === q) return 100;
  if (t.startsWith(q)) return 80;
  if (t.split(/[\s\-/]+/).some((w) => w.startsWith(q))) return 60;
  if (t.includes(q)) return 40;
  // subsequence: every query char appears in order
  let i = 0;
  for (const c of t) if (c === q[i]) i++;
  return i === q.length ? 15 : 0;
}

export function rank<T extends Searchable>(items: T[], query: string): T[] {
  const q = query.trim().toLowerCase();
  if (!q) return items;
  return items
    .map((it) => ({
      it,
      s: Math.max(scoreText(it.label, q), ...(it.keywords ?? []).map((k) => scoreText(k, q) - 5)),
    }))
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .map((x) => x.it);
}
