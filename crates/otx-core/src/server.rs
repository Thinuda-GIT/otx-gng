//! Named-pipe server, authentication and the allow-list dispatcher.

use crate::security::{self, SecurityDescriptor};
use crate::state::CoreState;
use otx_protocol::{
    check_len, decode, encode, ErrorCode, Request, RequestFrame, Response, ResponseFrame, Welcome, PIPE_NAME,
    PROTOCOL_VERSION,
};
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::{watch, Semaphore};
use windows::Win32::Foundation::HANDLE;

/// At most this many UI connections at once (the app normally uses one).
const MAX_CLIENTS: usize = 8;
/// A client must send Hello within this time.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

fn create_instance(name: &str, sd: &SecurityDescriptor, first: bool) -> std::io::Result<NamedPipeServer> {
    let mut attrs = sd.attributes();
    // SAFETY: `attrs` points to a descriptor that outlives this call; tokio copies nothing from it later.
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first) // fail if someone else already owns the name (squatting)
            .reject_remote_clients(true)
            .max_instances(MAX_CLIENTS + 1)
            .create_with_security_attributes_raw(name, &mut attrs as *mut _ as *mut core::ffi::c_void)
    }
}

/// Runs until `shutdown` flips to true.
pub async fn serve(state: Arc<CoreState>, shutdown: watch::Receiver<bool>) -> anyhow::Result<()> {
    serve_on(PIPE_NAME, state, shutdown).await
}

/// Same as [serve] on a custom pipe name (used by the integration test).
pub async fn serve_on(name: &str, state: Arc<CoreState>, mut shutdown: watch::Receiver<bool>) -> anyhow::Result<()> {
    let sd = Arc::new(SecurityDescriptor::from_sddl(security::PIPE_SDDL)?);
    let mut server = create_instance(name, &sd, true).map_err(|e| {
        anyhow::anyhow!("cannot create {name} (is another Core already running, or is the name squatted?): {e}")
    })?;
    tracing::info!(pipe = name, "listening");
    let slots = Arc::new(Semaphore::new(MAX_CLIENTS));

    loop {
        tokio::select! {
            r = server.connect() => {
                r?;
                let connected = server;
                server = create_instance(name, &sd, false)?; // keep accepting while this client is served
                let Ok(permit) = slots.clone().try_acquire_owned() else {
                    tracing::warn!("too many clients; dropping connection");
                    continue;
                };
                let st = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(connected, st).await {
                        tracing::debug!(error = %e, "client session ended");
                    }
                    drop(permit);
                });
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("pipe server stopping");
                    return Ok(());
                }
            }
        }
    }
}

async fn read_frame(pipe: &mut NamedPipeServer) -> anyhow::Result<Vec<u8>> {
    let mut prefix = [0u8; 4];
    pipe.read_exact(&mut prefix).await?;
    let len = check_len(prefix)?;
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body).await?;
    Ok(body)
}

async fn write_frame(pipe: &mut NamedPipeServer, f: &ResponseFrame) -> anyhow::Result<()> {
    pipe.write_all(&encode(f)?).await?;
    pipe.flush().await?;
    Ok(())
}

