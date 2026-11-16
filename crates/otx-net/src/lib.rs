//! Unprivileged network tools (run inside the app; nothing here needs elevation).

pub mod conn;
pub mod dns;

use std::net::IpAddr;
use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
use windows::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, IP_ADAPTER_ADDRESSES_LH};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6};

/// DNS servers Windows currently uses on the primary (lowest-metric, connected) adapter.
pub fn current_dns_servers() -> Vec<IpAddr> {
    let mut size: u32 = 16 * 1024;
    let mut buf: Vec<u8>;
    for _ in 0..4 {
        buf = vec![0u8; size as usize];
        let r = unsafe { GetAdaptersAddresses(AF_UNSPEC.0 as u32, GAA_FLAG_INCLUDE_GATEWAYS, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size) };
        if r == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if r != ERROR_SUCCESS.0 {
            return Vec::new();
        }
        let mut best: Option<(u32, Vec<IpAddr>)> = None;
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            // SAFETY: OS-written linked list inside `buf`.
            let a = unsafe { &*p };
            if a.OperStatus == IfOperStatusUp && !a.FirstGatewayAddress.is_null() {
                let mut servers = Vec::new();
                let mut d = a.FirstDnsServerAddress;
                while !d.is_null() {
                    let dd = unsafe { &*d };
                    let sa = dd.Address.lpSockaddr;
                    if !sa.is_null() {
                        unsafe {
                            match (*sa).sa_family {
                                f if f == AF_INET => {
                                    let s = &*(sa as *const SOCKADDR_IN);
                                    servers.push(IpAddr::from(s.sin_addr.S_un.S_addr.to_ne_bytes()));
                                }
                                f if f == AF_INET6 => {
                                    let s = &*(sa as *const SOCKADDR_IN6);
                                    let ip = std::net::Ipv6Addr::from(s.sin6_addr.u.Byte);
                                    // fec0:0:0:ffff::1-3 are Windows' placeholder site-local resolvers.
                                    if !ip.segments().starts_with(&[0xfec0, 0, 0, 0xffff]) {
                                        servers.push(IpAddr::V6(ip));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    d = dd.Next;
                }
                if best.as_ref().is_none_or(|(m, _)| a.Ipv4Metric < *m) {
                    best = Some((a.Ipv4Metric, servers));
                }
            }
            p = a.Next;
        }
        return best.map(|(_, s)| s).unwrap_or_default();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_current_dns_without_errors() {
        let s = super::current_dns_servers();
        println!("current DNS: {s:?}");
    }
}
