//! OTX Relay wire protocol and server.
//!
//! Client → relay data frame (all integers big-endian):
//! ```text
//! "OTX1" | client_id (8) | seq (4) | dest ip (4) | dest port (2) | tag (16) | payload
//! ```
//! `tag` = first 16 bytes of HMAC-SHA256(token, everything before the tag + payload).
//! Relay → client: "OTXR" | seq-less | src ip (4) | src port (2) | payload (no tag: the client
//! only accepts replies from the relay address it is talking to).
//! Ping: "OTXP" | 8 bytes nonce → echoed back unchanged (rate-limited, no auth needed).
//!
//! Safety properties (all unit/integration tested):
//! * unknown client ids or bad tags are dropped;
//! * destinations outside the allow-list are dropped (never an open proxy);
//! * per-client token-bucket rate limits;
//! * duplicate `seq` within a window is dropped (dual-path: first copy wins);
//! * payloads are never logged or stored.

pub mod server;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::net::{Ipv4Addr, SocketAddrV4};

pub const DATA: &[u8; 4] = b"OTX1";
pub const REPLY: &[u8; 4] = b"OTXR";
pub const PING: &[u8; 4] = b"OTXP";
pub const HEADER: usize = 4 + 8 + 4 + 4 + 2 + 16;
pub const MAX_PAYLOAD: usize = 1400;

type HmacSha256 = Hmac<Sha256>;

fn tag(token: &[u8], head: &[u8], payload: &[u8]) -> [u8; 16] {
    let mut mac = HmacSha256::new_from_slice(token).expect("HMAC accepts any key length");
    mac.update(head);
    mac.update(payload);
    let full = mac.finalize().into_bytes();
    let mut t = [0u8; 16];
    t.copy_from_slice(&full[..16]);
    t
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataFrame {
    pub client_id: u64,
    pub seq: u32,
    pub dest: SocketAddrV4,
    pub payload: Vec<u8>,
}

pub fn encode_data(token: &[u8], f: &DataFrame) -> Vec<u8> {
    let mut head = Vec::with_capacity(HEADER + f.payload.len());
    head.extend_from_slice(DATA);
    head.extend_from_slice(&f.client_id.to_be_bytes());
    head.extend_from_slice(&f.seq.to_be_bytes());
    head.extend_from_slice(&f.dest.ip().octets());
    head.extend_from_slice(&f.dest.port().to_be_bytes());
    let t = tag(token, &head, &f.payload);
    head.extend_from_slice(&t);
    head.extend_from_slice(&f.payload);
    head
}

/// Parses the header without verifying (to look up the client's token by id).
pub fn peek_client(buf: &[u8]) -> Option<u64> {
    if buf.len() < HEADER || &buf[..4] != DATA {
        return None;
    }
    Some(u64::from_be_bytes(buf[4..12].try_into().ok()?))
}

/// Verifies the tag with `token` and decodes.
pub fn decode_data(token: &[u8], buf: &[u8]) -> Option<DataFrame> {
    if buf.len() < HEADER || &buf[..4] != DATA || buf.len() - HEADER > MAX_PAYLOAD {
        return None;
    }
    let head = &buf[..HEADER - 16];
    let got = &buf[HEADER - 16..HEADER];
    let payload = &buf[HEADER..];
    let want = tag(token, head, payload);
    // Constant-time compare.
    if got.iter().zip(want.iter()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) != 0 {
        return None;
    }
    Some(DataFrame {
        client_id: u64::from_be_bytes(buf[4..12].try_into().ok()?),
        seq: u32::from_be_bytes(buf[12..16].try_into().ok()?),
        dest: SocketAddrV4::new(Ipv4Addr::new(buf[16], buf[17], buf[18], buf[19]), u16::from_be_bytes([buf[20], buf[21]])),
        payload: payload.to_vec(),
    })
}

pub fn encode_reply(src: SocketAddrV4, payload: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(10 + payload.len());
    v.extend_from_slice(REPLY);
    v.extend_from_slice(&src.ip().octets());
    v.extend_from_slice(&src.port().to_be_bytes());
    v.extend_from_slice(payload);
    v
}

pub fn decode_reply(buf: &[u8]) -> Option<(SocketAddrV4, &[u8])> {
    if buf.len() < 10 || &buf[..4] != REPLY {
        return None;
    }
    Some((SocketAddrV4::new(Ipv4Addr::new(buf[4], buf[5], buf[6], buf[7]), u16::from_be_bytes([buf[8], buf[9]])), &buf[10..]))
}

/// An IPv4 CIDR range ("162.249.72.0/21").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr {
    net: u32,
    mask: u32,
}

