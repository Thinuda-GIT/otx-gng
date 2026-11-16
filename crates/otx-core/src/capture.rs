//! Frame-time capture from Windows' own DXGI events (Microsoft-Windows-DXGI, Present_Start), the
//! same source PresentMon builds on. Nothing is injected and the game is never opened; we only
//! record when the target process calls Present.
//!
//! Limits (shown in the UI): this measures the interval between Present calls (CPU-side frame
//! pacing), not the moment frames reach the display; DirectX 9, OpenGL and Vulkan games that do
//! not use DXGI produce no events.

use ferrisetw::provider::Provider;
use ferrisetw::schema_locator::SchemaLocator;
use ferrisetw::trace::UserTrace;
use ferrisetw::EventRecord;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

const DXGI: &str = "CA11C036-0102-4A2D-A6AD-F03CFED5D3C9";
/// Present_Start and PresentMultiplaneOverlay_Start.
const PRESENT_EVENTS: [u16; 2] = [42, 55];

static TARGET: AtomicU32 = AtomicU32::new(0);
static STAMPS: Mutex<Vec<i64>> = Mutex::new(Vec::new());
static TRACE: Mutex<Option<UserTrace>> = Mutex::new(None);

fn callback(record: &EventRecord, _: &SchemaLocator) {
    let target = TARGET.load(Ordering::Relaxed);
    if target != 0 && record.process_id() == target && PRESENT_EVENTS.contains(&record.event_id()) {
        let mut s = STAMPS.lock().unwrap_or_else(|e| e.into_inner());
        if s.len() < 2_000_000 {
            s.push(record.raw_timestamp());
        }
    }
}

pub fn start(pid: u32) -> Result<(), String> {
    stop_trace();
    STAMPS.lock().unwrap_or_else(|e| e.into_inner()).clear();
    TARGET.store(pid, Ordering::Relaxed);
    let provider = Provider::by_guid(DXGI).add_callback(callback).build();
    let trace = UserTrace::new().enable(provider).start_and_process().map_err(|e| format!("could not start frame capture: {e:?}"))?;
    *TRACE.lock().unwrap_or_else(|e| e.into_inner()) = Some(trace);
    Ok(())
}

fn stop_trace() {
    if let Some(t) = TRACE.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = t.stop();
    }
}

/// Stops and returns frame times in milliseconds.
pub fn stop() -> Vec<f32> {
    stop_trace();
    TARGET.store(0, Ordering::Relaxed);
    let stamps = std::mem::take(&mut *STAMPS.lock().unwrap_or_else(|e| e.into_inner()));
    frame_times(&stamps)
}

/// Intervals between consecutive presents (100 ns ticks → ms); gaps over 1 s (alt-tab, loading)
/// are dropped so they do not distort the statistics.
pub fn frame_times(stamps: &[i64]) -> Vec<f32> {
    let mut s = stamps.to_vec();
    s.sort_unstable();
    s.windows(2).map(|w| (w[1] - w[0]) as f32 / 10_000.0).filter(|ms| *ms > 0.0 && *ms < 1000.0).collect()
}

#[cfg(test)]
mod tests {
    /// Needs elevation: `cargo test -p otx-core -- --ignored capture_live --nocapture`.
    #[test]
    #[ignore]
    fn capture_live() {
        let procs = otx_monitor::nt::processes().unwrap();
        let mut any = false;
        for name in ["msedgewebview2.exe", "chrome.exe", "dwm.exe", "claude.exe"] {
            for p in procs.iter().filter(|p| p.name.eq_ignore_ascii_case(name)).take(6) {
                super::start(p.pid).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(1500));
                let ft = super::stop();
                if !ft.is_empty() {
                    let avg = ft.iter().sum::<f32>() / ft.len() as f32;
                    println!("{name} pid {}: {} frames, avg {:.2} ms ({:.0} fps)", p.pid, ft.len(), avg, 1000.0 / avg);
                    any = true;
                }
            }
        }
        assert!(any, "no DXGI presents captured from any candidate process");
    }

    #[test]
    fn frame_time_math() {
        // 60 fps = 166 667 ticks apart; a 2 s pause is dropped.
        let stamps = [0, 166_667, 333_334, 20_333_334, 20_500_001];
        let ft = super::frame_times(&stamps);
        assert_eq!(ft.len(), 3);
        assert!((ft[0] - 16.6667).abs() < 0.01);
    }
}
