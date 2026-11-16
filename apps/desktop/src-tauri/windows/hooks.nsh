; OTX.GNG installer hooks (Tauri 2 NSIS `installerHooks`).
; The installer is per-machine and elevated, so it can register the Core service.
; UNVERIFIED until tested in a VM: the exact command line Tauri passes to the old uninstaller
; during an upgrade. We look for "/UPDATE" and, if present, only stop the service instead of
; reverting the user's changes.

!macro NSIS_HOOK_PREINSTALL
  ; Upgrade: stop the running Core so otx-core.exe can be replaced.
  ${If} ${FileExists} "$INSTDIR\otx-core.exe"
    nsExec::ExecToLog '"$INSTDIR\otx-core.exe" stop'
    Pop $0
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Re-register so the service always points at this install's exe, then start it.
  nsExec::ExecToLog '"$INSTDIR\otx-core.exe" uninstall'
  Pop $0
  nsExec::ExecToLog '"$INSTDIR\otx-core.exe" install'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONEXCLAMATION|MB_OK "The OTX.GNG Core service could not be installed (exit code $0).$\r$\nOTX.GNG will still open and show your hardware, but it cannot change Windows settings.$\r$\nReinstalling usually fixes this."
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ClearErrors
  ${GetOptions} $CMDLINE "/UPDATE" $1
  ${If} ${Errors}
    ; Real uninstall: undo every OTX.GNG change, then remove the service.
    nsExec::ExecToLog '"$INSTDIR\otx-core.exe" revert-all'
    Pop $0
    nsExec::ExecToLog '"$INSTDIR\otx-core.exe" uninstall'
    Pop $0
  ${Else}
    ; Update: keep the user's changes, just stop the Core.
    nsExec::ExecToLog '"$INSTDIR\otx-core.exe" stop'
    Pop $0
  ${EndIf}
!macroend
