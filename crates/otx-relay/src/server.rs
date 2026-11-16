//! The relay server. One UDP socket faces clients; each (client, destination) pair gets its own
//! upstream socket so replies can be routed back. Idle sessions expire after 120 s.

use crate::{decode_data, encode_reply, peek_client, Bucket, Cidr, Dedup, PING};
use std::collections::HashMap;
use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct Config {
    pub listen: SocketAddr,
    /// client id → token
    pub clients: HashMap<u64, Vec<u8>>,
    pub allow: Vec<Cidr>,
    pub packets_per_s: f64,
    pub bytes_per_s: f64,
}

struct Client {
    dedup: Dedup,
    packets: Bucket,
    bytes: Bucket,
    /// Up to two recent source addresses (dual-path: two connections).
    paths: Vec<(SocketAddr, Instant)>,
    upstream: HashMap<SocketAddrV4, (Arc<UdpSocket>, Instant)>,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub forwarded: u64,
    pub dropped_auth: u64,
    pub dropped_dest: u64,
    pub dropped_rate: u64,
    pub dropped_dup: u64,
}

pub struct Relay {
    pub socket: Arc<UdpSocket>,
    cfg: Config,
    clients: Mutex<HashMap<u64, Client>>,
    pub stats: Mutex<Stats>,
    ping_bucket: Mutex<Bucket>,
}

