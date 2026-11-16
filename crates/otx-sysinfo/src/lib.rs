//! Read-only hardware and OS detection.
//!
//! Every function here works without elevation, so the same code runs inside the Core service
//! (LocalSystem) and inside the unprivileged UI as a fallback when the Core is not installed.
//! Nothing is guessed: a fact that cannot be read is reported in `SystemInfo::unavailable`.

pub mod logic;

use logic::{AdapterCandidate, CoreEntry};
use otx_protocol::{
    AntiCheatInfo, CpuInfo, DiskInfo, DiskKind, FormFactor, GpuInfo, NetworkInfo, OsInfo, PowerInfo, PowerPlan,
    SystemInfo,
};
use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree, ERROR_BUFFER_OVERFLOW, ERROR_SUCCESS};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
use windows::Win32::NetworkManagement::IpHelper::{GetAdaptersAddresses, GAA_FLAG_INCLUDE_GATEWAYS, IP_ADAPTER_ADDRESSES_LH};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::AF_UNSPEC;
use windows::Win32::Storage::FileSystem::{
    BusTypeNvme, CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
    IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, StorageDeviceSeekPenaltyProperty, DEVICE_SEEK_PENALTY_DESCRIPTOR,
    IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_PROPERTY_QUERY, VOLUME_DISK_EXTENTS,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Power::{GetSystemPowerStatus, PowerGetActiveScheme, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{
    GetLogicalProcessorInformationEx, GetWindowsDirectoryW, GlobalMemoryStatusEx, RelationProcessorCore,
    MEMORYSTATUSEX, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

/// Collects everything. Each part fails independently.
pub fn collect() -> SystemInfo {
    let mut unavailable = Vec::new();
    let mut note = |what: &str, e: String| {
        tracing::warn!(part = what, error = %e, "system info part unavailable");
        unavailable.push(format!("{what}: {e}"));
    };

    let os = os_info().unwrap_or_else(|e| {
        note("Windows version", e);
        OsInfo { name: "Windows".into(), build: 0, ubr: 0, display_version: None, edition: None, supported: false }
    });
    let cpu = cpu_info().unwrap_or_else(|e| {
        note("CPU", e);
        CpuInfo { model: "Unknown CPU".into(), cores: 0, threads: 0, hybrid: false, performance_cores: None, efficiency_cores: None }
    });
    let gpus = gpu_info().unwrap_or_else(|e| {
        note("GPU", e);
        Vec::new()
    });
    let ram_mb = ram_total_mb().unwrap_or_else(|e| {
        note("Memory", e);
        0
    });
    let disk = disk_info().map_err(|e| note("System disk", e)).ok();
    let power = power_info().unwrap_or_else(|e| {
        note("Power", e);
        PowerInfo { form_factor: FormFactor::Desktop, on_battery: false, battery_saver: false, battery_percent: None, plan: PowerPlan::Unknown }
    });
    let network = match network_info() {
        Ok(n) => n,
        Err(e) => {
            note("Network adapter", e);
            None
        }
    };
    let startup_apps_machine = count_run_values(HKEY_LOCAL_MACHINE);
    let anti_cheats = anti_cheat_info();

    SystemInfo { os, cpu, gpus, ram_mb, disk, power, network, startup_apps_machine, anti_cheats, unavailable }
}

/// Per-user startup entries (HKCU Run). Call from the user's own process, never from the service.
pub fn startup_apps_current_user() -> u32 {
    count_run_values(HKEY_CURRENT_USER)
}

/// Counts enabled startup entries the way Task Manager does: a Run value is skipped when its
/// StartupApproved flag marks it disabled.
fn count_run_values(root: winreg::HKEY) -> u32 {
    const APPROVED: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved";
    let pairs = [
        (r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run", "Run"),
        (r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run", "Run32"),
    ];
    let key = RegKey::predef(root);
    pairs
        .iter()
        .filter_map(|(run, approved)| {
            let run = key.open_subkey_with_flags(run, KEY_READ).ok()?;
            let flags = key.open_subkey_with_flags(format!(r"{APPROVED}\{approved}"), KEY_READ).ok();
            let enabled = run
                .enum_values()
                .filter_map(Result::ok)
                .filter(|(name, _)| {
                    let flag = flags.as_ref().and_then(|f| f.get_raw_value(name).ok()).map(|v| v.bytes.to_vec());
                    !logic::startup_disabled(flag.as_deref())
                })
                .count() as u32;
            Some(enabled)
        })
        .sum()
}

pub fn os_info() -> Result<OsInfo, String> {
    let k = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion", KEY_READ)
        .map_err(|e| e.to_string())?;
    let build: u32 = k
        .get_value::<String, _>("CurrentBuildNumber")
        .map_err(|e| e.to_string())?
        .parse()
        .map_err(|_| "CurrentBuildNumber is not a number".to_string())?;
    let ubr: u32 = k.get_value("UBR").unwrap_or(0);
    Ok(OsInfo {
        name: logic::os_name(build).to_string(),
        build,
        ubr,
        display_version: k.get_value("DisplayVersion").ok(),
        edition: k.get_value("EditionID").ok(),
        supported: logic::os_supported(build),
    })
}

pub fn cpu_info() -> Result<CpuInfo, String> {
    let model: String = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(r"HARDWARE\DESCRIPTION\System\CentralProcessor\0", KEY_READ)
        .and_then(|k| k.get_value("ProcessorNameString"))
        .map(|s: String| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown CPU".into());

    // First call gets the required size, second fills the buffer.
    let mut len = 0u32;
    unsafe {
        let _ = GetLogicalProcessorInformationEx(RelationProcessorCore, None, &mut len);
    }
    if len == 0 {
        return Err("GetLogicalProcessorInformationEx returned no data".into());
    }
    let mut buf = vec![0u8; len as usize];
    unsafe {
        GetLogicalProcessorInformationEx(
            RelationProcessorCore,
            Some(buf.as_mut_ptr() as *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX),
            &mut len,
        )
        .map_err(|e| e.to_string())?;
    }
    // Records are variable-size: each is only as long as its own `Size`, which is SMALLER than
    // size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX>() (that type contains a union of every
    // relationship). Bounds-check against `Size`, never against the full struct size, or the
    // last core is silently dropped.
    let len = len as usize;
    let mut cores = Vec::new();
    let mut off = 0usize;
    while off + 8 <= len {
        // Header: Relationship (u32) + Size (u32).
        let size = u32::from_ne_bytes(buf[off + 4..off + 8].try_into().expect("4 bytes")) as usize;
        if size < 8 || off + size > len {
            break;
        }
        // SAFETY: `off + size <= len` and the OS wrote a complete record of `size` bytes here;
        // we only read fields of PROCESSOR_RELATIONSHIP, which fit inside a core record.
        let rec = unsafe { &*(buf.as_ptr().add(off) as *const SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX) };
        if rec.Relationship == RelationProcessorCore {
            let p = unsafe { &rec.Anonymous.Processor };
            let groups = p.GroupCount.max(1) as usize;
            let masks = unsafe { std::slice::from_raw_parts(p.GroupMask.as_ptr(), groups) };
            let threads = masks.iter().map(|m| m.Mask.count_ones()).sum::<u32>().max(1);
            cores.push(CoreEntry { efficiency_class: p.EfficiencyClass, threads });
        }
        off += size;
    }
    if cores.is_empty() {
        return Err("no processor cores reported".into());
    }
    Ok(logic::summarize_cores(model, &cores))
}

pub fn gpu_info() -> Result<Vec<GpuInfo>, String> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(|e| e.to_string())?;
    let mut out: Vec<GpuInfo> = Vec::new();
    for i in 0.. {
        let adapter = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(_) => break, // DXGI_ERROR_NOT_FOUND ends the list
        };
        let desc = unsafe { adapter.GetDesc1() }.map_err(|e| e.to_string())?;
        if desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0 {
            continue; // Microsoft Basic Render Driver
        }
        let name = logic::utf16_until_nul(&desc.Description);
        if out.iter().any(|g| g.name == name) {
            continue; // the same adapter can appear once per output configuration
        }
        let vendor = logic::gpu_vendor(desc.VendorId);
        let vram = (desc.DedicatedVideoMemory as u64) / (1024 * 1024);
        out.push(GpuInfo { name, vendor, dedicated_vram_mb: vram, integrated: logic::gpu_is_integrated(vendor, vram) });
        let _ = adapter.as_raw(); // keep the Interface import meaningful across windows versions
    }
    // Discrete first: that is the GPU games should use.
    out.sort_by_key(|g| g.integrated);
    Ok(out)
}

pub fn ram_total_mb() -> Result<u64, String> {
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    unsafe { GlobalMemoryStatusEx(&mut m) }.map_err(|e| e.to_string())?;
    Ok(m.ullTotalPhys / (1024 * 1024))
}

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn open_device(path: &str) -> Result<OwnedHandle, String> {
    let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
    // Access 0 = query only. Works without elevation for the IOCTLs used here (FILE_ANY_ACCESS).
    let h = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )
    }
    .map_err(|e| format!("open {path}: {e}"))?;
    Ok(OwnedHandle(h))
}

fn ioctl<I, O: Default>(h: &OwnedHandle, code: u32, input: Option<&I>, out: &mut O, out_size: usize) -> Result<(), String> {
    let mut returned = 0u32;
    unsafe {
        DeviceIoControl(
            h.0,
            code,
            input.map(|i| i as *const I as *const core::ffi::c_void),
            input.map(|_| std::mem::size_of::<I>() as u32).unwrap_or(0),
            Some(out as *mut O as *mut core::ffi::c_void),
            out_size as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|e| e.to_string())
}

pub fn disk_info() -> Result<DiskInfo, String> {
    let mut win = [0u16; 260];
    let n = unsafe { GetWindowsDirectoryW(Some(&mut win)) } as usize;
    if n < 2 {
        return Err("could not read the Windows directory".into());
    }
    let drive = String::from_utf16_lossy(&win[..2]); // "C:"

    let vol = open_device(&format!(r"\\.\{drive}"))?;
    let mut ext = VOLUME_DISK_EXTENTS::default();
    ioctl::<(), _>(&vol, IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, None, &mut ext, std::mem::size_of::<VOLUME_DISK_EXTENTS>())
        .map_err(|e| format!("volume extents: {e}"))?;
    if ext.NumberOfDiskExtents == 0 {
        return Err("system volume has no disk extents".into());
    }
    let disk_no = ext.Extents[0].DiskNumber;
    let disk = open_device(&format!(r"\\.\PhysicalDrive{disk_no}"))?;

    // Model and bus type. The descriptor is followed by its strings, so use a larger buffer.
    let q = STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceProperty, QueryType: PropertyStandardQuery, ..Default::default() };
    let mut raw = [0u8; 1024];
    let mut returned = 0u32;
    unsafe {
        DeviceIoControl(
            disk.0,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&q as *const _ as *const core::ffi::c_void),
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            Some(raw.as_mut_ptr() as *mut core::ffi::c_void),
            raw.len() as u32,
            Some(&mut returned),
            None,
        )
    }
    .map_err(|e| format!("device descriptor: {e}"))?;
    // SAFETY: the buffer is larger than the fixed header and was filled by the driver.
    let dd = unsafe { std::ptr::read_unaligned(raw.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
    let read_str = |off: u32| -> String {
        let off = off as usize;
        if off == 0 || off >= raw.len() {
            return String::new();
        }
        let end = raw[off..].iter().position(|&b| b == 0).map(|p| off + p).unwrap_or(raw.len());
        String::from_utf8_lossy(&raw[off..end]).trim().to_string()
    };
    let model = {
        let v = read_str(dd.VendorIdOffset);
        let p = read_str(dd.ProductIdOffset);
        if v.is_empty() || p.starts_with(&v) {
            p
        } else {
            format!("{v} {p}")
        }
    };

    let kind = if dd.BusType == BusTypeNvme {
        DiskKind::Nvme
    } else {
        let q = STORAGE_PROPERTY_QUERY { PropertyId: StorageDeviceSeekPenaltyProperty, QueryType: PropertyStandardQuery, ..Default::default() };
        let mut sp = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
        match ioctl(&disk, IOCTL_STORAGE_QUERY_PROPERTY, Some(&q), &mut sp, std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>()) {
            Ok(()) if sp.IncursSeekPenalty => DiskKind::Hdd,
            Ok(()) => DiskKind::Ssd,
            Err(_) => DiskKind::Unknown,
        }
    };
    Ok(DiskInfo { model: if model.is_empty() { "Unknown disk".into() } else { model }, kind, system_drive: drive })
}

pub fn power_info() -> Result<PowerInfo, String> {
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut s) }.map_err(|e| e.to_string())?;
    let battery = logic::has_battery(s.BatteryFlag);
    let plan = unsafe {
        let mut p: *mut GUID = std::ptr::null_mut();
        if PowerGetActiveScheme(None, &mut p) == ERROR_SUCCESS && !p.is_null() {
            let g = format!("{:?}", *p);
            let _ = LocalFree(Some(HLOCAL(p as *mut core::ffi::c_void)));
            logic::power_plan_from_guid(&g)
        } else {
            PowerPlan::Unknown
        }
    };
    Ok(PowerInfo {
        form_factor: if battery { FormFactor::Laptop } else { FormFactor::Desktop },
        on_battery: battery && s.ACLineStatus == 0,
        battery_saver: s.SystemStatusFlag == 1,
        battery_percent: (battery && s.BatteryLifePercent <= 100).then_some(s.BatteryLifePercent),
        plan,
    })
}

/// Interface GUID (e.g. "{6B29FC40-...}") of the primary adapter, used by network tweaks.
pub fn primary_adapter_guid() -> Option<String> {
    adapters().ok()?.into_iter().min_by_key(|(c, _)| c.metric).map(|(_, guid)| guid)
}

fn adapters() -> Result<Vec<(AdapterCandidate, String)>, String> {
    let mut size: u32 = 16 * 1024;
    let mut buf: Vec<u8>;
    let mut tries = 0;
    loop {
        buf = vec![0u8; size as usize];
        let r = unsafe {
            GetAdaptersAddresses(AF_UNSPEC.0 as u32, GAA_FLAG_INCLUDE_GATEWAYS, None, Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH), &mut size)
        };
        if r == ERROR_SUCCESS.0 {
            break;
        }
        tries += 1;
        if r != ERROR_BUFFER_OVERFLOW.0 || tries > 3 {
            return Err(format!("GetAdaptersAddresses failed with code {r}"));
        }
    }
    let mut out = Vec::new();
    let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        // SAFETY: linked list written by the OS inside `buf`, which outlives this loop.
        let a = unsafe { &*p };
        if a.OperStatus == IfOperStatusUp && !a.FirstGatewayAddress.is_null() {
            let guid = unsafe { a.AdapterName.to_string() }.unwrap_or_default();
            out.push((
                AdapterCandidate {
                    name: unsafe { a.FriendlyName.to_string() }.unwrap_or_default(),
                    description: unsafe { a.Description.to_string() }.unwrap_or_default(),
                    if_type: a.IfType,
                    link_bps: a.TransmitLinkSpeed,
                    metric: a.Ipv4Metric,
                },
                guid,
            ));
        }
        p = a.Next;
    }
    Ok(out)
}

pub fn network_info() -> Result<Option<NetworkInfo>, String> {
    let candidates = adapters()?.into_iter().map(|(c, _)| c).collect();
    Ok(logic::pick_primary(candidates).map(|c| NetworkInfo {
        kind: logic::link_kind(c.if_type),
        name: c.name,
        description: c.description,
        link_mbps: if c.link_bps == u64::MAX { 0 } else { c.link_bps / 1_000_000 },
    }))
}

pub fn anti_cheat_info() -> Vec<AntiCheatInfo> {
    use windows_service::service::{ServiceAccess, ServiceState};
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    let Ok(scm) = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT) else {
        return Vec::new();
    };
    logic::ANTI_CHEATS
        .iter()
        .filter_map(|(name, svc, kernel)| {
            let s = scm.open_service(svc, ServiceAccess::QUERY_STATUS).ok()?;
            let running = s.query_status().map(|st| st.current_state == ServiceState::Running).unwrap_or(false);
            Some(AntiCheatInfo { name: (*name).into(), service: (*svc).into(), kernel_driver: *kernel, running })
        })
        .collect()
}
