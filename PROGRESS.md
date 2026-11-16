# OTX.GNG: PROGRESS

**OTX.GNG — Built and performed by Thinuda.** Paste this file into a fresh chat to continue.

## Where we are

- **Step 0 (plan):** done. Defaults accepted.
- **Phase 1 (Glass UI on mock data):** done, committed (`b5c33a1`).
- **Phase 2 (Core service + IPC + installer skeleton + real hardware info):** done. See "Phase 2 verification".
- **Next:** Phase 3, Usage Monitor with real per-process CPU/RAM/disk/GPU, categorization, the
  protected-process list, process actions (priority, EcoQoS, end task) and analyzer v1 on real data.

## Decisions (locked)

| # | Decision |
|---|---|
| D1 | Stack: Tauri 2 + WebView2, React 19 + TypeScript + Vite 7 + Tailwind 4 + Motion 12 + Zustand 5 + TanStack Virtual + react-i18next. Custom canvas charts. |
| D2 | License: GPL-3.0-or-later. Every npm and Rust dependency (466 crates) is GPL-compatible; see THIRD_PARTY_LICENSES.md. |
| D3 | Repo at `C:\dev\otx-gng`. Dev server on port **1420** (5173 is used by another project on this PC). Cargo workspace at the repo root; `target/` is shared. |
| D4 | Privileged/unprivileged split: **Core service** (`otx-core`, LocalSystem) for machine-scope changes; the **Tauri process** (normal user) hosts tray, hotkeys, autostart, per-user (HKCU/SPI) tweaks from Phase 4, and local read-only detection when the Core is down. |
| D5 | IPC: pipe `\\.\pipe\otx-gng-core-v1`, DACL `D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x12008b;;;IU)`, first-instance flag, remote clients rejected, `u32 LE + JSON` frames ≤ 1 MiB, typed `Request` enum = allow-list. |
| D5a | **Changed from the Step 0 plan:** no HMAC key file. Authentication uses kernel-reported client identity (PID → exe path, session, user SID). In service mode the client must be `OTX.GNG.exe`/`otx-gng.exe` in the Core's own Program Files directory. A key file readable by the user adds nothing against same-user malware. Release UI builds verify the pipe server is the installed `otx-core.exe`. Rationale in docs/SECURITY.md. |
| D6 | Tweaks are declarative manifests run by one generic engine (Phase 4). |
| D7 | Tweak 5 (perf counters): scoped, Experimental, never in a preset; Usage Monitor CPU/RAM/disk via `NtQuerySystemInformation`, GPU via PDH. |
| D8 | Tier 2: WireGuard-style split tunnel; dual paths converge on one exit relay. |
| D9 | No telemetry backend in v1. |
| D10 | IP geo/ASN: DB-IP Lite (CC BY 4.0). |
| D11 | Frame capture: bundle the MIT PresentMon CLI (verify at bundling). |
| D12 | Design-system CSS in `@layer components`/`@layer base` so Tailwind utilities win. |
| D13 | Glass auto rule as before; Phase 2 feeds it REAL inputs: GPU tier from the detected adapters, battery saver from `GetSystemPowerStatus`. |
| D14 | Protocol types are generated from Rust by ts-rs into `apps/desktop/src/backend/generated/` (`npm run gen:types`). `u64` fields are annotated `#[ts(type = "number")]`; a UI test fails if any generated type contains `bigint`. |
| D15 | Data dir: `%ProgramData%\OTX.GNG` with a protected DACL (SYSTEM/Admins full, Users read). Unelevated dev console uses `%LOCALAPPDATA%\OTX.GNG\dev`. Logs roll daily in `logs\core.log`. |
| D16 | Write-ahead journal (`journal.json`, atomic replace). The watchdog reverts temporary entries newest-first at Core start; failures are kept and retried; a corrupt journal is moved aside. Service restarts itself after a crash (5 s, 30 s). |
| D17 | The Core ships as a Tauri sidecar (`externalBin`); NSIS hooks: stop before install, re-register and start after install, `revert-all` + remove on uninstall, stop-only on update (`/UPDATE` detection is UNVERIFIED until tested in a VM). |
| D18 | Hotkeys are global and rebindable: restart connections `Shift+Ctrl+Q` (from your screenshot), restore normal internet `Shift+Ctrl+Alt+R` (chosen to avoid clashing with common app shortcuts). |

