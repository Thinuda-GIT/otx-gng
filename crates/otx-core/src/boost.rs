//! PC Boost handlers in the Core: machine-scope tweaks (with a System Restore point before the
//! first change of the day), RAM cleaning and DNS.

use crate::journal::{Entry, RevertOp, SystemReverter};
use crate::process::ActionError;
use crate::state::CoreState;
use otx_protocol::{ActionResult, ErrorCode, RamMethod, Response, TweakState, TweakStatus};
use otx_tweaks::{win::WinSys, Scope, State};
use windows::Win32::System::Restore::{SRSetRestorePointW, BEGIN_SYSTEM_CHANGE, END_SYSTEM_CHANGE, MODIFY_SETTINGS, RESTOREPOINTINFOW, STATEMGRSTATUS};

fn err(code: ErrorCode, m: impl Into<String>) -> ActionError {
    ActionError { code, message: m.into() }
}

fn sys() -> WinSys {
    WinSys { scope: Scope::Machine }
}

fn state_of(s: State) -> TweakState {
    match s {
        State::Applied => TweakState::Applied,
        State::NotApplied => TweakState::NotApplied,
        State::Partial => TweakState::Partial,
        State::Unsupported => TweakState::Unsupported,
    }
}

pub fn list_tweaks(state: &CoreState) -> Response {
    let j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    let sys = sys();
    Response::Tweaks(
        otx_tweaks::manifests()
            .into_iter()
            .filter(|m| m.scope == Scope::Machine)
            .map(|m| {
                let st = otx_tweaks::check(&m, &sys);
                let by_otx = j.entries().iter().any(|e| matches!(&e.revert, RevertOp::Tweak { id, .. } if *id == m.id));
                TweakStatus { id: m.id, state: state_of(st.state), reason: st.reason, by_otx }
            })
            .collect(),
    )
}

/// One restore point per day at most (Windows itself also throttles them to one per 24 h).
fn restore_point(state: &CoreState) -> String {
    let marker = state.data_dir.join("last-restore-point");
    let today = crate::process::now_unix() / 86_400;
    if std::fs::read_to_string(&marker).ok().and_then(|s| s.trim().parse::<u64>().ok()) == Some(today) {
        return "Restore point already created today".into();
    }
    let mut desc = [0u16; 256];
    for (i, c) in "OTX.GNG: before changes".encode_utf16().enumerate() {
        desc[i] = c;
    }
    let mut info = RESTOREPOINTINFOW { dwEventType: BEGIN_SYSTEM_CHANGE, dwRestorePtType: MODIFY_SETTINGS, llSequenceNumber: 0, szDescription: desc };
    let mut st = STATEMGRSTATUS::default();
    let ok = unsafe { SRSetRestorePointW(&info, &mut st) }.as_bool();
    if !ok {
        let code = { st.nStatus }.0;
        return format!("No restore point (System Protection may be off, code {code}); OTX.GNG's own snapshot still covers this change");
    }
    info.dwEventType = END_SYSTEM_CHANGE;
    info.llSequenceNumber = st.llSequenceNumber;
    let _ = unsafe { SRSetRestorePointW(&info, &mut st) };
    let _ = std::fs::write(&marker, today.to_string());
    "System Restore point created".into()
}

pub fn set_tweak(state: &CoreState, id: &str, on: bool, dry_run: bool) -> Result<Response, ActionError> {
    if state.safe_mode {
        return Err(err(ErrorCode::SafeMode, "Safe Mode is on, so the Core makes no changes."));
    }
    let m = otx_tweaks::find(id).ok_or_else(|| err(ErrorCode::NotAllowed, "unknown tweak"))?;
    if m.scope != Scope::Machine {
        return Err(err(ErrorCode::NotAllowed, "this tweak is applied by the app, not the Core"));
    }
    let dry = dry_run || state.dry_run;
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    if !on {
        // Revert only what OTX.GNG changed, from its snapshot.
        let Some(entry) = j.entries().iter().rev().find(|e| matches!(&e.revert, RevertOp::Tweak { id: t, .. } if t == id)).cloned() else {
            return Err(err(ErrorCode::Validation, "OTX.GNG did not make this change, so there is no snapshot to restore. Windows or another app set it."));
        };
        if dry {
            return Ok(Response::Action(ActionResult { dry_run: true, changes: vec![format!("Would revert: {}", entry.title)], change_id: None }));
        }
        let (ok, failed) = j.revert_where(&SystemReverter { dry_run: false }, |e| e.id == entry.id).map_err(|e| err(ErrorCode::Internal, e.to_string()))?;
        if failed > 0 || ok == 0 {
            return Err(err(ErrorCode::Internal, "The revert did not complete; it is kept for retry (see the Core log)."));
        }
        return Ok(Response::Action(ActionResult { dry_run: false, changes: vec![format!("Reverted: {}", m.title)], change_id: None }));
    }

    let sys = sys();
    if dry {
        let a = otx_tweaks::apply(&m, &sys, true).map_err(|e| err(ErrorCode::Unsupported, e))?;
        return Ok(Response::Action(ActionResult { dry_run: true, changes: a.changes, change_id: None }));
    }
    let note = restore_point(state);
    // Snapshot first (dry run), journal it, then apply for real.
    let snap = otx_tweaks::apply(&m, &sys, true).map_err(|e| err(ErrorCode::Unsupported, e))?;
    let entry_id = crate::server::random_hex(8);
    j.record(Entry {
        id: entry_id.clone(),
        created_unix: crate::process::now_unix(),
        title: m.title.clone(),
        detail: snap.changes.join("; "),
        temporary: false,
        revert: RevertOp::Tweak { id: m.id.clone(), prev: snap.prev },
    })
    .map_err(|e| err(ErrorCode::Internal, e.to_string()))?;
    match otx_tweaks::apply(&m, &sys, false) {
        Ok(a) => {
            let mut changes = a.changes;
            changes.push(note);
            if m.requires_reboot {
                changes.push("Takes effect after a restart".into());
            }
            tracing::info!(tweak = %m.id, "applied");
            Ok(Response::Action(ActionResult { dry_run: false, changes, change_id: Some(entry_id) }))
        }
        Err(e) => {
            let _ = j.forget(&entry_id);
            Err(err(ErrorCode::Internal, e))
        }
    }
}

