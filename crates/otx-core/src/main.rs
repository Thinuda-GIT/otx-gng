//! otx-core: the OTX.GNG Core.
//!
//! Commands (run from an elevated PowerShell unless noted):
//!   otx-core service                   Entry point used by the Service Control Manager.
//!   otx-core console [--dev] [--safe-mode] [--dry-run]
//!                                      Run in the foreground for development. No elevation
//!                                      needed; with --dev any process in your session may connect.
//!   otx-core install | uninstall | start | stop | status
//!   otx-core revert-all                Undo every journaled change (used by the uninstaller).
//!
//! Safe Mode (`--safe-mode` or env OTX_SAFE_MODE=1) disables every privileged module; the
//! Core still answers status and read-only queries. Dry run (`--dry-run`) logs what would change.

mod actions;
mod install;
mod journal;
mod process;
mod security;
mod server;
mod state;

use journal::{Journal, SystemReverter};
use otx_protocol::{CoreMode, SERVICE_NAME};
use state::CoreState;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    dev: bool,
    safe_mode: bool,
    dry_run: bool,
}

fn parse_flags(args: &[String]) -> Flags {
    Flags {
        dev: args.iter().any(|a| a == "--dev"),
        safe_mode: args.iter().any(|a| a == "--safe-mode") || std::env::var("OTX_SAFE_MODE").map(|v| v == "1").unwrap_or(false),
        dry_run: args.iter().any(|a| a == "--dry-run"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let result = match cmd {
        "service" => service::run(),
        "console" => run_console(parse_flags(&args)),
        "install" => install::install(),
        "uninstall" => install::uninstall(),
        "start" => install::start(),
        "stop" => install::stop(),
        "status" => install::status().map(|s| println!("{s}")),
        "revert-all" => revert_all(),
        _ => {
            println!("usage: otx-core <service|console [--dev] [--safe-mode] [--dry-run]|install|uninstall|start|stop|status|revert-all>");
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("otx-core {cmd}: {e:#}");
        std::process::exit(1);
    }
}

/// Where logs and the journal live.
/// * Service / elevated: %ProgramData%\OTX.GNG, with a protected DACL.
/// * Development console without elevation: %LOCALAPPDATA%\OTX.GNG\dev.
fn data_dir(service_mode: bool) -> anyhow::Result<PathBuf> {
    if service_mode || security::is_elevated() {
        let base = std::env::var_os("ProgramData").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
        let dir = base.join("OTX.GNG");
        security::secure_directory(&dir)?;
        Ok(dir)
    } else {
        let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA not set"))?;
        let dir = base.join("OTX.GNG").join("dev");
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }
}

fn init_logging(dir: &std::path::Path, to_stdout: bool) -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::prelude::*;
    let file = tracing_appender::rolling::daily(dir.join("logs"), "core.log");
    let (writer, guard) = tracing_appender::non_blocking(file);
    let filter = tracing_subscriber::EnvFilter::try_from_env("OTX_LOG").unwrap_or_else(|_| "info".into());
    let file_layer = tracing_subscriber::fmt::layer().with_ansi(false).with_writer(writer);
    let stdout_layer = to_stdout.then(|| tracing_subscriber::fmt::layer().with_target(false));
    tracing_subscriber::registry().with(filter).with(file_layer).with(stdout_layer).init();
    guard
}

/// Shared startup: open the journal, run the watchdog recovery, build state.
fn build_state(mode: CoreMode, flags: Flags, dir: PathBuf) -> anyhow::Result<Arc<CoreState>> {
    let core_dir = std::env::current_exe()?.parent().map(PathBuf::from).unwrap_or_default();
    let mut journal = Journal::open(&dir)?;
    let reverter = SystemReverter { dry_run: flags.dry_run };
    let recovered = journal.recover(&reverter)?;
    if recovered > 0 {
        tracing::warn!(recovered, "watchdog reverted temporary changes left from a previous session");
    }
    let elevated = security::is_elevated();
    tracing::info!(?mode, dev = flags.dev, safe_mode = flags.safe_mode, dry_run = flags.dry_run, elevated, data = %dir.display(), "OTX.GNG Core {} starting", env!("CARGO_PKG_VERSION"));
    if flags.dev {
        tracing::warn!("DEV MODE: any process in an interactive session may connect. Never use --dev on a normal install.");
    }
    Ok(Arc::new(CoreState::new(mode, flags.dev, flags.safe_mode, flags.dry_run, elevated, core_dir, dir, recovered, journal)))
}

fn run_console(flags: Flags) -> anyhow::Result<()> {
    let dir = data_dir(false)?;
    let _guard = init_logging(&dir, true);
    let state = build_state(CoreMode::Console, flags, dir)?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let (tx, rx) = watch::channel(false);
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(true);
        });
        server::serve(state, rx).await
    })
}

fn revert_all() -> anyhow::Result<()> {
    let dir = data_dir(true)?;
    let _guard = init_logging(&dir, true);
    let mut j = Journal::open(&dir)?;
    let (ok, failed) = j.revert_where(&SystemReverter { dry_run: false }, |_| true)?;
    println!("Reverted {ok} change(s); {failed} failed (see the log in {}).", dir.join("logs").display());
    Ok(())
}

mod service {
    //! Windows service host (Service Control Manager integration).
    use super::*;
    use std::ffi::OsString;
    use std::time::Duration;
    use windows_service::service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType};
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};

    define_windows_service!(ffi_service_main, service_main);

    pub fn run() -> anyhow::Result<()> {
        service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
        Ok(())
    }

    fn service_main(_args: Vec<OsString>) {
        if let Err(e) = run_service() {
            tracing::error!(error = %e, "service failed");
        }
    }

    fn run_service() -> anyhow::Result<()> {
        let (tx, rx) = watch::channel(false);
        let handler = move |c: ServiceControl| match c {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                let _ = tx.send(true);
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        };
        let status = service_control_handler::register(SERVICE_NAME, handler)?;
        let set = |state: ServiceState, accept: ServiceControlAccept, code: u32| {
            status.set_service_status(ServiceStatus {
                service_type: ServiceType::OWN_PROCESS,
                current_state: state,
                controls_accepted: accept,
                exit_code: ServiceExitCode::Win32(code),
                checkpoint: 0,
                wait_hint: Duration::from_secs(10),
                process_id: None,
            })
        };
        set(ServiceState::StartPending, ServiceControlAccept::empty(), 0)?;

        let dir = data_dir(true)?;
        let _guard = init_logging(&dir, false);
        let flags = Flags { dev: false, safe_mode: std::env::var("OTX_SAFE_MODE").map(|v| v == "1").unwrap_or(false), dry_run: false };
        let state = build_state(CoreMode::Service, flags, dir)?;

        set(ServiceState::Running, ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN, 0)?;
        let rt = tokio::runtime::Runtime::new()?;
        let result = rt.block_on(server::serve(state, rx));
        if let Err(e) = &result {
            tracing::error!(error = %e, "pipe server stopped with an error");
        }
        set(ServiceState::Stopped, ServiceControlAccept::empty(), if result.is_ok() { 0 } else { 1 })?;
        Ok(())
    }
}
