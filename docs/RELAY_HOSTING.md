# Hosting an OTX Relay (Tier 2, optional)

An OTX Relay forwards game traffic for players you give access to. It is a single small binary
(or one Docker container). It is **never an open proxy**:

- every packet is authenticated with the player's own token (HMAC-SHA256);
- it only reaches the game-server IP ranges you list in `allow.txt`;
- each player is rate-limited (default 2000 packets/s and 2 MB/s);
- packet contents are never logged or stored; only counters exist, in memory.

## What works today, and what does not

- **Works:** the relay server; authenticated forwarding; dual-path de-duplication (the first copy
  wins); the latency ping used by OTX.GNG's Community Servers page.
- **Not finished:** redirecting a game's traffic on Windows into the relay. That needs a network
  driver on the player's PC (WireGuard-NT or WinDivert), which must be bundled, code-signed and
  checked against each anti-cheat in a VM before it ships. Until then, OTX.GNG can measure relay
  latency but does not route games through relays.

## Run it

1. Create `relay-config/clients.txt`, one line per player: `<client id in hex> <token>`. Make each
   token random and at least 16 characters (for example from a password manager).
   ```
   1a2b3c4d  replace-with-a-long-random-token
   ```
2. Create `relay-config/allow.txt` with the game-server ranges this relay may reach, one IPv4 CIDR
   per line. Use ranges the game's publisher documents; do not allow `0.0.0.0/0`.
3. Start it:
   ```
   docker build -f crates/otx-relay/Dockerfile -t otx-relay .
   docker run -d --name otx-relay --restart unless-stopped -p 51888:51888/udp -v "$PWD/relay-config:/config:ro" otx-relay
   ```
   Or without Docker: `cargo run --release -p otx-relay -- --listen 0.0.0.0:51888 --clients clients.txt --allow allow.txt`.
4. Open UDP port 51888 in the server's firewall.
5. Give each player the address (`your.host:51888`) and their token. They add it in OTX.GNG →
   Community Servers.

## Free hosting and bandwidth (estimates, check current terms)

Typical competitive game traffic is about 100 kbit/s down + 60 kbit/s up per player, which is
about **72 MB per player-hour** the relay must send out. Dual-path roughly doubles the
client-to-relay leg.

| Host | Free egress | Player-hours per month (≈) |
|---|---|---|
| Oracle Cloud Always Free (ARM VM) | 10 TB/month | ≈ 139,000 (≈ 190 players around the clock) |
| Google Cloud e2-micro free tier | 1 GB/month | ≈ 14 (not usable) |
| A home PC on a fast connection | your upload | limited by your upload speed and ISP terms |

Free tiers change; Oracle asks for a card to verify identity (it is not charged on the free tier).
CPU is not the bottleneck: about 128 packets/s per player is trivial.

## Latency

A relay only helps when it gives a better path than your ISP's. Put it close to the game servers
(same city or data centre region) and compare in OTX.GNG: Community Servers → Test shows your
latency to the relay; Connections → Route analysis shows the direct path.
