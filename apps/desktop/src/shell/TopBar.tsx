/** Floating glass top bar: collapse, search (opens the command palette), help, local profile, bell. */
import { AnimatePresence, motion } from "motion/react";
import { Bell, CircleHelp, Eye, PanelLeftClose, PanelLeftOpen, Search, UserRound } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { isPreviewData } from "../backend";
import { Glass, GlassBadge, spring } from "../design-system";
import { timeAgo } from "../lib/format";
import { useUi } from "../store/ui";
import { WindowControls } from "./TitleBar";

export function TopBar({ left }: { left: number }) {
  const { t } = useTranslation();
  const collapsed = useUi((s) => s.sidebarCollapsed);
  const toggle = useUi((s) => s.toggleSidebar);
  const setPalette = useUi((s) => s.setPalette);
  const unread = useUi((s) => s.unread);
  const activity = useUi((s) => s.activity);
  const markRead = useUi((s) => s.markRead);
  const go = useUi((s) => s.go);
  const [bellOpen, setBellOpen] = useState(false);

  const iconBtn = "relative inline-flex h-10 w-10 items-center justify-center rounded-[12px] text-t2 transition-colors hover:bg-white/[0.08] hover:text-t1";

  return (
    <motion.header className="absolute right-4 top-4 z-50" animate={{ left }} transition={spring}>
      <Glass className="flex h-[60px] items-center gap-2 px-3" radius={20} data-tauri-drag-region>
        <button type="button" className={iconBtn} onClick={toggle} aria-label={collapsed ? t("top.expand") : t("top.collapse")}>
          {collapsed ? <PanelLeftOpen size={19} aria-hidden /> : <PanelLeftClose size={19} aria-hidden />}
        </button>

        <button
          type="button"
          onClick={() => setPalette(true)}
          className="flex h-10 w-[360px] min-w-[200px] shrink items-center gap-2.5 rounded-[12px] bg-white/[0.05] px-3.5 text-left text-t2 shadow-[inset_0_0_0_1px_rgb(255_255_255/0.08)] transition-colors hover:bg-white/[0.08]"
        >
          <Search size={16} aria-hidden />
          <span className="flex-1 truncate">{t("top.search")}</span>
          <span className="kbd">Ctrl K</span>
        </button>

        <div className="flex-1" data-tauri-drag-region />

        {isPreviewData && (
          <span title={t("top.previewTitle")}>
            <GlassBadge tone="warn">
              <Eye size={12} aria-hidden /> {t("top.preview")}
            </GlassBadge>
          </span>
        )}

        <button type="button" className={iconBtn} aria-label={t("top.help")} onClick={() => go("settings", "about")}>
          <CircleHelp size={19} aria-hidden />
        </button>
        <button type="button" className={iconBtn} aria-label={t("top.profile")} title={t("top.profile")}>
          <UserRound size={19} aria-hidden />
        </button>
        <div className="relative">
          <button
            type="button"
            className={iconBtn}
            aria-label={`${t("top.notifications")}${unread ? ` (${unread})` : ""}`}
            aria-expanded={bellOpen}
            onClick={() => {
              setBellOpen((o) => !o);
              markRead();
            }}
          >
            <Bell size={19} aria-hidden />
            {unread > 0 && (
              <span className="num absolute right-1 top-1 inline-flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-[var(--accent-red)] px-1 text-[11px] font-bold text-white shadow-[0_0_10px_rgb(var(--rgb-red)/0.8)]">
                {unread > 9 ? "9+" : unread}
              </span>
            )}
          </button>
          <AnimatePresence>
            {bellOpen && (
              <motion.div
                initial={{ opacity: 0, y: -8, scale: 0.97 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, y: -6 }}
                transition={spring}
                className="glass glass--thick absolute right-0 top-12 w-80 p-2"
                style={{ ["--radius" as string]: "18px" }}
                onMouseLeave={() => setBellOpen(false)}
              >
                {activity.length === 0 ? (
                  <div className="p-4 text-t2">{t("top.noNotifications")}</div>
                ) : (
                  <ul className="max-h-80 overflow-y-auto">
                    {activity.slice(0, 12).map((a) => (
                      <li key={a.id} className="rounded-[12px] px-3 py-2 hover:bg-white/[0.06]">
                        <div className="text-t1">{a.text}</div>
                        <div className="text-[12px] text-t2">{timeAgo(a.at)}</div>
                      </li>
                    ))}
                  </ul>
                )}
              </motion.div>
            )}
          </AnimatePresence>
        </div>
        <WindowControls />
      </Glass>
    </motion.header>
  );
}
