/** Non-persisted UI state: navigation, overlays, toasts and the activity feed. */
import { create } from "zustand";

export type PageId =
  | "home"
  | "connections"
  | "library"
  | "community"
  | "network"
  | "usage"
  | "boost"
  | "multi"
  | "shaper"
  | "settings"
  | "benchmark";

export interface Toast {
  id: number;
  tone: "ok" | "warn" | "error" | "info";
  title: string;
  body?: string;
}

export interface Activity {
  id: number;
  at: number;
  text: string;
}

interface UiStore {
  page: PageId;
  subTab: Record<string, string>;
  sidebarCollapsed: boolean;
  paletteOpen: boolean;
  toasts: Toast[];
  activity: Activity[];
  unread: number;
  go: (page: PageId, tab?: string) => void;
  setSubTab: (page: PageId, tab: string) => void;
  toggleSidebar: () => void;
  setPalette: (open: boolean) => void;
  toast: (t: Omit<Toast, "id">) => void;
  dismissToast: (id: number) => void;
  logActivity: (text: string) => void;
  markRead: () => void;
}

let seq = 0;

export const useUi = create<UiStore>()((set) => ({
  page: "home",
  subTab: {},
  sidebarCollapsed: false,
  paletteOpen: false,
  toasts: [],
  activity: [],
  unread: 0,
  go: (page, tab) => set((s) => ({ page, subTab: tab ? { ...s.subTab, [page]: tab } : s.subTab })),
  setSubTab: (page, tab) => set((s) => ({ subTab: { ...s.subTab, [page]: tab } })),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setPalette: (paletteOpen) => set({ paletteOpen }),
  toast: (t) => {
    const id = ++seq;
    set((s) => ({ toasts: [...s.toasts.slice(-3), { ...t, id }] }));
    setTimeout(() => set((s) => ({ toasts: s.toasts.filter((x) => x.id !== id) })), 5200);
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((x) => x.id !== id) })),
  logActivity: (text) =>
    set((s) => ({ activity: [{ id: ++seq, at: Date.now(), text }, ...s.activity].slice(0, 50), unread: s.unread + 1 })),
  markRead: () => set({ unread: 0 }),
}));
