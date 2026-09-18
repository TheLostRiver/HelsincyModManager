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
  ;
  ; $1 keeps the ASCII token: it is the stable key quoted in support reports and
  ; lines up with the exit codes above. $3 carries the same reason as copy for
  ; the localized message and is only ever read by the SimpChinese branch, so
  ; even its fallback stays Chinese.
  StrCpy $1 "cleanup_failed"
  StrCpy $3 "未知原因"
  ${If} $0 = 20
    StrCpy $1 "busy"
    StrCpy $3 "后台备份任务正在运行"
  ${ElseIf} $0 = 21
    StrCpy $1 "ownership_unverified"
    StrCpy $3 "无法确认后台备份任务的归属"
  ${ElseIf} $0 = 22
    StrCpy $1 "removal_unverified"
    StrCpy $3 "无法确认计划任务是否已删除"
  ${ElseIf} $0 = 23
    StrCpy $1 "platform_unavailable"
    StrCpy $3 "当前平台或清理程序不可用"
  ${ElseIf} $0 = 64
    StrCpy $1 "invalid_invocation"
    StrCpy $3 "清理程序调用参数无效"
  ${EndIf}

  ; NSIS rewrites $LANGUAGE to the id of the language table it matched, so a
  ; Chinese system reports 2052 here even when only SimpChinese is bundled.
  ${If} $LANGUAGE == 2052
    StrCpy $2 "卸载已取消：存档备份保护清理未能完成（$3，退出码 $0）。"
    StrCpy $2 "$2$\r$\n$\r$\n请关闭 Helsincy Mod Manager 后重试；若后台备份正在进行，请等待其结束后再卸载。"
  ${Else}
    StrCpy $2 "Uninstall cancelled: backup protection cleanup returned $0 ($1)."
  ${EndIf}

  ${If} ${Silent}
    SetErrorLevel $0
    Abort
  ${EndIf}

  MessageBox MB_ICONSTOP|MB_OK "$2"
  SetErrorLevel $0
  Abort

  nsis_cleanup_hook_done:
!macroend
