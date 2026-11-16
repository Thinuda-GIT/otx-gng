//! Live monitoring for the Usage Monitor and Home. Runs in the unprivileged app: everything here
//! works without elevation and without opening handles to other processes.
//!
//! Per-process network traffic needs ETW (elevated) and arrives with the Core in Phase 6.

pub mod details;
pub mod gpu;
pub mod nt;
pub mod pdh;
pub mod policy;

use otx_protocol::{PriorityLevel, ProcessCategory, ProcessGroup, ProcessRef, SystemMetrics};
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use windows::core::HSTRING;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Storage::FileSystem::QueryDosDeviceW;
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

/// Pure helpers (unit-tested).
pub mod calc {
    use otx_protocol::PriorityLevel;

    /// Share of total CPU capacity used over `wall_100ns` by `cpu_delta_100ns` of CPU time.
    pub fn cpu_percent(cpu_delta_100ns: u64, wall_100ns: u64, logical_cpus: usize) -> f32 {
        if wall_100ns == 0 || logical_cpus == 0 {
            return 0.0;
        }
        ((cpu_delta_100ns as f64 / (wall_100ns as f64 * logical_cpus as f64)) * 100.0).clamp(0.0, 100.0) as f32
    }

    /// Busy % of one core from (idle, kernel-including-idle, user) deltas.
    pub fn core_busy(idle: u64, kernel: u64, user: u64) -> f32 {
        let total = kernel + user;
        if total == 0 {
            return 0.0;
        }
        ((1.0 - idle as f64 / total as f64) * 100.0).clamp(0.0, 100.0) as f32
    }

    /// Base priority (thread priority number) → priority class.
    pub fn priority_from_base(base: i32) -> Option<PriorityLevel> {
        Some(match base {
            4 => PriorityLevel::Idle,
            6 => PriorityLevel::BelowNormal,
            8 => PriorityLevel::Normal,
            10 => PriorityLevel::AboveNormal,
            13 => PriorityLevel::High,
            _ => return None, // 24 = Realtime (shown as unknown; OTX.GNG never sets it)
        })
    }

    /// `\Device\HarddiskVolume3\x.exe` → `C:\x.exe` using a (device → drive) map.
    pub fn nt_to_dos(nt: &str, devices: &[(String, String)]) -> String {
        for (device, drive) in devices {
            if let Some(rest) = nt.strip_prefix(device.as_str()) {
                if rest.starts_with('\\') {
                    return format!("{drive}{rest}");
                }
            }
        }
        nt.to_string()
    }
}

struct ProcPrev {
    cpu: u64,
    read: u64,
    write: u64,
}

