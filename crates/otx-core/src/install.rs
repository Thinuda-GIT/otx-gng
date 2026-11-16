//! Service installation and control through the Service Control Manager (needs elevation).

use otx_protocol::{SERVICE_DISPLAY_NAME, SERVICE_NAME};
use std::ffi::OsString;
use std::time::{Duration, Instant};
use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceErrorControl, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

pub fn install() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)?;
    let info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe,
        launch_arguments: vec![OsString::from("service")],
        dependencies: vec![],
        account_name: None, // LocalSystem
        account_password: None,
    };
    let svc = scm.create_service(&info, ServiceAccess::CHANGE_CONFIG | ServiceAccess::START | ServiceAccess::QUERY_STATUS)?;
    svc.set_description("Applies and reverts OTX.GNG optimizations. Accepts only typed requests from the OTX.GNG app on this PC.")?;
    // Restart after a crash (the watchdog then reverts temporary changes on startup).
    svc.update_failure_actions(ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 3600)),
        reboot_msg: None,
        command: None,
        actions: Some(vec![
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(5) },
            ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(30) },
            ServiceAction { action_type: ServiceActionType::None, delay: Duration::default() },
        ]),
    })?;
    svc.start::<&str>(&[])?;
    println!("{SERVICE_DISPLAY_NAME} installed and started.");
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let svc = match scm.open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE) {
        Ok(s) => s,
        Err(_) => {
            println!("{SERVICE_DISPLAY_NAME} is not installed.");
            return Ok(());
        }
    };
    if svc.query_status()?.current_state != ServiceState::Stopped {
        let _ = svc.stop();
        wait_for(&svc, ServiceState::Stopped, Duration::from_secs(20));
    }
    svc.delete()?;
    println!("{SERVICE_DISPLAY_NAME} removed.");
    Ok(())
}

pub fn start() -> anyhow::Result<()> {
    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let svc = scm.open_service(SERVICE_NAME, ServiceAccess::START | ServiceAccess::QUERY_STATUS)?;
    if svc.query_status()?.current_state != ServiceState::Running {
        svc.start::<&str>(&[])?;
        wait_for(&svc, ServiceState::Running, Duration::from_secs(20));
    }
    Ok(())
}

pub fn stop() -> anyhow::Result<()> {
    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    let Ok(svc) = scm.open_service(SERVICE_NAME, ServiceAccess::STOP | ServiceAccess::QUERY_STATUS) else {
        return Ok(()); // not installed: nothing to stop (used by the installer before upgrades)
    };
    if svc.query_status()?.current_state != ServiceState::Stopped {
        svc.stop()?;
        wait_for(&svc, ServiceState::Stopped, Duration::from_secs(20));
    }
    Ok(())
}

pub fn status() -> anyhow::Result<String> {
    let scm = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    Ok(match scm.open_service(SERVICE_NAME, ServiceAccess::QUERY_STATUS) {
        Ok(s) => format!("{:?}", s.query_status()?.current_state),
        Err(_) => "NotInstalled".into(),
    })
}

fn wait_for(svc: &windows_service::service::Service, want: ServiceState, max: Duration) {
    let t = Instant::now();
    while t.elapsed() < max {
        if svc.query_status().map(|s| s.current_state == want).unwrap_or(false) {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}
