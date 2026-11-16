//! Process actions: priority, efficiency mode (EcoQoS) and end task.
//!
//! Safety rules enforced HERE (the UI's checks are only cosmetic):
//! * the target is identified by PID AND creation time; a recycled PID is refused;
//! * protected processes (core Windows, security, audio, GPU drivers, anti-cheat, the Core) are refused;
//! * session-0 processes (Windows services) are refused in Phase 3;
//! * Realtime priority cannot be expressed in the protocol;
//! * every reversible change is journaled BEFORE it is applied and verified afterwards.

use crate::journal::{Entry, RevertOp};
use otx_protocol::{ErrorCode, PriorityLevel, ProcessRef};
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetExitCodeProcess, GetPriorityClass, GetProcessInformation, GetProcessTimes, OpenProcess, ProcessPowerThrottling, QueryFullProcessImageNameW,
    SetPriorityClass, SetProcessInformation, TerminateProcess, ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS,
    HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, PROCESS_ACCESS_RIGHTS, PROCESS_CREATION_FLAGS,
    PROCESS_NAME_WIN32, PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_STATE, PROCESS_QUERY_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
    PROCESS_TERMINATE,
};

#[derive(Debug)]
pub struct ActionError {
    pub code: ErrorCode,
    pub message: String,
}

