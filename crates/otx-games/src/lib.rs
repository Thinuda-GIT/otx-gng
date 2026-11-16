//! Game library detection. Read-only: launcher metadata files and the registry are read, game
//! files are never opened or modified. Every parser is unit-tested with sample data.

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DetectedGame {
    /// Stable id: "<launcher>:<launcher's own id>".
    pub id: String,
    pub name: String,
    pub launcher: String,
    pub install_path: String,
    /// Main executable file name when it could be determined (used to detect the running game).
    pub exe: Option<String>,
    pub size_gb: Option<f64>,
    /// How to start it: a launcher URI, or the exe path.
    pub launch: Option<String>,
    pub anti_cheat: Option<String>,
}

/* ------------------------------ Steam ------------------------------ */

/// Minimal Valve KeyValues reader: returns every `"key" "value"` pair (any depth), in order.
pub fn vdf_pairs(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('"').collect();
        // "key"  "value" → ["", key, ws, value, ""]
        if parts.len() >= 5 && parts[2].trim().is_empty() {
            out.push((parts[1].to_string(), parts[3].replace("\\\\", "\\")));
        }
    }
    out
}

/// Lower-case, backslashes, no trailing separator: `c:/Steam/` and `C:\steam` compare equal.
pub fn norm(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_ascii_lowercase()
}

pub fn steam_libraries(steam_root: &Path) -> Vec<PathBuf> {
    let mut libs = vec![PathBuf::from(steam_root.to_string_lossy().replace('/', "\\"))];
    if let Ok(t) = std::fs::read_to_string(steam_root.join("steamapps").join("libraryfolders.vdf")) {
        for (k, v) in vdf_pairs(&t) {
            if k == "path" {
                let p = PathBuf::from(v);
                if !libs.iter().any(|l| norm(&l.to_string_lossy()) == norm(&p.to_string_lossy())) {
                    libs.push(p);
                }
            }
        }
    }
    libs
}

pub fn parse_steam_manifest(text: &str, library: &Path) -> Option<DetectedGame> {
    let kv: HashMap<String, String> = vdf_pairs(text).into_iter().collect();
    let appid = kv.get("appid")?;
    let name = kv.get("name")?.clone();
    let installdir = kv.get("installdir")?;
    // Skip runtimes and tools that Steam installs as "apps".
    let lname = name.to_ascii_lowercase();
    if ["steamworks common redistributables", "proton", "steam linux runtime", "directx"].iter().any(|x| lname.starts_with(x)) {
        return None;
    }
    let install = library.join("steamapps").join("common").join(installdir);
    Some(DetectedGame {
        id: format!("steam:{appid}"),
        name,
        launcher: "steam".into(),
        install_path: install.to_string_lossy().into_owned(),
        exe: None,
        size_gb: kv.get("SizeOnDisk").and_then(|s| s.parse::<f64>().ok()).map(|b| b / 1_073_741_824.0),
        launch: Some(format!("steam://rungameid/{appid}")),
        anti_cheat: None,
    })
}

fn steam_root() -> Option<PathBuf> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(r"Software\Valve\Steam", KEY_READ)
        .and_then(|k| k.get_value::<String, _>("SteamPath"))
        .or_else(|_| RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(r"SOFTWARE\WOW6432Node\Valve\Steam", KEY_READ).and_then(|k| k.get_value::<String, _>("InstallPath")))
        .ok()
        .map(PathBuf::from)
}

pub fn steam_games() -> Vec<DetectedGame> {
    let Some(root) = steam_root() else { return Vec::new() };
    let mut out = Vec::new();
    for lib in steam_libraries(&root) {
        let Ok(dir) = std::fs::read_dir(lib.join("steamapps")) else { continue };
        for e in dir.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("appmanifest_") && n.ends_with(".acf") {
                if let Some(g) = std::fs::read_to_string(e.path()).ok().and_then(|t| parse_steam_manifest(&t, &lib)) {
                    out.push(g);
                }
            }
        }
    }
    out
}

