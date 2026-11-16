//! Traffic Shaper, driver-free:
//! * Block: Windows Firewall rules (outbound + inbound) for the app's executable paths, grouped
//!   "OTX.GNG" and named so they can always be found and removed.
//! * Limit: Policy-based QoS throttle (outbound only), written as local QoS policy values and
//!   applied with a Group Policy refresh. UNVERIFIED on every Windows edition: if Windows ignores
//!   the policy, nothing breaks, the limit simply has no effect (the UI labels limits Experimental).
//! Everything is journaled as temporary: "Restore normal internet", the end of a session, a crash
//! or a reboot removes it.

use crate::journal::{Entry, RevertOp};
use crate::process::ActionError;
use crate::state::CoreState;
use otx_protocol::{ActionResult, ErrorCode, Response, ShaperAction, ShaperRuleSpec};
use windows::core::BSTR;
use windows::Win32::Foundation::{VARIANT_FALSE, VARIANT_TRUE};
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwPolicy2, INetFwRule, NetFwPolicy2, NetFwRule, NET_FW_ACTION_BLOCK, NET_FW_PROFILE2_ALL, NET_FW_RULE_DIR_IN, NET_FW_RULE_DIR_OUT,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::GroupPolicy::RefreshPolicyEx;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS};
use winreg::RegKey;

pub const GROUP: &str = "OTX.GNG";
const QOS_KEY: &str = r"SOFTWARE\Policies\Microsoft\Windows\QoS";

/// Never blocked or limited: Windows networking, security, DNS/DHCP/time (all in svchost),
/// anti-cheat and the Core itself.
pub fn may_shape(exe: &str) -> bool {
    let e = exe.to_ascii_lowercase();
    !otx_monitor::policy::is_protected(&e)
        && !["svchost.exe", "system", "lsass.exe", "services.exe", "wininit.exe", "dnscache", "otx-gng.exe", "otx.gng.exe"].contains(&e.as_str())
        && e.ends_with(".exe")
        && !e.contains(['\\', '/', ':', '"'])
        && e.len() < 128
}

fn com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn policy() -> Result<INetFwPolicy2, String> {
    com();
    unsafe { CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER) }.map_err(|e| format!("Windows Firewall is not available: {e}"))
}

pub fn rule_name(exe: &str, path_index: usize, inbound: bool) -> String {
    format!("{GROUP}: block {exe} #{path_index} {}", if inbound { "in" } else { "out" })
}

