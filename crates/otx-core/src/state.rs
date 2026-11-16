//! Shared Core state.

use crate::journal::Journal;
use otx_protocol::{CoreMode, CoreStatus, SystemInfo, PROTOCOL_VERSION};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct CoreState {
    pub mode: CoreMode,
    pub dev: bool,
    pub safe_mode: bool,
    pub dry_run: bool,
    pub elevated: bool,
    /// Directory holding otx-core.exe; the UI exe must live here in service mode.
    pub core_dir: PathBuf,
    /// Logs and journal location (reported in diagnostics bundles from Phase 10).
    #[allow(dead_code)]
    pub data_dir: PathBuf,
    pub started: Instant,
    pub recovered_changes: u32,
    /// Write-ahead change journal.
    pub journal: Mutex<Journal>,
    /// System info is cached briefly: it is cheap but not free (disk IOCTLs, DXGI).
    sysinfo_cache: Mutex<Option<(Instant, SystemInfo)>>,
}

const SYSINFO_TTL: Duration = Duration::from_secs(30);

impl CoreState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: CoreMode,
        dev: bool,
        safe_mode: bool,
        dry_run: bool,
        elevated: bool,
        core_dir: PathBuf,
        data_dir: PathBuf,
        recovered_changes: u32,
        journal: Journal,
    ) -> Self {
        Self {
            mode,
            dev,
            safe_mode,
            dry_run,
            elevated,
            core_dir,
            data_dir,
            started: Instant::now(),
            recovered_changes,
            journal: Mutex::new(journal),
            sysinfo_cache: Mutex::new(None),
        }
    }

    pub fn status(&self) -> CoreStatus {
        CoreStatus {
            core_version: env!("CARGO_PKG_VERSION").into(),
            protocol: PROTOCOL_VERSION,
            mode: self.mode,
            elevated: self.elevated,
            safe_mode: self.safe_mode,
            dry_run: self.dry_run,
            recovered_changes: self.recovered_changes,
            uptime_secs: self.started.elapsed().as_secs(),
        }
    }

    pub fn system_info(&self) -> SystemInfo {
        let mut cache = self.sysinfo_cache.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((at, info)) = cache.as_ref() {
            if at.elapsed() < SYSINFO_TTL {
                return info.clone();
            }
        }
        let info = otx_sysinfo::collect();
        *cache = Some((Instant::now(), info.clone()));
        info
    }
}
