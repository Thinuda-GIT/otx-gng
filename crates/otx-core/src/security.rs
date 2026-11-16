//! Windows security helpers: pipe and directory DACLs, client identification, elevation.

use std::path::{Path, PathBuf};
use windows::core::{BOOL, HSTRING, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows::Win32::Security::{
    GetSecurityDescriptorDacl, GetTokenInformation, TokenElevation, TokenUser, ACL, DACL_SECURITY_INFORMATION,
    PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_ELEVATION, TOKEN_QUERY,
    TOKEN_USER,
};
use windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};

/// Pipe DACL:
/// * SYSTEM, Administrators and the pipe's owner: full control (needed to create instances).
/// * Interactive users: read + FILE_WRITE_DATA only (0x12008b = FILE_GENERIC_READ | FILE_WRITE_DATA).
///   Deliberately NOT FILE_APPEND_DATA, which for pipes is FILE_CREATE_PIPE_INSTANCE: a normal
///   user must not be able to create a rogue instance of our pipe.
/// * Network logons get nothing (remote clients are also rejected by the pipe flags).
pub const PIPE_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x12008b;;;IU)";

/// Data directory DACL (ProgramData\OTX.GNG): protected (no inheritance from ProgramData, where
/// normal users may create files), SYSTEM + Administrators full, Users read-only.
pub const DATA_DIR_SDDL: &str = "D:PAI(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;0x1200a9;;;BU)";

/// An owned security descriptor allocated by LocalAlloc inside the SDDL converter.
pub struct SecurityDescriptor(PSECURITY_DESCRIPTOR);

impl SecurityDescriptor {
    pub fn from_sddl(sddl: &str) -> windows::core::Result<Self> {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(&HSTRING::from(sddl), SDDL_REVISION_1, &mut sd, None)? };
        Ok(Self(sd))
    }

    /// SECURITY_ATTRIBUTES pointing at this descriptor. `self` must outlive every use.
    pub fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0 .0,
            bInheritHandle: BOOL(0),
        }
    }

    fn dacl(&self) -> windows::core::Result<*mut ACL> {
        let mut present = BOOL(0);
        let mut defaulted = BOOL(0);
        let mut dacl: *mut ACL = std::ptr::null_mut();
        unsafe { GetSecurityDescriptorDacl(self.0, &mut present, &mut dacl, &mut defaulted)? };
        Ok(dacl)
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0 .0)));
        }
    }
}

// SAFETY: the descriptor is immutable after creation and only read by the OS.
unsafe impl Send for SecurityDescriptor {}
unsafe impl Sync for SecurityDescriptor {}

/// Creates `dir` if needed and applies the protected data-directory DACL (service mode only).
pub fn secure_directory(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let sd = SecurityDescriptor::from_sddl(DATA_DIR_SDDL)?;
    let dacl = sd.dacl()?;
    let err = unsafe {
        SetNamedSecurityInfoW(
            &HSTRING::from(dir.as_os_str()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(dacl),
            None,
        )
    };
    if err.is_err() {
        anyhow::bail!("SetNamedSecurityInfoW failed: {err:?}");
    }
    Ok(())
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// What the kernel says about a process. Nothing here comes from the client's own claims.
#[derive(Debug, Clone)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub session_id: u32,
    pub image: PathBuf,
    pub user_sid: String,
}

pub fn identify_process(pid: u32) -> anyhow::Result<ProcessIdentity> {
    let proc = Handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)? });

    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    unsafe { QueryFullProcessImageNameW(proc.0, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)? };
    let image = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));

    let mut session_id = 0u32;
    unsafe { ProcessIdToSessionId(pid, &mut session_id)? };

    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(proc.0, TOKEN_QUERY, &mut token)? };
    let token = Handle(token);
    let mut needed = 0u32;
    unsafe {
        let _ = GetTokenInformation(token.0, TokenUser, None, 0, &mut needed);
    }
    let mut tu = vec![0u8; needed.max(64) as usize];
    unsafe { GetTokenInformation(token.0, TokenUser, Some(tu.as_mut_ptr().cast()), tu.len() as u32, &mut needed)? };
    // SAFETY: the buffer was sized by the first call and filled with a TOKEN_USER.
    let user = unsafe { &*(tu.as_ptr() as *const TOKEN_USER) };
    let mut sid_str = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user.User.Sid, &mut sid_str)? };
    let user_sid = unsafe { sid_str.to_string() }.unwrap_or_default();
    unsafe {
        let _ = LocalFree(Some(HLOCAL(sid_str.0.cast())));
    }
    Ok(ProcessIdentity { pid, session_id, image, user_sid })
}