impl ActionError {
    fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

pub struct Target {
    handle: HANDLE,
    pub exe: String,
}

impl Target {
    pub fn handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn filetime_u64(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

/// Opens the target with exactly the rights needed, after every safety check.
pub fn open(target: ProcessRef, rights: PROCESS_ACCESS_RIGHTS) -> Result<Target, ActionError> {
    if target.pid <= 4 {
        return Err(ActionError::new(ErrorCode::ProtectedProcess, "System processes are never changed."));
    }
    let mut session = 0u32;
    if unsafe { ProcessIdToSessionId(target.pid, &mut session) }.is_err() {
        return Err(ActionError::new(ErrorCode::Validation, "That process is no longer running."));
    }
    if session == 0 {
        return Err(ActionError::new(ErrorCode::ProtectedProcess, "Windows services are not changed from the Usage Monitor."));
    }
    let handle = unsafe { OpenProcess(rights | PROCESS_QUERY_LIMITED_INFORMATION, false, target.pid) }.map_err(|e| {
        ActionError::new(ErrorCode::ProtectedProcess, format!("Windows did not allow access to this process ({e})."))
    })?;
    // Owns the handle from here on; every early return closes it exactly once.
    let mut t = Target { handle, exe: String::new() };

    // A process that has exited can still be openable while another program holds a handle
    // to it. Treat it as gone.
    const STILL_ACTIVE: u32 = 259;
    let mut code = 0u32;
    if unsafe { GetExitCodeProcess(t.handle, &mut code) }.is_err() || code != STILL_ACTIVE {
        return Err(ActionError::new(ErrorCode::Validation, "That process is no longer running."));
    }

    let (mut c, mut x, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    unsafe { GetProcessTimes(t.handle, &mut c, &mut x, &mut k, &mut u) }
        .map_err(|e| ActionError::new(ErrorCode::Internal, e.to_string()))?;
    if filetime_u64(c) != target.create_time {
        return Err(ActionError::new(ErrorCode::Validation, "That process has exited and its ID was reused. Nothing was changed."));
    }

    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    unsafe { QueryFullProcessImageNameW(t.handle, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) }
        .map_err(|e| ActionError::new(ErrorCode::Internal, e.to_string()))?;
    let path = String::from_utf16_lossy(&buf[..len as usize]);
    let exe = path.rsplit('\\').next().unwrap_or(&path).to_string();
    if otx_monitor::policy::is_protected(&exe) {
        return Err(ActionError::new(ErrorCode::ProtectedProcess, format!("{exe} is protected and is never changed.")));
    }
    // Update in place: `Target { exe, ..t }` would copy the handle and then close it when `t` drops.
    t.exe = exe;
    Ok(t)
}

pub fn class_of(level: PriorityLevel) -> PROCESS_CREATION_FLAGS {
    match level {
        PriorityLevel::Idle => IDLE_PRIORITY_CLASS,
        PriorityLevel::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
        PriorityLevel::Normal => NORMAL_PRIORITY_CLASS,
        PriorityLevel::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        PriorityLevel::High => HIGH_PRIORITY_CLASS,
    }
}

pub fn level_of(class: u32) -> Option<PriorityLevel> {
    [PriorityLevel::Idle, PriorityLevel::BelowNormal, PriorityLevel::Normal, PriorityLevel::AboveNormal, PriorityLevel::High]
        .into_iter()
        .find(|l| class_of(*l).0 == class)
}

pub fn current_priority(t: &Target) -> Option<PriorityLevel> {
    level_of(unsafe { GetPriorityClass(t.handle) })
}

pub fn set_priority(t: &Target, level: PriorityLevel) -> Result<(), ActionError> {
    unsafe { SetPriorityClass(t.handle, class_of(level)) }.map_err(|e| ActionError::new(ErrorCode::Internal, e.to_string()))?;
    // Verify it took effect.
    if current_priority(t) != Some(level) {
        return Err(ActionError::new(ErrorCode::Internal, "Windows did not apply the new priority."));
    }
    Ok(())
}

/// (control mask, state mask) of the process's power throttling, if readable.
pub fn throttling(t: &Target) -> Option<(u32, u32)> {
    let mut s = PROCESS_POWER_THROTTLING_STATE { Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION, ..Default::default() };
    unsafe {
        GetProcessInformation(t.handle, ProcessPowerThrottling, (&mut s as *mut PROCESS_POWER_THROTTLING_STATE).cast(), std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32)
    }
    .ok()?;
    Some((s.ControlMask, s.StateMask))
}

pub fn set_throttling(t: &Target, control: u32, state: u32) -> Result<(), ActionError> {
    let s = PROCESS_POWER_THROTTLING_STATE { Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION, ControlMask: control, StateMask: state };
    unsafe {
        SetProcessInformation(t.handle, ProcessPowerThrottling, (&s as *const PROCESS_POWER_THROTTLING_STATE).cast(), std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32)
    }
    .map_err(|e| ActionError::new(ErrorCode::Internal, e.to_string()))
}

pub fn efficiency_masks(on: bool) -> (u32, u32) {
    // On: we control execution speed and ask for throttling (EcoQoS). Off: hand control back to Windows.
    if on {
        (PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_EXECUTION_SPEED)
    } else {
        (0, 0)
    }
}

pub fn terminate(t: &Target) -> Result<(), ActionError> {
    unsafe { TerminateProcess(t.handle, 1) }.map_err(|e| ActionError::new(ErrorCode::Internal, e.to_string()))
}

pub const RIGHTS_SET: PROCESS_ACCESS_RIGHTS = PROCESS_ACCESS_RIGHTS(PROCESS_SET_INFORMATION.0 | PROCESS_QUERY_INFORMATION.0);
pub const RIGHTS_END: PROCESS_ACCESS_RIGHTS = PROCESS_TERMINATE;

pub fn now_unix() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn priority_entry(id: String, target: ProcessRef, exe: &str, previous: PriorityLevel, new: PriorityLevel) -> Entry {
    Entry {
        id,
        created_unix: now_unix(),
        title: format!("{exe}: priority {}", otx_monitor::priority_class_name(new)),
        detail: format!("was {} (PID {})", otx_monitor::priority_class_name(previous), target.pid),
        temporary: true,
        revert: RevertOp::ProcessPriority { target, level: previous },
    }
}

pub fn efficiency_entry(id: String, target: ProcessRef, exe: &str, on: bool, previous: (u32, u32)) -> Entry {
    Entry {
        id,
        created_unix: now_unix(),
        title: format!("{exe}: efficiency mode {}", if on { "on" } else { "off" }),
        detail: format!("PID {}", target.pid),
        temporary: true,
        revert: RevertOp::ProcessEfficiency { target, control: previous.0, state: previous.1 },
    }
}

/// Revert helpers used by the journal. A process that has exited counts as reverted.
pub fn revert_priority(target: ProcessRef, level: PriorityLevel) -> Result<(), String> {
    match open(target, RIGHTS_SET) {
        Ok(t) => set_priority(&t, level).map_err(|e| e.message),
        Err(e) if e.code == ErrorCode::Validation => Ok(()), // gone or PID reused: nothing to undo
        Err(e) => Err(e.message),
    }
}

pub fn revert_efficiency(target: ProcessRef, control: u32, state: u32) -> Result<(), String> {
    match open(target, RIGHTS_SET) {
        Ok(t) => set_throttling(&t, control, state).map_err(|e| e.message),
        Err(e) if e.code == ErrorCode::Validation => Ok(()),
        Err(e) => Err(e.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    fn create_time_of(pid: u32) -> u64 {
        let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.unwrap();
        let (mut c, mut x, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        unsafe { GetProcessTimes(h, &mut c, &mut x, &mut k, &mut u) }.unwrap();
        unsafe {
            let _ = CloseHandle(h);
        }
        filetime_u64(c)
    }

    /// A throwaway child process (ping, ~30 s), so the tests never touch anything else.
    fn spawn_dummy() -> std::process::Child {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("ping")
            .args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .unwrap()
    }

    #[test]
    fn priority_efficiency_and_end_on_a_throwaway_process() {
        let mut child = spawn_dummy();
        let r = ProcessRef { pid: child.id(), create_time: create_time_of(child.id()) };

        let t = open(r, RIGHTS_SET).unwrap();
        assert_eq!(t.exe.to_ascii_lowercase(), "ping.exe");
        let before = current_priority(&t).unwrap();
        set_priority(&t, PriorityLevel::BelowNormal).unwrap();
        assert_eq!(current_priority(&t), Some(PriorityLevel::BelowNormal));
        revert_priority(r, before).unwrap();
        assert_eq!(current_priority(&t), Some(before));

        let prev = throttling(&t).unwrap_or((0, 0));
        let (c, s) = efficiency_masks(true);
        set_throttling(&t, c, s).unwrap();
        assert_eq!(throttling(&t), Some((c, s)));
        revert_efficiency(r, prev.0, prev.1).unwrap();
        drop(t);

        // Wrong creation time = recycled PID: refused.
        let stale = ProcessRef { pid: r.pid, create_time: r.create_time - 1 };
        assert_eq!(open(stale, RIGHTS_SET).err().unwrap().code, ErrorCode::Validation);

        let t = open(r, RIGHTS_END).unwrap();
        terminate(&t).unwrap();
        let status = child.wait().unwrap();
        assert!(!status.success());
        // Reverting a change on an exited process is a no-op success.
        assert!(revert_priority(r, PriorityLevel::Normal).is_ok());
    }

    #[test]
    fn protected_and_system_processes_are_refused() {
        assert_eq!(open(ProcessRef { pid: 4, create_time: 0 }, RIGHTS_SET).err().unwrap().code, ErrorCode::ProtectedProcess);
        // Find a session-0 service process (lsass/services) via the monitor and try it.
        let procs = otx_monitor::nt::processes().unwrap();
        let svc = procs.iter().find(|p| p.name.eq_ignore_ascii_case("services.exe")).expect("services.exe exists");
        let r = ProcessRef { pid: svc.pid, create_time: svc.create_time };
        assert_eq!(open(r, RIGHTS_SET).err().unwrap().code, ErrorCode::ProtectedProcess);
    }

    #[test]
    fn priority_class_mapping_round_trips_and_has_no_realtime() {
        for l in [PriorityLevel::Idle, PriorityLevel::BelowNormal, PriorityLevel::Normal, PriorityLevel::AboveNormal, PriorityLevel::High] {
            assert_eq!(level_of(class_of(l).0), Some(l));
        }
        assert_eq!(level_of(0x100), None); // REALTIME_PRIORITY_CLASS is not representable
    }
}
