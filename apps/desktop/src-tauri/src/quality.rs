//! Quality Test, Route Lab and Multi Internet commands (unprivileged probes; adapter preference
//! goes to the Core).

use crate::AppState;
use otx_net::probe::{self, HopResult, Stats};
use otx_protocol::{ActionResult, Request, Response};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

fn target_v4(target: &str) -> Result<std::net::Ipv4Addr, String> {
    let host = target.trim();
    if host.is_empty() || host.len() > 253 || host.contains(['/', '\\', ' ']) {
        return Err("Enter a host name or IPv4 address.".into());
    }
    probe::resolve_v4(host).ok_or_else(|| format!("Could not resolve {host} to an IPv4 address."))
}

/// `count` probes: ICMP, or TCP connect to `port` (for servers that ignore ICMP).
#[tauri::command]
pub async fn net_probe(target: String, count: u32, tcp_port: Option<u16>) -> Result<Stats, String> {
    let ip = target_v4(&target)?;
    let count = count.clamp(1, 60);
    tokio::task::spawn_blocking(move || match tcp_port {
        Some(p) => probe::tcp_series(SocketAddr::new(IpAddr::V4(ip), p), count, Duration::from_millis(250)),
        None => probe::ping_series(ip, count, Duration::from_millis(250)),
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn net_traceroute(target: String) -> Result<Vec<HopResult>, String> {
    let ip = target_v4(&target)?;
    tokio::task::spawn_blocking(move || probe::traceroute(ip, 30)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn net_mtu(target: String) -> Result<Option<u32>, String> {
    let ip = target_v4(&target)?;
    tokio::task::spawn_blocking(move || probe::path_mtu(ip)).await.map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterHealth {
    #[serde(flatten)]
    row: otx_net::AdapterRow,
    ping_ms: Option<f64>,
    loss_pct: Option<f64>,
}

/// All real adapters; connected ones with a gateway get a 4-ping health check bound to their
/// own address (so each connection is measured separately).
#[tauri::command]
pub async fn net_adapters() -> Result<Vec<AdapterHealth>, String> {
    tokio::task::spawn_blocking(|| {
        let target = std::net::Ipv4Addr::new(1, 1, 1, 1);
        otx_net::adapters()
            .into_iter()
            .map(|row| {
                let src = row.ipv4.as_deref().and_then(|s| s.parse().ok());
                let (ping_ms, loss_pct) = if row.up && row.has_gateway {
                    let samples: Vec<Option<f64>> = (0..4)
                        .map(|_| match probe::echo(target, 128, 32, false, 1000, src) {
                            probe::Reply::Echo(ms) => Some(ms),
                            _ => None,
                        })
                        .collect();
                    let s = probe::stats(&samples);
                    (s.median_ms, Some(s.loss_pct))
                } else {
                    (None, None)
                };
                AdapterHealth { row, ping_ms, loss_pct }
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Name of a running VPN client that carries all traffic, if any.
#[tauri::command]
pub async fn net_vpn() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(|| {
        let procs = otx_monitor::nt::processes().unwrap_or_default();
        otx_net::VPN_PROCESSES.iter().find(|(exe, _)| procs.iter().any(|p| p.name.eq_ignore_ascii_case(exe))).map(|(_, name)| name.to_string())
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn net_prefer(state: tauri::State<'_, AppState>, luid: Option<String>, dry_run: bool) -> Result<ActionResult, String> {
    let req = match luid {
        Some(luid) => Request::PreferAdapter { luid, dry_run },
        None => Request::ClearAdapterPreference,
    };
    match state.core.call(req).await {
        Ok(Response::Action(a)) => Ok(a),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) if e.link != otx_protocol::CoreLink::Connected => Err("Changing connection priority needs the OTX.GNG Core service.".into()),
        Err(e) => Err(e.message),
    }
}
