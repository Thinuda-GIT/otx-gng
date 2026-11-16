//! Unprivileged network tools (run inside the app; nothing here needs elevation).

pub mod conn;
pub mod dns;
pub mod probe;

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

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AdapterRow {
    /// Interface LUID as a decimal string (u64).
    pub luid: String,
    pub name: String,
    pub description: String,
    /// "ethernet", "wifi", "mobile", "ppp", "vpn" or "other".
    pub kind: &'static str,
    pub up: bool,
    pub has_gateway: bool,
    pub ipv4: Option<String>,
    pub metric: u32,
}

/// Every adapter Windows knows (connected or not), except loopback and tunnels.
pub fn adapters() -> Vec<AdapterRow> {
    use windows::Win32::NetworkManagement::IpHelper::{GAA_FLAG_INCLUDE_ALL_INTERFACES, GAA_FLAG_INCLUDE_GATEWAYS};
    let mut size: u32 = 32 * 1024;
    for _ in 0..4 {
        let mut buf = vec![0u8; size as usize];
        let r = unsafe {
            GetAdaptersAddresses(AF_UNSPEC.0 as u32, GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_INCLUDE_ALL_INTERFACES, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size)
        };
        if r == ERROR_BUFFER_OVERFLOW.0 {
            continue;
        }
        if r != ERROR_SUCCESS.0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
        while !p.is_null() {
            // SAFETY: OS-written linked list inside `buf`.
            let a = unsafe { &*p };
            let kind = match a.IfType {
                6 => "ethernet",
                71 => "wifi",
                243 | 244 => "mobile",
                23 => "ppp",
                _ => "other",
            };
            let name = unsafe { a.FriendlyName.to_string() }.unwrap_or_default();
            let description = unsafe { a.Description.to_string() }.unwrap_or_default();
            let n = name.to_ascii_lowercase();
            let d = description.to_ascii_lowercase();
            let kind = if ["warp", "openvpn", "tap-windows", "wireguard", "wintun", "vpn"].iter().any(|v| n.contains(v) || d.contains(v)) { "vpn" } else { kind };
            // Windows' filter layers, Wi-Fi Direct virtual adapters and debug NICs are not connections.
            let skip = a.IfType == 24
                || a.IfType == 131
                || n.contains("loopback")
                || n.contains("-wfp ")
                || n.contains("qos packet scheduler")
                || n.contains("lightweight filter")
                || n.starts_with("local area connection*")
                || n.contains("kernel debugger");
            if !skip {
                let mut ipv4 = None;
                let mut u = a.FirstUnicastAddress;
                while !u.is_null() && ipv4.is_none() {
                    let uu = unsafe { &*u };
                    let sa = uu.Address.lpSockaddr;
                    if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                        let s = unsafe { &*(sa as *const SOCKADDR_IN) };
                        ipv4 = Some(std::net::Ipv4Addr::from(unsafe { s.sin_addr.S_un.S_addr }.to_ne_bytes()).to_string());
                    }
                    u = uu.Next;
                }
                if ipv4.is_none() {
                    p = a.Next;
                    continue;
                }
                out.push(AdapterRow {
                    luid: unsafe { a.Luid.Value }.to_string(),
                    name,
                    description,
                    kind,
                    up: a.OperStatus == IfOperStatusUp,
                    has_gateway: !a.FirstGatewayAddress.is_null(),
                    ipv4,
                    metric: a.Ipv4Metric,
                });
            }
            p = a.Next;
        }
        return out;
    }
    Vec::new()
}

/// Known VPN clients that carry all traffic (their presence changes every route diagnosis).
pub const VPN_PROCESSES: &[(&str, &str)] = &[
    ("warp-svc.exe", "Cloudflare WARP"),
    ("openvpn.exe", "OpenVPN"),
    ("wireguard.exe", "WireGuard"),
    ("nordvpn-service.exe", "NordVPN"),
    ("expressvpnd.exe", "ExpressVPN"),
    ("protonvpn.wireguardservice.exe", "Proton VPN"),
    ("mullvad-daemon.exe", "Mullvad VPN"),
    ("surfshark.service.exe", "Surfshark"),
    ("exitlag.exe", "ExitLag"),
];

#[cfg(test)]
mod tests {
    #[test]
    fn lists_adapters() {
        let a = super::adapters();
        for r in &a {
            println!("{:<24} {:<9} up={} gw={} metric={} ip={:?}", r.name, r.kind, r.up, r.has_gateway, r.metric, r.ipv4);
        }
        assert!(a.iter().all(|r| !r.luid.is_empty()));
    }

    #[test]
    fn reads_current_dns_without_errors() {
        let s = super::current_dns_servers();
        println!("current DNS: {s:?}");
    }
}
