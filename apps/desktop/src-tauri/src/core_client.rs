//! Named-pipe client for the OTX.GNG Core service.
//!
//! Before trusting the pipe, release builds verify that the process serving it is the
//! `otx-core.exe` installed next to this app (Program Files, admin-write-only). That defeats
//! pipe-name squatting by another program. Debug builds skip the check so `otx-core console --dev`
//! works from the source tree.

use otx_protocol::{
    check_len, decode, encode, CoreConnection, CoreLink, ErrorCode, Request, RequestFrame, Response, ResponseBody,
    ResponseFrame, PIPE_NAME, PROTOCOL_VERSION,
};
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
use tokio::sync::Mutex;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_BUSY, HANDLE};
use windows::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};

const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct ClientError {
    pub link: CoreLink,
    pub message: String,
}

impl ClientError {
    fn new(link: CoreLink, message: impl Into<String>) -> Self {
        Self { link, message: message.into() }
    }
}

struct Conn {
    pipe: NamedPipeClient,
    next_id: u32,
}

/// One cached connection, re-established on demand.
#[derive(Default)]
pub struct CoreClient {
    conn: Mutex<Option<Conn>>,
}

fn server_image(pipe: &NamedPipeClient) -> Result<PathBuf, String> {
    unsafe {
        let mut pid = 0u32;
        GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut pid).map_err(|e| e.to_string())?;
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).map_err(|e| e.to_string())?;
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.map_err(|e| e.to_string())?;
        Ok(PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
    }
}

fn expected_core_path() -> Option<PathBuf> {
    std::env::current_exe().ok()?.parent().map(|d| d.join("otx-core.exe"))
}

async fn write_frame(pipe: &mut NamedPipeClient, f: &RequestFrame) -> Result<(), ClientError> {
    let bytes = encode(f).map_err(|e| ClientError::new(CoreLink::Error, e.to_string()))?;
    pipe.write_all(&bytes).await.map_err(|e| ClientError::new(CoreLink::Error, e.to_string()))
}

async fn read_frame(pipe: &mut NamedPipeClient) -> Result<ResponseFrame, ClientError> {
    let io = |e: std::io::Error| ClientError::new(CoreLink::Error, e.to_string());
    let mut prefix = [0u8; 4];
    pipe.read_exact(&mut prefix).await.map_err(io)?;
    let len = check_len(prefix).map_err(|e| ClientError::new(CoreLink::Error, e.to_string()))?;
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body).await.map_err(io)?;
    decode(&body).map_err(|e| ClientError::new(CoreLink::Error, e.to_string()))
}

async fn connect() -> Result<Conn, ClientError> {
    let mut tries = 0;
    let mut pipe = loop {
        match ClientOptions::new().open(PIPE_NAME) {
            Ok(p) => break p,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) && tries < 10 => {
                tries += 1;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(ClientError::new(CoreLink::NotRunning, "The OTX.GNG Core service is not running."));
            }
            Err(e) => return Err(ClientError::new(CoreLink::Error, e.to_string())),
        }
    };

    if !cfg!(debug_assertions) {
        let actual = server_image(&pipe).map_err(|e| ClientError::new(CoreLink::Untrusted, e))?;
        let expected = expected_core_path().ok_or_else(|| ClientError::new(CoreLink::Untrusted, "cannot locate otx-core.exe"))?;
        if !actual.to_string_lossy().eq_ignore_ascii_case(&expected.to_string_lossy()) {
            return Err(ClientError::new(
                CoreLink::Untrusted,
                format!("The pipe is served by {} instead of the installed Core. Not connecting.", actual.display()),
            ));
        }
    }

    let nonce = format!("{:x}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos());
    write_frame(&mut pipe, &RequestFrame { id: 1, request: Request::Hello { protocol: PROTOCOL_VERSION, client_nonce: nonce.clone() } }).await?;
    let reply = read_frame(&mut pipe).await?;
    match reply.body {
        ResponseBody::Ok(Response::Welcome(w)) if w.client_nonce == nonce => Ok(Conn { pipe, next_id: 2 }),
        ResponseBody::Ok(_) => Err(ClientError::new(CoreLink::Error, "unexpected handshake reply")),
        ResponseBody::Err(e) => {
            let link = match e.code {
                ErrorCode::Unauthorized | ErrorCode::VersionMismatch => CoreLink::Rejected,
                _ => CoreLink::Error,
            };
            Err(ClientError::new(link, e.message))
        }
    }
}

impl CoreClient {
    /// Sends one request, reconnecting once if the cached connection broke.
    pub async fn call(&self, request: Request) -> Result<Response, ClientError> {
        let mut guard = self.conn.lock().await;
        for attempt in 0..2 {
            if guard.is_none() {
                *guard = Some(tokio::time::timeout(TIMEOUT, connect()).await.map_err(|_| ClientError::new(CoreLink::Error, "timed out connecting to the Core"))??);
            }
            let conn = guard.as_mut().expect("connected above");
            let id = conn.next_id;
            conn.next_id = conn.next_id.wrapping_add(1).max(2);
            let round_trip = async {
                write_frame(&mut conn.pipe, &RequestFrame { id, request: request.clone() }).await?;
                read_frame(&mut conn.pipe).await
            };
            match tokio::time::timeout(TIMEOUT, round_trip).await {
                Ok(Ok(f)) if f.id == id => {
                    return match f.body {
                        ResponseBody::Ok(r) => Ok(r),
                        ResponseBody::Err(e) => Err(ClientError::new(CoreLink::Connected, format!("{:?}: {}", e.code, e.message))),
                    };
                }
                _ if attempt == 0 => {
                    *guard = None; // stale connection (Core restarted): reconnect once
                }
                Ok(Ok(_)) => return Err(ClientError::new(CoreLink::Error, "response id mismatch")),
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(ClientError::new(CoreLink::Error, "the Core did not answer in time")),
            }
        }
        Err(ClientError::new(CoreLink::Error, "could not reach the Core"))
    }

    pub async fn connection(&self) -> CoreConnection {
        match self.call(Request::GetStatus).await {
            Ok(Response::Status(s)) => CoreConnection { link: CoreLink::Connected, message: None, status: Some(s) },
            Ok(_) => CoreConnection { link: CoreLink::Error, message: Some("unexpected reply".into()), status: None },
            Err(e) => CoreConnection { link: e.link, message: Some(e.message), status: None },
        }
    }
}
