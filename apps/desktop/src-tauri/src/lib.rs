//! OTX.GNG desktop shell.
//!
//! Runs as the normal user (never elevated). Privileged work goes to the Core service over the
//! named pipe (`core_client`). This process also hosts the tray, global hotkeys, autostart and
//! "close to tray", and collects system facts itself when the Core is not running.

mod core_client;

use core_client::CoreClient;
use otx_protocol::{CoreConnection, InfoSource, Request, Response, SystemSnapshot};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub const BRAND_LINE: &str = "OTX.GNG — Built and performed by Thinuda";

#[derive(Default)]
struct AppState {
    core: CoreClient,
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
        .invoke_handler(tauri::generate_handler![get_system_info, core_connection, set_close_to_tray, set_shortcuts])
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
