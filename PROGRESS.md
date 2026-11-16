# OTX.GNG: PROGRESS

**OTX.GNG — Built and performed by Thinuda.** Paste this file into a fresh chat to continue.

## Where we are

- **Step 0 (plan):** done. All 8 open questions were answered "defaults are fine".
- **Phase 1 (Foundation + Glass UI on mock data):** done and verified in a browser. The Tauri shell
  compiles (see "Phase 1 verification").
- **Next:** Phase 2, the Core service, IPC, installer skeleton and real hardware info.

## Decisions (locked)

| # | Decision |
|---|---|
| D1 | Stack: Tauri 2 + WebView2, React 19 + TypeScript + Vite 7 + Tailwind 4 + Motion 12 + Zustand 5 + TanStack Virtual + react-i18next. Custom canvas charts (no chart library). |
| D2 | License: GPL-3.0-or-later. All current dependencies are MIT, ISC, Apache-2.0 or OFL-1.1. |
| D3 | Repo at `C:\dev\otx-gng` (short path, no spaces). Dev server on port **1420** (5173 is used by another project on this PC). |
| D4 | Two privileged/unprivileged halves: the **Core service** (LocalSystem, machine-scope changes only) and an unprivileged **User Agent** inside the Tauri process (HKCU, `SystemParametersInfo`, per-user tweaks). |
| D5 | UI ↔ Core: named pipe `\\.\pipe\otx-gng-core-v1`, DACL = interactive user SID + SYSTEM, HMAC handshake, length-prefixed JSON frames, strict allow-list, PID + creation time to identify processes. Types generated from Rust with `ts-rs`. |
| D6 | Tweaks are declarative JSON manifests run by one generic engine (check → snapshot → apply → verify → journal; revert from the snapshot, never a guessed default). |
| D7 | Tweak 5 (performance counters): ships scoped (third-party extensible providers only), Experimental, never in a preset. Usage Monitor reads CPU/RAM/disk via `NtQuerySystemInformation`, so only GPU columns depend on PDH and they disable themselves with a reason if PDH fails. |
| D8 | Tier 2 redirection: WireGuard-style split tunnel (`wireguard-nt`), only game-server ranges routed. Dual-path duplication must converge on ONE exit relay. Duplication is Experimental. |
| D9 | No telemetry backend in v1. Crash reports are local, redacted, opt-in. |
| D10 | IP geolocation/ASN: DB-IP Lite (CC BY 4.0). Not MaxMind (no redistribution). |
| D11 | Frame-time capture: bundle the MIT-licensed PresentMon CLI (verify license at bundling time) instead of writing our own Present-event parser. |
| D12 | Design-system CSS lives in `@layer components` / `@layer base` so Tailwind utilities always win (fixes `.glass { position: relative }` overriding `absolute`). |
| D13 | Glass auto rule: reduce transparency → Off; manual setting wins; Auto → Off while a game runs, Off on software rendering, Lite on integrated GPU or battery saver, else Full. UI is "paused" (no animations, no chart redraw, no metric polling) when hidden, or when a game runs and our window is not focused. |

## Architecture state

```
apps/desktop (Tauri 2 app)
  src/            React UI. Backend interface → MockBackend (Phase 1). TauriBackend arrives in Phase 2.
  src-tauri/      Rust shell: window + frameless chrome only. No privileged code yet.
crates/           (Phase 2) otx-protocol, otx-sys, otx-tweaks, otx-monitor, otx-net, otx-core-service, otx-relay
```

## File tree (Phase 1)

