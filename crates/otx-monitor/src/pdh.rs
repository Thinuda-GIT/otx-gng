//! Performance counters (PDH): GPU engines, disk activity and the standby list.
//! English counter names are used so localized Windows works. Every counter is optional; a
//! missing one becomes `None` plus a note, never an error.

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhGetFormattedCounterValue,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA,
};

pub struct Pdh {
    query: PDH_HQUERY,
    gpu: Option<PDH_HCOUNTER>,
    disk_idle: Option<PDH_HCOUNTER>,
    disk_read: Option<PDH_HCOUNTER>,
    disk_write: Option<PDH_HCOUNTER>,
    standby: Vec<PDH_HCOUNTER>,
    pub notes: Vec<String>,
}

// SAFETY: PDH handles are process-wide and the Monitor serialises access behind a Mutex.
unsafe impl Send for Pdh {}

impl Pdh {
    pub fn open() -> Option<Self> {
        let mut query = PDH_HQUERY::default();
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
            return None;
        }
        let mut p = Pdh { query, gpu: None, disk_idle: None, disk_read: None, disk_write: None, standby: Vec::new(), notes: Vec::new() };
        p.gpu = p.add(r"\GPU Engine(*)\Utilization Percentage");
        if p.gpu.is_none() {
            p.notes.push("GPU counters are unavailable on this PC (driver or Windows version), so GPU columns are hidden.".into());
        }
        p.disk_idle = p.add(r"\PhysicalDisk(_Total)\% Idle Time");
        p.disk_read = p.add(r"\PhysicalDisk(_Total)\Disk Read Bytes/sec");
        p.disk_write = p.add(r"\PhysicalDisk(_Total)\Disk Write Bytes/sec");
        for c in [
            r"\Memory\Standby Cache Core Bytes",
            r"\Memory\Standby Cache Normal Priority Bytes",
            r"\Memory\Standby Cache Reserve Bytes",
        ] {
            if let Some(h) = p.add(c) {
                p.standby.push(h);
            }
        }
        unsafe {
            let _ = PdhCollectQueryData(p.query); // rate counters need two collections
        }
        Some(p)
    }

    fn add(&self, path: &str) -> Option<PDH_HCOUNTER> {
        let mut h = PDH_HCOUNTER::default();
        let st = unsafe { PdhAddEnglishCounterW(self.query, &HSTRING::from(path), 0, &mut h) };
        (st == 0).then_some(h)
    }

    pub fn collect(&self) {
        unsafe {
            let _ = PdhCollectQueryData(self.query);
        }
    }

    fn value(h: Option<PDH_HCOUNTER>) -> Option<f64> {
        let h = h?;
        let mut v = PDH_FMT_COUNTERVALUE::default();
        let st = unsafe { PdhGetFormattedCounterValue(h, PDH_FMT_DOUBLE, None, &mut v) };
        (st == 0 && v.CStatus == 0).then(|| unsafe { v.Anonymous.doubleValue })
    }

    /// (instance name, value) for every GPU engine instance.
    pub fn gpu_instances(&self) -> Option<Vec<(String, f64)>> {
        let h = self.gpu?;
        let mut bytes = 0u32;
        let mut count = 0u32;
        let st = unsafe { PdhGetFormattedCounterArrayW(h, PDH_FMT_DOUBLE, &mut bytes, &mut count, None) };
        if st != PDH_MORE_DATA {
            return if count == 0 { Some(Vec::new()) } else { None };
        }
        // u64-aligned buffer; the items are followed by their name strings inside it.
        let mut buf = vec![0u64; (bytes as usize).div_ceil(8)];
        let st = unsafe {
            PdhGetFormattedCounterArrayW(h, PDH_FMT_DOUBLE, &mut bytes, &mut count, Some(buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W))
        };
        if st != 0 {
            return None;
        }
        let items = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W, count as usize) };
        Some(
            items
                .iter()
                .filter(|i| i.FmtValue.CStatus == 0)
                .map(|i| (unsafe { i.szName.to_string() }.unwrap_or_default(), unsafe { i.FmtValue.Anonymous.doubleValue }))
                .collect(),
        )
    }

    pub fn disk_active_pct(&self) -> Option<f32> {
        Self::value(self.disk_idle).map(|idle| (100.0 - idle).clamp(0.0, 100.0) as f32)
    }
    pub fn disk_read_mbs(&self) -> Option<f32> {
        Self::value(self.disk_read).map(|b| (b / 1_048_576.0) as f32)
    }
    pub fn disk_write_mbs(&self) -> Option<f32> {
        Self::value(self.disk_write).map(|b| (b / 1_048_576.0) as f32)
    }
    pub fn standby_mb(&self) -> Option<f64> {
        if self.standby.is_empty() {
            return None;
        }
        let total: Option<f64> = self.standby.iter().map(|h| Self::value(Some(*h))).sum();
        total.map(|b| b / 1_048_576.0)
    }
}

impl Drop for Pdh {
    fn drop(&mut self) {
        unsafe {
            let _ = PdhCloseQuery(self.query);
        }
    }
}
