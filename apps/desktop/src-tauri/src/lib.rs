//! OTX.GNG desktop shell.
//!
//! Runs as the normal user (never elevated). Privileged work goes to the Core service over the
//! named pipe (`core_client`). This process also hosts the tray, global hotkeys, autostart and
//! "close to tray", and collects system facts itself when the Core is not running.

mod boost;
mod core_client;
mod games;
mod net;
mod quality;

use core_client::CoreClient;
use otx_protocol::{
    ActionResult, ChangeEntry, CoreConnection, InfoSource, PriorityLevel, ProcessDetails, ProcessGroup, ProcessRef, Request, Response,
    SignatureState, SystemMetrics, SystemSnapshot,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const BRAND_LINE: &str = "OTX.GNG — Built and performed by Thinuda";

#[derive(Default)]
pub struct AppState {
    pub core: CoreClient,
    pub library: games::Library,
    pub traffic: net::TrafficState,
    /// Unprivileged live monitor (no handles to other processes are opened).
    monitor: std::sync::Arc<Mutex<Option<otx_monitor::Monitor>>>,
    close_to_tray: AtomicBool,
    /// Shortcut string → action id, so the global handler knows what was pressed.
    shortcuts: Mutex<Vec<(Shortcut, String)>>,
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// System facts: from the Core when it is reachable, otherwise collected locally (read-only,
/// no elevation needed). HKCU startup entries are always counted here, in the user's process.
#[tauri::command]
async fn get_system_info(state: tauri::State<'_, AppState>) -> Result<SystemSnapshot, String> {
    let (source, info) = match state.core.call(Request::GetSystemInfo).await {
        Ok(Response::SystemInfo(i)) => (InfoSource::Core, i),
        _ => {
            let i = tokio::task::spawn_blocking(otx_sysinfo::collect).await.map_err(|e| e.to_string())?;
            (InfoSource::Local, i)
        }
    };
    let user_startup = tokio::task::spawn_blocking(otx_sysinfo::startup_apps_current_user).await.unwrap_or(0);
    Ok(SystemSnapshot { source, startup_apps: info.startup_apps_machine + user_startup, info })
}

#[tauri::command]
async fn core_connection(state: tauri::State<'_, AppState>) -> Result<CoreConnection, String> {
    Ok(state.core.connection().await)
}

/// Runs a closure on the shared monitor off the async runtime (Win32 calls block).
async fn with_monitor<T: Send + 'static>(state: &AppState, f: impl FnOnce(&mut otx_monitor::Monitor) -> T + Send + 'static) -> Result<T, String> {
    let m = state.monitor.clone();
    tokio::task::spawn_blocking(move || {
        let mut g = m.lock().unwrap_or_else(|p| p.into_inner());
        let mon = g.get_or_insert_with(otx_monitor::Monitor::new);
        f(mon)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn monitor_system(state: tauri::State<'_, AppState>) -> Result<SystemMetrics, String> {
    with_monitor(&state, |m| m.system()).await
}

#[tauri::command]
async fn monitor_processes(state: tauri::State<'_, AppState>) -> Result<Vec<ProcessGroup>, String> {
    with_monitor(&state, |m| m.processes()).await
}

/// File facts for the "What is this process?" panel. Reads the exe file only.
#[tauri::command]
async fn process_details(path: Option<String>) -> Result<ProcessDetails, String> {
    let Some(path) = path.filter(|p| p.len() < 1024 && p.to_ascii_lowercase().ends_with(".exe") && std::path::Path::new(p).is_file()) else {
        return Ok(ProcessDetails { path: None, signer: None, signature: SignatureState::Unknown, startup: false, efficiency_mode: None });
    };
    tokio::task::spawn_blocking(move || {
        let (signature, signer) = otx_monitor::details::signature(&path);
        let startup = otx_monitor::details::registered_at_startup(&path);
        ProcessDetails { path: Some(path), signer, signature, startup, efficiency_mode: None }
    })
    .await
    .map_err(|e| e.to_string())
}

fn action(r: Result<Response, core_client::ClientError>) -> Result<ActionResult, String> {
    match r {
        Ok(Response::Action(a)) => Ok(a),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) => Err(e.message),
    }
}

#[tauri::command]
async fn core_set_priority(state: tauri::State<'_, AppState>, target: ProcessRef, level: PriorityLevel, dry_run: bool) -> Result<ActionResult, String> {
    action(state.core.call(Request::SetProcessPriority { target, level, dry_run }).await)
}

#[tauri::command]
async fn core_set_efficiency(state: tauri::State<'_, AppState>, target: ProcessRef, on: bool, dry_run: bool) -> Result<ActionResult, String> {
    action(state.core.call(Request::SetEfficiencyMode { target, on, dry_run }).await)
}

#[tauri::command]
async fn core_end_process(state: tauri::State<'_, AppState>, target: ProcessRef, dry_run: bool) -> Result<ActionResult, String> {
    action(state.core.call(Request::EndProcess { target, dry_run }).await)
}

#[tauri::command]
async fn core_list_changes(state: tauri::State<'_, AppState>) -> Result<Vec<ChangeEntry>, String> {
    match state.core.call(Request::ListChanges).await {
        Ok(Response::Changes(c)) => Ok(c),
        Ok(_) => Err("unexpected reply from the Core".into()),
        Err(e) => Err(e.message),
    }
}

#[tauri::command]
async fn core_revert(state: tauri::State<'_, AppState>, id: Option<String>) -> Result<ActionResult, String> {
    let req = match id {
        Some(id) => Request::RevertChange { id },
        None => Request::RevertAll,
    };
    action(state.core.call(req).await)
}

#[tauri::command]
fn set_close_to_tray(state: tauri::State<'_, AppState>, enabled: bool) {
    state.close_to_tray.store(enabled, Ordering::Relaxed);
}

/// Registers the global hotkeys. `bindings` maps an action id to an accelerator such as
/// "Shift+Ctrl+Q". Returns the bindings that could not be registered (taken by another app).
#[tauri::command]
fn set_shortcuts(app: AppHandle, state: tauri::State<'_, AppState>, bindings: Vec<(String, String)>) -> Vec<String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut registered = Vec::new();
    let mut failed = Vec::new();
    for (action, accel) in bindings {
        match accel.parse::<Shortcut>() {
            Ok(sc) if gs.register(sc).is_ok() => registered.push((sc, action)),
            _ => failed.push(accel),
        }
    }
    *state.shortcuts.lock().unwrap_or_else(|p| p.into_inner()) = registered;
    failed
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let item = |id: &str, label: &str, enabled: bool| MenuItem::with_id(app, id, label, enabled, None::<&str>);
    let open = item("open", "Open OTX.GNG", true)?;
    let boost = item("boost", "Boost now", true)?;
    let game_mode = item("gameMode", "Game Mode (arrives in Phase 5)", false)?;
    let restore = item("restoreInternet", "Restore normal internet", true)?;
    let revert = item("revertAll", "Revert everything", true)?;
    let quit = item("quit", "Quit", true)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &sep1, &boost, &game_mode, &restore, &revert, &sep2, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip(BRAND_LINE)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            other => {
                // The UI owns these actions (and shows what happened).
                show_main(app);
                let _ = app.emit("tray-action", other.to_string());
            }
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch focuses the running window instead of starting again.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--minimized"])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let st = app.state::<AppState>();
                    let action = st
                        .shortcuts
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .iter()
                        .find(|(s, _)| s == shortcut)
                        .map(|(_, a)| a.clone());
                    if let Some(a) = action {
                        let _ = app.emit("shortcut", a);
                    }
                })
                .build(),
        )
        .manage(AppState { close_to_tray: AtomicBool::new(true), ..Default::default() })
        .invoke_handler(tauri::generate_handler![get_system_info,
            core_connection,
            set_close_to_tray,
            set_shortcuts,
            monitor_system,
            monitor_processes,
            process_details,
            core_set_priority,
            core_set_efficiency,
            core_end_process,
            core_list_changes,
            core_revert,
            boost::tweaks_status,
            boost::tweak_set,
            boost::user_changes,
            boost::user_revert,
            boost::ram_clean,
            boost::dns_benchmark,
            boost::dns_current,
            boost::dns_set,
            games::games_scan,
            games::game_launch,
            games::games_running,
            games::game_settings,
            games::session_start,
            games::session_end,
            games::capture_start,
            games::capture_stop,
            net::net_traffic,
            net::net_connections,
            net::shaper_apply,
            net::net_restore,
            quality::net_probe,
            quality::net_traceroute,
            quality::net_mtu,
            quality::net_adapters,
            quality::net_vpn,
            quality::net_prefer,
            quality::relay_ping
        ])
        .setup(|app| {
            build_tray(app)?;
            // Started by autostart with --minimized: stay in the tray.
            let minimized = std::env::args().any(|a| a == "--minimized");
            if let Some(w) = app.get_webview_window("main") {
                if !minimized {
                    let _ = w.show();
                }
                let handle = app.handle().clone();
                w.on_window_event(move |e| {
                    if let WindowEvent::CloseRequested { api, .. } = e {
                        if handle.state::<AppState>().close_to_tray.load(Ordering::Relaxed) {
                            api.prevent_close();
                            if let Some(w) = handle.get_webview_window("main") {
                                let _ = w.hide();
                            }
                        }
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("OTX.GNG failed to start the Tauri runtime");
}
