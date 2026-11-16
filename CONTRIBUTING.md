# Contributing to OTX.GNG

Thanks for helping. OTX.GNG is GPL-3.0-or-later.

## Ground rules

- **Honesty first.** No invented gains. New optimizations need a plausible mechanism, an evidence
  label (documented / community-reported / experimental) and, where possible, a Benchmark Lab result.
- **Safety first.** Never add: injection, game-memory access, Realtime priority, or tweaks that
  disable Defender, the firewall, UAC, Memory Integrity, Secure Boot or Windows Update.
- **Allow-list.** The Core accepts only typed requests (`crates/otx-protocol`). A new capability is
  a new enum variant with validated parameters, never a path, command line or registry path from the UI.
- **Reversible.** Every change is journaled before it is applied and has a tested revert.

## Adding a tweak

Add an entry to `crates/otx-tweaks/data/manifests.json` (data, not code) and its card text in
`apps/desktop/src/backend/mockData.ts`. The tests apply, verify and revert every manifest on a
fake system and reject forbidden targets.

## Build and test

```
cargo test -p otx-protocol -p otx-sysinfo -p otx-monitor -p otx-tweaks -p otx-net -p otx-games -p otx-core -p otx-relay
cd apps/desktop && npm ci && npm test && npm run typecheck
npm run tauri dev
```

Tests that change real system state are `#[ignore]` and must only be run in a VM (or, for the
firewall test, they use a rule for a non-existent path and remove it).

## Translations

UI strings live in `apps/desktop/src/i18n/en.json`. Add `<lang>.json` with the same keys.
