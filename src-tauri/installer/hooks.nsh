; Keep the standard Tauri/NSIS pages, adding branding and a save-safe process gate.
!define MUI_WELCOMEPAGE_TITLE "欢迎安装轻笺"
!define MUI_WELCOMEPAGE_TEXT "随手记录，清晰推进。$\r$\n$\r$\n轻笺将为你安装桌面便签、富文本编辑与轻量进展清单。内容保存在本机，无需注册账号。$\r$\n$\r$\n开始前，请从系统托盘正常退出轻笺，确保便签已保存。"
!define MUI_FINISHPAGE_TITLE "轻笺已安装完成"
!define MUI_FINISHPAGE_TEXT "现在可以从桌面或开始菜单打开轻笺。$\r$\n$\r$\n在便签的“更多”菜单开启“记录进展”，即可记录当前步骤与下一步。$\r$\n$\r$\n升级会保留原有便签和设置。"

; Tauri's default macro offers to terminate the app. Require a normal saved exit
; instead, including passive updates and uninstall. utils.nsh precedes this hook.
!ifmacrodef CheckIfAppIsRunning
  !macroundef CheckIfAppIsRunning
!endif
!macro CheckIfAppIsRunning executableName productName
  !define QJ_RUNNING_ID ${__LINE__}
  StrCpy $R8 0
  qj_check_${QJ_RUNNING_ID}:
  nsis_tauri_utils::FindProcessCurrentUser "${executableName}"
  Pop $R0
  ${If} $R0 = 0
    ; The updater starts its child installer immediately before exiting itself.
    ; Give a normal exit a bounded grace period, never terminate the process.
    ${If} $R8 < 20
      IntOp $R8 $R8 + 1
      Sleep 100
      Goto qj_check_${QJ_RUNNING_ID}
    ${EndIf}
    IfSilent qj_abort_${QJ_RUNNING_ID} 0
    ${If} $PassiveMode = 1
      Goto qj_abort_${QJ_RUNNING_ID}
    ${EndIf}
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "轻笺正在运行。请先从系统托盘选择“退出”，待便签保存完成后再重试。安装程序不会强制结束轻笺。" IDRETRY qj_check_${QJ_RUNNING_ID}
    qj_abort_${QJ_RUNNING_ID}:
    SetErrorLevel 2
    Abort "请正常退出轻笺后重试。"
  ${EndIf}
  !undef QJ_RUNNING_ID
!macroend

; Preserve Modern UI's standard callbacks, but gate BEFORE its maintenance
; page can launch a previous uninstaller (which may have a force-kill policy).
; Deferred expansion at MUI_LANGUAGE ensures Tauri's variables/plugins exist.
!macroundef MUI_FUNCTION_GUIINIT
!macro MUI_FUNCTION_GUIINIT
  Function .onGUIInit
    !insertmacro MUI_GUIINIT_OUTERDIALOG ""
    !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"
    !ifdef MUI_PAGE_FUNCTION_GUIINIT
      Call "${MUI_PAGE_FUNCTION_GUIINIT}"
    !endif
    !ifdef MUI_CUSTOMFUNCTION_GUIINIT
      Call "${MUI_CUSTOMFUNCTION_GUIINIT}"
    !endif
  FunctionEnd
!macroend
