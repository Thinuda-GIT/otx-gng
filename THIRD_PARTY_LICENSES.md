# Third-party licenses

OTX.GNG is licensed under **GPL-3.0-or-later** (see `LICENSE`). Every component below is free and
open source, and every license listed is compatible with GPL-3.0-or-later. Versions are the ones
pinned in `apps/desktop/package-lock.json` / `Cargo.lock` at the time of writing; CI re-checks
licenses on every build from Phase 10 on (`license-checker` for npm, `cargo-deny` for Rust).

## Bundled in the app (runtime)

| Component | Version | License | Use |
|---|---|---|---|
| React, React DOM | 19.3.1 | MIT | UI framework |
| Motion (formerly Framer Motion) | 12.43.0 | MIT | Spring animations |
| Zustand | 5.1.0 | MIT | State |
| TanStack Virtual | 3.14.26 | MIT | Virtualized tables |
| i18next / react-i18next | 25.10.10 / 15.7.4 | MIT | Translations |
| Lucide React | 1.73.0 | ISC | Icons |
| @tauri-apps/api | 2.12.4 | Apache-2.0 OR MIT | Window API bridge |
| Tauri (Rust crates) | 2.x | Apache-2.0 OR MIT | Desktop shell |
| Inter (via @fontsource) | 5.3.0 | SIL OFL 1.1 | Body font |
| Rajdhani (via @fontsource) | 5.3.0 | SIL OFL 1.1 | Heading font |
| JetBrains Mono (via @fontsource) | 5.3.0 | SIL OFL 1.1 | Numbers |
| Microsoft Edge WebView2 Runtime | evergreen | Microsoft redistribution terms (free) | Installed by the bootstrapper if missing; not bundled |

### Rust crates (direct dependencies, Phase 2)

| Crate | Version | License | Use |
|---|---|---|---|
| tauri, tauri-build | 2.12.5 / 2.7.4 | Apache-2.0 OR MIT | Desktop shell |
| tauri-plugin-autostart | 2.7.0 | Apache-2.0 OR MIT | Launch on startup (HKCU Run entry) |
| tauri-plugin-global-shortcut | 2.4.1 | Apache-2.0 OR MIT | Global hotkeys |
| tauri-plugin-single-instance | 2.5.5 | Apache-2.0 OR MIT | One running copy |
| windows (windows-rs) | 0.61.3 | MIT OR Apache-2.0 | Win32 APIs |
| windows-service | 0.8.1 | MIT OR Apache-2.0 | Service host and SCM control |
| winreg | 0.56.0 | MIT | Registry reads |
| tokio | 1.54 | MIT | Async named pipes |
| serde, serde_json | 1.0 | MIT OR Apache-2.0 | JSON protocol |
| ts-rs | 12.0.1 | MIT | Rust → TypeScript protocol types |
| thiserror, anyhow | 2.0 / 1.0 | MIT OR Apache-2.0 | Errors |
| tracing, tracing-subscriber, tracing-appender | 0.1 / 0.3 / 0.2 | MIT | Logging |
| rand, hex | 0.9 / 0.4 | MIT OR Apache-2.0 | Session ids |
| @tauri-apps/plugin-autostart (npm) | 2.7.0 | MIT OR Apache-2.0 | Autostart from the UI |

**Full transitive tree (466 crates, checked with `cargo metadata` on 2026-11-16):** every crate is
under MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, Unlicense, CC0/MIT-0, Unicode-3.0, or MPL-2.0
(7 crates pulled in by Tauri's HTML tooling). All are compatible with GPL-3.0-or-later; MPL-2.0 is
file-level copyleft and explicitly GPL-compatible. No crate is unlicensed.

## Build tools only (not shipped)

| Component | Version | License |
|---|---|---|
| Vite | 7.3.8 | MIT |
| @vitejs/plugin-react | 5.2.0 | MIT |
| Tailwind CSS / @tailwindcss/vite | 4.3.5 | MIT |
| TypeScript | 5.9.3 | Apache-2.0 |
| Vitest | 3.2.7 | MIT |
| @tauri-apps/cli | 2.12.4 | Apache-2.0 OR MIT |
| NSIS (via Tauri bundler) | 3.x | zlib/libpng |

## Planned (added in later phases, re-verified when added)

| Component | License | Phase | Note |
|---|---|---|---|
| ferrisetw | MIT OR Apache-2.0 | 3, 6 | ETW consumer |
| PresentMon | MIT (verify at bundling) | 8 | Frame-time capture |
| DB-IP Lite (country/ASN) | CC BY 4.0 (attribution required) | 6 | Offline IP database |
| WinDivert (optional) | LGPL-3.0 (chosen option of LGPLv3/GPLv2 dual license) | 6 | Inbound limiter, off by default |
| wireguard-nt (optional) | Prebuilt signed binaries, redistributable (verify terms) | 9 | Relay tunnel |
| LibreHardwareMonitor (optional) | MPL-2.0 | 10 | Sensors, off by default |

MaxMind GeoLite2 is deliberately **not** used: its license does not allow redistribution inside the app.
