# OTX.GNG user guide

**OTX.GNG — Built and performed by Thinuda.** Free, honest, measurable game boosting.

## Install

1. Download `OTX.GNG_<version>_x64-setup.exe` from the GitHub Releases page and check its SHA-256
   against the one published next to it.
2. The installer is not code-signed yet, so Windows SmartScreen shows "Windows protected your PC".
   Select **More info → Run anyway** if the checksum matches.
3. The installer adds the **OTX.GNG Core** service (it applies and reverts changes) and installs
   the WebView2 runtime if your PC lacks it.

Uninstalling reverts every change OTX.GNG made, then removes the service.

## First run

The setup shows your hardware, what OTX.GNG may change and what it never touches, and asks two
questions: start with Windows (off by default) and usage statistics (off; nothing is sent anyway).

## Pages

- **Home**: Boost (runs Game Mode for the running game, or a tune-up), PC Health Score from
  measured values, the live game card, and **What OTX.GNG changed** with Revert and Revert everything.
- **PC Boost**: tweaks with risk, evidence and "not recommended if" checked against your PC;
  Dry run shows exact values first. RAM Cleaner (Smart mode by default). DNS Optimizer with real
  benchmarks.
- **Usage Monitor**: CPU, GPU, memory and I/O per app; "What is this process?" with the file's
  signature; lower priority, efficiency mode or end task (protected processes are never touched).
- **Library**: games from Steam, Epic, Riot and installed programs. Profiles set priority,
  memory cleaning, power plan, GPU preference and the fullscreen-optimizations flag. Play cleans
  memory first, then starts the game; Game Mode turns on when the game appears and everything is
  reverted when it closes, even if OTX.GNG's window is closed.
- **Network Analyzer**: per-app traffic, connections, Quality Test (ping, jitter, loss,
  bufferbloat), Tune-up.
- **Traffic Shaper**: Focus mode blocks or limits listed apps with Windows Firewall and QoS;
  **Restore normal internet** removes every rule (also from the tray and Shift+Ctrl+Alt+R).
- **Connections**: route analysis to a server, live ping. Relays need Tier 2 (see below).
- **Multi Internet**: health of each connection, prefer one, automatic failover.
- **Benchmark Lab**: start the game, then run A/B tests (baseline vs Game Mode, at least 3 runs
  each). "Within noise" means no measurable difference, and that is a valid result.

## Safety

- Every change is journaled before it happens and can be undone. Temporary changes (Game Mode,
  Focus mode) revert automatically at the end of the session, after a crash and at reboot.
- A System Restore point is requested before the first tweak of the day (if System Protection is on).
- Safe Mode: set the system environment variable `OTX_SAFE_MODE=1` and restart the
  "OTX.GNG Core" service; it then makes no changes at all.

## Tier 2 relays

See [RELAY_HOSTING.md](RELAY_HOSTING.md). You can add a relay and measure its latency; routing
games through relays is not available yet.