pub fn pipe_client_pid(pipe: HANDLE) -> anyhow::Result<u32> {
    let mut pid = 0u32;
    unsafe { GetNamedPipeClientProcessId(pipe, &mut pid)? };
    Ok(pid)
}

pub fn is_elevated() -> bool {
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let token = Handle(token);
        let mut e = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some(&mut e as *mut _ as *mut core::ffi::c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .is_ok()
            && e.TokenIsElevated != 0
    }
}

/// UI executables allowed to talk to the Core in service mode. They must sit in the same
/// directory as the Core executable (Program Files, which only administrators can write).
pub const ALLOWED_UI_EXES: &[&str] = &["OTX.GNG.exe", "otx-gng.exe"];

/// Client policy (pure, unit-tested).
/// * Session 0 (services) is never a valid UI client.
/// * In service mode the client image must be an allowed UI exe in the Core's own directory.
/// * In dev console mode any interactive-session process is accepted (logged loudly).
pub fn client_allowed(id: &ProcessIdentity, core_dir: &Path, dev: bool) -> Result<(), String> {
    if id.session_id == 0 {
        return Err("clients in session 0 are not allowed".into());
    }
    if dev {
        return Ok(());
    }
    let dir_ok = id.image.parent().map(|p| p.eq_ignore_case(core_dir)).unwrap_or(false);
    let name_ok = id
        .image
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| ALLOWED_UI_EXES.iter().any(|a| a.eq_ignore_ascii_case(n)))
        .unwrap_or(false);
    if dir_ok && name_ok {
        Ok(())
    } else {
        Err(format!("client {} is not the installed OTX.GNG app", id.image.display()))
    }
}

trait EqIgnoreCase {
    fn eq_ignore_case(&self, other: &Path) -> bool;
}
impl EqIgnoreCase for Path {
    fn eq_ignore_case(&self, other: &Path) -> bool {
        self.to_string_lossy().trim_end_matches('\\').eq_ignore_ascii_case(other.to_string_lossy().trim_end_matches('\\'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(session: u32, image: &str) -> ProcessIdentity {
        ProcessIdentity { pid: 1, session_id: session, image: image.into(), user_sid: "S-1-5-21-1".into() }
    }

    #[test]
    fn client_policy() {
        let core = Path::new(r"C:\Program Files\OTX.GNG");
        assert!(client_allowed(&id(1, r"C:\Program Files\OTX.GNG\OTX.GNG.exe"), core, false).is_ok());
        assert!(client_allowed(&id(1, r"c:\program files\otx.gng\otx-gng.exe"), core, false).is_ok());
        assert!(client_allowed(&id(1, r"C:\Users\x\Downloads\OTX.GNG.exe"), core, false).is_err());
        assert!(client_allowed(&id(1, r"C:\Program Files\OTX.GNG\evil.exe"), core, false).is_err());
        assert!(client_allowed(&id(0, r"C:\Program Files\OTX.GNG\OTX.GNG.exe"), core, false).is_err());
        assert!(client_allowed(&id(1, r"C:\anything.exe"), core, true).is_ok());
        assert!(client_allowed(&id(0, r"C:\anything.exe"), core, true).is_err());
    }

    #[test]
    fn sddl_strings_parse() {
        assert!(SecurityDescriptor::from_sddl(PIPE_SDDL).is_ok());
        assert!(SecurityDescriptor::from_sddl(DATA_DIR_SDDL).is_ok());
    }

    #[test]
    fn identifies_own_process() {
        let me = identify_process(std::process::id()).unwrap();
        assert!(me.image.exists());
        assert!(me.user_sid.starts_with("S-1-5-"));
    }
}
