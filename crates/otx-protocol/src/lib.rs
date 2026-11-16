//! OTX.GNG IPC protocol, version 1.
//!
//! Transport: a Windows named pipe (`PIPE_NAME`) in byte mode carrying frames of
//! `u32 little-endian length` + `UTF-8 JSON`. Every frame is at most `MAX_FRAME` bytes.
//!
//! Security model (see docs/SECURITY.md):
//! * The UI sends ONLY typed requests from the [`Request`] enum. There is no "run command",
//!   "write registry path" or "open file" request, and unknown methods fail to deserialize,
//!   so the Core rejects them with [`ErrorCode::NotAllowed`]. That is the allow-list.
//! * The first frame on every connection must be [`Request::Hello`]. The Core authenticates the
//!   client from what the kernel reports about the pipe client (process id → token user SID,
//!   session, and executable path), not from anything the client claims.
//! * Processes are always identified by PID + creation time so a recycled PID can never be hit.
//!
//! TypeScript bindings: `cargo test -p otx-protocol` writes them to
//! `apps/desktop/src/backend/generated/` (configured in `.cargo/config.toml`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Protocol version. The Core refuses a Hello with a different major version.
pub const PROTOCOL_VERSION: u32 = 1;
/// The pipe the Core service listens on.
pub const PIPE_NAME: &str = r"\\.\pipe\otx-gng-core-v1";
/// Largest frame either side accepts (1 MiB). Larger frames close the connection.
pub const MAX_FRAME: usize = 1024 * 1024;
/// The Windows service name (SCM key name) and display name.
pub const SERVICE_NAME: &str = "OTXGNGCore";
pub const SERVICE_DISPLAY_NAME: &str = "OTX.GNG Core";

// ---------------------------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------------------------

/// A client → Core frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct RequestFrame {
    /// Client-chosen id echoed in the response.
    pub id: u32,
    #[serde(flatten)]
    pub request: Request,
}

/// Every request the Core accepts. Adding a capability means adding a variant here, which keeps
/// the allow-list in one reviewed place.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
#[ts(export)]
pub enum Request {
    /// Must be the first frame. `client_nonce` is echoed so the client can match the session.
    Hello {
        protocol: u32,
        #[serde(rename = "clientNonce")]
        client_nonce: String,
    },
    /// Liveness check.
    Ping,
    /// Core version, mode flags and module availability.
    GetStatus,
    /// Hardware and OS facts (read-only).
    GetSystemInfo,
    /// Change a process's CPU priority class. Realtime is not representable.
    #[serde(rename_all = "camelCase")]
    SetProcessPriority { target: ProcessRef, level: PriorityLevel, dry_run: bool },
    /// Turn Windows efficiency mode (EcoQoS power throttling) on or off for a process.
    #[serde(rename_all = "camelCase")]
    SetEfficiencyMode { target: ProcessRef, on: bool, dry_run: bool },
    /// End a process (not reversible; the UI must confirm first).
    #[serde(rename_all = "camelCase")]
    EndProcess { target: ProcessRef, dry_run: bool },
    /// Everything in the change journal.
    ListChanges,
    /// Undo one journaled change.
    RevertChange { id: String },
    /// Undo every journaled change ("Revert everything").
    RevertAll,
}

/// A process identified by PID AND creation time, so a recycled PID can never be targeted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessRef {
    pub pid: u32,
    /// FILETIME of process creation (100 ns ticks since 1601), as a decimal string because
    /// it exceeds the 2^53 range of JavaScript numbers.
    #[serde(with = "u64_string")]
    #[ts(type = "string")]
    pub create_time: u64,
}

/// Priority classes OTX.GNG may set. There is deliberately no Realtime variant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PriorityLevel {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
}

