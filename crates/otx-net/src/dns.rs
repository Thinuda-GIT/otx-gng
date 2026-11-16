//! DNS benchmark with real queries. Each sample asks for a random, uncached name under
//! `example.com` (IANA-reserved for exactly this kind of use), so the resolver must actually
//! resolve it instead of answering from its cache. Measures round-trip time to the answer.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Builds a standard recursive A query.
pub fn build_query(id: u16, name: &str) -> Vec<u8> {
    let mut q = Vec::with_capacity(64);
    q.extend_from_slice(&id.to_be_bytes());
    q.extend_from_slice(&[0x01, 0x00]); // recursion desired
    q.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0]); // 1 question
    for label in name.trim_end_matches('.').split('.') {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.push(0);
    q.extend_from_slice(&[0, 1, 0, 1]); // type A, class IN
    q
}

/// Returns the response code if `resp` answers query `id` (any rcode counts as an answer:
/// NXDOMAIN for a random name is the expected, correct reply).
pub fn parse_reply(resp: &[u8], id: u16) -> Option<u8> {
    if resp.len() < 12 || u16::from_be_bytes([resp[0], resp[1]]) != id || resp[2] & 0x80 == 0 {
        return None;
    }
    Some(resp[3] & 0x0F)
}

#[derive(Debug, Clone)]
pub struct Bench {
    pub server: IpAddr,
    pub samples: Vec<f64>,
    pub failures: u32,
}

impl Bench {
    pub fn median(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut s = self.samples.clone();
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let m = s.len() / 2;
        Some(if s.len() % 2 == 1 { s[m] } else { (s[m - 1] + s[m]) / 2.0 })
    }
    pub fn jitter(&self) -> f64 {
        if self.samples.len() < 2 {
            return 0.0;
        }
        let mean = self.samples.iter().sum::<f64>() / self.samples.len() as f64;
        (self.samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / self.samples.len() as f64).sqrt()
    }
}

fn random_label(seed: u64) -> String {
    // xorshift: no crypto needed, only uniqueness per query.
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut out = String::from("otx-");
    for _ in 0..12 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push((b'a' + (x % 26) as u8) as char);
    }
    out
}

pub fn benchmark(server: IpAddr, samples: u32, timeout: Duration) -> Bench {
    let bind: SocketAddr = if server.is_ipv4() { "0.0.0.0:0".parse().expect("valid") } else { "[::]:0".parse().expect("valid") };
    let mut b = Bench { server, samples: Vec::new(), failures: 0 };
    let Ok(sock) = UdpSocket::bind(bind) else {
        b.failures = samples;
        return b;
    };
    let _ = sock.set_read_timeout(Some(timeout));
    let seed0 = Instant::now().elapsed().as_nanos() as u64 ^ (std::process::id() as u64) << 20;
    for i in 0..samples {
        let id = (seed0.wrapping_add(i as u64 * 7919) & 0xFFFF) as u16;
        let name = format!("{}.example.com", random_label(seed0.wrapping_add(i as u64 * 104_729)));
        let q = build_query(id, &name);
        let t = Instant::now();
        if sock.send_to(&q, SocketAddr::new(server, 53)).is_err() {
            b.failures += 1;
            continue;
        }
        let mut buf = [0u8; 512];
        let mut ok = false;
        while t.elapsed() < timeout {
            match sock.recv_from(&mut buf) {
                Ok((n, from)) if from.ip() == server && parse_reply(&buf[..n], id).is_some() => {
                    b.samples.push(t.elapsed().as_secs_f64() * 1000.0);
                    ok = true;
                    break;
                }
                Ok(_) => continue, // late reply to an earlier query
                Err(_) => break,
            }
        }
        if !ok {
            b.failures += 1;
        }
        std::thread::sleep(Duration::from_millis(40)); // be polite to public resolvers
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_encoding_and_reply_parsing() {
        let q = build_query(0xBEEF, "abc.example.com");
        assert_eq!(&q[..2], &[0xBE, 0xEF]);
        assert_eq!(q[2], 0x01);
        assert_eq!(&q[12..16], &[3, b'a', b'b', b'c']);
        assert_eq!(&q[q.len() - 4..], &[0, 1, 0, 1]);
        let mut reply = q.clone();
        reply[2] |= 0x80; // QR = response
        reply[3] = 0x83; // RA + NXDOMAIN
        assert_eq!(parse_reply(&reply, 0xBEEF), Some(3));
        assert_eq!(parse_reply(&reply, 0x1234), None);
        assert_eq!(parse_reply(&q, 0xBEEF), None, "a query is not a reply");
        assert_ne!(random_label(1), random_label(2));
    }

    #[test]
    fn statistics() {
        let b = Bench { server: "1.1.1.1".parse().unwrap(), samples: vec![10.0, 30.0, 20.0], failures: 0 };
        assert_eq!(b.median(), Some(20.0));
        assert!((b.jitter() - 8.165).abs() < 0.01);
    }

    /// Sends a few real DNS queries; run with `cargo test -p otx-net -- --ignored`.
    #[test]
    #[ignore]
    fn live_benchmark() {
        let b = benchmark("1.1.1.1".parse().unwrap(), 3, Duration::from_secs(2));
        println!("1.1.1.1: median {:?} ms, failures {}", b.median(), b.failures);
        assert!(b.failures < 3);
    }
}
