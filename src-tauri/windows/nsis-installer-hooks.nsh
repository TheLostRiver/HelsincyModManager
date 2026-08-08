!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri passes /UPDATE when the uninstaller is only removing an old version.
  ${If} $UpdateMode = 1
    Goto nsis_cleanup_hook_done
  ${EndIf}

  ClearErrors
  ExecWait '"$INSTDIR\hmm-save-backup-installer-cleanup.exe"' $0
  ${If} ${Errors}
    ; A missing/unlaunchable helper must block true uninstall.
    StrCpy $0 23
  ${EndIf}

  ${If} $0 = 0
    Goto nsis_cleanup_hook_done
  ${EndIf}

  ; 20 busy, 21 ownership/state unverified, 22 removal unverified,
  ; 23 platform/helper unavailable, 64 invalid invocation.
  StrCpy $1 "cleanup_failed"
  ${If} $0 = 20
    StrCpy $1 "busy"
  ${ElseIf} $0 = 21
    StrCpy $1 "ownership_unverified"
  ${ElseIf} $0 = 22
    StrCpy $1 "removal_unverified"
  ${ElseIf} $0 = 23
    StrCpy $1 "platform_unavailable"
  ${ElseIf} $0 = 64
    StrCpy $1 "invalid_invocation"
  ${EndIf}

  ${If} ${Silent}
    SetErrorLevel $0
    Quit
  ${EndIf}

  MessageBox MB_ICONSTOP|MB_OK "Uninstall cancelled: backup protection cleanup returned $0 ($1)."
  SetErrorLevel $0
  Quit

  nsis_cleanup_hook_done:
!macroend
