//! Process knowledge and the protected-process policy. The Core uses `is_protected` as the
//! authoritative check before any process action; the UI uses it only to grey out buttons.

use otx_protocol::{ProcessCategory, SafeToClose};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Knowledge {
    pub name: String,
    pub category: String,
    pub safe: String,
    pub desc: String,
}

#[derive(Deserialize)]
struct KnowledgeFile {
    processes: HashMap<String, Knowledge>,
}

pub fn knowledge() -> &'static HashMap<String, Knowledge> {
    static K: OnceLock<HashMap<String, Knowledge>> = OnceLock::new();
    K.get_or_init(|| {
        serde_json::from_str::<KnowledgeFile>(include_str!("../data/process-knowledge.json"))
            .map(|f| f.processes)
            .unwrap_or_default()
    })
}

pub fn lookup(exe: &str) -> Option<&'static Knowledge> {
    knowledge().get(&exe.to_ascii_lowercase())
}

/// Never ended, never re-prioritised, never trimmed: core Windows, security software, audio,
/// GPU drivers, anti-cheat, and OTX.GNG's own Core.
const PROTECTED: &[&str] = &[
    "system", "registry", "memory compression", "idle", "secure system", "smss.exe", "csrss.exe", "wininit.exe",
    "winlogon.exe", "services.exe", "lsass.exe", "lsaiso.exe", "svchost.exe", "dwm.exe", "audiodg.exe",
    "fontdrvhost.exe", "msmpeng.exe", "nissrv.exe", "securityhealthservice.exe", "mssense.exe", "sgrmbroker.exe",
    "tiworker.exe", "trustedinstaller.exe", "gamingservices.exe",
    // GPU drivers
    "nvdisplay.container.exe", "atiesrxx.exe", "atieclxx.exe", "amdfendrsr.exe", "igfxcuiservice.exe", "igfxem.exe",
    // anti-cheat
    "vgc.exe", "vgtray.exe", "easyanticheat.exe", "easyanticheat_eos.exe", "beservice.exe", "beservice_x64.exe",
    "faceit.exe", "faceitservice.exe", "esea.exe", "pnkbstra.exe", "pnkbstrb.exe", "xigncode.exe", "gameguard.exe",
    // OTX.GNG Core
    "otx-core.exe",
];

pub fn is_protected(exe: &str) -> bool {
    let e = exe.to_ascii_lowercase();
    PROTECTED.contains(&e.as_str())
        || e.starts_with("easyanticheat")
        || e.starts_with("beservice")
        || e.contains("anticheat")
}

pub fn category_of(exe: &str, session_id: u32, path: Option<&str>) -> ProcessCategory {
    if let Some(k) = lookup(exe) {
        return match k.category.as_str() {
            "game" => ProcessCategory::Game,
            "launcher" => ProcessCategory::Launcher,
            "browser" => ProcessCategory::Browser,
            "voice" => ProcessCategory::Voice,
            "system" => ProcessCategory::System,
            _ => ProcessCategory::Background,
        };
    }
    if session_id == 0 {
        return ProcessCategory::System;
    }
    match path {
        Some(p) if p.to_ascii_lowercase().starts_with(r"c:\windows\") => ProcessCategory::System,
        Some(_) => ProcessCategory::Background,
        None => ProcessCategory::Unknown,
    }
}

pub fn safe_to_close(exe: &str, category: ProcessCategory) -> SafeToClose {
    if is_protected(exe) {
        return SafeToClose::No;
    }
    if let Some(k) = lookup(exe) {
        return match k.safe.as_str() {
            "yes" => SafeToClose::Yes,
            "no" => SafeToClose::No,
            _ => SafeToClose::Caution,
        };
    }
    match category {
        ProcessCategory::System => SafeToClose::No,
        _ => SafeToClose::Caution,
    }
}

/// "Resource hog": a background app using a lot of CPU, memory or I/O.
pub fn is_hog(category: ProcessCategory, cpu: f32, memory_mb: f32, io_kbs: f32) -> bool {
    !matches!(category, ProcessCategory::Game | ProcessCategory::System) && (cpu >= 5.0 || memory_mb >= 1500.0 || io_kbs >= 5000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knowledge_file_parses_and_is_consistent() {
        let k = knowledge();
        assert!(k.len() > 80, "knowledge file should load ({} entries)", k.len());
        for (exe, e) in k {
            assert_eq!(exe, &exe.to_ascii_lowercase(), "keys must be lower-case");
            assert!(["game", "launcher", "browser", "voice", "system", "background"].contains(&e.category.as_str()), "{exe}");
            assert!(["yes", "caution", "no"].contains(&e.safe.as_str()), "{exe}");
            // Anything protected must never be advertised as safe to close.
            if is_protected(exe) {
                assert_ne!(e.safe, "yes", "{exe} is protected but marked safe");
            }
        }
    }

    #[test]
    fn protected_processes() {
        for p in ["lsass.exe", "CSRSS.EXE", "svchost.exe", "audiodg.exe", "vgc.exe", "EasyAntiCheat_EOS.exe", "BEService_x64.exe", "otx-core.exe", "SomeGameAntiCheat.exe"] {
            assert!(is_protected(p), "{p}");
        }
        for p in ["chrome.exe", "Discord.exe", "steam.exe", "notepad.exe", "cs2.exe"] {
            assert!(!is_protected(p), "{p}");
        }
    }

    #[test]
    fn categories_and_safety() {
        assert_eq!(category_of("Chrome.exe", 1, None), ProcessCategory::Browser);
        assert_eq!(category_of("cs2.exe", 1, None), ProcessCategory::Game);
        assert_eq!(category_of("mystery.exe", 0, None), ProcessCategory::System);
        assert_eq!(category_of("tool.exe", 1, Some(r"C:\Windows\System32\tool.exe")), ProcessCategory::System);
        assert_eq!(category_of("tool.exe", 1, Some(r"C:\Apps\tool.exe")), ProcessCategory::Background);
        assert_eq!(category_of("tool.exe", 1, None), ProcessCategory::Unknown);
        assert_eq!(safe_to_close("lsass.exe", ProcessCategory::System), SafeToClose::No);
        assert_eq!(safe_to_close("spotify.exe", ProcessCategory::Background), SafeToClose::Yes);
        assert!(is_hog(ProcessCategory::Browser, 6.0, 100.0, 0.0));
        assert!(!is_hog(ProcessCategory::Game, 60.0, 9000.0, 0.0));
    }
}
