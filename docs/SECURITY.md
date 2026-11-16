# OTX.GNG security design

**OTX.GNG — Built and performed by Thinuda.** Report vulnerabilities privately through GitHub's
"Report a vulnerability" (Security tab), not in public issues.

## Threat model

| Asset | Threat | Main defence |
|---|---|---|
| SYSTEM privileges held by the Core service | A local unprivileged program uses the Core to run code or change arbitrary settings (local privilege escalation) | Strict typed allow-list; no command, path or registry-path parameters exist in the protocol |
| The pipe name | Another program creates `\\.\pipe\otx-gng-core-v1` first, or adds an instance, to impersonate the Core | Core creates the pipe with `FILE_FLAG_FIRST_PIPE_INSTANCE`; users lack `FILE_CREATE_PIPE_INSTANCE` in the DACL; release UI verifies the server process is the installed `otx-core.exe` |
| The Core's data directory (journal, logs) | A user plants or edits files to make the watchdog "revert" something harmful | `%ProgramData%\OTX.GNG` gets a protected DACL: SYSTEM and Administrators full, Users read-only. Journal entries can only describe typed revert operations |
| Game accounts | Anti-cheat bans | No injection, no memory access, no game-file edits (except the opt-in, backed-up Unreal config tuner), driver modules off by default and auto-disabled while anti-cheat games run |
| User privacy | Data leaving the PC | No telemetry backend; no listening network ports; system facts never leave the PC |
| Updates | Malicious update | Tauri updater signatures (ed25519) verified against a key embedded in the app; CI builds from tagged sources only |

## The pipe protocol

- Transport: named pipe, byte mode, frames of `u32 LE length + JSON`, at most 1 MiB. Remote
  clients are rejected (`PIPE_REJECT_REMOTE_CLIENTS`). A length above the limit closes the
  connection before any allocation.
- DACL (SDDL): `D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x12008b;;;IU)`. Interactive users get
  read + `FILE_WRITE_DATA` only, deliberately not `FILE_APPEND_DATA`, which on pipes means
  "create another instance".
- **Authentication comes from the kernel, not the client.** On connect, the Core calls
  `GetNamedPipeClientProcessId`, opens the process with `PROCESS_QUERY_LIMITED_INFORMATION`, and
  reads its image path (`QueryFullProcessImageNameW`), session (`ProcessIdToSessionId`) and user
  SID (token). In service mode the client must be `OTX.GNG.exe` / `otx-gng.exe` in the Core's own
  directory (Program Files, admin-write-only) and must not be in session 0. Then the client must
  send `hello` with the right protocol version within 5 seconds.
- **Why there is no shared-secret "session token" file.** The Step 0 plan proposed an HMAC over a
  per-user key file. On review it adds nothing: any malware running as the same user can read a
  file that user can read, and the kernel already tells us exactly who the client is. A per-connection
  session id is still issued for logging. This is a deliberate, documented change from the plan.
- **Allow-list.** Requests are a closed Rust enum (`otx_protocol::Request`). Anything else fails
  to deserialize and is answered `NOT_ALLOWED`. The complete list:
  - read-only: `hello`, `ping`, `getStatus`, `getSystemInfo`, `listChanges`, `listTweaks`, `getTraffic`;
  - processes: `setProcessPriority` (no Realtime variant exists), `setEfficiencyMode`, `endProcess`:
    PID + creation time, protected list and session-0 refusal enforced in the Core;
  - tweaks: `setTweak` by **id only**; the registry/service/power values live in manifests
    compiled into the Core;
  - memory: `cleanMemory` with method enums and plain exe names only;
  - network: `setDns` (IP literals only, validated), `applyShaper` (exe names; essential and
    protected processes refused), `restoreInternet`, `preferAdapter` (interface id),
    `clearAdapterPreference`;
  - sessions and capture: `startSession`, `endSession`, `startCapture`, `stopCapture`;
  - journal: `revertChange`, `revertAll`.
- **Residual risk (accepted):** malware already running as the logged-in user cannot pose as the
  UI, because it cannot write into Program Files without admin rights. It could, however, drive the
  real OTX.GNG window through UI automation to request allow-listed actions. Every such action is
  reversible, journaled and visible in "What OTX.GNG changed", and none grants code execution as SYSTEM.

## Development mode

`otx-core console --dev` accepts any process in an interactive session and logs a warning. It is
for working from the source tree only and is never used by the installer. Debug builds of the UI
skip the server-identity check for the same reason; release builds always enforce it.

## Safe Mode and dry run

- `OTX_SAFE_MODE=1` (or `--safe-mode` in console mode) disables every privileged module. The
  Core still answers status and read-only queries.
- `--dry-run` makes the Core log what it would change without changing anything.

## Watchdog

Every privileged change is written to `journal.json` **before** it is applied, together with the
typed operation that undoes it (write-ahead). The file is replaced atomically. When the Core starts
(after a crash, a forced close or a reboot), it reverts all temporary changes newest-first; failed
reverts stay in the journal for retry and are logged. A corrupt journal is moved aside, never
silently discarded. The service is configured to restart after a crash (5 s, then 30 s).

## Uninstall

The NSIS uninstaller runs `otx-core revert-all` (undo every journaled change) and then removes the
service. During an update it only stops the service, so your settings survive updates.
