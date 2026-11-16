//! Diagnostic: two samples one second apart, then system metrics and the top apps.
//! Run: cargo run -p otx-monitor --example top
fn main() {
    let mut m = otx_monitor::Monitor::new();
    let _ = m.system();
    let _ = m.processes();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let s = m.system();
    println!(
        "CPU {:.1}% | RAM {} / {} MB available | commit {} / {} MB | standby {:?} MB",
        s.cpu, s.ram_available_mb, s.ram_total_mb, s.commit_mb, s.commit_limit_mb, s.standby_mb.map(|v| v.round())
    );
    println!("GPU {:?}", s.gpu);
    println!("disk read {:?} MB/s write {:?} MB/s active {:?}% | net down {:.0} kbps up {:.0} kbps", s.disk_read_mbs, s.disk_write_mbs, s.disk_active_pct, s.net_down_kbps, s.net_up_kbps);
    for n in &s.notes { println!("note: {n}"); }
    let procs = m.processes();
    println!("{} apps", procs.len());
    for g in procs.iter().take(12) {
        println!("{:<28} x{:<3} cpu {:>5.1}% gpu {:>5.1}% {:<12} mem {:>7.0} MB io {:>7.0} KB/s {:?} {:?}{}",
            g.name, g.processes.len(), g.cpu, g.gpu, g.gpu_engine.clone().unwrap_or_default(), g.memory_mb, g.io_read_kbs + g.io_write_kbs, g.category, g.priority, if g.protected { " [protected]" } else { "" });
    }
    let mut by_gpu: Vec<_> = procs.iter().filter(|g| g.gpu > 0.0).collect();
    by_gpu.sort_by(|a, b| b.gpu.partial_cmp(&a.gpu).unwrap());
    for g in by_gpu.iter().take(3) { println!("GPU user: {} {:.1}% {:?}", g.name, g.gpu, g.gpu_engine); }
}
