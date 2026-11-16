import {
  Activity,
  BarChart3,
  Gauge,
  Globe2,
  Home,
  Library,
  Network,
  Rocket,
  Settings,
  Split,
  Timer,
  Waypoints,
  type LucideIcon,
} from "lucide-react";
import type { PageId } from "../store/ui";

export interface NavItem {
  id: PageId;
  icon: LucideIcon;
  isNew?: boolean;
}

/** Sidebar order from the brief. Benchmark Lab is reachable from Home and Usage Monitor. */
export const NAV: NavItem[] = [
  { id: "home", icon: Home },
  { id: "connections", icon: Waypoints },
  { id: "library", icon: Library },
  { id: "community", icon: Globe2 },
  { id: "network", icon: Network },
  { id: "usage", icon: Activity, isNew: true },
  { id: "boost", icon: Rocket },
  { id: "multi", icon: Split },
  { id: "shaper", icon: Gauge },
  { id: "settings", icon: Settings },
];

export const EXTRA_PAGES: NavItem[] = [{ id: "benchmark", icon: Timer }];

export const PAGE_ICON: Record<PageId, LucideIcon> = {
  ...(Object.fromEntries([...NAV, ...EXTRA_PAGES].map((n) => [n.id, n.icon])) as Record<PageId, LucideIcon>),
  benchmark: BarChart3,
};
