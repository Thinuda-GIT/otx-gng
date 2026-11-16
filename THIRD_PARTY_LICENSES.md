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

The Rust crate tree (`tauri` and its transitive dependencies) is listed in full by
`cargo about` / `cargo deny` from Phase 2 on, when the workspace gains its own crates.

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
| windows-rs (`windows` crate) | MIT OR Apache-2.0 | 2 | Win32/NT APIs |
| windows-service | MIT OR Apache-2.0 | 2 | Service host |
| ferrisetw | MIT OR Apache-2.0 | 3, 6 | ETW consumer |
| ts-rs | MIT | 2 | Rust → TypeScript types |
| PresentMon | MIT (verify at bundling) | 8 | Frame-time capture |
| DB-IP Lite (country/ASN) | CC BY 4.0 (attribution required) | 6 | Offline IP database |
| WinDivert (optional) | LGPL-3.0 (chosen option of LGPLv3/GPLv2 dual license) | 6 | Inbound limiter, off by default |
| wireguard-nt (optional) | Prebuilt signed binaries, redistributable (verify terms) | 9 | Relay tunnel |
| LibreHardwareMonitor (optional) | MPL-2.0 | 10 | Sensors, off by default |

MaxMind GeoLite2 is deliberately **not** used: its license does not allow redistribution inside the app.