pub struct Monitor {
    logical_cpus: usize,
    pdh: Option<pdh::Pdh>,
    prev_cores: Option<Vec<(u64, u64, u64)>>,
    prev_net: Option<(Instant, u64, u64)>,
    prev_procs: HashMap<(u32, u64), ProcPrev>,
    prev_procs_at: Option<Instant>,
    gpu_by_pid: HashMap<u32, (f32, String)>,
    paths: HashMap<(u32, u64), Option<String>>,
    devices: Vec<(String, String)>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Monitor {
    pub fn new() -> Self {
        let logical_cpus = nt::processor_times().map(|v| v.len()).unwrap_or(1).max(1);
        Self {
            logical_cpus,
            pdh: pdh::Pdh::open(),
            prev_cores: None,
            prev_net: None,
            prev_procs: HashMap::new(),
            prev_procs_at: None,
            gpu_by_pid: HashMap::new(),
            paths: HashMap::new(),
            devices: dos_devices(),
        }
    }

    /// System-wide sample. Call about once per second; rates are measured since the last call.
    pub fn system(&mut self) -> SystemMetrics {
        let mut notes = Vec::new();

        // CPU per core
        let cores = nt::processor_times().unwrap_or_default();
        let per_core: Vec<f32> = match &self.prev_cores {
            Some(prev) if prev.len() == cores.len() => cores
                .iter()
                .zip(prev)
                .map(|(c, p)| calc::core_busy(c.0.saturating_sub(p.0), c.1.saturating_sub(p.1), c.2.saturating_sub(p.2)))
                .collect(),
            _ => vec![0.0; cores.len()],
        };
        self.prev_cores = Some(cores);
        let cpu = if per_core.is_empty() { 0.0 } else { per_core.iter().sum::<f32>() / per_core.len() as f32 };

        // Memory
        let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        let _ = unsafe { GlobalMemoryStatusEx(&mut m) };
        let mb = |b: u64| b / 1_048_576;

        // PDH: GPU, disk, standby
        let (gpu, disk_read, disk_write, disk_active, standby) = match &self.pdh {
            Some(p) => {
                p.collect();
                notes.extend(p.notes.iter().cloned());
                let gpu = p.gpu_instances().map(|items| {
                    let samples: Vec<gpu::EngineSample> = items.iter().filter_map(|(n, v)| gpu::parse_instance(n, *v)).collect();
                    self.gpu_by_pid = gpu::per_process(&samples);
                    gpu::system(&samples)
                });
                (gpu, p.disk_read_mbs(), p.disk_write_mbs(), p.disk_active_pct(), p.standby_mb())
            }
            None => {
                notes.push("Performance counters could not be opened; GPU and disk activity are unavailable.".into());
                (None, None, None, None, None)
            }
        };

        // Network totals
        let (down, up) = self.network_rates();

        SystemMetrics {
            t_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
            cpu,
            per_core,
            gpu,
            ram_total_mb: mb(m.ullTotalPhys),
            ram_available_mb: mb(m.ullAvailPhys),
            commit_mb: mb(m.ullTotalPageFile.saturating_sub(m.ullAvailPageFile)),
            commit_limit_mb: mb(m.ullTotalPageFile),
            standby_mb: standby,
            disk_read_mbs: disk_read,
            disk_write_mbs: disk_write,
            disk_active_pct: disk_active,
            net_down_kbps: down,
            net_up_kbps: up,
            notes,
        }
    }

    fn network_rates(&mut self) -> (f32, f32) {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if unsafe { GetIfTable2(&mut table) }.is_err() || table.is_null() {
            return (0.0, 0.0);
        }
        let (mut rx, mut tx) = (0u64, 0u64);
        unsafe {
            let n = (*table).NumEntries as usize;
            let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
            for r in rows {
                let hardware = r.InterfaceAndOperStatusFlags._bitfield & 0x1 != 0;
                let filter = r.InterfaceAndOperStatusFlags._bitfield & 0x2 != 0;
                // Physical adapters plus PPP / mobile broadband links; skip filters, loopback, tunnels.
                let counted = !filter && (hardware || matches!(r.Type, 23 | 243 | 244));
                if counted && r.OperStatus == IfOperStatusUp {
                    rx += r.InOctets;
                    tx += r.OutOctets;
                }
            }
            FreeMibTable(table as *const core::ffi::c_void);
        }
        let now = Instant::now();
        let rates = match self.prev_net {
            Some((t, prx, ptx)) if rx >= prx && tx >= ptx => {
                let s = now.duration_since(t).as_secs_f64().max(0.001);
                (((rx - prx) as f64 * 8.0 / 1000.0 / s) as f32, ((tx - ptx) as f64 * 8.0 / 1000.0 / s) as f32)
            }
            _ => (0.0, 0.0),
        };
        self.prev_net = Some((now, rx, tx));
        rates
    }

    fn path_of(&mut self, pid: u32, create_time: u64) -> Option<String> {
        if let Some(p) = self.paths.get(&(pid, create_time)) {
            return p.clone();
        }
        let p = nt::image_nt_path(pid).map(|nt| calc::nt_to_dos(&nt, &self.devices));
        self.paths.insert((pid, create_time), p.clone());
        p
    }

    /// Per-app view (processes with the same exe merged). Rates are since the previous call.
    pub fn processes(&mut self) -> Vec<ProcessGroup> {
        let raw = nt::processes().unwrap_or_default();
        let now = Instant::now();
        let wall_100ns = self.prev_procs_at.map(|t| (now.duration_since(t).as_nanos() / 100) as u64).unwrap_or(0);
        let secs = wall_100ns as f64 / 10_000_000.0;

        let mut groups: HashMap<String, ProcessGroup> = HashMap::new();
        let mut next_prev = HashMap::with_capacity(raw.len());
        for p in &raw {
            if p.pid == 0 {
                continue; // Idle "process" is just unused CPU
            }
            let key = (p.pid, p.create_time);
            let (cpu, rk, wk) = match self.prev_procs.get(&key) {
                Some(prev) if wall_100ns > 0 => (
                    calc::cpu_percent(p.cpu_time_100ns.saturating_sub(prev.cpu), wall_100ns, self.logical_cpus),
                    (p.read_bytes.saturating_sub(prev.read) as f64 / 1024.0 / secs) as f32,
                    (p.write_bytes.saturating_sub(prev.write) as f64 / 1024.0 / secs) as f32,
                ),
                _ => (0.0, 0.0, 0.0),
            };
            next_prev.insert(key, ProcPrev { cpu: p.cpu_time_100ns, read: p.read_bytes, write: p.write_bytes });
            let path = if p.pid <= 4 { None } else { self.path_of(p.pid, p.create_time) };
            let (gpu, engine) = self.gpu_by_pid.get(&p.pid).cloned().unwrap_or((0.0, String::new()));

            let id = p.name.to_ascii_lowercase();
            let g = groups.entry(id.clone()).or_insert_with(|| {
                let category = policy::category_of(&p.name, p.session_id, path.as_deref());
                let k = policy::lookup(&p.name);
                ProcessGroup {
                    id: id.clone(),
                    name: k.map(|k| k.name.clone()).unwrap_or_else(|| p.name.trim_end_matches(".exe").to_string()),
                    exe: p.name.clone(),
                    path: path.clone(),
                    category,
                    processes: Vec::new(),
                    cpu: 0.0,
                    gpu: 0.0,
                    gpu_engine: None,
                    memory_mb: 0.0,
                    working_set_mb: 0.0,
                    commit_mb: 0.0,
                    io_read_kbs: 0.0,
                    io_write_kbs: 0.0,
                    priority: calc::priority_from_base(p.base_priority),
                    session_id: p.session_id,
                    protected: policy::is_protected(&p.name),
                    hog: false,
                    description: k.map(|k| k.desc.clone()),
                    safe_to_close: policy::safe_to_close(&p.name, category),
                }
            });
            g.processes.push(ProcessRef { pid: p.pid, create_time: p.create_time });
            g.cpu += cpu;
            if gpu > g.gpu {
                g.gpu = gpu;
                g.gpu_engine = (!engine.is_empty()).then_some(engine);
            }
            g.memory_mb += p.private_ws as f32 / 1_048_576.0;
            g.working_set_mb += p.working_set as f32 / 1_048_576.0;
            g.commit_mb += p.commit as f32 / 1_048_576.0;
            g.io_read_kbs += rk;
            g.io_write_kbs += wk;
            if g.path.is_none() {
                g.path = path;
            }
        }
        self.prev_procs = next_prev;
        self.prev_procs_at = Some(now);
        // Forget paths of processes that exited.
        self.paths.retain(|k, _| self.prev_procs.contains_key(k));

        let mut out: Vec<ProcessGroup> = groups
            .into_values()
            .map(|mut g| {
                g.hog = policy::is_hog(g.category, g.cpu, g.memory_mb, g.io_read_kbs + g.io_write_kbs);
                g
            })
            .collect();
        out.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    pub fn logical_cpus(&self) -> usize {
        self.logical_cpus
    }
}

/// (NT device path, "C:") for every drive letter.
pub fn dos_device_map() -> Vec<(String, String)> {
    dos_devices()
}

fn dos_devices() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = format!("{}:", letter as char);
        let mut buf = [0u16; 512];
        let n = unsafe { QueryDosDeviceW(&HSTRING::from(drive.as_str()), Some(&mut buf)) } as usize;
        if n > 0 {
            let end = buf.iter().position(|&c| c == 0).unwrap_or(n);
            out.push((String::from_utf16_lossy(&buf[..end]), drive));
        }
    }
    out
}