## Architecture state

```
crates/otx-protocol   Typed IPC messages, framing, limits; ts-rs exports        (31 tests)
crates/otx-sysinfo    Read-only detection: OS, CPU (P/E cores), GPUs (DXGI), RAM,
                      system disk (IOCTL: NVMe/SSD/HDD + model), power/battery/plan,
                      primary network link, startup entries (StartupApproved-aware),
                      anti-cheat services. Example: `cargo run -p otx-sysinfo --example sysinfo`  (7 tests)
crates/otx-core       Windows service + console mode, secured pipe server, kernel-based
                      client auth, allow-list dispatcher, journal/watchdog, SCM install   (10 tests, 4 over a real pipe)
apps/desktop/src-tauri  Tray, close-to-tray, single instance, autostart, global hotkeys,
                      pipe client with server verification, local fallback detection
apps/desktop/src      UI. TauriBackend = real system info + Core status; everything else
                      still MockBackend until its phase. First-run wizard. Core panel in Settings.
```

## Phase 2 verification (real results on this PC)

- Rust: `cargo test -p otx-protocol -p otx-sysinfo -p otx-core` → 48 passed, including real
  named-pipe tests: handshake, ping/status/system info, `runCommand` rejected as NOT_ALLOWED,
  non-hello first frame → UNAUTHORIZED, protocol 99 → VERSION_MISMATCH, a client outside the
  install directory refused by a production-mode Core, 2 MiB frame → connection closed.
- UI: `tsc` clean; `vitest` 43 passed.
- End to end: `otx-core console --dev` plus the real desktop app. The Core logged
  `client authenticated … image=C:\dev\otx-gng\target\debug\otx-gng.exe` and served
  `GetSystemInfo` and `GetStatus` over the pipe.
- Hardware detection on this PC: Windows 11 26H2 (build 26300.9711, CoreSingleLanguage); AMD Ryzen 5 220
  6 cores / 12 threads; NVIDIA GeForce RTX 5050 Laptop GPU (7.7 GB) + AMD Radeon 740M (integrated);
  15.3 GB RAM; Micron NVMe; laptop, plugged in, 79%, Balanced plan; primary link "Mobitel" (PPP);
  4 enabled startup entries; PunkBuster service running; Vanguard and BattlEye installed.
- Installer: `npm run tauri build` produced `target\release\bundle\nsis\OTX.GNG_0.2.0_x64-setup.exe`
  (2.04 MiB, SHA-256 `1998cd0abb703cbaa4ea9e2347de98cc0ed7a8e5d82366fc3db720a4df6ab170`, unsigned).
  makensis compiled the service hooks, so LogicLib and FileFunc are available. Release binaries:
  `otx-gng.exe` 4.0 MB and `otx-core.exe` 1.0 MB. The installer was NOT run on this PC.
- Bugs found by verification and fixed: last CPU core dropped by the record-walk bounds check
  (5/10 → 6/12); ts-rs mapped `u64` to `bigint`; a missing `Win32_Security` feature hidden by
  Cargo feature unification; disabled startup entries were counted (37 → 4); PPP links showed as "other".
- Side effect on this PC (expected): the elevated dev run created `C:\ProgramData\OTX.GNG` (logs +
  empty journal) with its protected DACL. Nothing else was changed. The service was NOT installed.

## Requirements status (R1–R15)

