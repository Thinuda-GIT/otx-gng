//! Background automation that must keep working while the window is closed to the tray (the
//! web view is destroyed then, to free its memory):
//! * game watcher: detects library/known games starting and exiting (by exe name, every 3 s);
//! * automatic Game Mode: starts a Core session with the game's profile when a game starts.
//! The UI pushes its settings and profiles here with `set_automation`; events tell the UI (when
//! open) what happened.

use crate::AppState;
use otx_protocol::{PriorityLevel, ProcessRef, Request, Response, SessionProfile};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Automation {
    pub config: Mutex<AutoConfig>,
    current: Mutex<Option<Running>>,
}

#[derive(Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoConfig {
    pub auto_game_mode: bool,
    /// game id → profile; "default" is used for games without their own profile.
    pub profiles: HashMap<String, SessionProfile>,
}

#[derive(Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Running {
    pub game_id: String,
    pub exe: String,
    pub process: ProcessRef,
}

#[tauri::command]
pub fn set_automation(state: tauri::State<'_, AppState>, config: AutoConfig) {
    *state.automation.config.lock().unwrap_or_else(|p| p.into_inner()) = config;
}

#[tauri::command]
pub fn running_game(state: tauri::State<'_, AppState>) -> Option<Running> {
    state.automation.current.lock().unwrap_or_else(|p| p.into_inner()).clone()
}

/// Library games first (by the scanned exe), then known games from the offline knowledge file.
pub fn detect(library: &[(String, String)]) -> Option<Running> {
    let procs = otx_monitor::nt::processes().ok()?;
    procs.iter().find_map(|p| {
        let n = p.name.to_ascii_lowercase();
        let id = library
            .iter()
            .find(|(_, e)| *e == n)
            .map(|(id, _)| id.clone())
            .or_else(|| otx_monitor::policy::lookup(&n).filter(|k| k.category == "game").map(|_| format!("known:{n}")))?;
        Some(Running { game_id: id, exe: p.name.clone(), process: ProcessRef { pid: p.pid, create_time: p.create_time } })
    })
}

fn default_profile() -> SessionProfile {
    SessionProfile { game_priority: PriorityLevel::High, background: vec![], clean_memory: true, allow: vec![], high_performance_power: true }
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        // First library scan so library games are recognised even if the UI never opens.
        if let Some(st) = app.try_state::<AppState>() {
            let games = otx_games::scan();
            *st.library.games.lock().unwrap_or_else(|p| p.into_inner()) = games;
        }
        loop {
            std::thread::sleep(Duration::from_secs(3));
            let Some(st) = app.try_state::<AppState>() else { continue };
            let library: Vec<(String, String)> = st
                .library
                .games
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .iter()
                .filter_map(|g| g.exe.clone().map(|e| (g.id.clone(), e.to_ascii_lowercase())))
                .collect();
            let now = detect(&library);
            let prev = st.automation.current.lock().unwrap_or_else(|p| p.into_inner()).clone();
            if now == prev {
                continue;
            }
            *st.automation.current.lock().unwrap_or_else(|p| p.into_inner()) = now.clone();
            if let Some(old) = prev {
                if now.is_none() {
                    // The Core's own watchdog reverts the session; just tell the UI.
                    let _ = app.emit("game-closed", old);
                }
            }
            if let Some(g) = now {
                let _ = app.emit("game-started", g.clone());
                let cfg = st.automation.config.lock().unwrap_or_else(|p| p.into_inner()).clone();
                if cfg.auto_game_mode {
                    let profile = cfg.profiles.get(&g.game_id).or_else(|| cfg.profiles.get("default")).cloned().unwrap_or_else(default_profile);
                    let r = tauri::async_runtime::block_on(st.core.call(Request::StartSession { game: Some(g.process), profile, dry_run: false }));
                    match r {
                        Ok(Response::Session(report)) => {
                            let _ = app.emit("game-mode-started", (g.exe.clone(), report));
                        }
                        Ok(_) => {}
                        Err(e) => {
                            let _ = app.emit("game-mode-failed", (g.exe.clone(), e.message));
                        }
                    }
                }
            }
        }
    });
}
