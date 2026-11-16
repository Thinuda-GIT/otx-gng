/** Floating glass sidebar, collapsible to icons. Active item = red-tinted glass capsule with glow. */
import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import { Glass, GlassBadge, spring } from "../design-system";
import { useUi } from "../store/ui";
import { NAV } from "./nav";

export const SIDEBAR_W = { open: 256, collapsed: 76 };
export const APP_VERSION = "0.1.0";

export function Sidebar() {
  const { t } = useTranslation();
  const page = useUi((s) => s.page);
  const go = useUi((s) => s.go);
  const collapsed = useUi((s) => s.sidebarCollapsed);

  return (
    <motion.nav
      aria-label="Main"
      className="absolute bottom-4 left-4 top-4 z-50"
      animate={{ width: collapsed ? SIDEBAR_W.collapsed : SIDEBAR_W.open }}
      transition={spring}
    >
      <Glass className="flex h-full flex-col overflow-hidden px-3 pb-4 pt-5" radius={24}>
        {/* Logo + wordmark (drag region so the window can be moved from here too) */}
        <div className="mb-6 flex h-10 items-center gap-3 px-1.5" data-tauri-drag-region>
          <img src="/logo.svg" alt="" className="h-9 w-9 shrink-0" draggable={false} />
          {!collapsed && (
            <div className="min-w-0 leading-tight" data-tauri-drag-region>
              <div className="font-head text-[22px] font-bold tracking-wider text-t1">OTX.GNG</div>
              <div className="truncate text-[11px] text-t2">by Thinuda</div>
            </div>
          )}
        </div>

        <ul className="flex flex-1 flex-col gap-1">
          {NAV.map((item) => {
            const active = item.id === page;
            const Icon = item.icon;
            return (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => go(item.id)}
                  aria-current={active ? "page" : undefined}
                  title={collapsed ? t(`nav.${item.id}`) : undefined}
                  className={[
                    "relative flex h-11 w-full items-center gap-3 rounded-[14px] px-3.5 text-left text-[14px] font-medium transition-colors",
                    active ? "text-white" : "text-t2 hover:bg-white/[0.06] hover:text-t1",
                  ].join(" ")}
                >
                  {active && (
                    <motion.span
                      layoutId="nav-active"
                      transition={spring}
                      className="glass glass--tinted absolute inset-0"
                      style={{ ["--tint" as string]: "var(--rgb-red)", ["--radius" as string]: "14px", boxShadow: "0 0 24px -6px rgb(var(--rgb-red) / .7), inset 0 1px 0 rgb(255 255 255 / .25)" }}
                      aria-hidden
                    />
                  )}
                  <Icon size={19} className="relative z-[1] shrink-0" aria-hidden />
                  {!collapsed && <span className="relative z-[1] flex-1 truncate">{t(`nav.${item.id}`)}</span>}
                  {!collapsed && item.isNew && (
                    <span className="relative z-[1]">
                      <GlassBadge tone="new">{t("nav.new")}</GlassBadge>
                    </span>
                  )}
                </button>
              </li>
            );
          })}
        </ul>

        <div className={`mt-4 px-2 text-[12px] text-t2 ${collapsed ? "text-center" : ""}`}>
          <span className="num">v{APP_VERSION}</span>
        </div>
      </Glass>
    </motion.nav>
  );
}
