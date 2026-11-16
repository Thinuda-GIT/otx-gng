# OTX.GNG

**OTX.GNG — Built and performed by Thinuda**

Free, honest, measurable game boosting for Windows 10 and 11. Free up RAM, give the game CPU
priority, tame background network use and lower lag, in a glass interface. No paywall, no
account, no ads, no telemetry unless you opt in.

> **Status: Phase 2 of 10.** The desktop app reads your real hardware and talks to the OTX.GNG
> Core service over a secured pipe. The Core can't change anything yet, and most pages still show
> preview data (the amber "Preview data" chip). See [PROGRESS.md](PROGRESS.md) and
> [docs/SECURITY.md](docs/SECURITY.md).

## What it will do

| Area | What it does | Tier |
|---|---|---|
| Home | One-click Boost (Game Mode pipeline), PC Health Score from measured factors, live game HUD, full change log with revert | 0 |
| PC Boost | Explained, reversible system tweaks; RAM Cleaner (Mem Reduct-style, safer defaults); DNS benchmark and switch | 0 |
| Usage Monitor | CPU/GPU/RAM/disk/network per app, "What is this process?", "Why is my game slow?" | 0 |
| Library | Detects installed games from launchers (read-only), per-game profiles, Play with profile | 0 |
| Network Analyzer | Per-app traffic and purpose, connections, quality and bufferbloat tests, Tune-up | 0 |
| Traffic Shaper | Focus mode, per-app block/limit/priority using Windows Firewall + Policy-based QoS (no driver) | 0 |
| Connections | Route Lab: traceroute with network names and plain-language diagnosis; relay routes | 0 / 2 |
| Multi Internet | Connection health and automatic failover; dual-path stability via a relay | 0 / 2 |
| Community Servers | Signed community DNS and relay lists, tested from your PC | 1 / 2 |
| Benchmark Lab | A/B frame-time tests with run-to-run spread; "within noise" is a valid result | 0 |

Tier 0 runs entirely on your PC. Tier 1 uses signed static lists from GitHub. Tier 2 needs an
optional community-run relay.

## Honesty rules

- Every number is measured or labelled as an estimate. No promised FPS or ping gains.
- Every change is logged, verified and reversible. "Revert everything" is always one click away.
- Never injects into games, never reads or writes game memory, never sets Realtime priority,
  never disables Defender, the firewall, UAC, Memory Integrity, Secure Boot or Windows Update for good.
- No tool can guarantee how an anti-cheat behaves; OTX.GNG never claims to be "ban-safe".

## Run the Phase 1 preview

Prerequisites (one time, PowerShell as Administrator): Git, Node.js LTS, Rust (MSVC) and the
Visual Studio C++ Build Tools. See [PROGRESS.md](PROGRESS.md#prerequisites).

```powershell
cd C:\dev\otx-gng\apps\desktop
npm install
npm run dev          # browser preview at http://localhost:1420
npm run tauri dev    # the real frameless window (first run compiles Rust: several minutes)
npm test             # unit tests
```

## License

GPL-3.0-or-later. See [LICENSE](LICENSE) and [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
