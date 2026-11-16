//! RAM Cleaner (Mem Reduct-style, safer defaults).
//!
//! Never touched: protected processes (system, security, audio, GPU drivers, anti-cheat, the
//! Core), Windows services (session 0), the running game (`skip`) and allow-listed apps (`allow`).
//! Memory priority is journaled as temporary and restored; working-set trimming is not
//! reversible but harmless (pages fault back in when needed). Standby purge and modified-list
//! flush use `NtSetSystemInformation(SystemMemoryListInformation)`, the same undocumented-but-stable
//! call RAMMap and Mem Reduct use; it needs SeProfileSingleProcessPrivilege.

use crate::journal::{Entry, RevertOp};
use otx_protocol::{ProcessRef, RamCleanReport, RamMethod};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, LUID};
use windows::Win32::Security::{AdjustTokenPrivileges, LookupPrivilegeValueW, LUID_AND_ATTRIBUTES, SE_PRIVILEGE_ENABLED, TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY};
use windows::Win32::System::Memory::SetSystemFileCacheSize;
use windows::Win32::System::ProcessStatus::EmptyWorkingSet;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessInformation, OpenProcessToken, ProcessMemoryPriority, SetProcessInformation, MEMORY_PRIORITY,
    MEMORY_PRIORITY_INFORMATION, MEMORY_PRIORITY_LOW, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SET_QUOTA,
};

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSetSystemInformation(class: i32, info: *mut core::ffi::c_void, len: u32) -> i32;
}
const SYSTEM_MEMORY_LIST_INFORMATION: i32 = 80;
const MEMORY_FLUSH_MODIFIED_LIST: u32 = 3;
const MEMORY_PURGE_STANDBY_LIST: u32 = 4;

pub fn available_mb() -> u64 {
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    let _ = unsafe { GlobalMemoryStatusEx(&mut m) };
    m.ullAvailPhys / 1_048_576
}

fn enable_privilege(name: &str) -> Result<(), String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token).map_err(|e| e.to_string())?;
        let mut luid = LUID::default();
        let r = LookupPrivilegeValueW(PCWSTR::null(), &HSTRING::from(name), &mut luid);
        let tp = TOKEN_PRIVILEGES { PrivilegeCount: 1, Privileges: [LUID_AND_ATTRIBUTES { Luid: luid, Attributes: SE_PRIVILEGE_ENABLED }] };
        let r2 = r.and_then(|_| AdjustTokenPrivileges(token, false, Some(&tp), 0, None, None));
        let last = windows::Win32::Foundation::GetLastError();
        let _ = CloseHandle(token);
        r2.map_err(|e| e.to_string())?;
        // AdjustTokenPrivileges "succeeds" with ERROR_NOT_ALL_ASSIGNED when the privilege is absent.
        if last.0 == 1300 {
            return Err(format!("{name} is not held by this account (run the Core as a service)"));
        }
        Ok(())
    }
}

fn memory_list_command(cmd: u32) -> Result<(), String> {
    enable_privilege("SeProfileSingleProcessPrivilege")?;
    let mut c = cmd;
    let st = unsafe { NtSetSystemInformation(SYSTEM_MEMORY_LIST_INFORMATION, (&mut c as *mut u32).cast(), 4) };
    if st >= 0 {
        Ok(())
    } else {
        Err(format!("Windows refused the memory command (0x{st:08X})"))
    }
}

pub fn get_memory_priority(h: HANDLE) -> Option<u32> {
    let mut i = MEMORY_PRIORITY_INFORMATION::default();
    unsafe { GetProcessInformation(h, ProcessMemoryPriority, (&mut i as *mut MEMORY_PRIORITY_INFORMATION).cast(), std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32) }.ok()?;
    Some(i.MemoryPriority.0)
}