impl Cidr {
    pub fn parse(s: &str) -> Option<Self> {
        let (ip, len) = s.trim().split_once('/').unwrap_or((s.trim(), "32"));
        let ip: Ipv4Addr = ip.parse().ok()?;
        let len: u32 = len.parse().ok().filter(|l| *l <= 32)?;
        let mask = if len == 0 { 0 } else { u32::MAX << (32 - len) };
        Some(Self { net: u32::from(ip) & mask, mask })
    }
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & self.mask == self.net
    }
}

/// Sliding window de-duplication of sequence numbers (dual-path: first copy wins).
#[derive(Debug, Default)]
pub struct Dedup {
    highest: u32,
    seen: u64, // bit i = highest - i was seen
    started: bool,
}

impl Dedup {
    /// True if `seq` is new (and records it).
    pub fn accept(&mut self, seq: u32) -> bool {
        if !self.started {
            self.started = true;
            self.highest = seq;
            self.seen = 1;
            return true;
        }
        if seq > self.highest {
            let shift = seq - self.highest;
            self.seen = if shift >= 64 { 0 } else { self.seen << shift };
            self.seen |= 1;
            self.highest = seq;
            true
        } else {
            let back = self.highest - seq;
            if back >= 64 {
                return false; // too old
            }
            let bit = 1u64 << back;
            if self.seen & bit != 0 {
                false
            } else {
                self.seen |= bit;
                true
            }
        }
    }
}

/// Token bucket.
#[derive(Debug)]
pub struct Bucket {
    rate: f64,
    burst: f64,
    level: f64,
    last: std::time::Instant,
}

impl Bucket {
    pub fn new(rate_per_s: f64, burst: f64) -> Self {
        Self { rate: rate_per_s, burst, level: burst, last: std::time::Instant::now() }
    }
    pub fn take(&mut self, n: f64) -> bool {
        let now = std::time::Instant::now();
        self.level = (self.level + now.duration_since(self.last).as_secs_f64() * self.rate).min(self.burst);
        self.last = now;
        if self.level >= n {
            self.level -= n;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_reject_tampering() {
        let f = DataFrame { client_id: 42, seq: 7, dest: "203.0.113.5:7086".parse().unwrap(), payload: b"hello game".to_vec() };
        let mut bytes = encode_data(b"secret-token", &f);
        assert_eq!(peek_client(&bytes), Some(42));
        assert_eq!(decode_data(b"secret-token", &bytes), Some(f.clone()));
        assert_eq!(decode_data(b"wrong-token", &bytes), None);
        let last = bytes.len() - 1;
        bytes[last] ^= 1; // tamper with the payload
        assert_eq!(decode_data(b"secret-token", &bytes), None);
        let mut redirect = encode_data(b"secret-token", &f);
        redirect[16] = 10; // tamper with the destination
        assert_eq!(decode_data(b"secret-token", &redirect), None);
        let r = encode_reply("203.0.113.5:7086".parse().unwrap(), b"pong");
        assert_eq!(decode_reply(&r), Some(("203.0.113.5:7086".parse().unwrap(), &b"pong"[..])));
    }

    #[test]
    fn cidr_allow_list() {
        let c = Cidr::parse("162.249.72.0/21").unwrap();
        assert!(c.contains("162.249.79.255".parse().unwrap()));
        assert!(!c.contains("162.249.80.0".parse().unwrap()));
        assert!(Cidr::parse("10.0.0.1").unwrap().contains("10.0.0.1".parse().unwrap()));
        assert!(Cidr::parse("1.2.3.4/33").is_none());
        assert!(Cidr::parse("nope/8").is_none());
    }

    #[test]
    fn dedup_window_first_copy_wins() {
        let mut d = Dedup::default();
        assert!(d.accept(10));
        assert!(!d.accept(10));
        assert!(d.accept(12));
        assert!(d.accept(11)); // late but new
        assert!(!d.accept(11));
        assert!(d.accept(200));
        assert!(!d.accept(100)); // too old
    }

    #[test]
    fn token_bucket_limits() {
        let mut b = Bucket::new(10.0, 3.0);
        assert!(b.take(1.0) && b.take(1.0) && b.take(1.0));
        assert!(!b.take(1.0));
    }
}
