//! Process enumeration through NtQuerySystemInformation.
//!
//! One system call returns every process with its CPU times, memory and I/O counters, and
//! `SystemProcessIdInformation` returns image paths, so the monitor never opens a handle to
//! any process (including games protected by anti-cheat) just to display it.
//!
//! `SystemProcessInformation` is documented by Microsoft only partially (CPU times and I/O
//! counters sit in "Reserved" fields). We use the full layout that Task Manager-style tools
//! have relied on for 20+ years, and a unit test proves it agrees with Microsoft's documented
//! definition at every documented field.

use windows::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessInformation, SystemProcessorPerformanceInformation, SYSTEM_INFORMATION_CLASS,
};
use windows::Win32::Foundation::{NTSTATUS, UNICODE_STRING};
use windows::Win32::System::WindowsProgramming::SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION;

const STATUS_INFO_LENGTH_MISMATCH: NTSTATUS = NTSTATUS(0xC000_0004_u32 as i32);
/// SystemProcessIdInformation (image path by PID, no handle needed).
const SYSTEM_PROCESS_ID_INFORMATION_CLASS: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(88);

/// Full x64 layout of SYSTEM_PROCESS_INFORMATION.
#[repr(C)]
#[allow(non_snake_case)]
pub struct SpiFull {
    pub NextEntryOffset: u32,
    pub NumberOfThreads: u32,
    pub WorkingSetPrivateSize: i64,
    pub HardFaultCount: u32,
    pub NumberOfThreadsHighWatermark: u32,
    pub CycleTime: u64,
    pub CreateTime: i64,
    pub UserTime: i64,
    pub KernelTime: i64,
    pub ImageName: UNICODE_STRING,
    pub BasePriority: i32,
    pub UniqueProcessId: usize,
    pub InheritedFromUniqueProcessId: usize,
    pub HandleCount: u32,
    pub SessionId: u32,
    pub UniqueProcessKey: usize,
    pub PeakVirtualSize: usize,
    pub VirtualSize: usize,
    pub PageFaultCount: u32,
    pub PeakWorkingSetSize: usize,
    pub WorkingSetSize: usize,
    pub QuotaPeakPagedPoolUsage: usize,
    pub QuotaPagedPoolUsage: usize,
    pub QuotaPeakNonPagedPoolUsage: usize,
    pub QuotaNonPagedPoolUsage: usize,
    pub PagefileUsage: usize,
    pub PeakPagefileUsage: usize,
    pub PrivatePageCount: usize,
    pub ReadOperationCount: i64,
    pub WriteOperationCount: i64,
    pub OtherOperationCount: i64,
    pub ReadTransferCount: i64,
    pub WriteTransferCount: i64,
    pub OtherTransferCount: i64,
}