mod u64_string {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// A Core → client frame.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct ResponseFrame {
    /// The request id this answers (0 for frames that could not be parsed).
    pub id: u32,
    #[serde(flatten)]
    pub body: ResponseBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ResponseBody {
    Ok(Response),
    Err(CoreError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
#[ts(export)]
pub enum Response {
    Welcome(Welcome),
    Pong,
    Status(CoreStatus),
    SystemInfo(SystemInfo),
    /// Result of a process action or revert.
    Action(ActionResult),
    Changes(Vec<ChangeEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionResult {
    pub dry_run: bool,
    /// Human-readable description of each value that changed (or would change).
    pub changes: Vec<String>,
    /// Journal id when the change can be reverted.
    pub change_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangeEntry {
    pub id: String,
    #[ts(type = "number")]
    pub created_unix: u64,
    pub title: String,
    pub detail: String,
    /// Temporary changes are reverted automatically when the session ends or after a reboot.
    pub temporary: bool,
}

// ---------------------------------------------------------------------------------------------
// Live monitoring (produced by the unprivileged app; no elevation needed)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GpuEngines {
    pub d3: f32,
    pub compute: f32,
    pub copy: f32,
    pub video_decode: f32,
    pub video_encode: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemMetrics {
    #[ts(type = "number")]
    pub t_ms: u64,
    pub cpu: f32,
    pub per_core: Vec<f32>,
    /// None when the GPU performance counters are unavailable (reason in `notes`).
    pub gpu: Option<GpuEngines>,
    #[ts(type = "number")]
    pub ram_total_mb: u64,
    #[ts(type = "number")]
    pub ram_available_mb: u64,
    #[ts(type = "number")]
    pub commit_mb: u64,
    #[ts(type = "number")]
    pub commit_limit_mb: u64,
    pub standby_mb: Option<f64>,
    pub disk_read_mbs: Option<f32>,
    pub disk_write_mbs: Option<f32>,
    pub disk_active_pct: Option<f32>,
    pub net_down_kbps: f32,
    pub net_up_kbps: f32,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProcessCategory {
    Game,
    Launcher,
    Browser,
    Voice,
    System,
    Background,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SafeToClose {
    Yes,
    Caution,
    No,
}

/// One app: every process with the same executable, merged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessGroup {
    /// Lower-case executable name (the grouping key).
    pub id: String,
    pub name: String,
    pub exe: String,
    pub path: Option<String>,
    pub category: ProcessCategory,
    pub processes: Vec<ProcessRef>,
    /// Share of total CPU capacity (0-100, all cores), like Task Manager.
    pub cpu: f32,
    pub gpu: f32,
    pub gpu_engine: Option<String>,
    /// Private working set ("Memory" in Task Manager).
    pub memory_mb: f32,
    pub working_set_mb: f32,
    /// Private commit.
    pub commit_mb: f32,
    /// All I/O the process performs (files, devices, network), per second.
    pub io_read_kbs: f32,
    pub io_write_kbs: f32,
    /// Base priority class of the main (first) process.
    pub priority: Option<PriorityLevel>,
    pub session_id: u32,
    pub protected: bool,
    pub hog: bool,
    pub description: Option<String>,
    pub safe_to_close: SafeToClose,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SignatureState {
    /// Embedded signature verified by Windows.
    Valid,
    /// Embedded signature present but not trusted (tampered, expired root, untrusted signer).
    Invalid,
    /// No embedded signature. Windows' own files are usually signed through system catalogs
    /// instead, which this check does not cover, so this is NOT proof the file is unsigned.
    NoEmbedded,
    /// The file could not be read.
    Unknown,
}

/// On-demand details for the "What is this process?" panel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessDetails {
    pub path: Option<String>,
    /// Signer from the file's embedded Authenticode signature, when it is valid.
    pub signer: Option<String>,
    pub signature: SignatureState,
    /// Registered to start with Windows (Run keys).
    pub startup: bool,
    pub efficiency_mode: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Welcome {
    pub protocol: u32,
    pub core_version: String,
    pub session_id: String,
    pub client_nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoreError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(export)]
pub enum ErrorCode {
    /// The method is unknown or not allowed (the allow-list rejected it).
    NotAllowed,
    /// The first frame was not a valid Hello, or the client failed identity checks.
    Unauthorized,
    /// Protocol major version mismatch.
    VersionMismatch,
    /// Parameters failed validation.
    Validation,
    /// The feature is not supported on this Windows build or hardware.
    Unsupported,
    /// Safe Mode is on, so privileged modules are disabled.
    SafeMode,
    /// An anti-cheat is running, so the requested driver-based module is disabled.
    AntiCheatActive,
    /// The target process is protected and is never touched.
    ProtectedProcess,
    /// Anything else (details are in the Core log).
    Internal,
}

// ---------------------------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoreStatus {
    pub core_version: String,
    pub protocol: u32,
    /// "service" when running under the Service Control Manager, "console" for development.
    pub mode: CoreMode,
    pub elevated: bool,
    pub safe_mode: bool,
    pub dry_run: bool,
    /// Temporary changes the watchdog reverted at startup (left over from a crash or reboot).
    pub recovered_changes: u32,
    /// JSON numbers are f64 on the JS side; uptime never approaches 2^53 seconds.
    #[ts(type = "number")]
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoreMode {
    Service,
    Console,
}

// ---------------------------------------------------------------------------------------------
// System info (read-only facts; every field is measured, or None when it could not be read)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub os: OsInfo,
    pub cpu: CpuInfo,
    pub gpus: Vec<GpuInfo>,
    #[ts(type = "number")]
    pub ram_mb: u64,
    pub disk: Option<DiskInfo>,
    pub power: PowerInfo,
    pub network: Option<NetworkInfo>,
    /// Programs registered to start with Windows in machine-wide Run keys. The UI adds the
    /// per-user (HKCU) count itself, because the Core runs as SYSTEM and has no HKCU of the user.
    pub startup_apps_machine: u32,
    pub anti_cheats: Vec<AntiCheatInfo>,
    /// Facts that could not be read, with the reason (shown in the UI instead of guessing).
    pub unavailable: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OsInfo {
    /// "Windows 10" or "Windows 11" (derived from the build number, because the registry
    /// ProductName still says "Windows 10" on Windows 11).
    pub name: String,
    pub build: u32,
    pub ubr: u32,
    pub display_version: Option<String>,
    pub edition: Option<String>,
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CpuInfo {
    pub model: String,
    pub cores: u32,
    pub threads: u32,
    pub hybrid: bool,
    pub performance_cores: Option<u32>,
    pub efficiency_cores: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: GpuVendor,
    #[ts(type = "number")]
    pub dedicated_vram_mb: u64,
    pub integrated: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DiskKind {
    Nvme,
    Ssd,
    Hdd,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiskInfo {
    pub model: String,
    pub kind: DiskKind,
    /// The drive Windows is installed on, e.g. "C:".
    pub system_drive: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PowerPlan {
    Ultimate,
    High,
    Balanced,
    Saver,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FormFactor {
    Laptop,
    Desktop,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PowerInfo {
    /// Laptop when a system battery is present. A desktop on a UPS that reports as a system
    /// battery would be misread as a laptop; the UI lets the user correct it.
    pub form_factor: FormFactor,
    pub on_battery: bool,
    pub battery_saver: bool,
    pub battery_percent: Option<u8>,
    pub plan: PowerPlan,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LinkKind {
    Ethernet,
    Wifi,
    /// Mobile broadband (built-in modem or USB dongle).
    Mobile,
    /// A PPP connection: a dial-up profile for a mobile dongle, or PPPoE broadband. Windows
    /// cannot tell these apart, so we do not guess.
    Ppp,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkInfo {
    /// The connected adapter with a default gateway and the lowest route metric.
    pub kind: LinkKind,
    pub name: String,
    pub description: String,
    #[ts(type = "number")]
    pub link_mbps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AntiCheatInfo {
    pub name: String,
    pub service: String,
    pub kernel_driver: bool,
    pub running: bool,
}

// ---------------------------------------------------------------------------------------------
// UI-side types (returned by the desktop app's own Tauri commands, not sent over the pipe)
// ---------------------------------------------------------------------------------------------

/// Where the UI's system facts came from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InfoSource {
    /// Answered by the Core service over the pipe.
    Core,
    /// Collected by the unprivileged app itself because the Core was unreachable.
    Local,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoreLink {
    Connected,
    /// No pipe: the service is not installed or not running.
    NotRunning,
    /// The pipe exists but its server is not the installed otx-core.exe (possible squatting).
    Untrusted,
    /// The Core refused us (wrong exe location, protocol mismatch).
    Rejected,
    /// Any other failure.
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoreConnection {
    pub link: CoreLink,
    pub message: Option<String>,
    pub status: Option<CoreStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemSnapshot {
    pub source: InfoSource,
    pub info: SystemInfo,
    /// Machine-wide plus current-user startup entries.
    pub startup_apps: u32,
}

// ---------------------------------------------------------------------------------------------
// Framing
// ---------------------------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("frame of {0} bytes exceeds the {MAX_FRAME}-byte limit")]
    TooLarge(usize),
    #[error("frame is not valid JSON for this message type: {0}")]
    Json(#[from] serde_json::Error),
}

/// Encodes a message as `len (u32 LE) + JSON`.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let body = serde_json::to_vec(msg)?;
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(body.len()));
    }
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Validates a length prefix read from the wire before any allocation happens.
pub fn check_len(prefix: [u8; 4]) -> Result<usize, FrameError> {
    let len = u32::from_le_bytes(prefix) as usize;
    if len > MAX_FRAME {
        Err(FrameError::TooLarge(len))
    } else {
        Ok(len)
    }
}

/// Decodes a frame body (without the length prefix).
pub fn decode<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<T, FrameError> {
    Ok(serde_json::from_slice(body)?)
}

impl ResponseFrame {
    pub fn ok(id: u32, r: Response) -> Self {
        Self { id, body: ResponseBody::Ok(r) }
    }
    pub fn err(id: u32, code: ErrorCode, message: impl Into<String>) -> Self {
        Self { id, body: ResponseBody::Err(CoreError { code, message: message.into() }) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_wire_format_is_stable() {
        let f = RequestFrame { id: 7, request: Request::GetSystemInfo };
        assert_eq!(serde_json::to_string(&f).unwrap(), r#"{"id":7,"method":"getSystemInfo"}"#);
        let h = RequestFrame { id: 1, request: Request::Hello { protocol: 1, client_nonce: "ab".into() } };
        assert_eq!(
            serde_json::to_string(&h).unwrap(),
            r#"{"id":1,"method":"hello","params":{"protocol":1,"clientNonce":"ab"}}"#
        );
    }

    #[test]
    fn unknown_methods_are_rejected_by_the_type_system() {
        // A request the allow-list does not contain must not deserialize.
        let raw = br#"{"id":3,"method":"runCommand","params":{"cmd":"calc.exe"}}"#;
        assert!(decode::<RequestFrame>(raw).is_err());
        let raw = br#"{"id":3,"method":"setRegistry","params":{"path":"HKLM\\X"}}"#;
        assert!(decode::<RequestFrame>(raw).is_err());
    }

    #[test]
    fn frames_round_trip_and_enforce_the_size_limit() {
        let f = ResponseFrame::err(9, ErrorCode::NotAllowed, "nope");
        let bytes = encode(&f).unwrap();
        let len = check_len(bytes[..4].try_into().unwrap()).unwrap();
        assert_eq!(len, bytes.len() - 4);
        let back: ResponseFrame = decode(&bytes[4..]).unwrap();
        assert_eq!(back, f);
        assert!(check_len(((MAX_FRAME + 1) as u32).to_le_bytes()).is_err());
    }

    #[test]
    fn process_refs_carry_64_bit_create_times_as_strings() {
        let raw = br#"{"id":4,"method":"setProcessPriority","params":{"target":{"pid":42,"createTime":"133795871234567890"},"level":"belowNormal","dryRun":true}}"#;
        let f: RequestFrame = decode(raw).unwrap();
        assert_eq!(
            f.request,
            Request::SetProcessPriority {
                target: ProcessRef { pid: 42, create_time: 133_795_871_234_567_890 },
                level: PriorityLevel::BelowNormal,
                dry_run: true
            }
        );
        // Realtime does not exist in the protocol.
        let rt = br#"{"id":4,"method":"setProcessPriority","params":{"target":{"pid":42,"createTime":"1"},"level":"realtime","dryRun":false}}"#;
        assert!(decode::<RequestFrame>(rt).is_err());
    }

    #[test]
    fn response_wire_format_is_stable() {
        let f = ResponseFrame::ok(2, Response::Pong);
        assert_eq!(serde_json::to_string(&f).unwrap(), r#"{"id":2,"ok":{"kind":"pong"}}"#);
        let e = ResponseFrame::err(2, ErrorCode::SafeMode, "x");
        assert_eq!(serde_json::to_string(&e).unwrap(), r#"{"id":2,"err":{"code":"SAFE_MODE","message":"x"}}"#);
    }
}
