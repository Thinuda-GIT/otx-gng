//! On-demand file facts for the "What is this process?" panel: embedded signature and signer,
//! and whether the exe is registered to start with Windows. Reads files and the registry only.

use otx_protocol::SignatureState;
use windows::core::{GUID, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HWND};
use windows::Win32::Security::Cryptography::{CertGetNameStringW, CERT_NAME_SIMPLE_DISPLAY_TYPE};
use windows::Win32::Security::WinTrust::{
    WTHelperGetProvSignerFromChain, WTHelperProvDataFromStateData, WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2,
    WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO, WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_NONE,
    WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE,
};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
use winreg::RegKey;

const TRUST_E_NOSIGNATURE: i32 = 0x800B_0100_u32 as i32;
const TRUST_E_SUBJECT_FORM_UNKNOWN: i32 = 0x800B_0003_u32 as i32;
const TRUST_E_PROVIDER_UNKNOWN: i32 = 0x800B_0001_u32 as i32;

/// Verifies the embedded Authenticode signature (no network: revocation off, cache-only URLs).
pub fn signature(path: &str) -> (SignatureState, Option<String>) {
    let wpath = HSTRING::from(path);
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: std::mem::size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wpath.as_ptr()),
        hFile: HANDLE::default(),
        pgKnownSubject: std::ptr::null_mut(),
    };
    let mut data = WINTRUST_DATA {
        cbStruct: std::mem::size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action: GUID = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let status = unsafe { WinVerifyTrust(HWND::default(), &mut action, (&mut data as *mut WINTRUST_DATA).cast()) };

    let mut signer = None;
    if status == 0 {
        unsafe {
            let prov = WTHelperProvDataFromStateData(data.hWVTStateData);
            if !prov.is_null() {
                let sgnr = WTHelperGetProvSignerFromChain(prov, 0, false, 0);
                if !sgnr.is_null() && (*sgnr).csCertChain > 0 && !(*sgnr).pasCertChain.is_null() {
                    let cert = (*(*sgnr).pasCertChain).pCert;
                    if !cert.is_null() {
                        let mut name = [0u16; 256];
                        let n = CertGetNameStringW(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, Some(&mut name)) as usize;
                        if n > 1 {
                            signer = Some(String::from_utf16_lossy(&name[..n - 1]));
                        }
                    }
                }
            }
        }
    }
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    unsafe {
        let _ = WinVerifyTrust(HWND::default(), &mut action, (&mut data as *mut WINTRUST_DATA).cast());
    }

    let state = match status {
        0 => SignatureState::Valid,
        TRUST_E_NOSIGNATURE | TRUST_E_SUBJECT_FORM_UNKNOWN | TRUST_E_PROVIDER_UNKNOWN => SignatureState::NoEmbedded,
        s if s == 0x8007_0002_u32 as i32 || s == 0x8007_0005_u32 as i32 => SignatureState::Unknown,
        _ => SignatureState::Invalid,
    };
    (state, signer)
}

/// True when any Run key value mentions this exe path (machine and current user).
pub fn registered_at_startup(path: &str) -> bool {
    let needle = path.to_ascii_lowercase();
    let keys = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
    ];
    keys.iter().any(|(root, sub)| {
        RegKey::predef(*root)
            .open_subkey_with_flags(sub, KEY_READ)
            .map(|k| {
                k.enum_values()
                    .filter_map(Result::ok)
                    .any(|(_, v)| v.to_string().to_ascii_lowercase().contains(&needle))
            })
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_a_signed_system_file_and_a_missing_file() {
        // notepad.exe is catalog-signed on modern Windows; either result is acceptable, but
        // the call must not report "Invalid" for a genuine Windows binary.
        let win = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let (state, _) = signature(&format!(r"{win}\System32\notepad.exe"));
        assert!(matches!(state, SignatureState::Valid | SignatureState::NoEmbedded), "{state:?}");
        let (missing, signer) = signature(r"C:\definitely\not\here.exe");
        assert!(signer.is_none());
        assert_ne!(missing, SignatureState::Valid);
    }
}
