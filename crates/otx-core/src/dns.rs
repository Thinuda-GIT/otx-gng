//! DNS servers of the primary adapter (SetInterfaceDnsSettings, Windows 10 2004+), with the
//! previous setting journaled and the resolver cache flushed afterwards.

use std::net::IpAddr;
use windows::core::{GUID, PWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::NetworkManagement::IpHelper::{
    FreeInterfaceDnsSettings, GetInterfaceDnsSettings, SetInterfaceDnsSettings, DNS_INTERFACE_SETTINGS, DNS_INTERFACE_SETTINGS_VERSION1,
    DNS_SETTING_IPV6, DNS_SETTING_NAMESERVER,
};

#[link(name = "dnsapi")]
unsafe extern "system" {
    /// Exported by dnsapi.dll (used by `ipconfig /flushdns`); not in the public headers.
    fn DnsFlushResolverCache() -> i32;
}

/// Validates IP literals and splits them into (IPv4 list, IPv6 list). At most 4 of each.
pub fn split_servers(servers: &[String]) -> Result<(Vec<String>, Vec<String>), String> {
    let (mut v4, mut v6) = (Vec::new(), Vec::new());
    for s in servers {
        match s.trim().parse::<IpAddr>().map_err(|_| format!("{s} is not an IP address"))? {
            IpAddr::V4(a) if !a.is_unspecified() && !a.is_broadcast() && !a.is_multicast() => v4.push(a.to_string()),
            IpAddr::V6(a) if !a.is_unspecified() && !a.is_multicast() => v6.push(a.to_string()),
            _ => return Err(format!("{s} cannot be used as a DNS server")),
        }
    }
    if v4.len() > 4 || v6.len() > 4 {
        return Err("at most 4 servers per address family".into());
    }
    Ok((v4, v6))
}

fn guid(adapter: &str) -> Result<GUID, String> {
    otx_tweaks::win::guid_from_str(adapter)
}

/// Current static name servers ("" means automatic/DHCP) for IPv4 or IPv6.
pub fn get(adapter: &str, ipv6: bool) -> Result<String, String> {
    let mut s = DNS_INTERFACE_SETTINGS { Version: DNS_INTERFACE_SETTINGS_VERSION1, Flags: (DNS_SETTING_NAMESERVER | if ipv6 { DNS_SETTING_IPV6 } else { 0 }) as u64, ..Default::default() };
    let r = unsafe { GetInterfaceDnsSettings(guid(adapter)?, &mut s) };
    if r != ERROR_SUCCESS {
        return Err(format!("could not read DNS settings ({})", r.0));
    }
    let v = unsafe { s.NameServer.to_string() }.unwrap_or_default();
    unsafe { FreeInterfaceDnsSettings(&mut s) };
    Ok(v)
}

pub fn set(adapter: &str, ipv6: bool, servers: &str) -> Result<(), String> {
    let mut wide: Vec<u16> = servers.encode_utf16().chain(std::iter::once(0)).collect();
    let s = DNS_INTERFACE_SETTINGS {
        Version: DNS_INTERFACE_SETTINGS_VERSION1,
        Flags: (DNS_SETTING_NAMESERVER | if ipv6 { DNS_SETTING_IPV6 } else { 0 }) as u64,
        NameServer: PWSTR(wide.as_mut_ptr()),
        ..Default::default()
    };
    let r = unsafe { SetInterfaceDnsSettings(guid(adapter)?, &s) };
    if r != ERROR_SUCCESS {
        return Err(format!("Windows refused the DNS change ({})", r.0));
    }
    // Verify.
    let now = get(adapter, ipv6)?;
    if normalize(&now) != normalize(servers) {
        return Err(format!("DNS did not take effect (now \"{now}\")"));
    }
    Ok(())
}

fn normalize(s: &str) -> String {
    s.split([',', ' ']).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(",").to_ascii_lowercase()
}

pub fn flush_cache() -> bool {
    unsafe { DnsFlushResolverCache() != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_splits_servers() {
        let (v4, v6) = split_servers(&["1.1.1.1".into(), " 2606:4700:4700::1111 ".into(), "1.0.0.1".into()]).unwrap();
        assert_eq!(v4, vec!["1.1.1.1", "1.0.0.1"]);
        assert_eq!(v6, vec!["2606:4700:4700::1111"]);
        assert!(split_servers(&["cloudflare.com".into()]).is_err());
        assert!(split_servers(&["0.0.0.0".into()]).is_err());
        assert!(split_servers(&["255.255.255.255".into()]).is_err());
        assert!(split_servers(&["1.1.1.1; rm".into()]).is_err());
        assert_eq!(normalize("1.1.1.1 1.0.0.1"), normalize("1.1.1.1,1.0.0.1"));
    }

    #[test]
    fn reads_dns_of_the_primary_adapter_read_only() {
        if let Some(g) = otx_sysinfo::primary_adapter_guid() {
            assert!(get(&g, false).is_ok());
        }
    }
}
