//! Pure decision logic (no Win32 calls) so every rule is unit-tested.

use otx_protocol::{CpuInfo, GpuVendor, LinkKind, PowerPlan};

/// Windows 11 starts at build 22000. The registry ProductName still says "Windows 10" on 11.
pub fn os_name(build: u32) -> &'static str {
    if build >= 22000 {
        "Windows 11"
    } else {
        "Windows 10"
    }
}

/// OTX.GNG supports Windows 10 22H2 (19045) and Windows 11 (22000+).
pub fn os_supported(build: u32) -> bool {
    build >= 19045
}

pub fn gpu_vendor(vendor_id: u32) -> GpuVendor {
    match vendor_id {
        0x10DE => GpuVendor::Nvidia,
        0x1002 | 0x1022 => GpuVendor::Amd,
        0x8086 => GpuVendor::Intel,
        _ => GpuVendor::Other,
    }
}

/// Integrated GPUs share system memory and report little or no dedicated VRAM.
/// Intel Arc discrete cards report several GB, so the VRAM rule covers them too.
pub fn gpu_is_integrated(vendor: GpuVendor, dedicated_vram_mb: u64) -> bool {
    match vendor {
        GpuVendor::Nvidia => false,
        _ => dedicated_vram_mb < 1024,
    }
}

/// Known built-in power scheme GUIDs (lower-case, no braces).
pub fn power_plan_from_guid(guid: &str) -> PowerPlan {
    match guid.to_ascii_lowercase().trim_matches(|c| c == '{' || c == '}') {
        "e9a42b02-d5df-448d-aa00-03f14749eb61" => PowerPlan::Ultimate,
        "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" => PowerPlan::High,
        "381b4222-f694-41f0-9685-ff5bb260df2e" => PowerPlan::Balanced,
        "a1841308-3541-4fab-bc81-f71556f20b4a" => PowerPlan::Saver,
        _ => PowerPlan::Unknown,
    }
}

pub fn link_kind(if_type: u32) -> LinkKind {
    match if_type {
        6 => LinkKind::Ethernet,
        71 => LinkKind::Wifi,
        // IF_TYPE_WWANPP / IF_TYPE_WWANPP2: mobile broadband (built-in modem or dongle)
        243 | 244 => LinkKind::Mobile,
        // IF_TYPE_PPP: dial-up profiles (many mobile dongles) and PPPoE broadband
        23 => LinkKind::Ppp,
        _ => LinkKind::Other,
    }
}

/// StartupApproved values: the first byte is even (0x02, 0x06) when enabled and odd
/// (0x03, 0x07) when the user disabled the entry in Task Manager. No value = enabled.
pub fn startup_disabled(flag: Option<&[u8]>) -> bool {
    matches!(flag, Some([first, ..]) if first & 1 == 1)
}

/// One physical core as reported by GetLogicalProcessorInformationEx(RelationProcessorCore).
#[derive(Debug, Clone, Copy)]
pub struct CoreEntry {
    pub efficiency_class: u8,
    pub threads: u32,
}

/// On hybrid CPUs, performance cores have the HIGHER efficiency class. On non-hybrid CPUs all
/// cores share one class and we report no P/E split.
pub fn summarize_cores(model: String, cores: &[CoreEntry]) -> CpuInfo {
    let threads = cores.iter().map(|c| c.threads).sum();
    let max_class = cores.iter().map(|c| c.efficiency_class).max().unwrap_or(0);
    let min_class = cores.iter().map(|c| c.efficiency_class).min().unwrap_or(0);
    let hybrid = max_class != min_class;
    let p = cores.iter().filter(|c| c.efficiency_class == max_class).count() as u32;
    CpuInfo {
        model,
        cores: cores.len() as u32,
        threads,
        hybrid,
        performance_cores: hybrid.then_some(p),
        efficiency_cores: hybrid.then_some(cores.len() as u32 - p),
    }
}

/// A candidate network adapter (only adapters that are up and have a default gateway).
#[derive(Debug, Clone)]
pub struct AdapterCandidate {
    pub name: String,
    pub description: String,
    pub if_type: u32,
    pub link_bps: u64,
    pub metric: u32,
}

/// Windows routes through the adapter with the lowest metric, so that is "the" connection.
pub fn pick_primary(mut c: Vec<AdapterCandidate>) -> Option<AdapterCandidate> {
    c.sort_by_key(|a| a.metric);
    c.into_iter().next()
}

/// Battery flag 128 = "no system battery", 255 = "unknown status".
pub fn has_battery(battery_flag: u8) -> bool {
    battery_flag != 128 && battery_flag != 255
}