/* ------------------------------ Epic ------------------------------ */

pub fn parse_epic_item(json: &str) -> Option<DetectedGame> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    let app = s("AppName")?;
    if v.get("bIsApplication").and_then(|b| b.as_bool()) == Some(false) {
        return None; // plugins / DLC
    }
    let install = s("InstallLocation")?;
    let exe_rel = s("LaunchExecutable");
    let launch = match (s("CatalogNamespace"), s("CatalogItemId")) {
        (Some(ns), Some(item)) => Some(format!("com.epicgames.launcher://apps/{ns}%3A{item}%3A{app}?action=launch&silent=true")),
        _ => None,
    };
    Some(DetectedGame {
        id: format!("epic:{app}"),
        name: s("DisplayName").unwrap_or_else(|| app.clone()),
        launcher: "epic".into(),
        exe: exe_rel.as_deref().and_then(|e| Path::new(e).file_name()).map(|f| f.to_string_lossy().into_owned()),
        install_path: install,
        size_gb: v.get("InstallSize").and_then(|x| x.as_f64()).map(|b| b / 1_073_741_824.0),
        launch,
        anti_cheat: None,
    })
}

pub fn epic_games() -> Vec<DetectedGame> {
    let dir = PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into())).join(r"Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    rd.flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| parse_epic_item(&t))
        .collect()
}

/* ------------------------------ Riot ------------------------------ */

/// Reads `product_install_full_path` from a Riot product_settings.yaml.
pub fn riot_install_path(yaml: &str) -> Option<String> {
    yaml.lines().find_map(|l| {
        let l = l.trim();
        l.strip_prefix("product_install_full_path:").map(|v| v.trim().trim_matches('"').replace('/', "\\"))
    })
}

pub fn riot_games() -> Vec<DetectedGame> {
    let pd = PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into())).join("Riot Games");
    let client = std::fs::read_to_string(pd.join("RiotClientInstalls.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("rc_default").and_then(|x| x.as_str()).map(str::to_string));
    let mut out = Vec::new();
    let products = [("valorant", "Valorant", "VALORANT-Win64-Shipping.exe", "Riot Vanguard"), ("league_of_legends", "League of Legends", "League of Legends.exe", "Riot Vanguard")];
    for (key, name, exe, ac) in products {
        let f = pd.join("Metadata").join(format!("{key}.live")).join(format!("{key}.live.product_settings.yaml"));
        if let Some(path) = std::fs::read_to_string(f).ok().and_then(|t| riot_install_path(&t)) {
            out.push(DetectedGame {
                id: format!("riot:{key}"),
                name: name.into(),
                launcher: "riot".into(),
                install_path: path,
                exe: Some(exe.into()),
                size_gb: None,
                launch: client.as_ref().map(|c| format!("{c}|--launch-product={key} --launch-patchline=live")),
                anti_cheat: Some(ac.into()),
            });
        }
    }
    out
}

/* ------------------- Installed programs (Battle.net, EA, Ubisoft) ------------------- */

pub fn publisher_launcher(publisher: &str) -> Option<&'static str> {
    let p = publisher.to_ascii_lowercase();
    if p.contains("blizzard") {
        Some("battlenet")
    } else if p.contains("electronic arts") {
        Some("ea")
    } else if p.contains("ubisoft") {
        Some("ubisoft")
    } else {
        None
    }
}

