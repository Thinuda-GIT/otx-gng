//! Library and Game Mode commands (app side). Games are launched by id only: the UI cannot pass
//! an arbitrary command line. Per-game GPU preference and fullscreen-optimizations flags are
//! per-user registry values applied with a snapshot (reverted from "What OTX.GNG changed").

use crate::AppState;
use otx_games::DetectedGame;
use otx_protocol::{ProcessRef, Request, Response, SessionProfile, SessionReport};
use otx_tweaks::model::{Op, RegValue, Root};
use std::path::Path;
use std::sync::Mutex;

#[derive(Default)]
pub struct Library {
    pub games: Mutex<Vec<DetectedGame>>,
}

#[tauri::command]
pub async fn games_scan(state: tauri::State<'_, AppState>) -> Result<Vec<DetectedGame>, String> {
    let games = tokio::task::spawn_blocking(otx_games::scan).await.map_err(|e| e.to_string())?;
    *state.library.games.lock().unwrap_or_else(|p| p.into_inner()) = games.clone();
    Ok(games)
}

fn game(state: &AppState, id: &str) -> Result<DetectedGame, String> {
    state.library.games.lock().unwrap_or_else(|p| p.into_inner()).iter().find(|g| g.id == id).cloned().ok_or_else(|| "Unknown game; rescan the Library.".to_string())
}

/// Starts a library game through its launcher (or its exe). Only launcher URIs OTX.GNG built
/// itself during the scan are used.
#[tauri::command]
pub async fn game_launch(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let g = game(&state, &id)?;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    use std::os::windows::process::CommandExt;
    match g.launch.as_deref() {
        Some(uri) if uri.starts_with("steam://rungameid/") || uri.starts_with("com.epicgames.launcher://apps/") => {
            // explorer.exe hands registered URI schemes to their launcher.
            std::process::Command::new("explorer.exe").arg(uri).creation_flags(CREATE_NO_WINDOW).spawn().map_err(|e| e.to_string())?;
        }
        Some(riot) if riot.contains("|--launch-product=") => {
            let (client, args) = riot.split_once('|').ok_or("bad Riot launch data")?;
            if !client.to_ascii_lowercase().ends_with("riotclientservices.exe") || !Path::new(client).is_file() {
                return Err("Riot Client not found".into());
            }
            std::process::Command::new(client).args(args.split_whitespace()).spawn().map_err(|e| e.to_string())?;
        }
        _ => {
            let exe = g.exe.as_deref().ok_or("This game's executable was not found; start it from its launcher.")?;
            let path = otx_games::find_exe_path(Path::new(&g.install_path), exe).ok_or("Executable not found in the install folder.")?;
            std::process::Command::new(&path).current_dir(path.parent().unwrap_or(Path::new("."))).spawn().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningGame {
    game_id: String,
    exe: String,
    process: ProcessRef,
}

/// Which library games are running right now (by exe name; no process is opened).
#[tauri::command]
pub async fn games_running(state: tauri::State<'_, AppState>) -> Result<Vec<RunningGame>, String> {
    let lib: Vec<(String, String)> = state
        .library
        .games
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .filter_map(|g| g.exe.clone().map(|e| (g.id.clone(), e.to_ascii_lowercase())))
        .collect();
    tokio::task::spawn_blocking(move || {
        let procs = otx_monitor::nt::processes().unwrap_or_default();
        let mut out = Vec::new();
        for p in procs {
            let n = p.name.to_ascii_lowercase();
            // Library games first, then known games from the offline knowledge file.
            let id = lib.iter().find(|(_, e)| *e == n).map(|(id, _)| id.clone()).or_else(|| {
                otx_monitor::policy::lookup(&n).filter(|k| k.category == "game").map(|_| format!("known:{n}"))
            });
            if let Some(game_id) = id {
                if !out.iter().any(|r: &RunningGame| r.game_id == game_id) {
                    out.push(RunningGame { game_id, exe: p.name.clone(), process: ProcessRef { pid: p.pid, create_time: p.create_time } });
                }
            }
        }
        out
    })
    .await
    .map_err(|e| e.to_string())
}

/// Applies per-game GPU preference (0 default, 1 power saving, 2 high performance) and the
/// fullscreen-optimizations flag. Both are per-user values keyed by the exe's full path.
#[tauri::command]
pub async fn game_settings(state: tauri::State<'_, AppState>, id: String, gpu: u32, fso_off: bool) -> Result<Vec<String>, String> {
    let g = game(&state, &id)?;
    let exe = g.exe.clone().ok_or("This game's executable was not found.")?;
    tokio::task::spawn_blocking(move || {
        let path = otx_games::find_exe_path(Path::new(&g.install_path), &exe).ok_or("Executable not found in the install folder.")?;
        let p = path.to_string_lossy().to_string();
        let mut changes = Vec::new();
        let gpu_ops = if gpu == 0 {
            vec![]
        } else {
            vec![Op::Reg { root: Root::Hkcu, key: r"Software\Microsoft\DirectX\UserGpuPreferences".into(), name: p.clone(), value: RegValue::Sz(format!("GpuPreference={};", gpu.min(2))) }]
        };
        changes.extend(crate::boost::set_user_ops(&format!("game-gpu:{id}"), &format!("{}: GPU preference", g.name), gpu_ops)?);
        let fso_ops = if fso_off {
            vec![Op::Reg { root: Root::Hkcu, key: r"Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers".into(), name: p, value: RegValue::Sz("~ DISABLEDXMAXIMIZEDWINDOWEDMODE".into()) }]
        } else {
            vec![]
        };
        changes.extend(crate::boost::set_user_ops(&format!("game-fso:{id}"), &format!("{}: fullscreen optimizations off", g.name), fso_ops)?);
        Ok(changes)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn session_start(state: tauri::State<'_, AppState>, game: Option<ProcessRef>, profile: SessionProfile, dry_run: bool) -> Result<SessionReport, String> {
    match state.core.call(Request::StartSession { game, profile, dry_run }).await {
        Ok(Response::Session(r)) => Ok(r),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) if e.link != otx_protocol::CoreLink::Connected => Err("Game Mode needs the OTX.GNG Core service, which is not running.".into()),
        Err(e) => Err(e.message),
    }
}

#[tauri::command]
pub async fn session_end(state: tauri::State<'_, AppState>) -> Result<SessionReport, String> {
    match state.core.call(Request::EndSession).await {
        Ok(Response::Session(r)) => Ok(r),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) => Err(e.message),
    }
}

#[tauri::command]
pub async fn capture_start(state: tauri::State<'_, AppState>, target: ProcessRef) -> Result<(), String> {
    match state.core.call(Request::StartCapture { target }).await {
        Ok(_) => Ok(()),
        Err(e) if e.link != otx_protocol::CoreLink::Connected => Err("Frame capture needs the OTX.GNG Core service.".into()),
        Err(e) => Err(e.message),
    }
}

#[tauri::command]
pub async fn capture_stop(state: tauri::State<'_, AppState>) -> Result<Vec<f32>, String> {
    match state.core.call(Request::StopCapture).await {
        Ok(Response::Capture(ft)) => Ok(ft),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) => Err(e.message),
    }
}