pub fn clean_memory(state: &CoreState, methods: &[RamMethod], allow: &[String], skip: &[String], dry_run: bool) -> Result<Response, ActionError> {
    if state.safe_mode {
        return Err(err(ErrorCode::SafeMode, "Safe Mode is on, so the Core makes no changes."));
    }
    let names_ok = |v: &[String]| v.iter().all(|n| n.len() < 128 && n.to_ascii_lowercase().ends_with(".exe") && !n.contains(['\\', '/', ':']));
    if !names_ok(allow) || !names_ok(skip) || allow.len() > 200 || skip.len() > 20 {
        return Err(err(ErrorCode::Validation, "allow and skip lists must be plain exe names"));
    }
    let o = crate::ram::clean(methods, allow, skip, dry_run || state.dry_run);
    if let Some(e) = o.journal {
        let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
        let _ = j.record(e);
    }
    Ok(Response::RamClean(o.report))
}

pub fn set_dns(state: &CoreState, servers: &[String], dry_run: bool) -> Result<Response, ActionError> {
    if state.safe_mode {
        return Err(err(ErrorCode::SafeMode, "Safe Mode is on, so the Core makes no changes."));
    }
    let (v4, v6) = crate::dns::split_servers(servers).map_err(|e| err(ErrorCode::Validation, e))?;
    let adapter = otx_sysinfo::primary_adapter_guid().ok_or_else(|| err(ErrorCode::Unsupported, "no connected network adapter"))?;
    let prev4 = crate::dns::get(&adapter, false).map_err(|e| err(ErrorCode::Unsupported, e))?;
    let prev6 = crate::dns::get(&adapter, true).unwrap_or_default();
    let (new4, new6) = (v4.join(","), v6.join(","));
    let show = |s: &str| if s.is_empty() { "automatic".to_string() } else { s.to_string() };
    let mut changes = vec![format!("IPv4 DNS: {} → {}", show(&prev4), show(&new4))];
    if !v6.is_empty() || !prev6.is_empty() {
        changes.push(format!("IPv6 DNS: {} → {}", show(&prev6), show(&new6)));
    }
    if dry_run || state.dry_run {
        return Ok(Response::Action(ActionResult { dry_run: true, changes, change_id: None }));
    }
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    // Replace any earlier DNS entry: the oldest snapshot is the true original.
    let original = j.entries().iter().find_map(|e| match &e.revert {
        RevertOp::Dns { adapter: a, v4, v6 } if *a == adapter => Some((v4.clone(), v6.clone())),
        _ => None,
    });
    let (orig4, orig6) = original.unwrap_or((prev4.clone(), prev6.clone()));
    let old: Vec<String> = j.entries().iter().filter(|e| matches!(e.revert, RevertOp::Dns { .. })).map(|e| e.id.clone()).collect();
    for id in old {
        let _ = j.forget(&id);
    }
    let id = crate::server::random_hex(8);
    if !servers.is_empty() {
        j.record(Entry {
            id: id.clone(),
            created_unix: crate::process::now_unix(),
            title: format!("DNS set to {}", if new4.is_empty() { &new6 } else { &new4 }),
            detail: format!("previous: {}", show(&orig4)),
            temporary: false,
            revert: RevertOp::Dns { adapter: adapter.clone(), v4: orig4, v6: orig6 },
        })
        .map_err(|e| err(ErrorCode::Internal, e.to_string()))?;
    }
    let r = crate::dns::set(&adapter, false, &new4).and_then(|_| if v6.is_empty() && prev6.is_empty() { Ok(()) } else { crate::dns::set(&adapter, true, &new6) });
    if let Err(e) = r {
        let _ = crate::dns::set(&adapter, false, &prev4);
        let _ = j.forget(&id);
        return Err(err(ErrorCode::Internal, e));
    }
    changes.push(if crate::dns::flush_cache() { "DNS cache flushed".into() } else { "DNS cache flush failed (it refreshes on its own)".into() });
    Ok(Response::Action(ActionResult { dry_run: false, changes, change_id: Some(id) }))
}
