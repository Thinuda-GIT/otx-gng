/** Virtualized, sortable table with a sticky header (TanStack Virtual). */
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDown, ArrowUp } from "lucide-react";
import { useMemo, useRef, useState, type ReactNode } from "react";

export interface Column<T> {
  key: string;
  header: string;
  width: string; // CSS grid track, e.g. "minmax(180px,2fr)" or "90px"
  align?: "left" | "right";
  render: (row: T) => ReactNode;
  sortValue?: (row: T) => number | string;
}

export interface GlassTableProps<T> {
  rows: T[];
  columns: Column<T>[];
  rowKey: (row: T) => string;
  height: number;
  selectedKey?: string | null;
  onSelect?: (row: T) => void;
  initialSort?: { key: string; dir: "asc" | "desc" };
  label: string;
  rowHeight?: number;
}

export function GlassTable<T>({
  rows,
  columns,
  rowKey,
  height,
  selectedKey,
  onSelect,
  initialSort,
  label,
  rowHeight = 44,
}: GlassTableProps<T>) {
  const [sort, setSort] = useState(initialSort ?? null);
  const scroller = useRef<HTMLDivElement>(null);
  const template = columns.map((c) => c.width).join(" ");

  const sorted = useMemo(() => {
    if (!sort) return rows;
    const col = columns.find((c) => c.key === sort.key);
    if (!col?.sortValue) return rows;
    const sv = col.sortValue;
    return [...rows].sort((a, b) => {
      const x = sv(a);
      const y = sv(b);
      const r = typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y));
      return sort.dir === "asc" ? r : -r;
    });
  }, [rows, columns, sort]);

  const v = useVirtualizer({
    count: sorted.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => rowHeight,
    overscan: 8,
  });

  const toggleSort = (key: string) =>
    setSort((s) => (s?.key === key ? { key, dir: s.dir === "asc" ? "desc" : "asc" } : { key, dir: "desc" }));

  return (
    <div role="table" aria-label={label} aria-rowcount={sorted.length} className="flex flex-col overflow-hidden">
      <div role="rowgroup">
        <div
          role="row"
          className="grid items-center gap-3 border-b border-white/[0.08] px-4 py-2.5 text-[12px] font-semibold uppercase tracking-wider text-t2"
          style={{ gridTemplateColumns: template }}
        >
          {columns.map((c) => {
            const active = sort?.key === c.key;
            return (
              <div key={c.key} role="columnheader" aria-sort={active ? (sort!.dir === "asc" ? "ascending" : "descending") : "none"} className={c.align === "right" ? "text-right" : ""}>
                {c.sortValue ? (
                  <button type="button" onClick={() => toggleSort(c.key)} className={`inline-flex items-center gap-1 hover:text-t1 ${active ? "text-t1" : ""}`}>
                    {c.header}
                    {active && (sort!.dir === "asc" ? <ArrowUp size={12} aria-hidden /> : <ArrowDown size={12} aria-hidden />)}
                  </button>
                ) : (
                  c.header
                )}
              </div>
            );
          })}
        </div>
      </div>
      <div ref={scroller} role="rowgroup" className="overflow-y-auto" style={{ height }}>
        <div style={{ height: v.getTotalSize(), position: "relative" }}>
          {v.getVirtualItems().map((vi) => {
            const row = sorted[vi.index]!;
            const key = rowKey(row);
            const sel = key === selectedKey;
            return (
              <div
                key={key}
                role="row"
                tabIndex={onSelect ? 0 : -1}
                aria-selected={onSelect ? sel : undefined}
                onClick={() => onSelect?.(row)}
                onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onSelect?.(row))}
                className={[
                  "absolute left-0 right-0 grid items-center gap-3 px-4 text-sm transition-colors",
                  onSelect ? "cursor-pointer" : "",
                  sel ? "bg-white/[0.09]" : "hover:bg-white/[0.045]",
                ].join(" ")}
                style={{ gridTemplateColumns: template, height: vi.size, transform: `translateY(${vi.start}px)` }}
              >
                {columns.map((c) => (
                  <div key={c.key} role="cell" className={`min-w-0 truncate ${c.align === "right" ? "num text-right" : ""}`}>
                    {c.render(row)}
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