pub fn set_memory_priority(h: HANDLE, p: u32) -> Result<(), String> {
    let i = MEMORY_PRIORITY_INFORMATION { MemoryPriority: MEMORY_PRIORITY(p) };
    unsafe { SetProcessInformation(h, ProcessMemoryPriority, (&i as *const MEMORY_PRIORITY_INFORMATION).cast(), std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32) }
        .map_err(|e| e.to_string())
}

/// Revert for one journaled memory-priority batch. Exited processes are skipped.
pub fn restore_memory_priority(targets: &[(ProcessRef, u32)]) -> Result<(), String> {
    for (t, prio) in targets {
        if let Ok(p) = crate::process::open(*t, PROCESS_SET_INFORMATION) {
            let _ = set_memory_priority(p.handle(), *prio);
        }
    }
    Ok(())
}

/// Processes the cleaner may touch: user-session, not protected, not the game, not allow-listed.
pub fn candidates(allow: &[String], skip: &[String]) -> Vec<(ProcessRef, String)> {
    let me = std::process::id();
    let lower = |v: &[String]| v.iter().map(|s| s.to_ascii_lowercase()).collect::<Vec<_>>();
    let (allow, skip) = (lower(allow), lower(skip));
    otx_monitor::nt::processes()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.pid > 4 && p.pid != me && p.session_id != 0)
        .filter(|p| {
            let n = p.name.to_ascii_lowercase();
            !otx_monitor::policy::is_protected(&n)
                && !allow.contains(&n)
                && !skip.contains(&n)
                && otx_monitor::policy::category_of(&n, p.session_id, None) != otx_protocol::ProcessCategory::Game
                && n != "otx-gng.exe"
                && n != "otx.gng.exe"
        })
        .map(|p| (ProcessRef { pid: p.pid, create_time: p.create_time }, p.name))
        .collect()
}

pub struct Outcome {
    pub report: RamCleanReport,
    pub journal: Option<Entry>,
}

