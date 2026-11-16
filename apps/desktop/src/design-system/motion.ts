/** Shared motion constants: one spring for the whole app (stiffness 300, damping 26). */
import type { Transition } from "motion/react";

export const spring: Transition = { type: "spring", stiffness: 300, damping: 26 };
export const softSpring: Transition = { type: "spring", stiffness: 220, damping: 30 };
export const pressScale = 0.97;
