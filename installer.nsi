; 快译NG 安装脚本（NSIS 3，Unicode）
; 构建命令: makensis installer.nsi
; 产物: 快译NG-安装包.exe（含目录选择 + uninst.exe 卸载器）

Unicode true
ManifestDPIAware true

!define APPNAME "快译NG"
!define VERSION "1.4.0"
!define PUBLISHER "快译NG"

Name "${APPNAME} ${VERSION}"
OutFile "快译NG-安装包.exe"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "Software\${APPNAME}" "InstallDir"
RequestExecutionLevel admin

!include "MUI2.nsh"
!include "FileFunc.nsh"
!insertmacro GetSize

!define MUI_ICON "res\kuaiyi.ico"
!define MUI_UNICON "res\kuaiyi.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\快译NG.exe"
!define MUI_FINISHPAGE_RUN_TEXT "立即运行 ${APPNAME}"

!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"


;-----------------------------------------------------------
Section "主程序（必需）" SecCore
  SectionIn RO

  ; 结束运行中的快译NG，避免文件占用
  nsExec::Exec 'taskkill /IM 快译NG.exe /F'

  SetOutPath "$INSTDIR"
  File "快译NG.exe"

  ; 卸载器
  WriteUninstaller "$INSTDIR\uninst.exe"

  ; 注册表：记住安装目录 + 控制面板“卸载程序”条目
  SetRegView 64    ; 写 64 位视图（否则被重定向到 WOW6432Node）
  WriteRegStr HKLM "Software\${APPNAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayName" "${APPNAME}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "DisplayIcon" "$INSTDIR\快译NG.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "UninstallString" "$\"$INSTDIR\uninst.exe$\""
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "NoModify" 1
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "NoRepair" 1
  ; 大小（KB）
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}" "EstimatedSize" "$0"
SectionEnd

Section "桌面快捷方式" SecDesktop
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\快译NG.exe"
SectionEnd

Section "开始菜单快捷方式" SecStartMenu
  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\快译NG.exe"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\卸载 ${APPNAME}.lnk" "$INSTDIR\uninst.exe"
SectionEnd

Section "开机自启动" SecAutoRun
  ; 与程序内设置使用同一个 Run 键（键名 = 快译NG），互相识别
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${APPNAME}" "$\"$INSTDIR\快译NG.exe$\""
SectionEnd

;-----------------------------------------------------------
Section Uninstall
  ; 结束运行中的快译NG
  nsExec::Exec 'taskkill /IM 快译NG.exe /F'
  SetRegView 64

  ; 取消开机启动（若设置过）
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${APPNAME}"

  ; 快捷方式
  Delete "$DESKTOP\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$SMPROGRAMS\${APPNAME}\卸载 ${APPNAME}.lnk"
  RMDir "$SMPROGRAMS\${APPNAME}"

  ; 文件与数据（fastT：配置/历史）
  Delete "$INSTDIR\快译NG.exe"
  Delete "$INSTDIR\uninst.exe"
  RMDir /r "$INSTDIR\fastT"
  RMDir "$INSTDIR"

  ; 注册表清理
  DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"
  DeleteRegKey HKLM "Software\${APPNAME}"

  ; 若安装目录还有残留文件（用户自建），尝试删除空目录
  RMDir "$INSTDIR"
SectionEnd

; 组件说明（鼠标悬停显示）
LangString DESC_Desktop ${LANG_SIMPCHINESE} "在桌面创建 ${APPNAME} 快捷方式"
LangString DESC_StartMenu ${LANG_SIMPCHINESE} "在开始菜单创建 ${APPNAME} 与卸载快捷方式"
LangString DESC_AutoRun ${LANG_SIMPCHINESE} "登录 Windows 后自动启动 ${APPNAME}（程序设置里也可开关）"

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "主程序与卸载器（必装）"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_Desktop)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecStartMenu} $(DESC_StartMenu)
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutoRun} $(DESC_AutoRun)
!insertmacro MUI_FUNCTION_DESCRIPTION_END