#[derive(Debug, Clone)]
pub struct RawProcess {
    pub pid: u32,
    pub parent_pid: u32,
    pub create_time: u64,
    pub name: String,
    pub cpu_time_100ns: u64,
    pub base_priority: i32,
    pub session_id: u32,
    pub private_ws: u64,
    pub working_set: u64,
    pub commit: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

/// Snapshot of all processes.
pub fn processes() -> windows::core::Result<Vec<RawProcess>> {
    let mut size: u32 = 512 * 1024;
    let buf = loop {
        let mut buf = vec![0u8; size as usize];
        let mut needed = 0u32;
        let st = unsafe { NtQuerySystemInformation(SystemProcessInformation, buf.as_mut_ptr().cast(), size, &mut needed) };
        if st == STATUS_INFO_LENGTH_MISMATCH {
            size = needed.max(size) + 64 * 1024; // processes can start between calls
            continue;
        }
        st.ok()?;
        break buf;
    };
    let mut out = Vec::with_capacity(400);
    let mut off = 0usize;
    loop {
        if off + std::mem::size_of::<SpiFull>() > buf.len() {
            break;
        }
        // SAFETY: the kernel filled `buf` with a chain of SYSTEM_PROCESS_INFORMATION records
        // (each followed by its thread array); NextEntryOffset links them and the name buffers
        // point inside `buf`.
        let p = unsafe { &*(buf.as_ptr().add(off) as *const SpiFull) };
        let name = if p.UniqueProcessId == 0 {
            "Idle".to_string()
        } else if p.ImageName.Buffer.is_null() || p.ImageName.Length == 0 {
            "System".to_string()
        } else {
            let s = unsafe { std::slice::from_raw_parts(p.ImageName.Buffer.0, (p.ImageName.Length / 2) as usize) };
            String::from_utf16_lossy(s)
        };
        out.push(RawProcess {
            pid: p.UniqueProcessId as u32,
            parent_pid: p.InheritedFromUniqueProcessId as u32,
            create_time: p.CreateTime.max(0) as u64,
            name,
            cpu_time_100ns: (p.UserTime + p.KernelTime).max(0) as u64,
            base_priority: p.BasePriority,
            session_id: p.SessionId,
            private_ws: p.WorkingSetPrivateSize.max(0) as u64,
            working_set: p.WorkingSetSize as u64,
            commit: p.PagefileUsage as u64,
            read_bytes: p.ReadTransferCount.max(0) as u64,
            write_bytes: p.WriteTransferCount.max(0) as u64,
        });
        if p.NextEntryOffset == 0 {
            break;
        }
        off += p.NextEntryOffset as usize;
    }
    Ok(out)
}

#[repr(C)]
struct SystemProcessIdInformation {
    process_id: usize,
    image_name: UNICODE_STRING,
}

/// NT image path (e.g. `\Device\HarddiskVolume3\Windows\explorer.exe`) without opening the process.
pub fn image_nt_path(pid: u32) -> Option<String> {
    let mut cap: u16 = 520;
    for _ in 0..3 {
        let mut buf = vec![0u16; cap as usize / 2];
        let mut info = SystemProcessIdInformation {
            process_id: pid as usize,
            image_name: UNICODE_STRING { Length: 0, MaximumLength: cap, Buffer: windows::core::PWSTR(buf.as_mut_ptr()) },
        };
        let st = unsafe {
            NtQuerySystemInformation(
                SYSTEM_PROCESS_ID_INFORMATION_CLASS,
                (&mut info as *mut SystemProcessIdInformation).cast(),
                std::mem::size_of::<SystemProcessIdInformation>() as u32,
                std::ptr::null_mut(),
            )
        };
        if st == STATUS_INFO_LENGTH_MISMATCH {
            cap = info.image_name.MaximumLength.max(cap.saturating_mul(2));
            continue;
        }
        if st.is_err() || info.image_name.Length == 0 {
            return None;
        }
        return Some(String::from_utf16_lossy(&buf[..(info.image_name.Length / 2) as usize]));
    }
    None
}

/// Per-logical-processor (idle, kernel incl. idle, user) times in 100 ns units.
pub fn processor_times() -> windows::core::Result<Vec<(u64, u64, u64)>> {
    let n = 256usize;
    let mut buf = vec![SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION::default(); n];
    let mut ret = 0u32;
    unsafe {
        NtQuerySystemInformation(
            SystemProcessorPerformanceInformation,
            buf.as_mut_ptr().cast(),
            (n * std::mem::size_of::<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION>()) as u32,
            &mut ret,
        )
        .ok()?;
    }
    let count = ret as usize / std::mem::size_of::<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION>();
    Ok(buf[..count].iter().map(|p| (p.IdleTime as u64, p.KernelTime as u64, p.UserTime as u64)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;
    use windows::Win32::System::WindowsProgramming::SYSTEM_PROCESS_INFORMATION as Doc;

    #[test]
    fn full_layout_matches_microsofts_documented_fields() {
        assert_eq!(offset_of!(SpiFull, ImageName), offset_of!(Doc, ImageName));
        assert_eq!(offset_of!(SpiFull, BasePriority), offset_of!(Doc, BasePriority));
        assert_eq!(offset_of!(SpiFull, UniqueProcessId), offset_of!(Doc, UniqueProcessId));
        assert_eq!(offset_of!(SpiFull, HandleCount), offset_of!(Doc, HandleCount));
        assert_eq!(offset_of!(SpiFull, SessionId), offset_of!(Doc, SessionId));
        assert_eq!(offset_of!(SpiFull, PeakVirtualSize), offset_of!(Doc, PeakVirtualSize));
        assert_eq!(offset_of!(SpiFull, VirtualSize), offset_of!(Doc, VirtualSize));
        assert_eq!(offset_of!(SpiFull, PeakWorkingSetSize), offset_of!(Doc, PeakWorkingSetSize));
        assert_eq!(offset_of!(SpiFull, WorkingSetSize), offset_of!(Doc, WorkingSetSize));
        assert_eq!(offset_of!(SpiFull, QuotaPagedPoolUsage), offset_of!(Doc, QuotaPagedPoolUsage));
        assert_eq!(offset_of!(SpiFull, QuotaNonPagedPoolUsage), offset_of!(Doc, QuotaNonPagedPoolUsage));
        assert_eq!(offset_of!(SpiFull, PagefileUsage), offset_of!(Doc, PagefileUsage));
        assert_eq!(offset_of!(SpiFull, PrivatePageCount), offset_of!(Doc, PrivatePageCount));
        assert_eq!(offset_of!(SpiFull, ReadOperationCount), offset_of!(Doc, Reserved7));
        assert_eq!(std::mem::size_of::<SpiFull>(), std::mem::size_of::<Doc>());
    }

    #[test]
    fn enumerates_this_process_with_plausible_values() {
        let me = std::process::id();
        let all = processes().unwrap();
        assert!(all.len() > 20);
        let p = all.iter().find(|p| p.pid == me).expect("own process listed");
        assert!(p.name.to_ascii_lowercase().ends_with(".exe"));
        assert!(p.create_time > 130_000_000_000_000_000, "FILETIME after 2012");
        assert!(p.private_ws > 0 && p.working_set >= p.private_ws);
        assert!(processor_times().unwrap().len() >= 1);
    }

    #[test]
    fn image_path_without_a_handle() {
        let path = image_nt_path(std::process::id()).expect("own path");
        assert!(path.starts_with(r"\Device\"), "{path}");
        assert!(path.to_ascii_lowercase().ends_with(".exe"));
    }
}
