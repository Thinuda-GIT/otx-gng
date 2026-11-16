import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useUi } from "../store/ui";
import { spring } from "./motion";

const ICON = { ok: CheckCircle2, warn: AlertTriangle, error: XCircle, info: Info };
const TINT = { ok: "var(--rgb-green)", warn: "var(--rgb-amber)", error: "var(--rgb-red)", info: "var(--rgb-cyan)" };

export function Toasts() {
  const toasts = useUi((s) => s.toasts);
  const dismiss = useUi((s) => s.dismissToast);
  return (
    <div className="pointer-events-none absolute bottom-6 right-6 z-[90] flex w-[360px] flex-col gap-2" aria-live="polite">
      <AnimatePresence initial={false}>
        {toasts.map((t) => {
          const Icon = ICON[t.tone];
          return (
            <motion.div
              key={t.id}
              layout
              initial={{ opacity: 0, y: 24, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, x: 40 }}
              transition={spring}
              className="glass glass--thick pointer-events-auto flex items-start gap-3 p-4"
              style={{ ["--radius" as string]: "18px", ["--shadow-tint" as string]: TINT[t.tone] }}
              role="status"
            >
              <Icon size={20} style={{ color: `rgb(${TINT[t.tone]})` }} className="mt-0.5 shrink-0" aria-hidden />
              <div className="min-w-0 flex-1">
                <div className="font-semibold text-t1">{t.title}</div>
                {t.body && <div className="mt-0.5 text-[13px] text-t2">{t.body}</div>}
              </div>
              <button type="button" aria-label="Dismiss" onClick={() => dismiss(t.id)} className="text-t2 hover:text-t1">
                <X size={16} aria-hidden />
              </button>
            </motion.div>
          );
        })}
      </AnimatePresence>
    </div>
  );
}
