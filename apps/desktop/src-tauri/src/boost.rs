//! PC Boost commands. User-scope tweaks run here (the signed-in user's own HKCU and session)
//! with a local snapshot ledger; machine-scope tweaks, RAM cleaning and DNS go to the Core.

use crate::AppState;
use otx_protocol::{ActionResult, ChangeEntry, RamCleanReport, RamMethod, Request, Response, TweakState, TweakStatus};
use otx_tweaks::{win::WinSys, Prev, Scope, State};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn ledger_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let dir = PathBuf::from(base).join("OTX.GNG");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("user-tweaks.json"))
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct Ledger {
    /// tweak id → (unix time, snapshot)
    applied: BTreeMap<String, (u64, Vec<Prev>)>,
}

fn load() -> Ledger {
    ledger_path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(l: &Ledger) -> Result<(), String> {
    let p = ledger_path().ok_or("no local app data folder")?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(l).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, p).map_err(|e| e.to_string())
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn map_state(s: State) -> TweakState {
    match s {
        State::Applied => TweakState::Applied,
        State::NotApplied => TweakState::NotApplied,
        State::Partial => TweakState::Partial,
        State::Unsupported => TweakState::Unsupported,
    }
}

const CORE_DOWN: &str = "Needs the OTX.GNG Core service, which is not running (General Settings → Core service).";

#[tauri::command]
pub async fn tweaks_status(state: tauri::State<'_, AppState>) -> Result<Vec<TweakStatus>, String> {
    let machine = match state.core.call(Request::ListTweaks).await {
        Ok(Response::Tweaks(t)) => Some(t),
        _ => None,
    };
    let user = tokio::task::spawn_blocking(|| {
        let ledger = load();
        let sys = WinSys { scope: Scope::User };
        otx_tweaks::manifests()
            .into_iter()
            .filter(|m| m.scope == Scope::User)
            .map(|m| {
                let st = otx_tweaks::check(&m, &sys);
                TweakStatus { by_otx: ledger.applied.contains_key(&m.id), id: m.id, state: map_state(st.state), reason: st.reason }
            })
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| e.to_string())?;
    let mut out = user;
    match machine {
        Some(m) => out.extend(m),
        None => out.extend(otx_tweaks::manifests().into_iter().filter(|m| m.scope == Scope::Machine).map(|m| TweakStatus {
            id: m.id,
            state: TweakState::Unsupported,
            reason: Some(m.unsupported.unwrap_or_else(|| CORE_DOWN.into())),
            by_otx: false,
        })),
    }
    Ok(out)
}

#[tauri::command]
pub async fn tweak_set(state: tauri::State<'_, AppState>, id: String, on: bool, dry_run: bool) -> Result<ActionResult, String> {
    let m = otx_tweaks::find(&id).ok_or("unknown tweak")?;
    if m.scope == Scope::Machine {
        return match state.core.call(Request::SetTweak { id, on, dry_run }).await {
            Ok(Response::Action(a)) => Ok(a),
            Ok(_) => Err("unexpected reply from the Core".into()),
            Err(e) if e.link != otx_protocol::CoreLink::Connected => Err(CORE_DOWN.into()),
            Err(e) => Err(e.message),
        };
    }
    tokio::task::spawn_blocking(move || {
        let sys = WinSys { scope: Scope::User };
        let mut ledger = load();
        if on {
            let a = otx_tweaks::apply(&m, &sys, dry_run)?;
            if !dry_run {
                // Keep the OLDEST snapshot if it was already applied by us (re-apply is a no-op).
                ledger.applied.entry(m.id.clone()).or_insert((now(), a.prev));
                save(&ledger)?;
            }
            Ok(ActionResult { dry_run, changes: a.changes, change_id: Some(format!("user:{}", m.id)) })
        } else {
            let Some((_, prev)) = ledger.applied.get(&m.id).cloned() else {
                return Err("OTX.GNG did not make this change, so there is no snapshot to restore.".to_string());
            };
            if !dry_run {
                otx_tweaks::revert(&sys, &prev)?;
                ledger.applied.remove(&m.id);
                save(&ledger)?;
            }
            Ok(ActionResult { dry_run, changes: vec![format!("Reverted: {}", m.title)], change_id: None })
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Per-user tweaks applied by OTX.GNG, for the "What OTX.GNG changed" list.
#[tauri::command]
pub fn user_changes() -> Vec<ChangeEntry> {
    load()
        .applied
        .into_iter()
        .map(|(id, (t, _))| ChangeEntry {
            title: otx_tweaks::find(&id).map(|m| m.title).unwrap_or_else(|| id.clone()),
            id: format!("user:{id}"),
            created_unix: t,
            detail: "Your Windows account setting".into(),
            temporary: false,
        })
        .collect()
}

/// Reverts per-user tweaks (one by id, or all when id is None).
#[tauri::command]
pub async fn user_revert(id: Option<String>) -> Result<u32, String> {
    tokio::task::spawn_blocking(move || {
        let sys = WinSys { scope: Scope::User };
        let mut ledger = load();
        let ids: Vec<String> = ledger.applied.keys().filter(|k| id.as_ref().is_none_or(|i| *i == **k)).cloned().collect();
        let mut n = 0;
        for k in ids {
            if let Some((_, prev)) = ledger.applied.get(&k).cloned() {
                otx_tweaks::revert(&sys, &prev)?;
                ledger.applied.remove(&k);
                n += 1;
            }
        }
        save(&ledger)?;
        Ok(n)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn ram_clean(state: tauri::State<'_, AppState>, methods: Vec<RamMethod>, allow: Vec<String>, skip: Vec<String>, dry_run: bool) -> Result<RamCleanReport, String> {
    match state.core.call(Request::CleanMemory { methods, allow, skip, dry_run }).await {
        Ok(Response::RamClean(r)) => Ok(r),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) if e.link != otx_protocol::CoreLink::Connected => Err(CORE_DOWN.into()),
        Err(e) => Err(e.message),
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsBenchResult {
    server_id: String,
    median_ms: Option<f64>,
    jitter_ms: f64,
    failures: u32,
    samples: u32,
}

/// Benchmarks each server (first address) with real queries. Runs in parallel, one thread per server.
#[tauri::command]
pub async fn dns_benchmark(servers: Vec<(String, String)>, samples: u32) -> Result<Vec<DnsBenchResult>, String> {
    let samples = samples.clamp(3, 20);
    let handles: Vec<_> = servers
        .into_iter()
        .take(16)
        .filter_map(|(id, ip)| ip.parse::<std::net::IpAddr>().ok().map(|ip| (id, ip)))
        .map(|(id, ip)| {
            tokio::task::spawn_blocking(move || {
                let b = otx_net::dns::benchmark(ip, samples, std::time::Duration::from_millis(1500));
                DnsBenchResult { server_id: id, median_ms: b.median(), jitter_ms: b.jitter(), failures: b.failures, samples }
            })
        })
        .collect();
    let mut out = Vec::new();
    for h in handles {
        out.push(h.await.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn dns_current() -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(|| otx_net::current_dns_servers().into_iter().map(|i| i.to_string()).collect()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dns_set(state: tauri::State<'_, AppState>, servers: Vec<String>, dry_run: bool) -> Result<ActionResult, String> {
    match state.core.call(Request::SetDns { servers, dry_run }).await {
        Ok(Response::Action(a)) => Ok(a),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) if e.link != otx_protocol::CoreLink::Connected => Err(CORE_DOWN.into()),
        Err(e) => Err(e.message),
    }
}