async fn handle_client(mut pipe: NamedPipeServer, state: Arc<CoreState>) -> anyhow::Result<()> {
    // 1) Who is this? Ask the kernel, not the client.
    let raw = HANDLE(pipe.as_raw_handle());
    let identity = security::pipe_client_pid(raw).and_then(security::identify_process);
    let identity = match identity {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(error = %e, "could not identify pipe client");
            return Ok(());
        }
    };
    if let Err(why) = security::client_allowed(&identity, &state.core_dir, state.dev) {
        tracing::warn!(pid = identity.pid, image = %identity.image.display(), %why, "rejected client");
        write_frame(&mut pipe, &ResponseFrame::err(0, ErrorCode::Unauthorized, why)).await?;
        return Ok(());
    }

    // 2) Hello handshake (must be first, within the timeout, matching protocol major version).
    let body = tokio::time::timeout(HELLO_TIMEOUT, read_frame(&mut pipe)).await??;
    let hello: RequestFrame = match decode(&body) {
        Ok(f) => f,
        Err(_) => {
            write_frame(&mut pipe, &ResponseFrame::err(0, ErrorCode::Unauthorized, "first frame must be hello")).await?;
            return Ok(());
        }
    };
    let Request::Hello { protocol, client_nonce } = hello.request else {
        write_frame(&mut pipe, &ResponseFrame::err(hello.id, ErrorCode::Unauthorized, "first frame must be hello")).await?;
        return Ok(());
    };
    if protocol != PROTOCOL_VERSION {
        let msg = format!("Core speaks protocol {PROTOCOL_VERSION}, client sent {protocol}");
        write_frame(&mut pipe, &ResponseFrame::err(hello.id, ErrorCode::VersionMismatch, msg)).await?;
        return Ok(());
    }
    let session_id = random_hex(16);
    tracing::info!(pid = identity.pid, session = %session_id, user = %identity.user_sid, image = %identity.image.display(), "client authenticated");
    let welcome = Welcome {
        protocol: PROTOCOL_VERSION,
        core_version: env!("CARGO_PKG_VERSION").into(),
        session_id,
        client_nonce,
    };
    write_frame(&mut pipe, &ResponseFrame::ok(hello.id, Response::Welcome(welcome))).await?;

    // 3) Request loop. Anything that is not in the Request enum fails to decode → NOT_ALLOWED.
    loop {
        let body = match read_frame(&mut pipe).await {
            Ok(b) => b,
            Err(_) => return Ok(()), // client disconnected
        };
        let reply = match decode::<RequestFrame>(&body) {
            Ok(f) => {
                let id = f.id;
                // Blocking Win32 queries run off the async reactor.
                let st = state.clone();
                tokio::task::spawn_blocking(move || dispatch(&st, f))
                    .await
                    .unwrap_or_else(|_| ResponseFrame::err(id, ErrorCode::Internal, "handler panicked"))
            }
            Err(_) => {
                let id = serde_json::from_slice::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("id")?.as_u64())
                    .unwrap_or(0) as u32;
                tracing::warn!(pid = identity.pid, "rejected a request that is not on the allow-list");
                ResponseFrame::err(id, ErrorCode::NotAllowed, "unknown or malformed request")
            }
        };
        write_frame(&mut pipe, &reply).await?;
    }
}

/// The allow-list dispatcher: one arm per permitted request.
pub fn dispatch(state: &CoreState, f: RequestFrame) -> ResponseFrame {
    tracing::debug!(id = f.id, request = ?f.request, "request");
    match f.request {
        Request::Hello { .. } => ResponseFrame::err(f.id, ErrorCode::Validation, "already authenticated"),
        Request::Ping => ResponseFrame::ok(f.id, Response::Pong),
        Request::GetStatus => ResponseFrame::ok(f.id, Response::Status(state.status())),
        Request::GetSystemInfo => ResponseFrame::ok(f.id, Response::SystemInfo(state.system_info())),
        Request::ListChanges => ResponseFrame::ok(f.id, crate::actions::list_changes(state)),
        Request::SetProcessPriority { target, level, dry_run } => result(f.id, crate::actions::set_priority(state, target, level, dry_run)),
        Request::SetEfficiencyMode { target, on, dry_run } => result(f.id, crate::actions::set_efficiency(state, target, on, dry_run)),
        Request::EndProcess { target, dry_run } => result(f.id, crate::actions::end_process(state, target, dry_run)),
        Request::RevertChange { id } => result(f.id, crate::actions::revert(state, Some(&id))),
        Request::RevertAll => result(f.id, crate::actions::revert(state, None)),
        Request::ListTweaks => ResponseFrame::ok(f.id, crate::boost::list_tweaks(state)),
        Request::SetTweak { id, on, dry_run } => result(f.id, crate::boost::set_tweak(state, &id, on, dry_run)),
        Request::CleanMemory { methods, allow, skip, dry_run } => result(f.id, crate::boost::clean_memory(state, &methods, &allow, &skip, dry_run)),
        Request::SetDns { servers, dry_run } => result(f.id, crate::boost::set_dns(state, &servers, dry_run)),
    }
}

fn result(id: u32, r: Result<Response, crate::process::ActionError>) -> ResponseFrame {
    match r {
        Ok(resp) => ResponseFrame::ok(id, resp),
        Err(e) => ResponseFrame::err(id, e.code, e.message),
    }
}

pub fn random_hex(bytes: usize) -> String {
    use rand::RngCore;
    let mut b = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut b);
    hex::encode(b)
}

#[cfg(test)]
mod tests {
    //! End-to-end IPC over a real named pipe (unique name per test run).
    use super::*;
    use crate::journal::Journal;
    use otx_protocol::{CoreMode, ResponseBody};
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

