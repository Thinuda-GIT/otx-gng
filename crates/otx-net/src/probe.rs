//! Latency probes, traceroute and path-MTU discovery (Windows ICMP API, no raw sockets, no
//! elevation). ICMP is IPv4 here; many game servers drop ICMP, so a TCP-connect probe is offered.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::IpHelper::{
    IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho2Ex, ICMP_ECHO_REPLY, IP_FLAG_DF, IP_OPTION_INFORMATION, IP_PACKET_TOO_BIG, IP_SUCCESS,
    IP_TTL_EXPIRED_TRANSIT,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reply {
    /// Echo reply from the target, round-trip ms.
    Echo(f64),
    /// A router on the way answered (TTL expired), with its address and ms.
    Hop(Ipv4Addr, f64),
    TooBig,
    Timeout,
}

struct Icmp(HANDLE);
impl Drop for Icmp {
    fn drop(&mut self) {
        unsafe {
            let _ = IcmpCloseHandle(self.0);
        }
    }
}

/// Resolves a host name or IP literal to its first IPv4 address.
pub fn resolve_v4(host: &str) -> Option<Ipv4Addr> {
    if let Ok(IpAddr::V4(a)) = host.parse::<IpAddr>() {
        return Some(a);
    }
    (host, 0).to_socket_addrs().ok()?.find_map(|a| match a.ip() {
        IpAddr::V4(v) => Some(v),
        _ => None,
    })
}

/// One ICMP echo. `source` binds the probe to an adapter's address (Multi Internet health checks).
pub fn echo(target: Ipv4Addr, ttl: u8, payload: u16, dont_fragment: bool, timeout_ms: u32, source: Option<Ipv4Addr>) -> Reply {
    let Ok(h) = (unsafe { IcmpCreateFile() }) else { return Reply::Timeout };
    let h = Icmp(h);
    let data = vec![0x4Fu8; payload as usize];
    let opts = IP_OPTION_INFORMATION { Ttl: ttl, Tos: 0, Flags: if dont_fragment { IP_FLAG_DF as u8 } else { 0 }, OptionsSize: 0, OptionsData: std::ptr::null_mut() };
    let mut reply = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + payload as usize + 64];
    let t = Instant::now();
    let n = unsafe {
        IcmpSendEcho2Ex(
            h.0,
            None,
            None,
            None,
            source.map(|s| u32::from_ne_bytes(s.octets())).unwrap_or(0),
            u32::from_ne_bytes(target.octets()),
            data.as_ptr().cast(),
            payload,
            Some(&opts),
            reply.as_mut_ptr().cast(),
            reply.len() as u32,
            timeout_ms,
        )
    };
    let elapsed = t.elapsed().as_secs_f64() * 1000.0;
    // SAFETY: the buffer starts with an ICMP_ECHO_REPLY when the call wrote at least one reply,
    // and on failure the status field still describes the error.
    let r = unsafe { std::ptr::read_unaligned(reply.as_ptr() as *const ICMP_ECHO_REPLY) };
    let from = Ipv4Addr::from(r.Address.to_ne_bytes());
    // The API's RoundTripTime is whole milliseconds; prefer our own sub-ms timer, bounded by it.
    let ms = if (r.RoundTripTime as f64) > elapsed { r.RoundTripTime as f64 } else { elapsed.max(r.RoundTripTime as f64) };
    match r.Status {
        IP_SUCCESS if n > 0 => Reply::Echo(ms),
        IP_TTL_EXPIRED_TRANSIT => Reply::Hop(from, ms),
        IP_PACKET_TOO_BIG => Reply::TooBig,
        _ => Reply::Timeout,
    }
}

/// TCP connect time to host:port (works where ICMP is filtered; one TCP handshake per sample).
pub fn tcp_connect(addr: SocketAddr, timeout: Duration) -> Option<f64> {
    let t = Instant::now();
    let s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    drop(s);
    Some(ms)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub sent: u32,
    pub received: u32,
    pub median_ms: Option<f64>,
    /// Mean absolute difference between consecutive replies (RFC 3550-style jitter, simplified).
    pub jitter_ms: f64,
    pub loss_pct: f64,
    pub samples: Vec<f64>,
}

