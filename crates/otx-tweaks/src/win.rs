//! Real Windows implementation of `Sys`. Machine scope runs inside the Core (LocalSystem);
//! user scope runs inside the app (the signed-in user's HKCU and session).

use crate::engine::Sys;
use crate::model::{RegValue, Root, Scope, SpiSetting, StartType};
use windows::core::{GUID, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HLOCAL, LocalFree, ERROR_SUCCESS};
use windows::Win32::System::Power::{PowerDuplicateScheme, PowerGetActiveScheme, PowerReadACValueIndex, PowerSetActiveScheme, PowerWriteACValueIndex};
use windows::Win32::System::Services::{
    ChangeServiceConfig2W, ChangeServiceConfigW, CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, QueryServiceConfig2W,
    QueryServiceConfigW, StartServiceW, ENUM_SERVICE_TYPE, QUERY_SERVICE_CONFIGW, SC_HANDLE, SC_MANAGER_CONNECT, SERVICE_AUTO_START,
    SERVICE_CHANGE_CONFIG, SERVICE_CONFIG_DELAYED_AUTO_START_INFO, SERVICE_CONTROL_STOP, SERVICE_DELAYED_AUTO_START_INFO, SERVICE_DEMAND_START,
    SERVICE_DISABLED, SERVICE_ERROR, SERVICE_NO_CHANGE, SERVICE_QUERY_CONFIG, SERVICE_START, SERVICE_START_TYPE, SERVICE_STATUS, SERVICE_STOP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_GETKEYBOARDDELAY, SPI_GETKEYBOARDSPEED, SPI_GETMOUSE, SPI_SETKEYBOARDDELAY,
    SPI_SETKEYBOARDSPEED, SPI_SETMOUSE,
};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_SET_VALUE, REG_DWORD, REG_SZ};
use winreg::{RegKey, RegValue as WinRegValue};

const ULTIMATE: &str = "e9a42b02-d5df-448d-aa00-03f14749eb61";
const HIGH: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
const NET_CLASS: &str = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}";
const OTX_KEY: &str = r"SOFTWARE\OTX.GNG";

pub struct WinSys {
    pub scope: Scope,
}

fn hive(root: Root) -> RegKey {
    RegKey::predef(match root {
        Root::Hklm => HKEY_LOCAL_MACHINE,
        Root::Hkcu => HKEY_CURRENT_USER,
    })
}

pub fn guid_from_str(s: &str) -> Result<GUID, String> {
    let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() != 32 {
        return Err(format!("not a GUID: {s}"));
    }
    u128::from_str_radix(&hex, 16).map(GUID::from_u128).map_err(|e| e.to_string())
}

pub fn guid_to_str(g: &GUID) -> String {
    format!("{g:?}").to_ascii_lowercase()
}

struct Svc(SC_HANDLE);
impl Drop for Svc {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

fn open_service(name: &str, access: u32) -> Result<Option<(Svc, Svc)>, String> {
    unsafe {
        let scm = Svc(OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_CONNECT).map_err(|e| e.to_string())?);
        match OpenServiceW(scm.0, &HSTRING::from(name), access) {
            Ok(h) => Ok(Some((scm, Svc(h)))),
            Err(e) if e.code().0 as u32 == 0x8007_0424 => Ok(None), // ERROR_SERVICE_DOES_NOT_EXIST
            Err(e) => Err(e.to_string()),
        }
    }
}

impl Sys for WinSys {
    fn scope(&self) -> Scope {
        self.scope
    }