```
otx-gng/
├─ .github/workflows/ci.yml        typecheck, tests, build, npm audit; cargo check of the shell
├─ .gitignore
├─ LICENSE                         GPL-3.0 full text
├─ PROGRESS.md  README.md  THIRD_PARTY_LICENSES.md
└─ apps/desktop/
   ├─ index.html  package.json  package-lock.json  tsconfig.json  vite.config.ts
   ├─ public/logo.svg               original OTX.GNG mark (hex ring + chevron)
   ├─ src/
   │  ├─ main.tsx  App.tsx
   │  ├─ styles/   tokens.css  glass.css  components.css  app.css
   │  ├─ i18n/     index.ts  en.json
   │  ├─ lib/      format  glassQuality  healthScore  analyzer  benchStats  search  tauri  dom
   │  │            actions  useSpecular  logic.test.ts (33 tests)
   │  ├─ store/    settings.ts (persisted)  ui.ts  live.ts
   │  ├─ backend/  types.ts  mock.ts  mockData.ts  index.ts
   │  ├─ design-system/  Glass  GlassButton  GlassToggle  GlassSegmented  controls (Slider, Select,
   │  │            Tooltip, Badge, TierBadge, LensFilterDefs)  GlassModal  Toasts  GlassGauge
   │  │            LiveChart (+Sparkline)  GlassTable  motion  index
   │  ├─ shell/    Backdrop  TitleBar (DragStrip, WindowControls)  Sidebar  TopBar  CommandPalette  Splash  nav
   │  ├─ components/ui.tsx           PageHeader, Panel, Stat, Row, ToggleRow, Divider, Empty, GameTile
   │  └─ pages/    Home  PcBoost  UsageMonitor  Library  NetworkAnalyzer  TrafficShaper  Connections
   │               CommunityServers  MultiInternet  BenchmarkLab  Settings
   └─ src-tauri/
      ├─ Cargo.toml  build.rs  tauri.conf.json
      ├─ capabilities/default.json   window permissions only
      ├─ icons/                      generated from public/logo.svg
      └─ src/main.rs  src/lib.rs
```

## Phase 1 verification (real results)

- `npx tsc --noEmit`: 0 errors.
- `npx vitest run`: 33/33 tests pass (glass rule, health score, analyzer, benchmark statistics,
  validation, palette ranking, settings import sanitising, shortcut capture).
- `npm run build`: succeeds. The main chunk is about 517 kB (169 kB gzip) and every page is its own lazy chunk.
- Browser preview at 1440×810: Home, PC Boost (all 3 tabs), Usage Monitor and the command palette
  were checked by hand. The Boost pipeline animates step by step. "Simulate a running game" switches
  glass to Off, removes the backdrop, tints the page with the game colour and fills the HUD. No console errors.
- Bugs found and fixed during verification: CSS layer order (absolute positioning lost), a tint too faint
  to read, truncated stats, a sparkline wired to the wrong series, and a port clash with another local project.
- Tauri shell: `cargo build` (debug) succeeded in 2 min 33 s with Rust 1.99 and produced `otx-gng.exe`.
  Smoke test: the window opens titled "OTX.GNG" and responds. The shell process used 35 MB (debug build).
  WebView2 memory could not be attributed (other apps on this PC also use WebView2). Real UI memory
  budgets are measured in Phase 10, not claimed now.
- Also fixed: the HUD corner brackets collided with the glass `::before` layer and now use their own overlay element.

## Requirements status (R1–R15)

| Req | Status | Note |
|---|---|---|
| R1 RAM before launch | Partial | UI for RAM Cleaner, routine, allow list and history is done on mock data; the real purge comes in Phase 4/5 |
| R2 CPU priority | Partial | UI done (profiles, process actions); real `SetPriorityClass` / CPU Sets in Phase 3/5 |
| R3 Many boost features | Partial | All 20 tweak cards with risk, evidence and "not recommended if"; the engine comes in Phase 4 |
| R4 Network per app / focus | Partial | Live Traffic, Connections, Traffic Shaper and Focus mode UIs; ETW + Firewall/QoS in Phase 6 |
| R5 Network booster | Partial | Quality Test and Tune-up UIs; real probes in Phase 7 |
| R6 Usage tabs | Partial | Charts, grouped virtualized table, process panel, analyzer v1 (logic real and tested; data mocked) |
| R7 ExitLag-like IA, glass | Done (look) | Sidebar order, pages and settings structure match the brief; screenshots still needed for fine layout |
| R8 Branding | Done | Splash, footer, About, sidebar, installer metadata. The tray tooltip comes in Phase 2 |
| R9 Win10 + Win11 | Partial | In-app glass looks the same on both. Real version detection in Phase 2 |
| R10 Gaming outcomes | Not started | Measured only, from Phases 3–9 |
| R11 Screenshot pages | Done (on mock) | Sidebar, Preferences, PC Boost (System Tweaks / RAM Cleaner / DNS Optimizer) |
| R12 Feature references | Ongoing | Ideas only; no copied code, names or art |
| R13 100% free | Done so far | $0 toolchain, all licenses GPL-compatible, no services used |
| R14 Liquid-glass UI | Done | One Glass primitive (blur, fill, masked edge light, inner highlight, pointer sheen, noise, tinted shadow), 4 variants, SVG lens refraction where supported, quality tiers with auto-degrade |
| R15 Correct engineering | Ongoing | Strict TS, 33 unit tests, CI workflow; no privileged code yet |