fn add_block(name: &str, path: &str, inbound: bool) -> Result<(), String> {
    let rules = unsafe { policy()?.Rules() }.map_err(|e| e.to_string())?;
    let rule: INetFwRule = unsafe { CoCreateInstance(&NetFwRule, None, CLSCTX_INPROC_SERVER) }.map_err(|e| e.to_string())?;
    unsafe {
        rule.SetName(&BSTR::from(name)).map_err(|e| e.to_string())?;
        rule.SetDescription(&BSTR::from("Temporary rule from OTX.GNG Focus mode. Removed by \"Restore normal internet\", after a crash or at reboot.")).map_err(|e| e.to_string())?;
        rule.SetApplicationName(&BSTR::from(path)).map_err(|e| e.to_string())?;
        rule.SetAction(NET_FW_ACTION_BLOCK).map_err(|e| e.to_string())?;
        rule.SetDirection(if inbound { NET_FW_RULE_DIR_IN } else { NET_FW_RULE_DIR_OUT }).map_err(|e| e.to_string())?;
        rule.SetProfiles(NET_FW_PROFILE2_ALL.0).map_err(|e| e.to_string())?;
        rule.SetGrouping(&BSTR::from(GROUP)).map_err(|e| e.to_string())?;
        rule.SetEnabled(VARIANT_TRUE).map_err(|e| e.to_string())?;
        rules.Add(&rule).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Removes every rule with this name (Remove deletes one match per call).
pub fn remove_rule(name: &str) -> Result<(), String> {
    let rules = unsafe { policy()?.Rules() }.map_err(|e| e.to_string())?;
    for _ in 0..16 {
        let exists = unsafe { rules.Item(&BSTR::from(name)) }.is_ok();
        if !exists {
            return Ok(());
        }
        unsafe { rules.Remove(&BSTR::from(name)) }.map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn rule_exists(name: &str) -> bool {
    policy().ok().and_then(|p| unsafe { p.Rules() }.ok()).is_some_and(|r| unsafe { r.Item(&BSTR::from(name)) }.is_ok_and(|rule| unsafe { rule.Enabled() }.is_ok_and(|e| e != VARIANT_FALSE)))
}

pub fn qos_name(exe: &str) -> String {
    format!("OTX.GNG limit {exe}")
}

fn refresh_policy() {
    // RP_FORCE = 1: reapply even if unchanged.
    let _ = unsafe { RefreshPolicyEx(true, 1) };
}

pub fn add_qos(exe: &str, up_kbps: u32) -> Result<(), String> {
    let (k, _) = RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey(format!(r"{QOS_KEY}\{}", qos_name(exe))).map_err(|e| e.to_string())?;
    let set = |n: &str, v: &str| k.set_value(n, &v.to_string()).map_err(|e| e.to_string());
    set("Version", "1.0")?;
    set("Application Name", exe)?;
    set("Protocol", "*")?;
    set("Local Port", "*")?;
    set("Local IP", "*")?;
    set("Local IP Prefix Length", "*")?;
    set("Remote Port", "*")?;
    set("Remote IP", "*")?;
    set("Remote IP Prefix Length", "*")?;
    set("DSCP Value", "-1")?;
    set("Throttle Rate", &(up_kbps.clamp(64, 1_000_000)).to_string())?;
    refresh_policy();
    Ok(())
}

pub fn remove_qos(exe: &str) -> Result<(), String> {
    match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(QOS_KEY, KEY_ALL_ACCESS) {
        Ok(k) => match k.delete_subkey_all(qos_name(exe)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => {}
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    refresh_policy();
    Ok(())
}

/// Distinct full paths of running processes with this exe name.
fn paths_of(exe: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let devices = otx_monitor::dos_device_map();
    for p in otx_monitor::nt::processes().unwrap_or_default() {
        if p.name.eq_ignore_ascii_case(exe) {
            if let Some(nt) = otx_monitor::nt::image_nt_path(p.pid) {
                let dos = otx_monitor::calc::nt_to_dos(&nt, &devices);
                if !out.iter().any(|x| x.eq_ignore_ascii_case(&dos)) {
                    out.push(dos);
                }
            }
        }
    }
    out
}

fn err(code: ErrorCode, m: impl Into<String>) -> ActionError {
    ActionError { code, message: m.into() }
}

/// Replaces the active shaper rules with `rules`.
pub fn apply(state: &CoreState, rules: &[ShaperRuleSpec], dry_run: bool) -> Result<Response, ActionError> {
    if state.safe_mode {
        return Err(err(ErrorCode::SafeMode, "Safe Mode is on, so the Core makes no changes."));
    }
    if rules.len() > 64 {
        return Err(err(ErrorCode::Validation, "too many rules"));
    }
    for r in rules {
        if !may_shape(&r.exe) {
            return Err(err(ErrorCode::ProtectedProcess, format!("{} is essential to Windows or protected and is never blocked or limited.", r.exe)));
        }
    }
    let dry = dry_run || state.dry_run;
    let mut changes = Vec::new();
    if !dry {
        restore(state);
    }
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    for r in rules {
        match r.action {
            ShaperAction::Block => {
                let paths = paths_of(&r.exe);
                if paths.is_empty() {
                    changes.push(format!("{}: not running, nothing to block now (rules use the app's full path)", r.exe));
                    continue;
                }
                for (i, path) in paths.iter().enumerate() {
                    for inbound in [false, true] {
                        let name = rule_name(&r.exe, i, inbound);
                        if dry {
                            changes.push(format!("Would add firewall rule \"{name}\""));
                            continue;
                        }
                        let id = crate::server::random_hex(8);
                        j.record(Entry {
                            id: id.clone(),
                            created_unix: crate::process::now_unix(),
                            title: format!("Blocked {} ({})", r.exe, if inbound { "in" } else { "out" }),
                            detail: path.clone(),
                            temporary: true,
                            revert: RevertOp::FirewallRule { name: name.clone() },
                        })
                        .map_err(|e| err(ErrorCode::Internal, e.to_string()))?;
                        match add_block(&name, path, inbound) {
                            Ok(()) => changes.push(format!("Blocked {} {}", r.exe, if inbound { "inbound" } else { "outbound" })),
                            Err(e) => {
                                let _ = j.forget(&id);
                                changes.push(format!("{}: firewall rule failed ({e})", r.exe));
                            }
                        }
                    }
                }
            }
            ShaperAction::Limit => {
                let up = r.up_kbps.unwrap_or(1000);
                if dry {
                    changes.push(format!("Would limit {} upload to {up} kbps (QoS policy)", r.exe));
                    continue;
                }
                let id = crate::server::random_hex(8);
                j.record(Entry {
                    id: id.clone(),
                    created_unix: crate::process::now_unix(),
                    title: format!("Limited {} upload to {up} kbps", r.exe),
                    detail: "Policy-based QoS (experimental)".into(),
                    temporary: true,
                    revert: RevertOp::QosPolicy { exe: r.exe.clone() },
                })
                .map_err(|e| err(ErrorCode::Internal, e.to_string()))?;
                match add_qos(&r.exe, up) {
                    Ok(()) => changes.push(format!("Limited {} upload to {up} kbps", r.exe)),
                    Err(e) => {
                        let _ = j.forget(&id);
                        changes.push(format!("{}: QoS policy failed ({e})", r.exe));
                    }
                }
            }
        }
    }
    Ok(Response::Action(ActionResult { dry_run: dry, changes, change_id: None }))
}

/// Removes every shaper rule OTX.GNG added (the "Restore normal internet" button).
pub fn restore(state: &CoreState) -> u32 {
    let mut j = state.journal.lock().unwrap_or_else(|p| p.into_inner());
    let (ok, _) = j
        .revert_where(&crate::journal::SystemReverter { dry_run: false }, |e| matches!(e.revert, RevertOp::FirewallRule { .. } | RevertOp::QosPolicy { .. }))
        .unwrap_or((0, 0));
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn essential_apps_are_never_shaped() {
        for e in ["svchost.exe", "lsass.exe", "vgc.exe", "EasyAntiCheat.exe", "otx-core.exe", "System", "C:\\x\\a.exe", "a\"b.exe"] {
            assert!(!may_shape(e), "{e}");
        }
        for e in ["OneDrive.exe", "steam.exe", "chrome.exe"] {
            assert!(may_shape(e), "{e}");
        }
        assert_eq!(rule_name("x.exe", 0, true), "OTX.GNG: block x.exe #0 in");
    }

    /// Adds and removes a real firewall rule for a throwaway exe path that is never run.
    /// Needs an elevated shell: `cargo test -p otx-core -- --ignored firewall`.
    #[test]
    #[ignore]
    fn firewall_rule_round_trip() {
        let name = "OTX.GNG: test rule (safe to delete)";
        add_block(name, r"C:\otx-gng-test\does-not-exist.exe", false).unwrap();
        assert!(rule_exists(name));
        remove_rule(name).unwrap();
        assert!(!rule_exists(name));
    }
}
