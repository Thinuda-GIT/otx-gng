/** Splash: logo ignites, then the branding line. Skipped instantly under reduced motion. */
import { motion } from "motion/react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";

export function Splash({ onDone, reduced }: { onDone: () => void; reduced: boolean }) {
  const { t } = useTranslation();
  useEffect(() => {
    const h = setTimeout(onDone, reduced ? 150 : 1900);
    return () => clearTimeout(h);
  }, [onDone, reduced]);

  return (
    <motion.div
      className="absolute inset-0 z-[100] flex flex-col items-center justify-center"
      style={{ background: "radial-gradient(80% 80% at 50% 40%, #0d1120, #05060a 70%)" }}
      exit={{ opacity: 0, scale: 1.03 }}
      transition={{ duration: 0.45 }}
      onClick={onDone}
      role="presentation"
    >
      <motion.div
        initial={{ scale: 0.6, opacity: 0, rotate: -20 }}
        animate={{ scale: 1, opacity: 1, rotate: 0 }}
        transition={{ type: "spring", stiffness: 160, damping: 16 }}
        className="relative"
      >
        <div className="absolute inset-[-40%] rounded-full" style={{ background: "radial-gradient(circle, rgb(30 242 160 / .35), transparent 65%)" }} />
        <img src="/logo.svg" alt="" className="relative h-28 w-28" draggable={false} />
      </motion.div>
      <motion.h1
        className="mt-8 font-head text-5xl tracking-[0.18em] text-t1"
        initial={{ opacity: 0, y: 12, letterSpacing: "0.4em" }}
        animate={{ opacity: 1, y: 0, letterSpacing: "0.18em" }}
        transition={{ delay: 0.35, duration: 0.7, ease: [0.2, 0.8, 0.2, 1] }}
      >
        OTX.GNG
      </motion.h1>
      <motion.p className="mt-3 text-t2" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.8, duration: 0.6 }}>
        {t("brand.line")}
      </motion.p>
      <motion.div
        className="mt-8 h-[3px] w-56 overflow-hidden rounded-full bg-white/10"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 0.6 }}
      >
        <motion.div
          className="h-full rounded-full"
          style={{ background: "linear-gradient(90deg,#2DE2FF,#1EF2A0)", boxShadow: "0 0 12px #1EF2A0" }}
          initial={{ width: "0%" }}
          animate={{ width: "100%" }}
          transition={{ delay: 0.6, duration: 1.2, ease: "easeInOut" }}
        />
      </motion.div>
    </motion.div>
  );
}