## UNVERIFIED (to check when the item is built)

- Verisign Public DNS (64.6.64.6) is still in service. The mock shows it failing every query to demonstrate how failure is reported.
- `CSS.supports("backdrop-filter", "url(#id)")` and the visual result of SVG lens refraction inside
  WebView2. This is a progressive enhancement that falls back to plain blur.
- Window effects (Mica/Acrylic) are not enabled yet. Planned as a setting in Phase 2.
- Exact launcher metadata formats for Battle.net, EA app and Xbox (Phase 5).
- Policy-based QoS behaviour on Windows Home without a domain (Phase 6).
- SignPath Foundation eligibility (Phase 10).

## Known limitations of Phase 1

- Everything is mock data. Buttons that would change the system show "Preview only" toasts or change the mock state.
- i18n: shell, navigation and Settings strings are in `en.json`. Page body copy is still inline English
  and moves to resource files in the Phase 10 i18n pass.
- Close button closes the app. Minimize-to-tray needs the tray (Phase 2).
- Hardware sensor, battery-saver and real GPU signals come from the Core in Phase 2. Phase 1 uses the
  WebGL renderer string for the GPU tier.
- The first-run wizard needs real hardware detection, so it is scheduled with Phase 2.
- Performance budgets (idle CPU, UI RAM) are not measured yet. That happens in Phase 10, and nothing is claimed before then.

## Prerequisites

Run in PowerShell **as Administrator**:

```powershell
winget install --id Git.Git -e
winget install --id OpenJS.NodeJS.LTS -e
winget install --id Rustlang.Rustup -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Then, in a normal PowerShell: `rustup default stable-x86_64-pc-windows-msvc`.

## Manual test checklist (Phase 1)

1. `npm run dev`, open http://localhost:1420: the splash shows the branding line, then Home.
2. Sidebar: every item navigates; the active item is a red glass capsule; the collapse button shrinks it to icons.
3. Ctrl+K (or click the search pill): type "dns", press Enter → PC Boost.
4. Home → Boost: the ring sweeps, steps tick one by one, the freed MB counts up, and a toast appears. "What OTX.GNG changed" lists the session.
5. PC Boost → System Tweaks: turn on Dry run, toggle a tweak → a preview sheet lists exact values and nothing changes.
   "Not recommended if…" on "Keep all CPU cores awake" shows "Applies to this PC" (the mock PC is a laptop).
6. RAM Cleaner: Clean → a toast with measured before and after values; History gains an entry; Add app adds to the allow list.
7. DNS Optimizer: Run benchmark → a ranked list with a "Fastest" badge; Verisign shows failures; Choose updates "In use".
8. Settings: change Glass quality to Lite/Off; Reduce transparency On; accent colour; UI scale 120%; reload → settings persist. Export and re-import settings.
9. Settings → Developer preview → Simulate a game: glass goes Off and Home shows the live HUD. Click outside the window → animations and charts pause.
10. `npm run tauri dev`: a frameless window with 28 px corners; drag from the top strip, the top bar or the logo; minimize, maximize and close work.

## Next: Phase 2

Core service (`otx-core-service`, `windows-service` crate) + `otx-protocol` (ts-rs types) + pipe
server/client with the handshake, the allow-list dispatcher skeleton, logging, the watchdog journal,
NSIS hooks to install/stop/uninstall the service, the WebView2 bootstrapper, the tray and hotkeys,
the first-run wizard with real hardware info (CPU, GPUs, RAM, disk type, laptop/desktop, adapter type,
Windows version, anti-cheat services), and the TauriBackend for `getSystemInfo`.
