//! The one generic tweak engine: check → snapshot → apply → verify → (rollback on failure);
//! revert restores the snapshot, never a guessed default.

use crate::model::{Manifest, Op, Prev, Root, Scope, StartType};

/// System access. `WinSys` implements it for real; tests use `FakeSys`.
pub trait Sys {
    fn scope(&self) -> Scope;
    fn reg_get(&self, root: Root, key: &str, name: &str) -> Result<Option<crate::model::RegValue>, String>;
    fn reg_set(&self, root: Root, key: &str, name: &str, value: &crate::model::RegValue) -> Result<(), String>;
    fn reg_delete(&self, root: Root, key: &str, name: &str) -> Result<(), String>;
    /// None when the service is not installed.
    fn service_start(&self, name: &str) -> Result<Option<StartType>, String>;
    fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String>;
    fn active_scheme(&self) -> Result<String, String>;
    fn set_active_scheme(&self, scheme: &str) -> Result<(), String>;
    /// Creates (or finds) a high-performance scheme and returns its GUID.
    fn performance_scheme(&self) -> Result<String, String>;
    fn power_ac(&self, scheme: &str, subgroup: &str, setting: &str) -> Result<u32, String>;
    fn set_power_ac(&self, scheme: &str, subgroup: &str, setting: &str, value: u32) -> Result<(), String>;
    fn spi_get(&self, setting: crate::model::SpiSetting) -> Result<Vec<u32>, String>;
    fn spi_set(&self, setting: crate::model::SpiSetting, value: &[u32]) -> Result<(), String>;
    /// Resolves `$adapterGuid` / `$adapterClassKey`.
    fn resolve(&self, placeholder: &str) -> Result<String, String>;
}

pub const BALANCED: &str = "381b4222-f694-41f0-9685-ff5bb260df2e";
pub const SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Applied,
    NotApplied,
    Partial,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub state: State,
    pub reason: Option<String>,
}

fn expand(sys: &dyn Sys, key: &str) -> Result<String, String> {
    let mut k = key.to_string();
    for p in ["$adapterClassKey", "$adapterGuid"] {
        if k.contains(p) {
            k = k.replace(p, &sys.resolve(p)?);
        }
    }
    Ok(k)
}

/// Is this single operation already in its desired state?
fn op_applied(sys: &dyn Sys, op: &Op) -> Result<bool, String> {
    Ok(match op {
        Op::Reg { root, key, name, value } => sys.reg_get(*root, &expand(sys, key)?, name)?.as_ref() == Some(value),
        Op::Service { name, start } => match sys.service_start(name)? {
            Some(s) => s == *start,
            None => return Err(format!("the {name} service is not installed")),
        },
        Op::PowerAc { subgroup, setting, value } => {
            let scheme = sys.active_scheme()?;
            sys.power_ac(&scheme, subgroup, setting)? == *value
        }
        Op::PerformanceScheme => {
            let s = sys.active_scheme()?.to_ascii_lowercase();
            s != BALANCED && s != SAVER
        }
        Op::Spi { setting, value } => sys.spi_get(*setting)? == *value,
    })
}

fn check_scope(m: &Manifest, sys: &dyn Sys) -> Result<(), String> {
    if m.scope != sys.scope() {
        return Err(format!("{} must be applied from the {:?} side", m.id, m.scope));
    }
    Ok(())
}

pub fn check(m: &Manifest, sys: &dyn Sys) -> Status {
    if let Some(r) = &m.unsupported {
        return Status { state: State::Unsupported, reason: Some(r.clone()) };
    }
    let mut applied = 0;
    for op in &m.ops {
        match op_applied(sys, op) {
            Ok(true) => applied += 1,
            Ok(false) => {}
            Err(e) => return Status { state: State::Unsupported, reason: Some(format!("Not available on this PC: {e}.")) },
        }
    }
    let state = if applied == m.ops.len() {
        State::Applied
    } else if applied == 0 {
        State::NotApplied
    } else {
        State::Partial
    };
    Status { state, reason: None }
}

/// Reads the current value for one op (the snapshot).
fn snapshot(sys: &dyn Sys, op: &Op) -> Result<Prev, String> {
    Ok(match op {
        Op::Reg { root, key, name, .. } => {
            let key = expand(sys, key)?;
            Prev::Reg { root: *root, value: sys.reg_get(*root, &key, name)?, key, name: name.clone() }
        }
        Op::Service { name, .. } => Prev::Service {
            name: name.clone(),
            start: sys.service_start(name)?.ok_or_else(|| format!("the {name} service is not installed"))?,
        },
        Op::PowerAc { subgroup, setting, .. } => {
            let scheme = sys.active_scheme()?;
            Prev::PowerAc { value: sys.power_ac(&scheme, subgroup, setting)?, scheme, subgroup: subgroup.clone(), setting: setting.clone() }
        }
        Op::PerformanceScheme => Prev::Scheme { scheme: sys.active_scheme()? },
        Op::Spi { setting, .. } => Prev::Spi { setting: *setting, value: sys.spi_get(*setting)? },
    })
}