impl Relay {
    pub async fn bind(cfg: Config) -> std::io::Result<Arc<Self>> {
        let socket = Arc::new(UdpSocket::bind(cfg.listen).await?);
        Ok(Arc::new(Self { socket, cfg, clients: Mutex::new(HashMap::new()), stats: Mutex::new(Stats::default()), ping_bucket: Mutex::new(Bucket::new(200.0, 400.0)) }))
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Main loop: never returns unless the socket fails.
    pub async fn run(self: Arc<Self>) -> std::io::Result<()> {
        let mut buf = vec![0u8; 2048];
        let me = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                me.expire().await;
            }
        });
        loop {
            let (n, from) = self.socket.recv_from(&mut buf).await?;
            self.handle(&buf[..n], from).await;
        }
    }

    async fn expire(&self) {
        let mut c = self.clients.lock().await;
        for cl in c.values_mut() {
            cl.upstream.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(120));
            cl.paths.retain(|(_, t)| t.elapsed() < Duration::from_secs(120));
        }
        c.retain(|_, cl| !cl.paths.is_empty());
    }

    async fn handle(self: &Arc<Self>, pkt: &[u8], from: SocketAddr) {
        if pkt.len() == 12 && &pkt[..4] == PING {
            if self.ping_bucket.lock().await.take(1.0) {
                let _ = self.socket.send_to(pkt, from).await;
            }
            return;
        }
        let Some(id) = peek_client(pkt) else { return };
        let Some(token) = self.cfg.clients.get(&id) else {
            self.stats.lock().await.dropped_auth += 1;
            return;
        };
        let Some(f) = decode_data(token, pkt) else {
            self.stats.lock().await.dropped_auth += 1;
            return;
        };
        if !self.cfg.allow.iter().any(|c| c.contains(*f.dest.ip())) {
            self.stats.lock().await.dropped_dest += 1;
            return;
        }
        let up = {
            let mut clients = self.clients.lock().await;
            let cl = clients.entry(id).or_insert_with(|| Client {
                dedup: Dedup::default(),
                packets: Bucket::new(self.cfg.packets_per_s, self.cfg.packets_per_s),
                bytes: Bucket::new(self.cfg.bytes_per_s, self.cfg.bytes_per_s),
                paths: Vec::new(),
                upstream: HashMap::new(),
            });
            // Remember the path even for duplicates, so replies go out on both connections.
            match cl.paths.iter_mut().find(|(a, _)| *a == from) {
                Some(p) => p.1 = Instant::now(),
                None => {
                    cl.paths.push((from, Instant::now()));
                    if cl.paths.len() > 2 {
                        cl.paths.remove(0);
                    }
                }
            }
            if !cl.dedup.accept(f.seq) {
                drop(clients);
                self.stats.lock().await.dropped_dup += 1;
                return;
            }
            if !cl.packets.take(1.0) || !cl.bytes.take(f.payload.len() as f64) {
                drop(clients);
                self.stats.lock().await.dropped_rate += 1;
                return;
            }
            match cl.upstream.get_mut(&f.dest) {
                Some((s, t)) => {
                    *t = Instant::now();
                    s.clone()
                }
                None => {
                    let Ok(s) = UdpSocket::bind("0.0.0.0:0").await.map(Arc::new) else { return };
                    cl.upstream.insert(f.dest, (s.clone(), Instant::now()));
                    self.spawn_return_path(id, f.dest, s.clone());
                    s
                }
            }
        };
        if up.send_to(&f.payload, f.dest).await.is_ok() {
            self.stats.lock().await.forwarded += 1;
        }
    }

    /// Replies from the game server go back to every recent client path.
    fn spawn_return_path(self: &Arc<Self>, id: u64, dest: SocketAddrV4, up: Arc<UdpSocket>) {
        let me = self.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
            loop {
                let r = tokio::time::timeout(Duration::from_secs(130), up.recv_from(&mut buf)).await;
                let Ok(Ok((n, src))) = r else { return };
                if src != SocketAddr::V4(dest) {
                    continue; // only the server this socket was opened for
                }
                let paths: Vec<SocketAddr> = me.clients.lock().await.get(&id).map(|c| c.paths.iter().map(|(a, _)| *a).collect()).unwrap_or_default();
                if paths.is_empty() {
                    return;
                }
                let reply = encode_reply(dest, &buf[..n]);
                for p in paths {
                    let _ = me.socket.send_to(&reply, p).await;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_reply, encode_data, DataFrame};

    async fn setup(allow: &str) -> (Arc<Relay>, SocketAddr, UdpSocket, SocketAddrV4) {
        // A fake "game server" that echoes.
        let game = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let game_addr = match game.local_addr().unwrap() {
            SocketAddr::V4(a) => a,
            _ => unreachable!(),
        };
        tokio::spawn(async move {
            let mut b = [0u8; 2048];
            loop {
                let (n, from) = game.recv_from(&mut b).await.unwrap();
                game.send_to(&b[..n], from).await.unwrap();
            }
        });
        let mut clients = HashMap::new();
        clients.insert(7u64, b"tok".to_vec());
        let cfg = Config { listen: "127.0.0.1:0".parse().unwrap(), clients, allow: vec![Cidr::parse(allow).unwrap()], packets_per_s: 1000.0, bytes_per_s: 1e6 };
        let relay = Relay::bind(cfg).await.unwrap();
        let addr = relay.local_addr().unwrap();
        tokio::spawn(relay.clone().run());
        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        (relay, addr, client, game_addr)
    }

    async fn recv(s: &UdpSocket) -> Option<Vec<u8>> {
        let mut b = [0u8; 2048];
        tokio::time::timeout(Duration::from_millis(500), s.recv_from(&mut b)).await.ok().and_then(|r| r.ok()).map(|(n, _)| b[..n].to_vec())
    }

    #[tokio::test]
    async fn relays_to_allowed_servers_with_auth_and_dedup() {
        let (relay, addr, client, game) = setup("127.0.0.0/8").await;
        let f = DataFrame { client_id: 7, seq: 1, dest: game, payload: b"move".to_vec() };
        // Send the same packet twice (dual path): the server must see it once.
        client.send_to(&encode_data(b"tok", &f), addr).await.unwrap();
        client.send_to(&encode_data(b"tok", &f), addr).await.unwrap();
        let r = recv(&client).await.expect("reply through the relay");
        assert_eq!(decode_reply(&r), Some((game, &b"move"[..])));
        assert!(recv(&client).await.is_none(), "duplicate was forwarded");
        let s = *relay.stats.lock().await;
        assert_eq!((s.forwarded, s.dropped_dup), (1, 1));

        // Wrong token: dropped.
        client.send_to(&encode_data(b"bad", &DataFrame { seq: 2, ..f.clone() }), addr).await.unwrap();
        assert!(recv(&client).await.is_none());
        // Ping is echoed.
        let ping = [b"OTXP".as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]].concat();
        client.send_to(&ping, addr).await.unwrap();
        assert_eq!(recv(&client).await.as_deref(), Some(ping.as_slice()));
        assert_eq!(relay.stats.lock().await.dropped_auth, 1);
    }

    #[tokio::test]
    async fn never_an_open_proxy() {
        let (relay, addr, client, game) = setup("203.0.113.0/24").await; // game (127.0.0.1) not allowed
        let f = DataFrame { client_id: 7, seq: 1, dest: game, payload: b"x".to_vec() };
        client.send_to(&encode_data(b"tok", &f), addr).await.unwrap();
        assert!(recv(&client).await.is_none());
        assert_eq!(relay.stats.lock().await.dropped_dest, 1);
    }
}
