//! OTX.GNG desktop shell (Phase 1).
//!
//! Phase 1 is UI-only: the window, frameless chrome and the React UI running on mock data.
//! No privileged code exists yet. Phase 2 adds the tray, global hotkeys, the unprivileged
//! User Agent (per-user tweaks) and the named-pipe client for the OTX.GNG Core service.

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("OTX.GNG failed to start the Tauri runtime");
}
