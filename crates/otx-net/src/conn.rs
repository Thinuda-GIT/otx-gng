//! Open TCP connections and UDP endpoints with their owning process (GetExtendedTcpTable /
//! GetExtendedUdpTable). Unprivileged; no packets are read.

use std::net::{Ipv4Addr, Ipv6Addr};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID, MIB_UDPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conn {
    pub pid: u32,
    pub protocol: &'static str,
    pub local: String,
    pub remote: String,
    pub state: String,
}

pub fn port(raw: u32) -> u16 {
    u16::from_be((raw & 0xFFFF) as u16)
}

pub fn tcp_state(s: u32) -> &'static str {
    match s {
        1 => "Closed",
        2 => "Listen",
        3 => "SYN sent",
        4 => "SYN received",
        5 => "Established",
        6 => "FIN wait 1",
        7 => "FIN wait 2",
        8 => "Close wait",
        9 => "Closing",
        10 => "Last ACK",
        11 => "Time wait",
        12 => "Delete TCB",
        _ => "Unknown",
    }
}

fn table(fetch: impl Fn(Option<*mut core::ffi::c_void>, *mut u32) -> u32) -> Option<Vec<u64>> {
    let mut size = 0u32;
    let _ = fetch(None, &mut size);
    for _ in 0..4 {
        let mut buf = vec![0u64; (size as usize).div_ceil(8) + 64];
        let mut sz = (buf.len() * 8) as u32;
        match fetch(Some(buf.as_mut_ptr().cast()), &mut sz) {
            0 => return Some(buf),
            122 => size = sz, // ERROR_INSUFFICIENT_BUFFER: table grew
            _ => return None,
        }
    }
    None
}

/// Rows are laid out as `u32 count` followed by the row array.
unsafe fn rows<T>(buf: &[u64]) -> &[T] {
    let count = *(buf.as_ptr() as *const u32) as usize;
    let first = (buf.as_ptr() as *const u8).add(std::mem::align_of::<T>().max(4)) as *const T;
    std::slice::from_raw_parts(first, count)
}

pub fn connections() -> Vec<Conn> {
    let mut out = Vec::new();
    unsafe {
        if let Some(buf) = table(|p, s| GetExtendedTcpTable(p, s, false, AF_INET.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0)) {
            for r in rows::<MIB_TCPROW_OWNER_PID>(&buf) {
                out.push(Conn {
                    pid: r.dwOwningPid,
                    protocol: "TCP",
                    local: format!("{}:{}", Ipv4Addr::from(u32::from_be(r.dwLocalAddr)), port(r.dwLocalPort)),
                    remote: if r.dwState == 2 { "-".into() } else { format!("{}:{}", Ipv4Addr::from(u32::from_be(r.dwRemoteAddr)), port(r.dwRemotePort)) },
                    state: tcp_state(r.dwState).into(),
                });
            }
        }
        if let Some(buf) = table(|p, s| GetExtendedTcpTable(p, s, false, AF_INET6.0 as u32, TCP_TABLE_OWNER_PID_ALL, 0)) {
            for r in rows::<MIB_TCP6ROW_OWNER_PID>(&buf) {
                out.push(Conn {
                    pid: r.dwOwningPid,
                    protocol: "TCP",
                    local: format!("[{}]:{}", Ipv6Addr::from(r.ucLocalAddr), port(r.dwLocalPort)),
                    remote: if r.dwState == 2 { "-".into() } else { format!("[{}]:{}", Ipv6Addr::from(r.ucRemoteAddr), port(r.dwRemotePort)) },
                    state: tcp_state(r.dwState).into(),
                });
            }
        }
        if let Some(buf) = table(|p, s| GetExtendedUdpTable(p, s, false, AF_INET.0 as u32, UDP_TABLE_OWNER_PID, 0)) {
            for r in rows::<MIB_UDPROW_OWNER_PID>(&buf) {
                out.push(Conn {
                    pid: r.dwOwningPid,
                    protocol: "UDP",
                    local: format!("{}:{}", Ipv4Addr::from(u32::from_be(r.dwLocalAddr)), port(r.dwLocalPort)),
                    remote: "-".into(),
                    state: "-".into(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_and_states() {
        assert_eq!(port(u16::to_be(443) as u32), 443);
        assert_eq!(tcp_state(5), "Established");
    }

    #[test]
    fn sees_a_listening_socket_of_this_process() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = l.local_addr().unwrap().port();
        let all = connections();
        assert!(
            all.iter().any(|c| c.pid == std::process::id() && c.state == "Listen" && c.local == format!("127.0.0.1:{p}")),
            "own listener not found among {} rows",
            all.len()
        );
    }
}
