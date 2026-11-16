//! Tweak manifest data model. Adding a tweak means adding data to `manifests.json`, not code.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Scope {
    /// Machine-wide: applied by the Core service (LocalSystem).
    Machine,
    /// Current user: applied by the app in the user's own session (HKCU, SystemParametersInfo).
    User,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Root {
    Hklm,
    Hkcu,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data", rename_all = "camelCase")]
pub enum RegValue {
    Dword(u32),
    Sz(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StartType {
    Automatic,
    AutomaticDelayed,
    Manual,
    Disabled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SpiSetting {
    /// 0 (shortest) to 3.
    KeyboardDelay,
    /// 0 (slowest) to 31.
    KeyboardSpeed,
    /// [threshold1, threshold2, acceleration]; [0,0,0] turns "Enhance pointer precision" off.
    Mouse,
}

/// One typed operation. Keys may contain placeholders resolved at run time:
/// `$adapterGuid` (active adapter's interface GUID) and `$adapterClassKey` (its driver key).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Op {
    Reg { root: Root, key: String, name: String, value: RegValue },
    Service { name: String, start: StartType },
    /// AC value of a power setting on the active scheme.
    PowerAc { subgroup: String, setting: String, value: u32 },
    /// Activate a high-performance scheme (Ultimate if it can be created, otherwise High).
    PerformanceScheme,
    Spi { setting: SpiSetting, value: Vec<u32> },
}

/// The value an operation found before it ran (the snapshot used for revert).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Prev {
    Reg { root: Root, key: String, name: String, value: Option<RegValue> },
    Service { name: String, start: StartType },
    PowerAc { scheme: String, subgroup: String, setting: String, value: u32 },
    Scheme { scheme: String },
    Spi { setting: SpiSetting, value: Vec<u32> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub scope: Scope,
    pub title: String,
    #[serde(default)]
    pub requires_reboot: bool,
    #[serde(default)]
    pub in_high_performance: bool,
    /// When set, the tweak is shown but cannot be applied (with this reason).
    #[serde(default)]
    pub unsupported: Option<String>,
    #[serde(default)]
    pub ops: Vec<Op>,
}

pub fn manifests() -> Vec<Manifest> {
    serde_json::from_str(include_str!("../data/manifests.json")).expect("embedded manifests are valid (checked by tests)")
}
