//! GPU engine counter parsing and aggregation (pure, unit-tested).
//!
//! PDH "\GPU Engine(*)\Utilization Percentage" instances look like
//! `pid_1234_luid_0x00000000_0x0000F1A2_phys_0_eng_3_engtype_3D`. Task Manager's rules:
//! * a process's usage on an engine type = sum over its engines of that type;
//! * a process's GPU % = the maximum over engine types; its "GPU engine" = that type;
//! * system usage per engine type = per adapter, the sum over all processes (capped at 100),
//!   then the busiest adapter.

use otx_protocol::GpuEngines;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct EngineSample {
    pub pid: u32,
    pub luid: String,
    pub engtype: String,
    pub value: f64,
}

pub fn parse_instance(name: &str, value: f64) -> Option<EngineSample> {
    let rest = name.strip_prefix("pid_")?;
    let (pid, rest) = rest.split_once('_')?;
    let rest = rest.strip_prefix("luid_")?;
    // luid is two hex parts: 0x..._0x...
    let mut parts = rest.splitn(3, '_');
    let luid = format!("{}_{}", parts.next()?, parts.next()?);
    let tail = parts.next()?;
    let engtype = tail.split("engtype_").nth(1)?.to_string();
    Some(EngineSample { pid: pid.parse().ok()?, luid, engtype, value: value.max(0.0) })
}

/// Per-process (gpu %, busiest engine type).
pub fn per_process(samples: &[EngineSample]) -> HashMap<u32, (f32, String)> {
    let mut by: HashMap<(u32, &str), f64> = HashMap::new();
    for s in samples {
        *by.entry((s.pid, s.engtype.as_str())).or_default() += s.value;
    }
    let mut out: HashMap<u32, (f32, String)> = HashMap::new();
    for ((pid, eng), v) in by {
        let v = v.min(100.0) as f32;
        let e = out.entry(pid).or_insert((0.0, String::new()));
        if v > e.0 {
            *e = (v, eng.to_string());
        }
    }
    out
}

/// System-wide utilisation per engine type (busiest adapter).
pub fn system(samples: &[EngineSample]) -> GpuEngines {
    let mut by: HashMap<(&str, &str), f64> = HashMap::new();
    for s in samples {
        *by.entry((s.luid.as_str(), s.engtype.as_str())).or_default() += s.value;
    }
    let pick = |want: &[&str]| -> f32 {
        by.iter()
            .filter(|((_, e), _)| want.iter().any(|w| e.eq_ignore_ascii_case(w)))
            .map(|(_, v)| v.min(100.0))
            .fold(0.0, f64::max) as f32
    };
    GpuEngines {
        d3: pick(&["3D", "Graphics_1"]),
        compute: pick(&["Compute_0", "Compute_1", "Compute"]),
        copy: pick(&["Copy"]),
        video_decode: pick(&["VideoDecode"]),
        video_encode: pick(&["VideoEncode"]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_instance_names() {
        let s = parse_instance("pid_1234_luid_0x00000000_0x0000F1A2_phys_0_eng_3_engtype_3D", 12.5).unwrap();
        assert_eq!((s.pid, s.luid.as_str(), s.engtype.as_str(), s.value), (1234, "0x00000000_0x0000F1A2", "3D", 12.5));
        assert!(parse_instance("_Total", 1.0).is_none());
        assert!(parse_instance("pid_x_luid_a_b_engtype_3D", 1.0).is_none());
    }

    #[test]
    fn aggregates_like_task_manager() {
        let mk = |pid, luid: &str, eng: &str, v| EngineSample { pid, luid: luid.into(), engtype: eng.into(), value: v };
        let samples = vec![
            mk(10, "A", "3D", 30.0),
            mk(10, "A", "3D", 25.0), // second 3D engine of the same process: summed
            mk(10, "A", "VideoDecode", 40.0),
            mk(20, "A", "3D", 20.0),
            mk(30, "B", "3D", 90.0),
        ];
        let p = per_process(&samples);
        assert_eq!(p[&10], (55.0, "3D".to_string()));
        assert_eq!(p[&20], (20.0, "3D".to_string()));
        let sys = system(&samples);
        assert_eq!(sys.d3, 90.0); // adapter B is busiest (A = 75)
        assert_eq!(sys.video_decode, 40.0);
        // capped at 100
        let over = system(&[mk(1, "A", "3D", 80.0), mk(2, "A", "3D", 70.0)]);
        assert_eq!(over.d3, 100.0);
    }
}
