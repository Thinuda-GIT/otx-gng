//! Network Analyzer and Traffic Shaper commands.

use crate::AppState;
use otx_protocol::{ActionResult, PidTraffic, Request, Response, ShaperRuleSpec};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

#[derive(Default)]
pub struct TrafficState {
    prev: Mutex<Option<(Instant, HashMap<u32, (u64, u64)>)>>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppTraffic {
    id: String,
    exe: String,
    pids: Vec<u32>,
    down_kbps: f64,
    up_kbps: f64,
    top_remote: Option<String>,
}

fn exe_names() -> HashMap<u32, String> {
    otx_monitor::nt::processes().unwrap_or_default().into_iter().map(|p| (p.pid, p.name)).collect()
}

/// Per-app rates since the previous call, from the Core's ETW counters.
#[tauri::command]
pub async fn net_traffic(state: tauri::State<'_, AppState>) -> Result<Vec<AppTraffic>, String> {
    let snap: Vec<PidTraffic> = match state.core.call(Request::GetTraffic).await {
        Ok(Response::Traffic(t)) => t,
        Ok(_) => return Err("unexpected reply from the Core".into()),
        Err(e) => return Err(format!("Per-app traffic needs the Core service: {}", e.message)),
    };
    let names = tokio::task::spawn_blocking(exe_names).await.map_err(|e| e.to_string())?;
    let now = Instant::now();
    let mut prev = state.traffic.prev.lock().unwrap_or_else(|p| p.into_inner());
    let (secs, old) = match prev.as_ref() {
        Some((t, m)) => (now.duration_since(*t).as_secs_f64().max(0.2), m.clone()),
        None => (1.0, HashMap::new()),
    };
    let mut apps: HashMap<String, AppTraffic> = HashMap::new();
    for t in &snap {
        let Some(exe) = names.get(&t.pid) else { continue };
        let (ps, pr) = old.get(&t.pid).copied().unwrap_or((t.sent_bytes, t.recv_bytes));
        let up = t.sent_bytes.saturating_sub(ps) as f64 * 8.0 / 1000.0 / secs;
        let down = t.recv_bytes.saturating_sub(pr) as f64 * 8.0 / 1000.0 / secs;
        let a = apps.entry(exe.to_ascii_lowercase()).or_insert_with(|| AppTraffic {
            id: exe.to_ascii_lowercase(),
            exe: exe.clone(),
            pids: Vec::new(),
            down_kbps: 0.0,
            up_kbps: 0.0,
            top_remote: None,
        });
        a.pids.push(t.pid);
        a.up_kbps += up;
        a.down_kbps += down;
        if a.top_remote.is_none() {
            a.top_remote = t.top_remote.clone();
        }
    }
    *prev = Some((now, snap.iter().map(|t| (t.pid, (t.sent_bytes, t.recv_bytes))).collect()));
    let mut out: Vec<AppTraffic> = apps.into_values().filter(|a| a.down_kbps + a.up_kbps > 0.05).collect();
    out.sort_by(|a, b| (b.down_kbps + b.up_kbps).partial_cmp(&(a.down_kbps + a.up_kbps)).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnRow {
    app: String,
    pid: u32,
    protocol: &'static str,
    local: String,
    remote: String,
    state: String,
}

#[tauri::command]
pub async fn net_connections() -> Result<Vec<ConnRow>, String> {
    tokio::task::spawn_blocking(|| {
        let names = exe_names();
        otx_net::conn::connections()
            .into_iter()
            .map(|c| ConnRow { app: names.get(&c.pid).cloned().unwrap_or_else(|| format!("PID {}", c.pid)), pid: c.pid, protocol: c.protocol, local: c.local, remote: c.remote, state: c.state })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn shaper_apply(state: tauri::State<'_, AppState>, rules: Vec<ShaperRuleSpec>, dry_run: bool) -> Result<ActionResult, String> {
    match state.core.call(Request::ApplyShaper { rules, dry_run }).await {
        Ok(Response::Action(a)) => Ok(a),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) if e.link != otx_protocol::CoreLink::Connected => Err("Traffic Shaper needs the OTX.GNG Core service, which is not running.".into()),
        Err(e) => Err(e.message),
    }
}

#[tauri::command]
pub async fn net_restore(state: tauri::State<'_, AppState>) -> Result<ActionResult, String> {
    match state.core.call(Request::RestoreInternet).await {
        Ok(Response::Action(a)) => Ok(a),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) => Err(e.message),
    }
}
