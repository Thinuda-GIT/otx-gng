//! Diagnostic: prints what OTX.GNG detects on this PC, as JSON.
//! Run: cargo run -p otx-sysinfo --example sysinfo
fn main() {
    let info = otx_sysinfo::collect();
    println!("{}", serde_json::to_string_pretty(&info).expect("SystemInfo serializes"));
    println!("current-user startup entries: {}", otx_sysinfo::startup_apps_current_user());
}