fn describe(op: &Op, prev: &Prev) -> String {
    match (op, prev) {
        (Op::Reg { root, name, value, .. }, Prev::Reg { key, value: old, .. }) => format!(
            "{}\\{key}\\{name}: {} → {}",
            if *root == Root::Hklm { "HKLM" } else { "HKCU" },
            old.as_ref().map(show).unwrap_or_else(|| "(not set)".into()),
            show(value)
        ),
        (Op::Service { name, start }, Prev::Service { start: old, .. }) => format!("Service {name}: {old:?} → {start:?}"),
        (Op::PowerAc { value, .. }, Prev::PowerAc { value: old, .. }) => format!("Active power plan setting: {old} → {value}"),
        (Op::PerformanceScheme, Prev::Scheme { scheme }) => format!("Power plan: {scheme} → high-performance plan"),
        (Op::Spi { setting, value }, Prev::Spi { value: old, .. }) => format!("{setting:?}: {old:?} → {value:?}"),
        _ => "change".into(),
    }
}

fn show(v: &crate::model::RegValue) -> String {
    match v {
        crate::model::RegValue::Dword(d) => format!("0x{d:X}"),
        crate::model::RegValue::Sz(s) => format!("\"{s}\""),
    }
}

fn write(sys: &dyn Sys, op: &Op, prev: &Prev) -> Result<(), String> {
    match (op, prev) {
        (Op::Reg { root, name, value, .. }, Prev::Reg { key, .. }) => sys.reg_set(*root, key, name, value),
        (Op::Service { name, start }, _) => sys.set_service_start(name, *start),
        (Op::PowerAc { subgroup, setting, value }, Prev::PowerAc { scheme, .. }) => sys.set_power_ac(scheme, subgroup, setting, *value),
        (Op::PerformanceScheme, _) => {
            let s = sys.performance_scheme()?;
            sys.set_active_scheme(&s)
        }
        (Op::Spi { setting, value }, _) => sys.spi_set(*setting, value),
        _ => Err("internal: snapshot does not match operation".into()),
    }
}

/// Restores one snapshot.
pub fn restore(sys: &dyn Sys, prev: &Prev) -> Result<(), String> {
    match prev {
        Prev::Reg { root, key, name, value: Some(v) } => sys.reg_set(*root, key, name, v),
        Prev::Reg { root, key, name, value: None } => sys.reg_delete(*root, key, name),
        Prev::Service { name, start } => sys.set_service_start(name, *start),
        Prev::PowerAc { scheme, subgroup, setting, value } => sys.set_power_ac(scheme, subgroup, setting, *value),
        Prev::Scheme { scheme } => sys.set_active_scheme(scheme),
        Prev::Spi { setting, value } => sys.spi_set(*setting, value),
    }
}

pub struct Applied {
    pub prev: Vec<Prev>,
    pub changes: Vec<String>,
}

/// Applies a manifest. On any failure, already-written operations are rolled back.
pub fn apply(m: &Manifest, sys: &dyn Sys, dry_run: bool) -> Result<Applied, String> {
    check_scope(m, sys)?;
    if let Some(r) = &m.unsupported {
        return Err(r.clone());
    }
    let mut prev = Vec::new();
    let mut changes = Vec::new();
    for op in &m.ops {
        let p = snapshot(sys, op)?;
        changes.push(describe(op, &p));
        prev.push(p);
    }
    if dry_run {
        return Ok(Applied { prev, changes });
    }
    for (i, (op, p)) in m.ops.iter().zip(&prev).enumerate() {
        if let Err(e) = write(sys, op, p) {
            for q in prev[..i].iter().rev() {
                let _ = restore(sys, q);
            }
            return Err(format!("{e} (earlier steps were rolled back)"));
        }
    }
    // Verify.
    let status = check(m, sys);
    if status.state != State::Applied {
        for q in prev.iter().rev() {
            let _ = restore(sys, q);
        }
        return Err("Windows did not keep the new values, so they were rolled back.".into());
    }
    Ok(Applied { prev, changes })
}

/// Reverts from a snapshot, newest first. Returns the first error after trying every step.
pub fn revert(sys: &dyn Sys, prev: &[Prev]) -> Result<(), String> {
    let mut first_err = None;
    for p in prev.iter().rev() {
        if let Err(e) = restore(sys, p) {
            first_err.get_or_insert(e);
        }
    }
    first_err.map_or(Ok(()), Err)
}
