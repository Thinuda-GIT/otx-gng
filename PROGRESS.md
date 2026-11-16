# OTX.GNG: PROGRESS

**OTX.GNG — Built and performed by Thinuda.** Paste this file into a fresh chat to continue.

## Status: all ten phases built (v0.2.0, not yet released)

| Phase | Commit | What it delivered |
|---|---|---|
| 1 | b5c33a1 | Glass design system, all pages, palette, splash, settings |
| 2 | 8376661 | Core service, secured pipe IPC, hardware detection, tray/hotkeys/autostart, installer |
| 3 | cd991bb | Real Usage Monitor (no handles to processes), process actions in the Core |
| 4 | bb97d48 | Tweak engine (manifests), RAM Cleaner, DNS Optimizer, restore point |
| 5 | 1f2094c | Library (Steam/Epic/Riot/installed programs), Game Mode with Core watchdog |
| 6 | 6063474 | Per-app traffic (ETW), connections, Traffic Shaper (Firewall + QoS) |
| 7 | ae0e2f0 | Quality Test with bufferbloat, traceroute, MTU, Multi Internet failover |
| 8 | de1666e | Benchmark Lab with DXGI frame-time capture |
| 9 | 5c75a34 | OTX Relay server, relay ping, user relays, hosting guide |
| 10 | (this commit) | Measured budgets, web view freed in the tray, Rust-side automation, docs, release workflow |

## Measured on this PC (Ryzen 5 220, 16 GB, Windows 11 26H2), release build

| Budget | Target | Measured | Result |
|---|---|---|---|
| UI idle CPU, window open | < 1% | 0.05% of total CPU | Met |
| UI idle CPU, in tray | ≈ 0 | 0.03% of total CPU | Met |
| UI memory, window open (all WebView2 processes) | < 150 MB | 164 MB private (369 MB working set) | **Missed by ~14 MB** |
| UI memory, closed to tray | < 40 MB | 5 MB private (26 MB working set), 1 process | Met |
| GPU when unfocused | ≈ 0% | not measured | Not verified |

The window-open figure is mostly WebView2's GPU and renderer processes; going lower means
disabling GPU acceleration, which would make the glass UI slow. Closing to the tray now destroys
the web view; game detection and automatic Game Mode run in Rust so they keep working.

## Tests (all passing at the last run)

- Rust: protocol 54, sysinfo 7, monitor 12, tweaks 6, net 10, games 4, core 24, relay 6
  (plus `#[ignore]` live tests run on this PC: firewall round trip, ETW traffic, frame capture,
  ping/traceroute/MTU, DNS).
- UI: 51 (vitest), type check clean, production build OK.
- CI on GitHub: all jobs green for Phase 2; later phases not yet pushed when this was written.

## Requirements audit (R1–R15)

| Req | Status | Evidence / gap |
|---|---|---|
| R1 RAM before launch | Done | Core RAM Cleaner (5 methods, Smart default, protected/allow/game skip); Play cleans first; automatic routine; tested on throwaway processes. Purge/flush not run on this PC |
| R2 CPU priority | Done | Game to High (never Realtime), background Below Normal + EcoQoS, reverted by the Core watchdog. Hybrid CPU Sets: detection only, no CPU Sets yet |
| R3 Many features | Done | 20 tweaks as data, Usage Monitor, Benchmark Lab, failover. Unreal config tuner, perf-counter and timer tweaks deliberately held back (explained in the UI) |
| R4 Network per app / focus | Done (outbound) | ETW per-app traffic, connections, Firewall block, QoS upload limits (experimental). Download limiting needs a driver: not included |
| R5 Network booster | Done | Tune-up from real checks, adapter power saving, DNS, MTU, bufferbloat test, VPN detection |
| R6 Usage tabs | Done | CPU/GPU/RAM/I-O per app; per-app network in Network Analyzer. Temperatures not included (need a sensor driver) |
| R7 ExitLag-like IA, glass | Done | Your screenshots never arrived; the layout follows the written description |
| R8 Branding | Done | Splash, About, footer, tray tooltip, installer metadata |
| R9 Win10 + Win11 | Partial | Detection and fallbacks for both; only tested on Windows 11 26H2. Windows 10 22H2 untested |
| R10 Gaming outcomes | Measured, not promised | Benchmark Lab A/B with spread; route diagnosis; real routing via relays not available (see R13/Tier 2) |
| R11 Screenshot pages | Done | Sidebar, Preferences, PC Boost (three tabs), all real in the desktop app |
| R12 References | Done | Ideas only; no copied code, names or art |
| R13 Free | Done | $0 toolchain and hosting; all 470+ dependencies GPL-compatible; release workflow on free GitHub Actions. Installer unsigned (SignPath not applied for) |
| R14 Glass UI | Done | Quality tiers with real auto-degrade (GPU tier, battery saver, running game), reduce motion/transparency |
| R15 Engineering | Done, with stated gaps | Threat model, allow-list IPC, journaled reverts, 173 automated tests (122 Rust, 51 UI), CI |

## Known gaps (honest list)

1. **Relay routing for games** (Tier 2): the relay server works; steering a game's traffic into
   it on Windows needs a signed network driver (WireGuard-NT/WinDivert). Not implemented.
2. **Inbound (download) limits**: need a driver; only upload limits (QoS) and blocking exist.
3. **QoS limits** use local QoS policy values plus a policy refresh; whether every Windows edition
   honours them is unverified (labelled Experimental).
4. **Country/ASN names** need the DB-IP Lite database; not bundled, so those columns show "-".
5. **Hardware sensors** (temperatures, clocks, fans) need a driver; not included.
6. **Unreal Engine config tuner, timer resolution, perf-counter tweak**: held back on purpose.
7. **Window-open memory** is ~14 MB over budget (see above). GPU use not measured.
8. **Windows 10** and the **installed service** were not tested (no VM used). Run the manual
   checklist in a VM before the first release.
9. **Code signing**: unsigned; SmartScreen warns. Apply to SignPath Foundation (free for OSS).
10. **Signed community lists**: no list or signing key exists yet; only user-added relays.
11. **i18n**: shell, navigation and Settings strings are in `en.json`; page copy is still inline English.
12. **Tauri auto-updater**: not configured (needs an update-signing key stored as a GitHub secret).

## Run, test, release

```powershell
cd C:\dev\otx-gng
cargo test -p otx-protocol -p otx-sysinfo -p otx-monitor -p otx-tweaks -p otx-net -p otx-games -p otx-core -p otx-relay
cd apps\desktop; npm test; npm run tauri dev     # dev: also run `cargo run -p otx-core -- console --dev`
npm run tauri build                              # installer in target\release\bundle\nsis
git tag v0.2.0; git push origin v0.2.0           # GitHub Actions builds a draft release with SHA-256
```

## Decisions

D1–D18 from Phase 2 stand. Added since:
- D19 Monitoring uses `NtQuerySystemInformation` + `SystemProcessIdInformation` so no process
  handle is ever opened just to display it (layout verified against Microsoft's definition).
- D20 Tweaks are JSON manifests with machine/user scope; user-scope tweaks run in the app with a
  local snapshot ledger; "Default" preset only reverts what OTX.GNG changed.
- D21 Game Mode sessions live in the Core; its watchdog polls for the game (no handle) and reverts.
- D22 Frame capture uses DXGI Present ETW events instead of bundling PresentMon.
- D23 Closing to the tray destroys the web view; game detection and auto Game Mode run in Rust.
- D24 Relay protocol: per-client HMAC tags, allow-listed destinations, token buckets, first-copy dedup.
