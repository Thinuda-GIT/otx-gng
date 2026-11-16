//! Per-process network traffic from Windows' own ETW events
//! (Microsoft-Windows-Kernel-Network). Only byte counts and endpoints are recorded; packet
//! contents are never seen. The trace starts on first use and runs while the Core runs.

use ferrisetw::parser::Parser;
use ferrisetw::provider::Provider;
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::UserTrace;
use ferrisetw::EventRecord;
use otx_protocol::PidTraffic;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Mutex, OnceLock};

const KERNEL_NETWORK: &str = "7DD42A49-5329-4832-8DFD-43D979153A88";

#[derive(Default)]
struct Counters {
    sent: u64,
    recv: u64,
    remotes: HashMap<String, u64>,
}

static TABLE: OnceLock<Mutex<HashMap<u32, Counters>>> = OnceLock::new();
static TRACE: OnceLock<Result<Mutex<UserTrace>, String>> = OnceLock::new();

fn table() -> &'static Mutex<HashMap<u32, Counters>> {
    TABLE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Event ids: send/recv × TCP/UDP × IPv4/IPv6.
fn direction(event_id: u16) -> Option<bool /* send */> {
    match event_id {
        10 | 26 | 42 | 58 => Some(true),
        11 | 27 | 43 | 59 => Some(false),
        _ => None,
    }
}

fn callback(record: &EventRecord, locator: &SchemaLocator) {
    let Some(send) = direction(record.event_id()) else { return };
    let Ok(schema) = locator.event_schema(record) else { return };
    let p = Parser::create(record, &schema);
    let Ok(pid) = p.try_parse::<u32>("PID") else { return };
    let size = p.try_parse::<u32>("size").unwrap_or(0) as u64;
    let remote = if record.event_id() < 26 || (42..58).contains(&record.event_id()) {
        p.try_parse::<u32>("daddr").ok().map(|a| IpAddr::V4(Ipv4Addr::from(u32::from_be(a))))
    } else {
        None
    };
    let port = p.try_parse::<u16>("dport").ok().map(u16::from_be);
    let mut t = table().lock().unwrap_or_else(|e| e.into_inner());
    let c = t.entry(pid).or_default();
    if send {
        c.sent += size;
    } else {
        c.recv += size;
    }
    if let (Some(ip), Some(port)) = (remote, port) {
        if c.remotes.len() < 64 || c.remotes.contains_key(&format!("{ip}:{port}")) {
            *c.remotes.entry(format!("{ip}:{port}")).or_default() += size;
        }
    }
}

/// Starts the trace once. Needs elevation (the Core service has it).
pub fn ensure_started() -> Result<(), String> {
    match TRACE.get_or_init(|| {
        let provider = Provider::by_guid(KERNEL_NETWORK).add_callback(callback).build();
        UserTrace::new().enable(provider).start_and_process().map(Mutex::new).map_err(|e| format!("could not start network tracing: {e:?}"))
    }) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.clone()),
    }
}

/// Cumulative counters per process since the trace started.
pub fn snapshot() -> Result<Vec<PidTraffic>, String> {
    ensure_started()?;
    let t = table().lock().unwrap_or_else(|e| e.into_inner());
    Ok(t.iter()
        .map(|(pid, c)| PidTraffic {
            pid: *pid,
            sent_bytes: c.sent,
            recv_bytes: c.recv,
            top_remote: c.remotes.iter().max_by_key(|(_, b)| **b).map(|(r, _)| r.clone()),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    /// Needs elevation: `cargo test -p otx-core -- --ignored traffic_live`.
    #[test]
    #[ignore]
    fn traffic_live() {
        super::ensure_started().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let s = std::net::UdpSocket::bind("0.0.0.0:0").unwrap();
        for _ in 0..5 {
            s.send_to(&[0u8; 200], "1.1.1.1:53").unwrap();
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
        let snap = super::snapshot().unwrap();
        let me = snap.iter().find(|t| t.pid == std::process::id()).expect("own traffic counted");
        println!("own pid sent {} bytes, top remote {:?}", me.sent_bytes, me.top_remote);
        assert!(me.sent_bytes >= 1000);
        assert_eq!(me.top_remote.as_deref(), Some("1.1.1.1:53"));
    }

    #[test]
    fn event_directions() {
        assert_eq!(super::direction(10), Some(true));
        assert_eq!(super::direction(43), Some(false));
        assert_eq!(super::direction(12), None);
    }
}
