//! Multi Internet: prefer one connection by giving it the lowest IPv4 interface metric.
//! Windows routes new connections through the lowest metric, so this is how failover works
//! without any driver. Previous metric settings are journaled and restored on revert.

use crate::journal::{Entry, RevertOp};
use crate::process::ActionError;
use crate::state::CoreState;
use otx_protocol::{ActionResult, ErrorCode, Response};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::NetworkManagement::IpHelper::{GetIpInterfaceEntry, InitializeIpInterfaceEntry, SetIpInterfaceEntry, MIB_IPINTERFACE_ROW};
use windows::Win32::Networking::WinSock::AF_INET;

fn row(luid: u64) -> Result<MIB_IPINTERFACE_ROW, String> {
    let mut r = MIB_IPINTERFACE_ROW::default();
    unsafe { InitializeIpInterfaceEntry(&mut r) };
    r.Family = AF_INET;
    r.InterfaceLuid.Value = luid;
    let e = unsafe { GetIpInterfaceEntry(&mut r) };
    if e != ERROR_SUCCESS {
        return Err(format!("interface not found ({})", e.0));
    }
    Ok(r)
}

/// Reads (automatic metric?, metric).
pub fn get(luid: u64) -> Result<(bool, u32), String> {
    let r = row(luid)?;
    Ok((r.UseAutomaticMetric, r.Metric))
}

pub fn set(luid: u64, automatic: bool, metric: u32) -> Result<(), String> {
    let mut r = row(luid)?;
    r.UseAutomaticMetric = automatic;
    r.Metric = metric;
    r.SitePrefixLength = 0; // must be 0 for IPv4 or SetIpInterfaceEntry fails
    let e = unsafe { SetIpInterfaceEntry(&mut r) };
    if e != ERROR_SUCCESS {
        return Err(format!("Windows refused the interface change ({})", e.0));
    }
    Ok(())
}

fn err(code: ErrorCode, m: impl Into<String>) -> ActionError {
    ActionError { code, message: m.into() }
}

/// Makes `luid` preferred (metric 5) and pushes other connected gateways behind it (metric 50).
pub fn prefer(state: &CoreState, luid: &str, dry_run: bool) -> Result<Response, ActionError> {
    if state.safe_mode {
        return Err(err(ErrorCode::SafeMode, "Safe Mode is on, so the Core makes no changes."));
    }
    let target: u64 = luid.parse().map_err(|_| err(ErrorCode::Validation, "bad adapter id"))?;
    let adapters = otx_net::adapters();
    let chosen = adapters.iter().find(|a| a.luid == luid && a.up && a.has_gateway).ok_or_else(|| err(ErrorCode::Validation, "That connection is not up or has no internet gateway."))?;
    let others: Vec<_> = adapters.iter().filter(|a| a.luid != luid && a.up && a.has_gateway).collect();
    let mut changes = vec![format!("{}: preferred (metric 5)", chosen.name)];
    changes.extend(others.iter().map(|o| format!("{}: backup (metric 50)", o.name)));
    if dry_run || state.dry_run {
        return Ok(Response::Action(ActionResult { dry_run: true, changes, change_id: None }));
    }
    // Undo any earlier preference first so the journal always holds the original settings.
    unprefer(state);
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    let plan: Vec<(u64, String, u32)> = std::iter::once((target, chosen.name.clone(), 5))
        .chain(others.iter().filter_map(|o| o.luid.parse().ok().map(|l| (l, o.name.clone(), 50))))
        .collect();
    for (l, name, metric) in plan {
        let (auto, prev) = get(l).map_err(|e| err(ErrorCode::Internal, e))?;
        let id = crate::server::random_hex(8);
        j.record(Entry {
            id: id.clone(),
            created_unix: crate::process::now_unix(),
            title: format!("{name}: interface metric {metric}"),
            detail: if auto { "was automatic".into() } else { format!("was {prev}") },
            temporary: false,
            revert: RevertOp::InterfaceMetric { luid: l.to_string(), automatic: auto, metric: prev },
        })
        .map_err(|e| err(ErrorCode::Internal, e.to_string()))?;
        if let Err(e) = set(l, false, metric) {
            let _ = j.forget(&id);
            return Err(err(ErrorCode::Internal, e));
        }
    }
    Ok(Response::Action(ActionResult { dry_run: false, changes, change_id: None }))
}

/// Restores automatic metrics (removes the preference).
pub fn unprefer(state: &CoreState) -> u32 {
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    j.revert_where(&crate::journal::SystemReverter { dry_run: false }, |e| matches!(e.revert, RevertOp::InterfaceMetric { .. })).map(|(ok, _)| ok).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_metrics_of_connected_adapters_read_only() {
        for a in otx_net::adapters().into_iter().filter(|a| a.up && a.has_gateway) {
            let (auto, metric) = super::get(a.luid.parse().unwrap()).unwrap();
            println!("{}: automatic={auto} metric={metric}", a.name);
        }
    }
}