pub fn clean(methods: &[RamMethod], allow: &[String], skip: &[String], dry_run: bool) -> Outcome {
    let before = available_mb();
    let mut done = Vec::new();
    let mut skipped = vec!["Protected system, security, audio, GPU-driver and anti-cheat processes".to_string(), "Windows services".to_string()];
    if !allow.is_empty() {
        skipped.push(format!("Allow-listed: {}", allow.join(", ")));
    }
    if !skip.is_empty() {
        skipped.push(format!("Game: {}", skip.join(", ")));
    }
    let procs = candidates(allow, skip);
    let mut journal = None;

    if methods.contains(&RamMethod::MemoryPriority) {
        if dry_run {
            done.push(format!("Would lower memory priority of {} background processes", procs.len()));
        } else {
            let mut changed = Vec::new();
            for (r, _) in &procs {
                if let Ok(t) = crate::process::open(*r, PROCESS_SET_INFORMATION) {
                    if let Some(prev) = get_memory_priority(t.handle()) {
                        if prev > MEMORY_PRIORITY_LOW.0 && set_memory_priority(t.handle(), MEMORY_PRIORITY_LOW.0).is_ok() {
                            changed.push((*r, prev));
                        }
                    }
                }
            }
            done.push(format!("Lowered memory priority of {} background processes (restored automatically)", changed.len()));
            if !changed.is_empty() {
                journal = Some(Entry {
                    id: crate::server::random_hex(8),
                    created_unix: crate::process::now_unix(),
                    title: "Background apps: low memory priority".into(),
                    detail: format!("{} processes", changed.len()),
                    temporary: true,
                    revert: RevertOp::MemoryPriority { targets: changed },
                });
            }
        }
    }
    if methods.contains(&RamMethod::TrimWorkingSets) {
        if dry_run {
            done.push(format!("Would trim working sets of {} background processes", procs.len()));
        } else {
            let mut n = 0;
            for (r, _) in &procs {
                if let Ok(t) = crate::process::open(*r, PROCESS_SET_QUOTA) {
                    if unsafe { EmptyWorkingSet(t.handle()) }.is_ok() {
                        n += 1;
                    }
                }
            }
            done.push(format!("Trimmed working sets of {n} background processes"));
        }
    }
    let mut system = |m: RamMethod, label: &str, f: &dyn Fn() -> Result<(), String>| {
        if methods.contains(&m) {
            if dry_run {
                done.push(format!("Would {label}"));
            } else {
                match f() {
                    Ok(()) => done.push(capitalize(label)),
                    Err(e) => skipped.push(format!("{}: {e}", capitalize(label))),
                }
            }
        }
    };
    system(RamMethod::FlushModified, "flush the modified page list", &|| memory_list_command(MEMORY_FLUSH_MODIFIED_LIST));
    system(RamMethod::PurgeStandby, "purge the standby list", &|| memory_list_command(MEMORY_PURGE_STANDBY_LIST));
    system(RamMethod::TrimFileCache, "trim the system file cache", &|| {
        enable_privilege("SeIncreaseQuotaPrivilege")?;
        unsafe { SetSystemFileCacheSize(usize::MAX, usize::MAX, 0) }.map_err(|e| e.to_string())
    });

    if !dry_run {
        std::thread::sleep(std::time::Duration::from_millis(400)); // let Windows settle before measuring
    }
    let after = available_mb();
    Outcome {
        report: RamCleanReport { dry_run, available_before_mb: before, available_after_mb: after, freed_mb: after as i64 - before as i64, done, skipped },
        journal,
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Kept for tests: open a process for memory-priority work.
pub fn open_for_priority(r: ProcessRef) -> Option<crate::process::Target> {
    crate::process::open(r, windows::Win32::System::Threading::PROCESS_ACCESS_RIGHTS(PROCESS_SET_INFORMATION.0 | PROCESS_QUERY_LIMITED_INFORMATION.0)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    #[test]
    fn dry_run_reports_without_changing_anything() {
        let all = [RamMethod::MemoryPriority, RamMethod::TrimWorkingSets, RamMethod::PurgeStandby, RamMethod::FlushModified, RamMethod::TrimFileCache];
        let o = clean(&all, &["discord.exe".into()], &["cs2.exe".into()], true);
        assert!(o.report.dry_run);
        assert!(o.journal.is_none());
        assert_eq!(o.report.done.len(), 5);
        assert!(o.report.done.iter().all(|d| d.starts_with("Would")));
    }

    #[test]
    fn candidates_never_include_protected_allowed_or_skipped() {
        let c = candidates(&["chrome.exe".into()], &["msedgewebview2.exe".into()]);
        for (_, n) in &c {
            let n = n.to_ascii_lowercase();
            assert!(!otx_monitor::policy::is_protected(&n), "{n}");
            assert!(n != "chrome.exe" && n != "msedgewebview2.exe" && n != "svchost.exe", "{n}");
        }
    }

    #[test]
    fn memory_priority_and_trim_on_a_throwaway_process() {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut child = std::process::Command::new("ping").args(["-n", "20", "127.0.0.1"]).stdout(std::process::Stdio::null()).creation_flags(CREATE_NO_WINDOW).spawn().unwrap();
        let p = otx_monitor::nt::processes().unwrap().into_iter().find(|p| p.pid == child.id()).unwrap();
        let r = ProcessRef { pid: p.pid, create_time: p.create_time };
        let t = open_for_priority(r).unwrap();
        let before = get_memory_priority(t.handle()).unwrap();
        set_memory_priority(t.handle(), MEMORY_PRIORITY_LOW.0).unwrap();
        assert_eq!(get_memory_priority(t.handle()), Some(MEMORY_PRIORITY_LOW.0));
        restore_memory_priority(&[(r, before)]).unwrap();
        assert_eq!(get_memory_priority(t.handle()), Some(before));
        let q = crate::process::open(r, PROCESS_SET_QUOTA).unwrap();
        assert!(unsafe { EmptyWorkingSet(q.handle()) }.is_ok());
        child.kill().unwrap();
        let _ = child.wait();
    }
}