    fn reg_get(&self, root: Root, key: &str, name: &str) -> Result<Option<RegValue>, String> {
        let k = match hive(root).open_subkey_with_flags(key, KEY_READ) {
            Ok(k) => k,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        match k.get_raw_value(name) {
            Ok(v) if v.vtype == REG_DWORD && v.bytes.len() >= 4 => Ok(Some(RegValue::Dword(u32::from_le_bytes(v.bytes[..4].try_into().expect("4 bytes"))))),
            Ok(v) if v.vtype == REG_SZ => Ok(Some(RegValue::Sz(k.get_value::<String, _>(name).map_err(|e| e.to_string())?))),
            Ok(v) => Err(format!("{name} has an unexpected registry type {:?}", v.vtype)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn reg_set(&self, root: Root, key: &str, name: &str, value: &RegValue) -> Result<(), String> {
        let (k, _) = hive(root).create_subkey(key).map_err(|e| e.to_string())?;
        match value {
            RegValue::Dword(d) => k.set_raw_value(name, &WinRegValue { vtype: REG_DWORD, bytes: d.to_le_bytes().to_vec().into() }),
            RegValue::Sz(s) => k.set_value(name, s),
        }
        .map_err(|e| e.to_string())
    }

    fn reg_delete(&self, root: Root, key: &str, name: &str) -> Result<(), String> {
        match hive(root).open_subkey_with_flags(key, KEY_SET_VALUE) {
            Ok(k) => match k.delete_value(name) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn service_start(&self, name: &str) -> Result<Option<StartType>, String> {
        let Some((_scm, svc)) = open_service(name, SERVICE_QUERY_CONFIG)? else { return Ok(None) };
        unsafe {
            let mut needed = 0u32;
            let _ = QueryServiceConfigW(svc.0, None, 0, &mut needed);
            let mut buf = vec![0u64; (needed as usize).div_ceil(8).max(8)];
            QueryServiceConfigW(svc.0, Some(buf.as_mut_ptr().cast()), (buf.len() * 8) as u32, &mut needed).map_err(|e| e.to_string())?;
            let cfg = &*(buf.as_ptr() as *const QUERY_SERVICE_CONFIGW);
            let mut delayed = SERVICE_DELAYED_AUTO_START_INFO::default();
            let mut n = 0u32;
            let _ = QueryServiceConfig2W(
                svc.0,
                SERVICE_CONFIG_DELAYED_AUTO_START_INFO,
                Some(std::slice::from_raw_parts_mut((&mut delayed as *mut SERVICE_DELAYED_AUTO_START_INFO).cast(), std::mem::size_of::<SERVICE_DELAYED_AUTO_START_INFO>())),
                &mut n,
            );
            Ok(Some(match cfg.dwStartType {
                SERVICE_AUTO_START if delayed.fDelayedAutostart.as_bool() => StartType::AutomaticDelayed,
                SERVICE_AUTO_START => StartType::Automatic,
                SERVICE_DISABLED => StartType::Disabled,
                _ => StartType::Manual,
            }))
        }
    }

    fn set_service_start(&self, name: &str, start: StartType) -> Result<(), String> {
        let (_scm, svc) = open_service(name, SERVICE_CHANGE_CONFIG | SERVICE_QUERY_CONFIG | SERVICE_STOP | SERVICE_START)?
            .ok_or_else(|| format!("the {name} service is not installed"))?;
        let t: SERVICE_START_TYPE = match start {
            StartType::Automatic | StartType::AutomaticDelayed => SERVICE_AUTO_START,
            StartType::Manual => SERVICE_DEMAND_START,
            StartType::Disabled => SERVICE_DISABLED,
        };
        unsafe {
            ChangeServiceConfigW(
                svc.0,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                t,
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                PCWSTR::null(),
                PCWSTR::null(),
                None,
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
                PCWSTR::null(),
            )
            .map_err(|e| e.to_string())?;
            if matches!(start, StartType::Automatic | StartType::AutomaticDelayed) {
                let mut d = SERVICE_DELAYED_AUTO_START_INFO { fDelayedAutostart: (start == StartType::AutomaticDelayed).into() };
                let _ = ChangeServiceConfig2W(svc.0, SERVICE_CONFIG_DELAYED_AUTO_START_INFO, Some((&mut d as *mut SERVICE_DELAYED_AUTO_START_INFO).cast()));
                let _ = StartServiceW(svc.0, None); // bring it back now, not only after a reboot
            }
            if start == StartType::Disabled {
                let mut st = SERVICE_STATUS::default();
                let _ = ControlService(svc.0, SERVICE_CONTROL_STOP, &mut st);
            }
        }
        Ok(())
    }

    fn active_scheme(&self) -> Result<String, String> {
        unsafe {
            let mut p: *mut GUID = std::ptr::null_mut();
            let r = PowerGetActiveScheme(None, &mut p);
            if r != ERROR_SUCCESS || p.is_null() {
                return Err(format!("PowerGetActiveScheme failed ({})", r.0));
            }
            let s = guid_to_str(&*p);
            let _ = LocalFree(Some(HLOCAL(p.cast())));
            Ok(s)
        }
    }

    fn set_active_scheme(&self, scheme: &str) -> Result<(), String> {
        let g = guid_from_str(scheme)?;
        let r = unsafe { PowerSetActiveScheme(None, Some(&g)) };
        if r == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(format!("this power plan is not available ({})", r.0))
        }
    }

    fn performance_scheme(&self) -> Result<String, String> {
        // Reuse the plan we created earlier instead of duplicating it on every apply.
        if let Ok(k) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(OTX_KEY, KEY_READ) {
            if let Ok(s) = k.get_value::<String, _>("UltimateScheme") {
                let g = guid_from_str(&s)?;
                let mut v = 0u32;
                let exists = unsafe { PowerReadACValueIndex(None, Some(&g), None, None, &mut v) } == ERROR_SUCCESS;
                if exists {
                    return Ok(s);
                }
            }
        }
        let src = guid_from_str(ULTIMATE)?;
        let mut out: *mut GUID = std::ptr::null_mut();
        let r = unsafe { PowerDuplicateScheme(None, &src, &mut out) };
        if r == ERROR_SUCCESS && !out.is_null() {
            let s = unsafe { guid_to_str(&*out) };
            unsafe {
                let _ = LocalFree(Some(HLOCAL(out.cast())));
            }
            if let Ok((k, _)) = RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey(OTX_KEY) {
                let _ = k.set_value("UltimateScheme", &s);
            }
            return Ok(s);
        }
        // Some laptops (Modern Standby) cannot add plans; fall back to High performance.
        Ok(HIGH.to_string())
    }

    fn power_ac(&self, scheme: &str, subgroup: &str, setting: &str) -> Result<u32, String> {
        let (s, g, t) = (guid_from_str(scheme)?, guid_from_str(subgroup)?, guid_from_str(setting)?);
        let mut v = 0u32;
        let r = unsafe { PowerReadACValueIndex(None, Some(&s), Some(&g), Some(&t), &mut v) };
        if r == ERROR_SUCCESS {
            Ok(v)
        } else {
            Err(format!("power setting not available ({})", r.0))
        }
    }

    fn set_power_ac(&self, scheme: &str, subgroup: &str, setting: &str, value: u32) -> Result<(), String> {
        let (s, g, t) = (guid_from_str(scheme)?, guid_from_str(subgroup)?, guid_from_str(setting)?);
        let r = unsafe { PowerWriteACValueIndex(None, &s, Some(&g), Some(&t), value) };
        if r != ERROR_SUCCESS {
            return Err(format!("could not write the power setting ({})", r.0));
        }
        // Re-activating the scheme makes Windows apply the new value now.
        if self.active_scheme()? == scheme.to_ascii_lowercase() {
            self.set_active_scheme(scheme)?;
        }
        Ok(())
    }

    fn spi_get(&self, setting: SpiSetting) -> Result<Vec<u32>, String> {
        if self.scope != Scope::User {
            return Err("per-user setting".into());
        }
        unsafe {
            match setting {
                SpiSetting::KeyboardDelay | SpiSetting::KeyboardSpeed => {
                    let mut v = 0u32;
                    let a = if setting == SpiSetting::KeyboardDelay { SPI_GETKEYBOARDDELAY } else { SPI_GETKEYBOARDSPEED };
                    SystemParametersInfoW(a, 0, Some((&mut v as *mut u32).cast()), Default::default()).map_err(|e| e.to_string())?;
                    Ok(vec![v])
                }
                SpiSetting::Mouse => {
                    let mut v = [0i32; 3];
                    SystemParametersInfoW(SPI_GETMOUSE, 0, Some(v.as_mut_ptr().cast()), Default::default()).map_err(|e| e.to_string())?;
                    Ok(v.iter().map(|x| *x as u32).collect())
                }
            }
        }
    }

    fn spi_set(&self, setting: SpiSetting, value: &[u32]) -> Result<(), String> {
        if self.scope != Scope::User {
            return Err("per-user setting".into());
        }
        let flags = SPIF_UPDATEINIFILE | SPIF_SENDCHANGE;
        unsafe {
            match setting {
                SpiSetting::KeyboardDelay | SpiSetting::KeyboardSpeed => {
                    let v = *value.first().ok_or("missing value")?;
                    let a = if setting == SpiSetting::KeyboardDelay { SPI_SETKEYBOARDDELAY } else { SPI_SETKEYBOARDSPEED };
                    SystemParametersInfoW(a, v, None, flags).map_err(|e| e.to_string())
                }
                SpiSetting::Mouse => {
                    if value.len() != 3 {
                        return Err("mouse setting needs 3 values".into());
                    }
                    let mut v = [value[0] as i32, value[1] as i32, value[2] as i32];
                    SystemParametersInfoW(SPI_SETMOUSE, 0, Some(v.as_mut_ptr().cast()), flags).map_err(|e| e.to_string())
                }
            }
        }
    }

    fn resolve(&self, placeholder: &str) -> Result<String, String> {
        let guid = otx_sysinfo::primary_adapter_guid().ok_or("no connected network adapter")?;
        match placeholder {
            "$adapterGuid" => Ok(guid),
            "$adapterClassKey" => {
                let class = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(NET_CLASS, KEY_READ).map_err(|e| e.to_string())?;
                for sub in class.enum_keys().filter_map(Result::ok) {
                    if let Ok(k) = class.open_subkey_with_flags(&sub, KEY_READ) {
                        if k.get_value::<String, _>("NetCfgInstanceId").map(|v| v.eq_ignore_ascii_case(&guid)).unwrap_or(false) {
                            return Ok(format!(r"{NET_CLASS}\{sub}"));
                        }
                    }
                }
                Err("the active connection has no physical adapter to configure (for example a dial-up or PPP link)".into())
            }
            other => Err(format!("unknown placeholder {other}")),
        }
    }
}
