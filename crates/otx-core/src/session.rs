//! Game Mode sessions. Every step is temporary and journaled; the session ends (and everything
//! is reverted) when the UI asks, when the game process exits (Core watchdog, independent of the
//! app), or at the next Core start after a crash or reboot (journal recovery).
//!
//! The watchdog polls the process list every 2 s instead of holding a handle to the game, so the
//! game process is never opened just to wait on it.

use crate::journal::{Entry, RevertOp, SystemReverter};
use crate::process::{self, ActionError};
use crate::state::CoreState;
use otx_protocol::{ErrorCode, PriorityLevel, ProcessRef, RamMethod, Response, SessionProfile, SessionReport, SessionStep, StepStatus};
use otx_tweaks::{win::WinSys, Prev, Scope, Sys};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Active {
    pub entries: Vec<String>,
    pub generation: u64,
}

fn step(id: &str, label: &str, status: StepStatus, detail: impl Into<String>) -> SessionStep {
    let d: String = detail.into();
    SessionStep { id: id.into(), label: label.into(), status, detail: (!d.is_empty()).then_some(d) }
}

fn record(state: &CoreState, e: Entry) -> Option<String> {
    let id = e.id.clone();
    state.journal.lock().unwrap_or_else(|p| p.into_inner()).record(e).ok().map(|_| id)
}

pub fn start(state: &Arc<CoreState>, game: Option<ProcessRef>, profile: &SessionProfile, dry_run: bool) -> Result<Response, ActionError> {
    if state.safe_mode {
        return Err(ActionError { code: ErrorCode::SafeMode, message: "Safe Mode is on, so the Core makes no changes.".into() });
    }
    if profile.game_priority == PriorityLevel::Idle || profile.background.len() > 100 || profile.allow.len() > 200 {
        return Err(ActionError { code: ErrorCode::Validation, message: "invalid session profile".into() });
    }
    let dry = dry_run || state.dry_run;
    if !dry {
        end(state); // one session at a time
    }
    let mut steps = vec![step("snapshot", "Snapshot current state", StepStatus::Done, "Every change below is journaled before it is made")];
    let mut entries = Vec::new();
    let mut freed = 0i64;

    let game_exe = game.and_then(|g| otx_monitor::nt::processes().ok()?.into_iter().find(|p| p.pid == g.pid && p.create_time == g.create_time).map(|p| p.name));
    if game.is_some() && game_exe.is_none() {
        return Err(ActionError { code: ErrorCode::Validation, message: "The game is no longer running.".into() });
    }

    // RAM
    if profile.clean_memory {
        let skip: Vec<String> = game_exe.iter().cloned().collect();
        let o = crate::ram::clean(&[RamMethod::MemoryPriority, RamMethod::TrimWorkingSets], &profile.allow, &skip, dry);
        freed = o.report.freed_mb;
        if let Some(e) = o.journal {
            entries.extend(record(state, e));
        }
        let detail = if dry { o.report.done.join("; ") } else { format!("{} MB more available (measured)", o.report.freed_mb.max(0)) };
        steps.push(step("ram", "Free memory", StepStatus::Done, detail));
    } else {
        steps.push(step("ram", "Free memory", StepStatus::Skipped, "Off in this profile"));
    }

    // CPU: game priority
    match (game, &game_exe) {
        (Some(g), Some(exe)) => match crate::actions::set_priority(state, g, profile.game_priority, dry) {
            Ok(Response::Action(a)) => {
                entries.extend(a.change_id);
                steps.push(step("cpu-game", "Prioritise the game", StepStatus::Done, format!("{exe}: {}", otx_monitor::priority_class_name(profile.game_priority))));
            }
            Ok(_) => {}
            Err(e) => steps.push(step("cpu-game", "Prioritise the game", StepStatus::Failed, format!("{} (some anti-cheats block this; nothing else is affected)", e.message))),
        },
        _ => steps.push(step("cpu-game", "Prioritise the game", StepStatus::Skipped, "No game running: general tune-up")),
    }

    // CPU: background apps
    let wanted: Vec<String> = profile.background.iter().map(|s| s.to_ascii_lowercase()).collect();
    let mut calmed = 0;
    if !wanted.is_empty() {
        for p in otx_monitor::nt::processes().unwrap_or_default() {
            if p.session_id == 0 || !wanted.contains(&p.name.to_ascii_lowercase()) || Some(&p.name) == game_exe.as_ref() {
                continue;
            }
            let r = ProcessRef { pid: p.pid, create_time: p.create_time };
            if let Ok(Response::Action(a)) = crate::actions::set_priority(state, r, PriorityLevel::BelowNormal, dry) {
                entries.extend(a.change_id);
                if let Ok(Response::Action(b)) = crate::actions::set_efficiency(state, r, true, dry) {
                    entries.extend(b.change_id);
                }
                calmed += 1;
            }
        }
    }
    steps.push(step(
        "cpu-bg",
        "Calm background apps",
        if wanted.is_empty() { StepStatus::Skipped } else { StepStatus::Done },
        if wanted.is_empty() { "No background apps listed in this profile".to_string() } else { format!("{calmed} processes: Below Normal + efficiency mode") },
    ));

    // Power
    if profile.high_performance_power {
        let sys = WinSys { scope: Scope::Machine };
        match sys.active_scheme() {
            Ok(prev) if !dry => match sys.performance_scheme().and_then(|s| sys.set_active_scheme(&s)) {
                Ok(()) => {
                    entries.extend(record(
                        state,
                        Entry {
                            id: crate::server::random_hex(8),
                            created_unix: process::now_unix(),
                            title: "Power plan: high performance (session)".into(),
                            detail: format!("was {prev}"),
                            temporary: true,
                            revert: RevertOp::Tweak { id: "session-power".into(), prev: vec![Prev::Scheme { scheme: prev }] },
                        },
                    ));
                    steps.push(step("power", "High-performance power plan", StepStatus::Done, "Restored when the session ends"));
                }
                Err(e) => steps.push(step("power", "High-performance power plan", StepStatus::Failed, e)),
            },
            Ok(_) => steps.push(step("power", "High-performance power plan", StepStatus::Done, "Would switch the power plan (dry run)")),
            Err(e) => steps.push(step("power", "High-performance power plan", StepStatus::Failed, e)),
        }
    } else {
        steps.push(step("power", "High-performance power plan", StepStatus::Skipped, "Off in this profile"));
    }

    steps.push(step("network", "Focus network on the game", StepStatus::Skipped, "Turn on Focus mode in Traffic Shaper"));
    let watch = game.is_some() && !dry;
    steps.push(step(
        "watchdog",
        "Auto-revert watchdog",
        if watch { StepStatus::Done } else { StepStatus::Skipped },
        if game.is_some() { "Everything reverts when the game exits" } else { "End the session from Home to revert" },
    ));

    if !dry {
        let generation = {
            let mut s = state.session.lock().unwrap_or_else(|p| p.into_inner());
            let generation = s.as_ref().map(|a| a.generation + 1).unwrap_or(1);
            *s = Some(Active { entries, generation });
            generation
        };
        if let Some(g) = game {
            spawn_watchdog(state.clone(), g, generation);
        }
    }
    Ok(Response::Session(SessionReport { dry_run: dry, steps, freed_mb: freed, active: !dry }))
}

