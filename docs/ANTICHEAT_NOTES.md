# OTX.GNG and anti-cheat

**No tool can guarantee how an anti-cheat will behave, and OTX.GNG never claims to be "ban-safe".**
This page lists exactly what OTX.GNG does and does not do, so you (and anti-cheat vendors) can judge.

## What OTX.GNG never does

- Inject code into, hook, debug, or read/write the memory of any process, including games.
- Modify game files. The one opt-in exception planned is the Unreal Engine config tuner, which
  edits the game's own user config file only after showing you a diff and making a backup; it is
  not enabled in this version.
- Draw overlays inside games.
- Load kernel drivers. This version ships none (the optional WinDivert limiter and sensor drivers
  are not included).
- Touch anti-cheat processes or services (Vanguard, Easy Anti-Cheat, BattlEye, FACEIT, ESEA,
  PunkBuster, nProtect, XIGNCODE). They are on the protected list in the Core, which refuses any
  action on them.

## What it does (all documented Windows interfaces)

| Feature | Interface | Touches the game? |
|---|---|---|
| Usage Monitor | `NtQuerySystemInformation` (one system-wide call), PDH counters | No handle to any process |
| Game detection | Process list by exe name | No handle to the game |
| Game Mode: game priority | `SetPriorityClass` (High at most, never Realtime) | Opens the game with `PROCESS_SET_INFORMATION`, like Task Manager. Some anti-cheats deny this; the step is then reported as failed and nothing else changes |
| Game Mode: watchdog | Polls the process list every 2 s | No handle |
| Background apps | Priority, EcoQoS, memory priority, working-set trim | Never the game (skip list) |
| Frame capture | DXGI ETW events (Windows' own tracing) | No handle; only Present timestamps |
| Network traffic | Kernel-Network ETW events | No handle; byte counts only |
| Traffic Shaper | Windows Firewall rules, Policy-based QoS | Never applied to the game |
| GPU preference / fullscreen flag | Per-user registry values Windows reads at launch | Registry only |

## Detected anti-cheats

The Compatibility page lists anti-cheat services found on your PC (read-only Service Control
Manager queries) and whether they are running.
