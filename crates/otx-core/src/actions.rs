//! Request handlers for privileged actions and the change journal.

use crate::journal::SystemReverter;
use crate::process::{self, ActionError};
use crate::server::random_hex;
use crate::state::CoreState;
use otx_protocol::{ActionResult, ChangeEntry, ErrorCode, PriorityLevel, ProcessRef, Response};

fn guard(state: &CoreState) -> Result<(), ActionError> {
    if state.safe_mode {
        return Err(ActionError { code: ErrorCode::SafeMode, message: "Safe Mode is on, so the Core makes no changes.".into() });
    }
    Ok(())
}

fn journal_err(e: std::io::Error) -> ActionError {
    ActionError { code: ErrorCode::Internal, message: format!("could not write the change journal: {e}") }
}

pub fn set_priority(state: &CoreState, target: ProcessRef, level: PriorityLevel, dry_run: bool) -> Result<Response, ActionError> {
    guard(state)?;
    let t = process::open(target, process::RIGHTS_SET)?;
    let previous = process::current_priority(&t).unwrap_or(PriorityLevel::Normal);
    let desc = format!(
        "{}: priority {} → {}",
        t.exe,
        otx_monitor::priority_class_name(previous),
        otx_monitor::priority_class_name(level)
    );
    if dry_run || state.dry_run {
        return Ok(Response::Action(ActionResult { dry_run: true, changes: vec![desc], change_id: None }));
    }
    let id = random_hex(8);
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    j.record(process::priority_entry(id.clone(), target, &t.exe, previous, level)).map_err(journal_err)?;
    if let Err(e) = process::set_priority(&t, level) {
        let _ = j.forget(&id);
        return Err(e);
    }
    tracing::info!(%desc, "applied");
    Ok(Response::Action(ActionResult { dry_run: false, changes: vec![desc], change_id: Some(id) }))
}

pub fn set_efficiency(state: &CoreState, target: ProcessRef, on: bool, dry_run: bool) -> Result<Response, ActionError> {
    guard(state)?;
    let t = process::open(target, process::RIGHTS_SET)?;
    let previous = process::throttling(&t).unwrap_or((0, 0));
    let desc = format!("{}: efficiency mode {}", t.exe, if on { "on" } else { "off" });
    if dry_run || state.dry_run {
        return Ok(Response::Action(ActionResult { dry_run: true, changes: vec![desc], change_id: None }));
    }
    let id = random_hex(8);
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    j.record(process::efficiency_entry(id.clone(), target, &t.exe, on, previous)).map_err(journal_err)?;
    let (c, s) = process::efficiency_masks(on);
    if let Err(e) = process::set_throttling(&t, c, s) {
        let _ = j.forget(&id);
        return Err(e);
    }
    tracing::info!(%desc, "applied");
    Ok(Response::Action(ActionResult { dry_run: false, changes: vec![desc], change_id: Some(id) }))
}

pub fn end_process(state: &CoreState, target: ProcessRef, dry_run: bool) -> Result<Response, ActionError> {
    guard(state)?;
    let t = process::open(target, process::RIGHTS_END)?;
    let desc = format!("{} (PID {}) ended", t.exe, target.pid);
    if dry_run || state.dry_run {
        return Ok(Response::Action(ActionResult { dry_run: true, changes: vec![desc], change_id: None }));
    }
    process::terminate(&t)?;
    tracing::info!(%desc, "applied (not reversible)");
    Ok(Response::Action(ActionResult { dry_run: false, changes: vec![desc], change_id: None }))
}

pub fn list_changes(state: &CoreState) -> Response {
    let j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    Response::Changes(
        j.entries()
            .iter()
            .rev()
            .map(|e| ChangeEntry { id: e.id.clone(), created_unix: e.created_unix, title: e.title.clone(), detail: e.detail.clone(), temporary: e.temporary })
            .collect(),
    )
}

pub fn revert(state: &CoreState, id: Option<&str>) -> Result<Response, ActionError> {
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    let titles: Vec<String> = j.entries().iter().filter(|e| id.is_none_or(|i| e.id == i)).map(|e| e.title.clone()).collect();
    if id.is_some() && titles.is_empty() {
        return Err(ActionError { code: ErrorCode::Validation, message: "That change is no longer in the journal.".into() });
    }
    let r = SystemReverter { dry_run: state.dry_run };
    let (ok, failed) = j.revert_where(&r, |e| id.is_none_or(|i| e.id == i)).map_err(journal_err)?;
    if failed > 0 {
        return Err(ActionError { code: ErrorCode::Internal, message: format!("{ok} reverted, {failed} could not be reverted (kept for retry; see the Core log).") });
    }
    Ok(Response::Action(ActionResult { dry_run: state.dry_run, changes: titles.into_iter().map(|t| format!("Reverted: {t}")).collect(), change_id: None }))
}
