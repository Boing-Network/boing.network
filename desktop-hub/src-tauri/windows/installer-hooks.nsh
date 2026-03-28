; Mitigate passive NSIS updates aborting in CheckIfAppIsRunning: the updater calls
; ShellExecuteW on this installer then immediately process::exit(0). The Hub process and
; WebView2 children can still appear in the process list for a short window, so the NSIS
; macro may try to kill a still-tearing-down process, fail, and Abort — leaving the old
; build on disk and no RunAsUser relaunch. Only delay in /UPDATE mode (not first install).
!macro NSIS_HOOK_PREINSTALL
  ${If} $UpdateMode = 1
    Sleep 2500
  ${EndIf}
!macroend