/// Trims a NUL-terminated UTF-16 buffer.
pub fn utf16_until_nul(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end]).trim().to_string()
}

/// Anti-cheat services and drivers we detect (service key names). Detection is read-only:
/// we only ask the Service Control Manager whether they exist and are running.
pub const ANTI_CHEATS: &[(&str, &str, bool)] = &[
    ("Riot Vanguard", "vgc", false),
    ("Riot Vanguard (driver)", "vgk", true),
    ("Easy Anti-Cheat", "EasyAntiCheat", false),
    ("Easy Anti-Cheat (EOS)", "EasyAntiCheat_EOS", false),
    ("BattlEye", "BEService", false),
    ("FACEIT", "FACEIT", true),
    ("FACEIT (service)", "FACEITService", false),
    ("ESEA", "ESEADriver2", true),
    ("PunkBuster", "PnkBstrA", false),
    ("nProtect GameGuard", "npggsvc", false),
    ("XIGNCODE3", "xhunter1", true),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_names_follow_build_numbers() {
        assert_eq!(os_name(19045), "Windows 10");
        assert_eq!(os_name(22000), "Windows 11");
        assert_eq!(os_name(26100), "Windows 11");
        assert!(os_supported(19045));
        assert!(!os_supported(19044));
    }

    #[test]
    fn gpus_are_classified() {
        assert_eq!(gpu_vendor(0x10DE), GpuVendor::Nvidia);
        assert_eq!(gpu_vendor(0x1002), GpuVendor::Amd);
        assert!(gpu_is_integrated(GpuVendor::Intel, 128));
        assert!(!gpu_is_integrated(GpuVendor::Intel, 8192)); // Arc A770
        assert!(gpu_is_integrated(GpuVendor::Amd, 512)); // APU UMA carve-out
        assert!(!gpu_is_integrated(GpuVendor::Amd, 12288));
        assert!(!gpu_is_integrated(GpuVendor::Nvidia, 0));
    }

    #[test]
    fn power_plans_map_from_guids() {
        assert_eq!(power_plan_from_guid("{381B4222-F694-41F0-9685-FF5BB260DF2E}"), PowerPlan::Balanced);
        assert_eq!(power_plan_from_guid("8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c"), PowerPlan::High);
        assert_eq!(power_plan_from_guid("e9a42b02-d5df-448d-aa00-03f14749eb61"), PowerPlan::Ultimate);
        assert_eq!(power_plan_from_guid("00000000-0000-0000-0000-000000000000"), PowerPlan::Unknown);
    }

    #[test]
    fn hybrid_cpus_split_into_p_and_e_cores() {
        // i7-12700H: 6 P-cores (2 threads, class 1) + 8 E-cores (1 thread, class 0)
        let mut cores = vec![CoreEntry { efficiency_class: 1, threads: 2 }; 6];
        cores.extend(vec![CoreEntry { efficiency_class: 0, threads: 1 }; 8]);
        let c = summarize_cores("x".into(), &cores);
        assert_eq!((c.cores, c.threads, c.hybrid), (14, 20, true));
        assert_eq!((c.performance_cores, c.efficiency_cores), (Some(6), Some(8)));

        let flat = summarize_cores("y".into(), &[CoreEntry { efficiency_class: 0, threads: 2 }; 8]);
        assert_eq!((flat.cores, flat.threads, flat.hybrid, flat.performance_cores), (8, 16, false, None));
    }

    #[test]
    fn primary_adapter_is_lowest_metric() {
        let a = |n: &str, m: u32| AdapterCandidate { name: n.into(), description: String::new(), if_type: 6, link_bps: 0, metric: m };
        assert_eq!(pick_primary(vec![a("wifi", 35), a("eth", 25)]).unwrap().name, "eth");
        assert!(pick_primary(vec![]).is_none());
    }

    #[test]
    fn battery_flags_and_strings() {
        assert!(!has_battery(128));
        assert!(!has_battery(255));
        assert!(has_battery(1));
        assert_eq!(utf16_until_nul(&[72, 105, 0, 99]), "Hi");
        assert_eq!(link_kind(71), LinkKind::Wifi);
        assert_eq!(link_kind(243), LinkKind::Mobile);
        assert_eq!(link_kind(23), LinkKind::Ppp);
    }

    #[test]
    fn startup_approved_flags() {
        assert!(!startup_disabled(None));
        assert!(!startup_disabled(Some(&[0x02, 0, 0])));
        assert!(!startup_disabled(Some(&[0x06, 0, 0])));
        assert!(startup_disabled(Some(&[0x03, 0, 0])));
        assert!(startup_disabled(Some(&[0x07, 0, 0])));
        assert!(!startup_disabled(Some(&[])));
    }
}