pub fn registry_games() -> Vec<DetectedGame> {
    let mut out = Vec::new();
    for path in [r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"] {
        let Ok(root) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(path, KEY_READ) else { continue };
        for sub in root.enum_keys().flatten() {
            let Ok(k) = root.open_subkey_with_flags(&sub, KEY_READ) else { continue };
            let publisher: String = k.get_value("Publisher").unwrap_or_default();
            let Some(launcher) = publisher_launcher(&publisher) else { continue };
            let name: String = k.get_value("DisplayName").unwrap_or_default();
            let install: String = k.get_value("InstallLocation").unwrap_or_default();
            let ln = name.to_ascii_lowercase();
            // The launchers themselves are not games.
            if name.is_empty() || install.is_empty() || ["battle.net", "ea app", "origin", "ubisoft connect", "uplay"].iter().any(|x| ln == *x || ln.starts_with(x)) {
                continue;
            }
            out.push(DetectedGame {
                id: format!("{launcher}:{sub}"),
                name,
                launcher: launcher.into(),
                install_path: install,
                exe: None,
                size_gb: k.get_value::<u32, _>("EstimatedSize").ok().map(|kb| kb as f64 / 1_048_576.0),
                launch: None,
                anti_cheat: None,
            });
        }
    }
    out
}

/* ------------------------------ Exe resolution ------------------------------ */

fn is_helper_exe(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["unins", "crash", "setup", "redist", "vc_", "dxsetup", "launcher", "helper", "report", "updater", "easyanticheat", "be_service", "prereq"].iter().any(|x| n.contains(x))
}

/// Finds the main exe: a known game exe from the knowledge file anywhere within 3 levels,
/// otherwise the largest non-helper exe in the top two levels.
pub fn resolve_exe(install: &Path) -> Option<String> {
    let mut found: Vec<(PathBuf, u64)> = Vec::new();
    fn walk(dir: &Path, depth: u32, out: &mut Vec<(PathBuf, u64)>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && depth < 3 {
                walk(&p, depth + 1, out);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")) {
                out.push((p.clone(), e.metadata().map(|m| m.len()).unwrap_or(0)));
            }
            if out.len() > 400 {
                return;
            }
        }
    }
    walk(install, 0, &mut found);
    let name = |p: &PathBuf| p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some((p, _)) = found.iter().find(|(p, _)| otx_monitor::policy::lookup(&name(p)).is_some_and(|k| k.category == "game")) {
        return Some(name(p));
    }
    found
        .iter()
        .filter(|(p, _)| !is_helper_exe(&name(p)) && p.strip_prefix(install).map(|r| r.components().count() <= 3).unwrap_or(false))
        .max_by_key(|(_, len)| *len)
        .map(|(p, _)| name(p))
}

/// Full path of `exe` inside `install` (up to three levels deep).
pub fn find_exe_path(install: &Path, exe: &str) -> Option<PathBuf> {
    fn walk(dir: &Path, exe: &str, depth: u32) -> Option<PathBuf> {
        let rd = std::fs::read_dir(dir).ok()?;
        let mut dirs = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                dirs.push(p);
            } else if p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(exe)) {
                return Some(p);
            }
        }
        if depth < 3 {
            for d in dirs {
                if let Some(p) = walk(&d, exe, depth + 1) {
                    return Some(p);
                }
            }
        }
        None
    }
    walk(install, exe, 0)
}

/// Known kernel anti-cheats by exe (detection is by name only; nothing is inspected).
pub fn anti_cheat_for(exe: &str) -> Option<&'static str> {
    match exe.to_ascii_lowercase().as_str() {
        "valorant-win64-shipping.exe" | "league of legends.exe" => Some("Riot Vanguard"),
        "fortniteclient-win64-shipping.exe" | "r5apex.exe" | "r5apex_dx12.exe" | "marvel-win64-shipping.exe" => Some("Easy Anti-Cheat"),
        "cod.exe" => Some("Ricochet"),
        "rainbowsix.exe" | "tslgame.exe" | "gta5.exe" | "gta5_enhanced.exe" => Some("BattlEye"),
        "cs2.exe" | "dota2.exe" | "deadlock.exe" => Some("VAC"),
        "robloxplayerbeta.exe" => Some("Hyperion"),
        _ => None,
    }
}

