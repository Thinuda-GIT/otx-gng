// Builds otx-core and places it where the Tauri bundler expects a sidecar:
//   src-tauri/binaries/otx-core-<target-triple>.exe
// The installer then ships it as otx-core.exe next to the app.
// Usage: node scripts/build-core.mjs [--release]
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..", "..", ".."); // repo root (Cargo workspace)
const release = process.argv.includes("--release");

const triple = execFileSync("rustc", ["-vV"], { encoding: "utf8" })
  .split("\n")
  .find((l) => l.startsWith("host:"))
  ?.slice(5)
  .trim();
if (!triple) throw new Error("could not read the Rust host triple from `rustc -vV`");

execFileSync("cargo", ["build", "-p", "otx-core", ...(release ? ["--release"] : [])], { cwd: root, stdio: "inherit" });

const src = join(root, "target", release ? "release" : "debug", "otx-core.exe");
const outDir = join(here, "..", "src-tauri", "binaries");
mkdirSync(outDir, { recursive: true });
const dest = join(outDir, `otx-core-${triple}.exe`);
copyFileSync(src, dest);
console.log(`sidecar ready: ${dest}`);