/// Re-export for the Core's process actions.
pub fn priority_class_name(level: PriorityLevel) -> &'static str {
    match level {
        PriorityLevel::Idle => "Low",
        PriorityLevel::BelowNormal => "Below normal",
        PriorityLevel::Normal => "Normal",
        PriorityLevel::AboveNormal => "Above normal",
        PriorityLevel::High => "High",
    }
}

#[doc(hidden)]
pub fn _category_name(c: ProcessCategory) -> &'static str {
    match c {
        ProcessCategory::Game => "game",
        ProcessCategory::Launcher => "launcher",
        ProcessCategory::Browser => "browser",
        ProcessCategory::Voice => "voice",
        ProcessCategory::System => "system",
        ProcessCategory::Background => "background",
        ProcessCategory::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::calc::*;
    use super::*;

    #[test]
    fn cpu_math() {
        // 1 s of CPU over 1 s wall on 4 logical CPUs = 25 % of total capacity
        assert_eq!(cpu_percent(10_000_000, 10_000_000, 4), 25.0);
        assert_eq!(cpu_percent(0, 0, 4), 0.0);
        assert_eq!(cpu_percent(99_000_000, 10_000_000, 4), 100.0); // clamped
        assert_eq!(core_busy(500, 1000, 0), 50.0);
        assert_eq!(core_busy(0, 0, 0), 0.0);
    }

    #[test]
    fn priorities_and_paths() {
        assert_eq!(priority_from_base(8), Some(PriorityLevel::Normal));
        assert_eq!(priority_from_base(13), Some(PriorityLevel::High));
        assert_eq!(priority_from_base(24), None);
        let dev = vec![(r"\Device\HarddiskVolume3".to_string(), "C:".to_string()), (r"\Device\HarddiskVolume30".to_string(), "D:".to_string())];
        assert_eq!(nt_to_dos(r"\Device\HarddiskVolume3\Windows\x.exe", &dev), r"C:\Windows\x.exe");
        assert_eq!(nt_to_dos(r"\Device\HarddiskVolume30\Games\g.exe", &dev), r"D:\Games\g.exe");
        assert_eq!(nt_to_dos(r"\Device\Mup\server\x.exe", &dev), r"\Device\Mup\server\x.exe");
    }

    #[test]
    fn live_sampling_on_this_machine() {
        let mut m = Monitor::new();
        let _ = m.system();
        let _ = m.processes();
        std::thread::sleep(std::time::Duration::from_millis(600));
        let s = m.system();
        assert!(s.ram_total_mb > 1000 && s.ram_available_mb <= s.ram_total_mb);
        assert!(s.commit_mb <= s.commit_limit_mb);
        assert_eq!(s.per_core.len(), m.logical_cpus());
        assert!((0.0..=100.0).contains(&s.cpu));
        let procs = m.processes();
        assert!(procs.len() > 10);
        let me = procs.iter().find(|g| g.processes.iter().any(|p| p.pid == std::process::id())).expect("own process");
        let path = me.path.as_deref().expect("own path resolved without a handle");
        assert!(path.chars().nth(1) == Some(':'), "converted to a drive path: {path}");
        assert!(procs.iter().any(|g| g.id == "svchost.exe" && g.protected && g.processes.len() > 3), "svchost grouped and protected");
    }
}