/// Reverts everything the current session changed. Returns how many changes were reverted.
pub fn end(state: &CoreState) -> u32 {
    let Some(active) = state.session.lock().unwrap_or_else(|p| p.into_inner()).take() else { return 0 };
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    let (ok, failed) = j.revert_where(&SystemReverter { dry_run: false }, |e| active.entries.contains(&e.id)).unwrap_or((0, 0));
    if failed > 0 {
        tracing::warn!(failed, "some session changes could not be reverted; kept for retry");
    }
    tracing::info!(reverted = ok, "session ended");
    ok
}

fn spawn_watchdog(state: Arc<CoreState>, game: ProcessRef, generation: u64) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(2));
        let current = state.session.lock().unwrap_or_else(|p| p.into_inner()).as_ref().map(|a| a.generation);
        if current != Some(generation) {
            return; // ended or replaced by a newer session
        }
        let alive = otx_monitor::nt::processes().map(|ps| ps.iter().any(|p| p.pid == game.pid && p.create_time == game.create_time)).unwrap_or(true);
        if !alive {
            tracing::info!(pid = game.pid, "game exited; reverting the session");
            end(&state);
            return;
        }
    });
}

pub fn handle_end(state: &CoreState) -> Response {
    let n = end(state);
    Response::Session(SessionReport { dry_run: false, steps: vec![step("revert", "Session ended", StepStatus::Done, format!("{n} changes reverted"))], freed_mb: 0, active: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::Journal;
    use otx_protocol::CoreMode;
    use std::os::windows::process::CommandExt;

    fn state() -> (Arc<CoreState>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let j = Journal::open(dir.path()).unwrap();
        (Arc::new(CoreState::new(CoreMode::Console, true, false, false, false, std::path::PathBuf::new(), dir.path().into(), 0, j)), dir)
    }

    #[test]
    fn session_on_a_throwaway_game_reverts_when_it_exits() {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut game = std::process::Command::new("ping").args(["-n", "60", "127.0.0.1"]).stdout(std::process::Stdio::null()).creation_flags(CREATE_NO_WINDOW).spawn().unwrap();
        let p = otx_monitor::nt::processes().unwrap().into_iter().find(|p| p.pid == game.id()).unwrap();
        let g = ProcessRef { pid: p.pid, create_time: p.create_time };
        let (st, _d) = state();
        // Power plan and RAM off: the test must not change this PC's power plan or trim real apps.
        let profile = SessionProfile { game_priority: PriorityLevel::AboveNormal, background: vec![], clean_memory: false, allow: vec![], high_performance_power: false };
        let Response::Session(r) = start(&st, Some(g), &profile, false).map_err(|e| e.message).unwrap() else { panic!() };
        assert!(r.active);
        assert_eq!(r.steps.iter().find(|s| s.id == "cpu-game").unwrap().status, StepStatus::Done);
        let t = crate::process::open(g, crate::process::RIGHTS_SET).unwrap();
        assert_eq!(crate::process::current_priority(&t), Some(PriorityLevel::AboveNormal));
        assert_eq!(st.journal.lock().unwrap().entries().len(), 1);

        game.kill().unwrap();
        let _ = game.wait();
        drop(t);
        // The watchdog notices within ~2 s and reverts; the journal empties.
        for _ in 0..30 {
            if st.session.lock().unwrap().is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        assert!(st.session.lock().unwrap().is_none(), "watchdog did not end the session");
        assert!(st.journal.lock().unwrap().entries().is_empty(), "session changes were not reverted");
    }

    #[test]
    fn dry_run_session_changes_nothing() {
        let (st, _d) = state();
        let profile = SessionProfile { game_priority: PriorityLevel::High, background: vec!["notepad.exe".into()], clean_memory: true, allow: vec![], high_performance_power: true };
        let Response::Session(r) = start(&st, None, &profile, true).map_err(|e| e.message).unwrap() else { panic!() };
        assert!(r.dry_run && !r.active);
        assert!(st.journal.lock().unwrap().entries().is_empty());
        assert!(st.session.lock().unwrap().is_none());
    }
}