/// Everything, with exes resolved and duplicates (same install folder) removed.
pub fn scan() -> Vec<DetectedGame> {
    let mut all = Vec::new();
    all.extend(steam_games());
    all.extend(epic_games());
    all.extend(riot_games());
    all.extend(registry_games());
    let mut seen = std::collections::HashSet::new();
    all.retain(|g| seen.insert(norm(&g.install_path)));
    for g in &mut all {
        if g.exe.is_none() {
            g.exe = resolve_exe(Path::new(&g.install_path));
        }
        if g.anti_cheat.is_none() {
            g.anti_cheat = g.exe.as_deref().and_then(anti_cheat_for).map(str::to_string);
        }
    }
    all.retain(|g| Path::new(&g.install_path).is_dir());
    all.sort_by(|a, b| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()));
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steam_files_parse() {
        let lib = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
        let pairs = vdf_pairs(lib);
        assert!(pairs.contains(&("path".into(), r"D:\SteamLibrary".into())));
        let acf = r#""AppState"
{
	"appid"		"730"
	"name"		"Counter-Strike 2"
	"installdir"		"Counter-Strike Global Offensive"
	"SizeOnDisk"		"35546939806"
}"#;
        let g = parse_steam_manifest(acf, Path::new(r"D:\SteamLibrary")).unwrap();
        assert_eq!(g.id, "steam:730");
        assert_eq!(g.launch.as_deref(), Some("steam://rungameid/730"));
        assert!(g.install_path.ends_with(r"steamapps\common\Counter-Strike Global Offensive"));
        assert!((g.size_gb.unwrap() - 33.1).abs() < 0.1);
        assert!(parse_steam_manifest(&acf.replace("Counter-Strike 2", "Steamworks Common Redistributables"), Path::new("x")).is_none());
    }

    #[test]
    fn epic_and_riot_files_parse() {
        let item = r#"{"AppName":"Fortnite","DisplayName":"Fortnite","InstallLocation":"C:\\Program Files\\Epic Games\\Fortnite","LaunchExecutable":"FortniteGame/Binaries/Win64/FortniteClient-Win64-Shipping.exe","CatalogNamespace":"fn","CatalogItemId":"4fe75","bIsApplication":true,"InstallSize":96000000000}"#;
        let g = parse_epic_item(item).unwrap();
        assert_eq!(g.exe.as_deref(), Some("FortniteClient-Win64-Shipping.exe"));
        assert_eq!(g.launch.as_deref(), Some("com.epicgames.launcher://apps/fn%3A4fe75%3AFortnite?action=launch&silent=true"));
        assert!(parse_epic_item(r#"{"AppName":"x","InstallLocation":"y","bIsApplication":false}"#).is_none());
        let yaml = "product_install_full_path: \"C:/Riot Games/VALORANT/live\"\nproduct_install_root: \"C:/Riot Games\"\n";
        assert_eq!(riot_install_path(yaml).as_deref(), Some(r"C:\Riot Games\VALORANT\live"));
        assert_eq!(publisher_launcher("Blizzard Entertainment"), Some("battlenet"));
        assert_eq!(publisher_launcher("Microsoft"), None);
    }

    #[test]
    fn exe_resolution_prefers_known_games_then_largest() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("bin/win64")).unwrap();
        std::fs::write(d.path().join("unins000.exe"), vec![0u8; 5000]).unwrap();
        std::fs::write(d.path().join("tool.exe"), vec![0u8; 100]).unwrap();
        std::fs::write(d.path().join("bin/win64/game.exe"), vec![0u8; 3000]).unwrap();
        assert_eq!(resolve_exe(d.path()).as_deref(), Some("game.exe"));
        std::fs::write(d.path().join("bin/win64/cs2.exe"), vec![0u8; 10]).unwrap();
        assert_eq!(resolve_exe(d.path()).as_deref(), Some("cs2.exe"));
        assert_eq!(anti_cheat_for("VALORANT-Win64-Shipping.exe"), Some("Riot Vanguard"));
    }

    #[test]
    fn scan_runs_on_this_pc() {
        for g in scan() {
            println!("{} | {} | {:?} | {:?}", g.launcher, g.name, g.exe, g.launch);
        }
    }
}
