# Privacy

OTX.GNG — Built and performed by Thinuda.

- **No account, no ads, no telemetry.** There is no OTX.GNG server. The "share anonymous usage
  statistics" switch exists for the future, is off by default, and sends nothing in this version.
- **Everything stays on your PC:** system facts, process lists, network counters, settings,
  game profiles, relay tokens and the change journal.
- **Network requests OTX.GNG makes, only when you start them:**
  - DNS Optimizer: DNS queries for random names under `example.com` to the resolvers you test.
  - Quality Test: pings to your target; downloads ~25 MB from and uploads ~10 MB to
    Cloudflare's public speed-test endpoints (`speed.cloudflare.com`) for the bufferbloat part.
  - Route analysis: ICMP probes toward your target.
  - Live ping: one small ICMP echo per second to the detected game server, or `1.1.1.1`, while
    the window is open.
  - Relay test: three small UDP pings to relays you added.
  - "Look up online" in the Usage Monitor opens your browser with the exe file name only.
- **Crash reports** (when enabled) stay on this PC as a file you can choose to attach to a GitHub issue.
- **Exported reports and settings** contain no user name, paths, hardware serials or IP addresses.