| Req | Status | Note |
|---|---|---|
| R1 RAM before launch | Partial | UI on mock; the real purge comes in Phase 4/5 (Core journal ready) |
| R2 CPU priority | Partial | UI on mock; real priority/CPU Sets in Phase 3/5. P/E-core detection is real now |
| R3 Many features | Partial | 20 tweak cards (mock); engine in Phase 4 |
| R4 Network per app | Partial | UI on mock; Phase 6 |
| R5 Network booster | Partial | UI on mock; real primary-link detection done; Phase 7 |
| R6 Usage tabs | Partial | UI on mock; Phase 3 |
| R7 IA + glass | Done (look) | Screenshots still not received |
| R8 Branding | Done | Now also tray tooltip and installer metadata |
| R9 Win10 + Win11 | Partial | Real version detection (Windows 11 from build ≥ 22000; supported ≥ 19045). Not yet tested on Win10 |
| R10 Outcomes | Not started | Measured only, Phases 3–9 |
| R11 Screenshot pages | Done on mock | |
| R12 References | Ongoing | |
| R13 Free | Done so far | $0; all licenses GPL-compatible |
| R14 Glass UI | Done | Auto-degrade now driven by real GPU and battery-saver facts |
| R15 Engineering | Ongoing | Threat model, secured IPC, 91 automated tests, CI covers Rust + UI + generated-type drift |

## UNVERIFIED

- NSIS `/UPDATE` detection in the uninstall hook at runtime (the hook compiles; behaviour during a real update is untested).
- Behaviour of the installed service on Windows 10 22H2 and a clean Windows 11 VM.
- The global-hotkey parser's acceptance of "Shift+Ctrl+Alt+R" style strings (failures are reported to the user with a toast).
- Earlier items: Verisign DNS availability, SVG lens refraction in WebView2, launcher formats, QoS on Home, SignPath eligibility.

## Known limitations

- Only system facts and Core status are real; every other page is still mock data ("Preview data" chip).
- Ricochet (Call of Duty) has no persistent service, so it is detected per game in Phase 5, not here.
- A desktop on a UPS that reports as a system battery would be shown as a laptop.
- Startup-entry count covers Run keys (HKLM, HKLM WOW6432Node, HKCU) and not the Startup folder or scheduled tasks; Phase 3's startup manager adds those.
- Close-to-tray hides the window; destroying the webview while a game runs (to save RAM) comes with game detection in Phase 5.
- Tray "Game Mode" is disabled until Phase 5; "Boost now" opens Home.

## Run and test

```powershell
# Terminal 1 (optional): the Core in developer mode, no install needed
cd C:\dev\otx-gng
cargo run -p otx-core -- console --dev

# Terminal 2: the desktop app (builds the Core sidecar first, then opens the window)
cd C:\dev\otx-gng\apps\desktop
npm run tauri dev

# Tests
cd C:\dev\otx-gng; cargo test -p otx-protocol -p otx-sysinfo -p otx-core
cd C:\dev\otx-gng\apps\desktop; npm test

# What does OTX.GNG detect on this PC?
cd C:\dev\otx-gng; cargo run -p otx-sysinfo --example sysinfo

# Installer (release build; produces the NSIS setup .exe, does not install it)
cd C:\dev\otx-gng\apps\desktop; npm run tauri build
```

Install the service only in a VM for now (elevated PowerShell): `otx-core.exe install`, `status`,
`stop`, `uninstall`. Safe Mode: set the system environment variable `OTX_SAFE_MODE=1` and restart the service.

## Manual test checklist (Phase 2)

1. `npm run tauri dev` with no Core running → wizard step 2 shows your real hardware ("Detected by the app"); Settings → Core service shows "Not running".
2. Start `otx-core console --dev`, reopen the app → wizard/Settings say "Detected by the Core service" and "Connected · developer console".
3. Close the window → it hides to the tray; the tray icon tooltip shows the branding line; left-click reopens; Quit exits.
4. Launch the app a second time → the first window comes to the front (single instance).
5. Settings → Launch on Windows startup ON → Task Manager › Startup apps lists OTX.GNG; OFF removes it.
6. Press Shift+Ctrl+Alt+R anywhere → "Normal internet restored" toast. Rebind it in Settings and try again.
7. In a VM: run the installer → service "OTX.GNG Core" is running (services.msc), app shows "Connected"; uninstall → service gone.