pub fn stats(samples: &[Option<f64>]) -> Stats {
    let ok: Vec<f64> = samples.iter().flatten().copied().collect();
    let mut sorted = ok.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if sorted.is_empty() {
        None
    } else if sorted.len() % 2 == 1 {
        Some(sorted[sorted.len() / 2])
    } else {
        Some((sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0)
    };
    let jitter = if ok.len() < 2 { 0.0 } else { ok.windows(2).map(|w| (w[1] - w[0]).abs()).sum::<f64>() / (ok.len() - 1) as f64 };
    let sent = samples.len() as u32;
    Stats {
        sent,
        received: ok.len() as u32,
        median_ms: median,
        jitter_ms: jitter,
        loss_pct: if sent == 0 { 0.0 } else { 100.0 * (sent - ok.len() as u32) as f64 / sent as f64 },
        samples: ok,
    }
}

/// `count` ICMP echoes, `gap` apart.
pub fn ping_series(target: Ipv4Addr, count: u32, gap: Duration) -> Stats {
    let mut s = Vec::with_capacity(count as usize);
    for i in 0..count {
        s.push(match echo(target, 128, 32, false, 1000, None) {
            Reply::Echo(ms) => Some(ms),
            _ => None,
        });
        if i + 1 < count {
            std::thread::sleep(gap);
        }
    }
    stats(&s)
}

pub fn tcp_series(addr: SocketAddr, count: u32, gap: Duration) -> Stats {
    let mut s = Vec::with_capacity(count as usize);
    for i in 0..count {
        s.push(tcp_connect(addr, Duration::from_millis(1500)));
        if i + 1 < count {
            std::thread::sleep(gap);
        }
    }
    stats(&s)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HopResult {
    pub n: u32,
    pub ip: Option<String>,
    pub avg_ms: Option<f64>,
    pub loss_pct: f64,
    pub segment: &'static str,
}

/// Private / shared address → which part of the path it is.
pub fn segment_of(ip: Ipv4Addr, is_last: bool) -> &'static str {
    if is_last {
        return "destination";
    }
    let o = ip.octets();
    if ip.is_private() || ip.is_link_local() {
        "local"
    } else if o[0] == 100 && (64..128).contains(&o[1]) {
        "isp" // carrier-grade NAT, always inside the ISP
    } else {
        "transit"
    }
}

/// Traceroute: 3 probes per hop, up to `max_hops`, stops at the destination.
pub fn traceroute(target: Ipv4Addr, max_hops: u8) -> Vec<HopResult> {
    let mut out: Vec<HopResult> = Vec::new();
    let mut seen_public = false;
    for ttl in 1..=max_hops {
        let mut ip = None;
        let mut times = Vec::new();
        let mut reached = false;
        for _ in 0..3 {
            match echo(target, ttl, 32, false, 1000, None) {
                Reply::Hop(a, ms) => {
                    ip = Some(a);
                    times.push(ms);
                }
                Reply::Echo(ms) => {
                    ip = Some(target);
                    times.push(ms);
                    reached = true;
                }
                _ => {}
            }
        }
        let avg = (!times.is_empty()).then(|| times.iter().sum::<f64>() / times.len() as f64);
        let mut segment = ip.map(|a| segment_of(a, reached)).unwrap_or("transit");
        // The first public hops after the home network belong to the ISP.
        if segment == "transit" && !seen_public {
            segment = "isp";
            seen_public = out.iter().filter(|h| h.segment == "isp").count() >= 2;
        }
        out.push(HopResult { n: ttl as u32, ip: ip.map(|a| a.to_string()), avg_ms: avg, loss_pct: 100.0 * (3 - times.len()) as f64 / 3.0, segment });
        if reached {
            break;
        }
    }
    out
}

/// Largest IPv4 MTU that reaches `target` without fragmentation (binary search on DF pings).
pub fn path_mtu(target: Ipv4Addr) -> Option<u32> {
    if !matches!(echo(target, 128, 32, true, 1500, None), Reply::Echo(_)) {
        return None; // target does not answer ICMP at all
    }
    let (mut lo, mut hi) = (548u16, 1472u16); // payload sizes; MTU = payload + 28
    if matches!(echo(target, 128, hi, true, 1500, None), Reply::Echo(_)) {
        return Some(hi as u32 + 28);
    }
    while lo + 1 < hi {
        let mid = (lo + hi) / 2;
        if matches!(echo(target, 128, mid, true, 1500, None), Reply::Echo(_)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo as u32 + 28)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statistics() {
        let s = stats(&[Some(10.0), None, Some(14.0), Some(12.0)]);
        assert_eq!((s.sent, s.received), (4, 3));
        assert_eq!(s.median_ms, Some(12.0));
        assert!((s.jitter_ms - 3.0).abs() < 1e-9);
        assert!((s.loss_pct - 25.0).abs() < 1e-9);
        assert_eq!(stats(&[None, None]).median_ms, None);
    }

    #[test]
    fn segments() {
        assert_eq!(segment_of("192.168.1.1".parse().unwrap(), false), "local");
        assert_eq!(segment_of("100.72.0.1".parse().unwrap(), false), "isp");
        assert_eq!(segment_of("8.8.8.8".parse().unwrap(), false), "transit");
        assert_eq!(segment_of("8.8.8.8".parse().unwrap(), true), "destination");
    }

    #[test]
    fn loopback_ping_and_mtu() {
        assert!(matches!(echo(Ipv4Addr::LOCALHOST, 128, 32, false, 1000, None), Reply::Echo(_)));
        assert_eq!(resolve_v4("127.0.0.1"), Some(Ipv4Addr::LOCALHOST));
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(tcp_connect(l.local_addr().unwrap(), Duration::from_secs(1)).is_some());
    }

    /// Real internet probes: `cargo test -p otx-net -- --ignored live_probes --nocapture`.
    #[test]
    #[ignore]
    fn live_probes() {
        let t = resolve_v4("1.1.1.1").unwrap();
        let s = ping_series(t, 5, Duration::from_millis(200));
        println!("ping 1.1.1.1: {:?} ms median, jitter {:.1}, loss {}%", s.median_ms, s.jitter_ms, s.loss_pct);
        for h in traceroute(t, 20) {
            println!("{:>2} {:<16} {:>7} {:>4}% {}", h.n, h.ip.clone().unwrap_or("*".into()), h.avg_ms.map(|m| format!("{m:.1}")).unwrap_or("-".into()), h.loss_pct, h.segment);
        }
        println!("path MTU to 1.1.1.1: {:?}", path_mtu(t));
    }
}