    async fn send(c: &mut NamedPipeClient, raw: &[u8]) {
        let mut out = (raw.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(raw);
        c.write_all(&out).await.unwrap();
    }

    async fn recv(c: &mut NamedPipeClient) -> ResponseFrame {
        let mut p = [0u8; 4];
        c.read_exact(&mut p).await.unwrap();
        let mut b = vec![0u8; check_len(p).unwrap()];
        c.read_exact(&mut b).await.unwrap();
        decode(&b).unwrap()
    }

    async fn start(dev: bool) -> (String, watch::Sender<bool>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::open(dir.path()).unwrap();
        let core_dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
        let state = Arc::new(CoreState::new(CoreMode::Console, dev, false, true, false, core_dir, dir.path().into(), 0, journal));
        let name = format!(r"\\.\pipe\otx-gng-test-{}", random_hex(8));
        let (tx, rx) = watch::channel(false);
        let n = name.clone();
        tokio::spawn(async move { serve_on(&n, state, rx).await.unwrap() });
        tokio::time::sleep(Duration::from_millis(150)).await;
        (name, tx, dir)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handshake_requests_and_allow_list() {
        let (name, stop, _dir) = start(true).await;
        let mut c = ClientOptions::new().open(&name).unwrap();

        send(&mut c, br#"{"id":1,"method":"hello","params":{"protocol":1,"clientNonce":"n1"}}"#).await;
        match recv(&mut c).await.body {
            ResponseBody::Ok(Response::Welcome(w)) => assert_eq!(w.client_nonce, "n1"),
            other => panic!("expected welcome, got {other:?}"),
        }

        send(&mut c, br#"{"id":2,"method":"ping"}"#).await;
        assert_eq!(recv(&mut c).await, ResponseFrame::ok(2, Response::Pong));

        send(&mut c, br#"{"id":3,"method":"getStatus"}"#).await;
        match recv(&mut c).await.body {
            ResponseBody::Ok(Response::Status(s)) => assert!(s.dry_run && s.mode == CoreMode::Console),
            other => panic!("expected status, got {other:?}"),
        }

        // Not on the allow-list: rejected, and the connection stays usable.
        send(&mut c, br#"{"id":4,"method":"runCommand","params":{"cmd":"calc"}}"#).await;
        let r = recv(&mut c).await;
        assert_eq!(r.id, 4);
        assert!(matches!(r.body, ResponseBody::Err(ref e) if e.code == ErrorCode::NotAllowed));

        send(&mut c, br#"{"id":5,"method":"getSystemInfo"}"#).await;
        match recv(&mut c).await.body {
            ResponseBody::Ok(Response::SystemInfo(i)) => {
                assert!(i.ram_mb > 0, "RAM should be detected on a real machine");
                assert!(i.cpu.threads > 0);
                assert!(i.os.build >= 10240);
            }
            other => panic!("expected system info, got {other:?}"),
        }
        let _ = stop.send(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn first_frame_must_be_hello_and_version_must_match() {
        let (name, stop, _dir) = start(true).await;

        let mut c = ClientOptions::new().open(&name).unwrap();
        send(&mut c, br#"{"id":1,"method":"getSystemInfo"}"#).await;
        assert!(matches!(recv(&mut c).await.body, ResponseBody::Err(ref e) if e.code == ErrorCode::Unauthorized));

        let mut c = ClientOptions::new().open(&name).unwrap();
        send(&mut c, br#"{"id":1,"method":"hello","params":{"protocol":99,"clientNonce":"x"}}"#).await;
        assert!(matches!(recv(&mut c).await.body, ResponseBody::Err(ref e) if e.code == ErrorCode::VersionMismatch));
        let _ = stop.send(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn non_dev_core_rejects_clients_outside_the_install_directory() {
        // The test binary is not OTX.GNG.exe, so a production-mode Core must refuse it.
        let (name, stop, _dir) = start(false).await;
        let mut c = ClientOptions::new().open(&name).unwrap();
        assert!(matches!(recv(&mut c).await.body, ResponseBody::Err(ref e) if e.code == ErrorCode::Unauthorized));
        let _ = stop.send(true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn oversized_frames_are_refused() {
        let (name, stop, _dir) = start(true).await;
        let mut c = ClientOptions::new().open(&name).unwrap();
        send(&mut c, br#"{"id":1,"method":"hello","params":{"protocol":1,"clientNonce":"n"}}"#).await;
        let _ = recv(&mut c).await;
        // Claim a 2 MiB body: the Core must drop the connection without allocating it.
        c.write_all(&((2 * 1024 * 1024) as u32).to_le_bytes()).await.unwrap();
        let mut b = [0u8; 1];
        let closed = matches!(tokio::time::timeout(Duration::from_secs(3), c.read(&mut b)).await, Ok(Ok(0)) | Ok(Err(_)));
        assert!(closed, "connection should be closed after an oversized frame");
        let _ = stop.send(true);
    }
}
