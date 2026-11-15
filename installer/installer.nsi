; WowType 拼音输入法 Windows 安装程序（NSIS）
; 稳定版：TSF 输入法 + 托盘程序 + 中等词库（34.6 万词条）
; 经典安装向导：欢迎 → 选择安装目录（可选任意盘符）→ 安装 → 完成

!define APP_NAME "WowType 拼音输入法"
!define APP_VERSION "1.0.0"
!define APP_PUBLISHER "WowType"

Name "${APP_NAME} ${APP_VERSION}"
OutFile "G:\WowType\dist\WowTypeSetup.exe"
InstallDir "$PROGRAMFILES64\WowType"
InstallDirRegKey HKLM "Software\WowType" "InstallDir"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
Unicode true

VIProductVersion "1.0.0.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "FileDescription" "${APP_NAME} 安装程序"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "MIT / Apache-2.0"

!include "MUI2.nsh"

!define MUI_WELCOMEPAGE_TITLE "欢迎安装 ${APP_NAME}"
!define MUI_WELCOMEPAGE_TEXT "这是一个基于 WowType 拼音引擎的 Windows 输入法。$\r$\n$\r$\n安装完成后：$\r$\n1. 按 Win+Space（或 Ctrl+Shift）切换到「WowType Pinyin」$\r$\n2. 在任意程序打拼音，数字键或空格选词$\r$\n3. 单按 Shift 切换中英文$\r$\n4. 托盘图标常驻任务栏，右键可退出$\r$\n$\r$\n注意：已打开的程序需要重新打开一次才能使用（与搜狗等输入法一致）。$\r$\n$\r$\n点击「下一步」继续。"

!define MUI_DIRECTORYPAGE_TEXT_TOP "请选择安装位置。可以安装到任意盘符（例如 D:\WowType）。"
!define MUI_FINISHPAGE_TITLE "安装完成"
!define MUI_FINISHPAGE_TEXT "${APP_NAME} 已安装完成。$\r$\n$\r$\n使用方法：$\r$\n1. 按 Win+Space 或 Ctrl+Shift$\r$\n2. 选择「WowType Pinyin」$\r$\n3. 打拼音，数字键选词，单按 Shift 切中英文$\r$\n$\r$\n托盘图标已常驻任务栏右下角。$\r$\n$\r$\n注意：已经打开的程序需要重新打开才能使用输入法（与搜狗一致）。"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"

Section "安装 WowType 拼音输入法" SecMain
  SetOutPath "$INSTDIR"
  File "G:\WowType\payload\wowtype_tsf.dll"
  File "G:\WowType\payload\wowtype_core.dll"
  File "G:\WowType\payload\libwinpthread-1.dll"
  File "G:\WowType\payload\WowTypeTray.exe"
  File "G:\WowType\payload\WowType.ico"
  SetOutPath "$INSTDIR\data"
  File "G:\WowType\payload\data\lexicon.tsv"
  File "G:\WowType\payload\data\words.lm"
  File "G:\WowType\payload\data\english.txt"

  DetailPrint "注册输入法到系统..."
  nsExec::ExecToLog 'regsvr32 /s "$INSTDIR\wowtype_tsf.dll"'
  Pop $0
  DetailPrint "regsvr32 返回: $0"

  ; 托盘自启动
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "WowTypeTray" '"$INSTDIR\WowTypeTray.exe"'

  ; 卸载信息
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\WowType" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType" "DisplayName" "${APP_NAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType" "NoRepair" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType" "EstimatedSize" 50000

  ; 启动托盘
  DetailPrint "启动托盘..."
  Exec '"$INSTDIR\WowTypeTray.exe"'
SectionEnd

Section "Uninstall"
  ; 结束托盘进程
  nsExec::ExecToLog 'taskkill /f /im WowTypeTray.exe'
  Pop $0
  nsExec::ExecToLog 'regsvr32 /u /s "$INSTDIR\wowtype_tsf.dll"'
  Pop $0
  Delete "$INSTDIR\wowtype_tsf.dll"
  Delete "$INSTDIR\wowtype_core.dll"
  Delete "$INSTDIR\libwinpthread-1.dll"
  Delete "$INSTDIR\WowTypeTray.exe"
  Delete "$INSTDIR\data\lexicon.tsv"
  Delete "$INSTDIR\data\words.lm"
  Delete "$INSTDIR\data\english.txt"
  RMDir "$INSTDIR\data"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "WowTypeTray"
  DeleteRegKey HKLM "Software\WowType"
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\WowType"
SectionEnd
