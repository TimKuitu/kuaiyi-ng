## 快译NG —— Bing 免费翻译（C 核心 + wNim 界面）
## 中英互译、历史记录、复制单词、全局热键、托盘；缓存目录可改、Bing 免费翻译接口。

import std/[os, strutils]
import wNim
import winim/lean
import fastcore

# winim/lean 未包含 shellapi，这里单独引入打开目录所需的函数
proc ShellExecuteW*(hwnd: HWND, lpOperation, lpFile, lpParameters,
                    lpDirectory: LPCWSTR, nShow: cint): HANDLE
    {.stdcall, dynlib: "shell32", importc: "ShellExecuteW".}

const
  AppTitle = "快译NG"
  AppVersion = "1.1"
  WmAppTranslated = WM_APP + 0x71
  WmAppUpdate = WM_APP + 0x72

  # 全局热键 ID
  HkToggle = 1
  HkHistUp = 2
  HkHistDown = 3
  HkCopy = 4

  # 热键默认值（功能键 1=Ctrl 2=Shift 4=Alt 按位相加，键码=Windows 虚拟键码）
  DefToggle = (5, 70)
  DefHistUp = (5, 38)
  DefHistDown = (5, 40)
  DefCopy = (5, 67)

  FmtNames = ["原样", "全小写", "全大写", "驼峰", "下划线", "中划线"]

  # AI 接口预设（名称, 地址, 模型）
  AiPresets = [("", "", ""),
               ("DeepSeek", "https://api.deepseek.com", "deepseek-chat"),
               ("OpenAI", "https://api.openai.com", "gpt-4o-mini"),
               ("Moonshot", "https://api.moonshot.cn", "moonshot-v1-8k"),
               ("Ollama 本地", "http://localhost:11434", "qwen2.5:7b")]

type
  Settings = object
    topmost: bool
    closeExit: bool
    autoStart: bool
    autoHide: bool
    summonNearMouse: bool   # 热键显示窗口时出现在鼠标旁
    cacheDir: string
    wordFmt: int
    aiEnabled: bool         # AI 接口翻译
    aiUrl: string
    aiKey: string
    aiModel: string
    hkToggle: (int, int)
    hkHistUp: (int, int)
    hkHistDown: (int, int)
    hkCopy: (int, int)

# ---------------- 配置读写 ----------------
var
  g_cfg: pointer
  g_hist: pointer
  g_appDir: string
  g_set: Settings
  g_histIdx = -1
  g_translating = false
  g_exiting = false
  g_stateSaved = false
  g_dpiScale = 1.0
  g_lastToEn = false      # 最近一次翻译的目标是否为英文（是则结果区是要复制的英文）
  g_resultValid = false   # 结果区当前是否有有效内容

proc nimHasCjk(s: string): bool =
  # UTF-8 字节扫描：CJK 表意字符都是 3 字节序列
  var i = 0
  while i < s.len:
    let b = ord(s[i])
    if (b and 0xF0) == 0xE0 and i + 2 < s.len:
      let cp = ((b and 0x0F) shl 12) or ((ord(s[i+1]) and 0x3F) shl 6) or
               (ord(s[i+2]) and 0x3F)
      if (cp >= 0x2E80 and cp <= 0x9FFF) or (cp >= 0xF900 and cp <= 0xFAFF):
        return true
      inc i, 3
    elif b < 0x80: inc i
    elif (b and 0xE0) == 0xC0: inc i, 2
    elif (b and 0xF8) == 0xF0: inc i, 4
    else: inc i
  return false

proc cfgStr(sec, key, def: string): string =
  var buf = newString(1024)
  discard fy_cfg_get(g_cfg, sec.cstring, key.cstring, def.cstring,
                     cstring(addr buf[0]), cint(1024))
  result = ($cstring(addr buf[0])).strip()

proc cfgInt(sec, key: string, def: int): int =
  let s = cfgStr(sec, key, $def)
  try: result = parseInt(s)
  except ValueError: result = def

proc cfgBool(sec, key: string, def: bool): bool =
  cfgInt(sec, key, if def: 1 else: 0) != 0

proc loadSettings() =
  g_set.topmost = cfgBool("通用设置", "窗口置顶", false)
  g_set.closeExit = cfgBool("通用设置", "关闭退出", false)
  g_set.autoStart = cfgBool("通用设置", "开机启动", false)
  # 开机启动以注册表 Run 键为准（安装包勾选的自启动也能被识别并同步到设置界面）
  var key: HKEY
  if RegOpenKeyExW(HKEY_CURRENT_USER,
                   r"Software\Microsoft\Windows\CurrentVersion\Run", 0,
                   KEY_QUERY_VALUE, addr key) == ERROR_SUCCESS:
    if RegQueryValueExW(key, AppTitle.newWideCString(), nil, nil, nil, nil) ==
        ERROR_SUCCESS:
      g_set.autoStart = true
    RegCloseKey(key)
  g_set.autoHide = cfgBool("通用设置", "开机隐藏", false)
  g_set.summonNearMouse = cfgBool("通用设置", "鼠标召唤", false)
  g_set.cacheDir = cfgStr("通用设置", "缓存目录", g_appDir)
  if g_set.cacheDir.len == 0: g_set.cacheDir = g_appDir
  g_set.wordFmt = cfgInt("通用设置", "命名格式", FY_FMT_CAMEL)
  if g_set.wordFmt < 0 or g_set.wordFmt > 5: g_set.wordFmt = FY_FMT_CAMEL
  g_set.aiEnabled = cfgBool("AI翻译", "启用", false)
  g_set.aiUrl = cfgStr("AI翻译", "地址", "")
  g_set.aiKey = cfgStr("AI翻译", "密钥", "")
  g_set.aiModel = cfgStr("AI翻译", "模型", "deepseek-chat")

  proc hk(key1, key2: string, def: (int, int)): (int, int) =
    (cfgInt("快捷按键", key1, def[0]), cfgInt("快捷按键", key2, def[1]))

  g_set.hkToggle = hk("隐藏界面1", "隐藏界面2", DefToggle)
  g_set.hkHistUp = hk("上历史1", "上历史2", DefHistUp)
  g_set.hkHistDown = hk("下历史1", "下历史2", DefHistDown)
  g_set.hkCopy = hk("复制1", "复制2", DefCopy)

# ---------------- 热键编码（1=Ctrl 2=Shift 4=Alt 按位相加；8=Win 扩展位） ----------------
proc origToWin(m: int): UINT =
  result = 0
  if (m and 1) != 0: result = result or MOD_CONTROL
  if (m and 2) != 0: result = result or MOD_SHIFT
  if (m and 4) != 0: result = result or MOD_ALT
  if (m and 8) != 0: result = result or MOD_WIN

proc winToOrig(m: int): int =
  result = 0
  if (m and MOD_CONTROL) != 0: result = result or 1
  if (m and MOD_SHIFT) != 0: result = result or 2
  if (m and MOD_ALT) != 0: result = result or 4
  if (m and MOD_WIN) != 0: result = result or 8

# ---------------- 开机启动（注册表 HKCU\...\Run） ----------------
proc setAutoStart(enable, hideOnStart: bool) =
  var key: HKEY
  const RunKey = r"Software\Microsoft\Windows\CurrentVersion\Run"
  if RegOpenKeyExW(HKEY_CURRENT_USER, RunKey, 0, KEY_SET_VALUE, addr key) != ERROR_SUCCESS:
    return
  defer: RegCloseKey(key)
  if enable:
    var exe = getAppFilename()
    exe = "\"" & exe & "\"" & (if hideOnStart: " -hide" else: "")
    let wpath = exe.newWideCString()
    RegSetValueExW(key, AppTitle, 0, REG_SZ, cast[ptr BYTE](wpath),
                   DWORD((exe.len + 1) * 2))
  else:
    RegDeleteValueW(key, AppTitle)

# ---------------- 控件 ----------------
var
  frame, setFrame: wFrame
  panel, setPanel, page1, page2, page3: wPanel
  input, resultBox, edtCache: wTextCtrl
  edtAiUrl, edtAiKey, edtAiModel: wTextCtrl
  btnZh2En, btnEn2Zh, btnTrans, btnSettings: wButton
  btnOpenCache, btnSaveSet1, btnSaveSet2, btnSaveSet3, btnResetHot: wButton
  btnCheckUpdate: wButton
  lblUpdateStatus: wStaticText
  cmbFmt: wComboBox
  cmbAiPreset: wComboBox
  chkTop, chkCloseExit, chkAutoStart, chkAutoHide, chkSummon, chkAi: wCheckBox
  hkToggleCtrl, hkUpCtrl, hkDownCtrl, hkCopyCtrl: wHotkeyCtrl
  g_checkingUpdate = false

# ---------------- 剪贴板 ----------------
proc copyText(s: string) =
  if s.len == 0: return
  discard wSetClipboard(DataObject(s))

proc flashTitle(msg: string) =
  # 短暂改标题作反馈，2 秒后自动恢复（标题被永久改动会破坏按标题找窗口的逻辑）
  frame.title = AppTitle & "  " & msg
  frame.startTimer(2.0, 88)

# ---------------- 翻译（后台线程） ----------------
type TranslateJob = object
  text: cstring
  mode: cint
  useAi: bool
  aiUrl: cstring
  aiKey: cstring
  aiModel: cstring

proc translateWorker(param: LPVOID): DWORD {.stdcall.} =
  let job = cast[ptr TranslateJob](param)
  var res: cstring = nil
  if job.useAi:
    res = fy_translate_ai(job.text, job.mode, job.aiUrl, job.aiKey, job.aiModel)
    if res.isNil:
      # AI 失败 → 回退必应，并在结果前说明原因
      let bing = fy_translate(job.text, job.mode)
      if not bing.isNil:
        let err = fy_strdup(fy_last_error())
        var buf = "（AI 接口失败，已用必应翻译：" & $err & "）\n" & $bing
        fy_free(cast[pointer](err))
        fy_free(cast[pointer](bing))
        res = fy_strdup(buf.cstring)
  else:
    res = fy_translate(job.text, job.mode)
  fy_free(job.text)
  fy_free(job.aiUrl)
  fy_free(job.aiKey)
  fy_free(job.aiModel)
  dealloc(job)
  PostMessageW(cast[HWND](frame.getHandle()), UINT(WmAppTranslated), 0,
               cast[LPARAM](res))
  return 0

proc doTranslate(mode: int) =
  if g_translating or frame.isNil: return
  let text = input.getValue().strip()
  if text.len == 0: return
  g_translating = true
  g_resultValid = false
  # 记录本次翻译目标：中→英时结果区是英文（复制目标）
  g_lastToEn = (mode == FY_ZH2EN) or (mode == FY_AUTO and nimHasCjk(text))
  resultBox.setValue("正在翻译……")
  btnZh2En.disable(); btnEn2Zh.disable(); btnTrans.disable()
  let job = cast[ptr TranslateJob](alloc(sizeof(TranslateJob)))
  job.text = fy_strdup(text.cstring)
  job.mode = cint(mode)
  let useAi = g_set.aiEnabled and g_set.aiKey.len > 0 and g_set.aiUrl.len > 0
  job.useAi = useAi
  job.aiUrl = fy_strdup(if useAi: g_set.aiUrl else: "")
  job.aiKey = fy_strdup(if useAi: g_set.aiKey else: "")
  job.aiModel = fy_strdup(if useAi: g_set.aiModel else: "")
  discard CreateThread(nil, 0, translateWorker, cast[LPVOID](job), 0, nil)

# ---------------- 检查更新（后台线程） ----------------
const
  UpdStageResult = 1      # lParam = UpdateInfo 指针（检查结果）
  UpdStageProgress = 2    # lParam = 百分比整数
  UpdStageDone = 3        # lParam = UpdateInfo 指针（下载完成，path 有效）
  UpdStageFail = 4        # lParam = UpdateInfo 指针（下载失败，err 有效）

type
  UpdateInfo = object
    hasNew: cint
    ver: array[64, char]
    url: array[512, char]
    err: array[256, char]
    path: array[512, char]

var g_hwndMain: HWND      # 工作线程里 PostMessage 用

proc postUpd(stage: UINT, lp: LPARAM) {.inline.} =
  discard PostMessageW(g_hwndMain, UINT(WmAppUpdate), stage, lp)

proc updateCheckWorker(param: LPVOID): DWORD {.stdcall.} =
  let info = cast[ptr UpdateInfo](param)
  let r = fy_check_update(AppVersion.cstring, cast[cstring](addr info.ver),
                          64, cast[cstring](addr info.url), 512)
  if r < 0:
    info.hasNew = -1
    let e = fy_last_error()
    var i = 0
    while e[i] != '\0' and i < sizeof(info.err) - 1:
      info.err[i] = e[i]
      inc i
    info.err[i] = '\0'
  else:
    info.hasNew = r
  postUpd(UpdStageResult, cast[LPARAM](info))
  return 0

proc dlProgress(done: int64, total: int64) {.cdecl.} =
  var pct = 0
  if total > 0: pct = int(done * 100 div total)
  postUpd(UpdStageProgress, cast[LPARAM](pct))

proc updateDownloadWorker(param: LPVOID): DWORD {.stdcall.} =
  let info = cast[ptr UpdateInfo](param)
  let dest = getTempDir() / "KuaiYiNG_Update.exe"
  if fy_download_update(cast[cstring](addr info.url), dest.cstring, dlProgress) == 0:
    copyMem(addr info.path, dest.cstring, min(dest.len + 1, sizeof(info.path)))
    postUpd(UpdStageDone, cast[LPARAM](info))
  else:
    let e = fy_last_error()
    var i = 0
    while e[i] != '\0' and i < sizeof(info.err) - 1:
      info.err[i] = e[i]
      inc i
    info.err[i] = '\0'
    postUpd(UpdStageFail, cast[LPARAM](info))
  return 0

proc onUpdateMessage(stage: int, lp: LPARAM) =
  case stage
  of UpdStageResult:
    let info = cast[ptr UpdateInfo](lp)
    g_checkingUpdate = false
    btnCheckUpdate.enable()
    if info.hasNew == 1:
      let msg = "发现新版本 " & ($cast[cstring](addr info.ver)) &
                "（当前 v" & AppVersion & "）。\n\n是否下载并安装？"
      let r = MessageBoxW(cast[HWND](frame.getHandle()), msg.newWideCString(),
                          AppTitle.newWideCString(), 0x24)   # MB_YESNO|MB_ICONQUESTION
      if r == 6:   # IDYES
        lblUpdateStatus.label = "开始下载……"
        let dl = cast[ptr UpdateInfo](alloc0(sizeof(UpdateInfo)))
        copyMem(addr dl.url, addr info.url, sizeof(dl.url))
        discard CreateThread(nil, 0, updateDownloadWorker, cast[LPVOID](dl), 0, nil)
      else:
        lblUpdateStatus.label = "已取消更新（可稍后再试）"
    elif info.hasNew == 0:
      lblUpdateStatus.label = "已是最新版本（v" & AppVersion & "）"
    else:
      lblUpdateStatus.label = "检查失败：" & $cast[cstring](addr info.err)
    dealloc(info)
  of UpdStageProgress:
    lblUpdateStatus.label = "下载中 " & $int(lp) & "%……"
  of UpdStageDone:
    let info = cast[ptr UpdateInfo](lp)
    lblUpdateStatus.label = "下载完成，启动安装程序……"
    discard ShellExecuteW(0, "open", newWideCString($cast[cstring](addr info.path)),
                          nil, nil, SW_SHOWNORMAL)
    dealloc(info)
  of UpdStageFail:
    let info = cast[ptr UpdateInfo](lp)
    lblUpdateStatus.label = "下载失败：" & $cast[cstring](addr info.err)
    dealloc(info)
  else:
    discard

proc onTranslated(res: cstring) =
  g_translating = false
  btnZh2En.enable(); btnEn2Zh.enable(); btnTrans.enable()
  if res.isNil:
    resultBox.setValue("翻译失败\n" & $fy_last_error())
    g_resultValid = false
    return
  resultBox.setValue($res)
  g_resultValid = true
  fy_free(cast[pointer](res))
  let text = input.getValue().strip()
  if text.len != 0:
    fy_hist_push(g_hist, text.cstring)
    g_histIdx = -1

# ---------------- 布局 ----------------
proc layoutMain() =
  # wNim 的 pos/size 参数是逻辑单位（按 96dpi 设计，高 DPI 下自动缩小渲染），
  # getClientSize 返回物理像素；两者换算后混用。
  template S(x: int): int = int(float(x) * g_dpiScale)
  let cs = frame.getClientSize()
  let w = cs.width
  let h = cs.height
  panel.setSize((0, 0), (w, h))
  input.setSize((S(8), S(8)), (w - S(16), S(96)))
  btnZh2En.setSize((S(8), S(112)), (S(92), S(30)))
  btnEn2Zh.setSize((S(106), S(112)), (S(92), S(30)))
  btnSettings.setSize((S(204), S(112)), (S(66), S(30)))
  btnTrans.setSize((S(274), S(112)), (w - S(282), S(30)))
  resultBox.setSize((S(8), S(150)), (w - S(16), h - S(158)))

# ---------------- 显示/隐藏 ----------------
proc forceForeground(h: HWND) =
  # Windows 前台锁定：后台进程直接 SetForegroundWindow 会被拒，用 AttachThreadInput 绕过
  let fg = GetForegroundWindow()
  var fgPid: DWORD = 0
  let fgTid = GetWindowThreadProcessId(fg, addr fgPid)
  let thisTid = GetCurrentThreadId()
  var tgtPid: DWORD = 0
  let tgtTid = GetWindowThreadProcessId(h, addr tgtPid)
  if fgTid != 0 and fgTid != thisTid:
    discard AttachThreadInput(thisTid, fgTid, TRUE)
  if tgtTid != 0 and tgtTid != thisTid:
    discard AttachThreadInput(thisTid, tgtTid, TRUE)
  if IsIconic(h) != 0:
    discard ShowWindow(h, SW_RESTORE)
  else:
    discard ShowWindow(h, SW_SHOW)
  discard SetForegroundWindow(h)
  discard BringWindowToTop(h)
  if tgtTid != 0 and tgtTid != thisTid:
    discard AttachThreadInput(thisTid, tgtTid, FALSE)
  if fgTid != 0 and fgTid != thisTid:
    discard AttachThreadInput(thisTid, fgTid, FALSE)

proc moveToMouse() =
  # 窗口移动到鼠标旁（偏移 24px），限制在鼠标所在显示器的工作区内
  var pt: POINT
  if GetCursorPos(addr pt) == 0:
    return
  let hwnd = cast[HWND](frame.getHandle())
  var r: RECT
  discard GetWindowRect(hwnd, addr r)
  let ww = int(r.right) - int(r.left)
  let wh = int(r.bottom) - int(r.top)
  var x = int(pt.x) + 24
  var y = int(pt.y) + 24
  var mi: MONITORINFO
  mi.cbSize = DWORD(sizeof(MONITORINFO))
  let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)
  if mon != HMONITOR(0) and GetMonitorInfoW(mon, addr mi) != 0:
    let wa = mi.rcWork
    if x + ww > int(wa.right):  x = int(wa.right) - ww
    if y + wh > int(wa.bottom): y = int(wa.bottom) - wh
    if x < int(wa.left):  x = int(wa.left)
    if y < int(wa.top):   y = int(wa.top)
  discard SetWindowPos(hwnd, HWND(0), int32(x), int32(y), 0, 0,
                       SWP_NOSIZE or SWP_NOZORDER or SWP_NOACTIVATE)

proc toggleMain() =
  let hwnd = cast[HWND](frame.getHandle())
  if IsWindowVisible(hwnd) != 0 and IsIconic(hwnd) == 0:
    frame.hide()
  else:
    if g_set.summonNearMouse:
      moveToMouse()
    forceForeground(hwnd)

# ---------------- 历史 ----------------
proc histJump(delta: int) =
  let n = int(fy_hist_count(g_hist))
  if n == 0: return
  if g_histIdx == -1:
    g_histIdx = n - 1
  else:
    g_histIdx = g_histIdx + delta
    if g_histIdx < 0: g_histIdx = 0
    if g_histIdx > n - 1: g_histIdx = n - 1
  let s = fy_hist_get(g_hist, cint(g_histIdx))
  if not s.isNil:
    input.setValue($s)
    fy_free(cast[pointer](s))
    doTranslate(FY_AUTO)

# ---------------- 复制单词 ----------------
proc copySource(): string =
  # 复制目标：最近一次翻译是"中→英"且结果有效 → 复制结果区的英文（首行）；
  # 否则复制输入框内容（英文单词查询时输入就是英文）
  if g_lastToEn and g_resultValid:
    let first = resultBox.getValue().splitLines()[0].strip()
    if first.len > 0 and first != "正在翻译……":
      return first
  return input.getValue().strip()

proc copyWord(formatted: bool) =
  let text = copySource()
  if text.len == 0: return
  if not formatted:
    copyText(text)
    flashTitle("（已复制）")
  else:
    let w = fy_format_word(text.cstring, cint(g_set.wordFmt))
    if not w.isNil:
      # 格式化后去掉所有空格（驼峰/下划线/中划线本身无空格，原样/大小写也按要求去空格）
      let s = ($w).replace(" ", "").replace("\t", "").replace("\r", "").replace("\n", "")
      copyText(s)
      fy_free(cast[pointer](w))
      flashTitle("（已复制：" & s & "）")

# ---------------- 全局热键注册 ----------------
proc registerHotkeys() =
  let hwnd = cast[HWND](frame.getHandle())
  discard UnregisterHotKey(hwnd, HkToggle)
  discard UnregisterHotKey(hwnd, HkHistUp)
  discard UnregisterHotKey(hwnd, HkHistDown)
  discard UnregisterHotKey(hwnd, HkCopy)
  let hks = [(HkToggle, g_set.hkToggle), (HkHistUp, g_set.hkHistUp),
             (HkHistDown, g_set.hkHistDown), (HkCopy, g_set.hkCopy)]
  var log = ""
  for pair in hks:
    if pair[1][1] != 0:
      let ok = RegisterHotKey(hwnd, UINT(pair[0]), origToWin(pair[1][0]),
                              UINT(pair[1][1]))
      if ok == 0:
        log.add("RegisterHotKey id=" & $pair[0] & " mods=" & $pair[1][0] &
                " vk=" & $pair[1][1] & " 失败 err=" & $GetLastError() & "\n")
  if log.len > 0:
    let f = open(g_appDir / "hotkey.log", fmAppend)
    f.writeLine("--- tick=" & $GetTickCount64())
    f.write(log)
    f.close()

# ---------------- 设置窗口 ----------------
proc openSettings() =
  chkTop.setValue(g_set.topmost)
  chkCloseExit.setValue(g_set.closeExit)
  chkAutoStart.setValue(g_set.autoStart)
  chkAutoHide.setValue(g_set.autoHide)
  chkSummon.setValue(g_set.summonNearMouse)
  chkAi.setValue(g_set.aiEnabled)
  edtAiUrl.setValue(g_set.aiUrl)
  edtAiKey.setValue(g_set.aiKey)
  edtAiModel.setValue(g_set.aiModel)
  lblUpdateStatus.label = "当前版本 v" & AppVersion
  edtCache.setValue(g_set.cacheDir)
  cmbFmt.setSelection(g_set.wordFmt)
  hkToggleCtrl.setHotkey(origToWin(g_set.hkToggle[0]), g_set.hkToggle[1])
  hkUpCtrl.setHotkey(origToWin(g_set.hkHistUp[0]), g_set.hkHistUp[1])
  hkDownCtrl.setHotkey(origToWin(g_set.hkHistDown[0]), g_set.hkHistDown[1])
  hkCopyCtrl.setHotkey(origToWin(g_set.hkCopy[0]), g_set.hkCopy[1])
  setFrame.center()
  setFrame.show()
  forceForeground(cast[HWND](setFrame.getHandle()))

proc saveSettings() =
  g_set.topmost = chkTop.getValue()
  g_set.closeExit = chkCloseExit.getValue()
  g_set.autoStart = chkAutoStart.getValue()
  g_set.autoHide = chkAutoHide.getValue()
  g_set.summonNearMouse = chkSummon.getValue()
  g_set.cacheDir = edtCache.getValue().strip()
  if g_set.cacheDir.len == 0: g_set.cacheDir = g_appDir
  g_set.wordFmt = cmbFmt.getSelection()
  g_set.aiEnabled = chkAi.getValue()
  g_set.aiUrl = edtAiUrl.getValue().strip()
  g_set.aiKey = edtAiKey.getValue().strip()
  g_set.aiModel = edtAiModel.getValue().strip()
  if g_set.aiModel.len == 0: g_set.aiModel = "deepseek-chat"

  let hkt = hkToggleCtrl.getHotkey()
  let hku = hkUpCtrl.getHotkey()
  let hkd = hkDownCtrl.getHotkey()
  let hkc = hkCopyCtrl.getHotkey()
  g_set.hkToggle = (winToOrig(hkt.modifiers), hkt.keyCode)
  g_set.hkHistUp = (winToOrig(hku.modifiers), hku.keyCode)
  g_set.hkHistDown = (winToOrig(hkd.modifiers), hkd.keyCode)
  g_set.hkCopy = (winToOrig(hkc.modifiers), hkc.keyCode)

  fy_cfg_set(g_cfg, "通用设置", "窗口置顶", if g_set.topmost: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "关闭退出", if g_set.closeExit: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "开机启动", if g_set.autoStart: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "开机隐藏", if g_set.autoHide: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "鼠标召唤", if g_set.summonNearMouse: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "缓存目录", g_set.cacheDir)
  fy_cfg_set(g_cfg, "通用设置", "命名格式", $g_set.wordFmt)
  fy_cfg_set(g_cfg, "AI翻译", "启用", if g_set.aiEnabled: "1" else: "0")
  fy_cfg_set(g_cfg, "AI翻译", "地址", g_set.aiUrl)
  fy_cfg_set(g_cfg, "AI翻译", "密钥", g_set.aiKey)
  fy_cfg_set(g_cfg, "AI翻译", "模型", g_set.aiModel)
  fy_cfg_set(g_cfg, "快捷按键", "隐藏界面1", $g_set.hkToggle[0])
  fy_cfg_set(g_cfg, "快捷按键", "隐藏界面2", $g_set.hkToggle[1])
  fy_cfg_set(g_cfg, "快捷按键", "上历史1", $g_set.hkHistUp[0])
  fy_cfg_set(g_cfg, "快捷按键", "上历史2", $g_set.hkHistUp[1])
  fy_cfg_set(g_cfg, "快捷按键", "下历史1", $g_set.hkHistDown[0])
  fy_cfg_set(g_cfg, "快捷按键", "下历史2", $g_set.hkHistDown[1])
  fy_cfg_set(g_cfg, "快捷按键", "复制1", $g_set.hkCopy[0])
  fy_cfg_set(g_cfg, "快捷按键", "复制2", $g_set.hkCopy[1])
  discard fy_cfg_save(g_cfg)

  setAutoStart(g_set.autoStart, g_set.autoHide)
  registerHotkeys()

  let hwnd = cast[HWND](frame.getHandle())
  discard SetWindowPos(hwnd, if g_set.topmost: HWND_TOPMOST else: HWND_NOTOPMOST,
                       0, 0, 0, 0, SWP_NOMOVE or SWP_NOSIZE)

  fy_hist_close(g_hist)
  g_hist = fy_hist_open(g_set.cacheDir.cstring)
  g_histIdx = -1

  setFrame.hide()
  discard MessageBoxW(cast[HWND](frame.getHandle()), "设置已保存。", AppTitle,
                      MB_OK or MB_ICONINFORMATION)

proc saveExitState() =
  # 退出/销毁时只保存窗口位置（尺寸不保存：每次启动恢复默认大小）
  if g_cfg.isNil or g_stateSaved: return
  g_stateSaved = true
  if not frame.isNil:
    let pos = frame.position
    fy_cfg_set(g_cfg, "通用设置", "窗口X", $pos.x)
    fy_cfg_set(g_cfg, "通用设置", "窗口Y", $pos.y)
  discard fy_cfg_save(g_cfg)

proc quitApp() =
  if g_exiting: return
  g_exiting = true
  saveExitState()
  fy_cfg_close(g_cfg)
  fy_hist_close(g_hist)
  frame.delete()
  # Nim 运行时退出路径可能死锁（实测会留下无窗口的残留进程占用互斥体），
  # 直接结束进程，确保彻底退出
  ExitProcess(0)

proc showTrayMenu() =
  let menu = Menu()
  menu.append(1, "软件设置")
  menu.appendSeparator()
  menu.append(2, "退出程序")
  # 不传坐标：wNim 会自动用当前鼠标位置（自己传 GetCursorPos 的屏幕坐标
  # 会被 wNim 当作客户区坐标再偏移一次，菜单会被顶到错误位置）
  let id = frame.popupMenu(menu, flag=wPopMenuReturnId)
  case id
  of 1: openSettings()
  of 2: quitApp()
  else: discard

# ================= 主程序 =================
# winim/lean 未包含的进程快照 API（单实例自愈用），自行声明
const TH32CS_SNAPPROCESS = 0x00000002'i32
type
  FY_PROCENTRY32W {.bycopy.} = object
    dwSize: int32
    cntUsage: int32
    th32ProcessID: int32
    th32DefaultHeap: int
    th32ModuleID: int32
    cntThreads: int32
    th32ParentProcessID: int32
    pcPriClassBase: int32
    dwFlags: int32
    szExeFile: array[260, Utf16Char]

proc CreateToolhelp32Snapshot*(flags: int32, pid: int32): HANDLE
    {.stdcall, dynlib: "kernel32", importc: "CreateToolhelp32Snapshot".}
proc Process32FirstW*(snap: HANDLE, e: ptr FY_PROCENTRY32W): int32
    {.stdcall, dynlib: "kernel32", importc: "Process32FirstW".}
proc Process32NextW*(snap: HANDLE, e: ptr FY_PROCENTRY32W): int32
    {.stdcall, dynlib: "kernel32", importc: "Process32NextW".}
proc EnumWindows*(lpEnumFunc: proc (h: HWND, l: LPARAM): int32 {.stdcall.},
                  l: LPARAM): int32
    {.stdcall, dynlib: "user32", importc: "EnumWindows".}

var g_enumFound: HWND

proc enumFindWindow(h: HWND, lp: LPARAM): int32 {.stdcall.} =
  var pid: DWORD = 0
  discard GetWindowThreadProcessId(h, addr pid)
  if pid == DWORD(lp):
    g_enumFound = h
    return 0        # 找到即停止枚举
  return 1

proc processHasAnyWindow(pid: int32): bool =
  # 僵尸进程 = 窗口已全部销毁；健康实例即使隐藏/最小化，窗口句柄仍存在
  g_enumFound = 0
  discard EnumWindows(enumFindWindow, LPARAM(pid))
  result = g_enumFound != HWND(0)

proc killZombieInstances() =
  # 结束同名且无任何窗口的残留进程（有窗口的正常实例不会被误杀）
  let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
  if snap == HANDLE(-1): return
  defer: discard CloseHandle(snap)
  var e: FY_PROCENTRY32W
  e.dwSize = int32(sizeof(FY_PROCENTRY32W))
  let myPid = GetCurrentProcessId()
  var ok = Process32FirstW(snap, addr e)
  while ok != 0:
    if e.th32ProcessID != 0 and e.th32ProcessID != int32(myPid):
      let name = $cast[WideCString](addr e.szExeFile[0])
      if name.toLowerAscii() == "快译ng.exe" and not processHasAnyWindow(e.th32ProcessID):
        let h = OpenProcess(PROCESS_TERMINATE, FALSE, DWORD(e.th32ProcessID))
        if h != HANDLE(0):
          discard TerminateProcess(h, 1)
          discard WaitForSingleObject(h, 2000)
          discard CloseHandle(h)
    ok = Process32NextW(snap, addr e)

proc main() =
  # 单实例：有窗口的旧实例 → 唤醒；无窗口的残留进程 → 清理后照常启动
  block:
    var retry = 0
    while true:
      discard CreateMutexW(nil, TRUE, "KuaiYiNG_SingleInstance")
      if GetLastError() != ERROR_ALREADY_EXISTS:
        break
      let old = FindWindowW(nil, AppTitle.newWideCString())
      if old != HWND(0):
        forceForeground(old)
        return
      inc retry
      if retry > 6: break     # 清不掉就放弃互斥保护，仍要能用
      killZombieInstances()
      sleep(300)

  let app = App(wSystemDpiAware)

  # 配置存物理像素；高 DPI 下统一用 wNim 的 dpiAutoScale 换算
  block:
    let hdc = GetDC(0)
    g_dpiScale = float(GetDeviceCaps(hdc, LOGPIXELSX)) / 96.0
    ReleaseDC(0, hdc)
    if g_dpiScale < 1.0: g_dpiScale = 1.0

  g_appDir = getAppDir() / "fastT"
  createDir(g_appDir)

  g_cfg = fy_cfg_open((g_appDir / "config").cstring)
  loadSettings()
  g_hist = fy_hist_open(g_set.cacheDir.cstring)

  # 主窗口：配置存物理像素，换算成 wNim 逻辑值传入
  # 每次启动固定默认大小；运行期间用户调整的尺寸在隐藏/召唤间自动保留
  frame = Frame(title=AppTitle,
                size=(int(453.0 * g_dpiScale), int(377.0 * g_dpiScale)))
  frame.icon = Icon("", 0)
  frame.dpiAutoScale:
    frame.minSize = (320, 260)

  panel = Panel(frame)
  input = TextCtrl(panel, style=wTeMultiLine or wTeRich)
  btnZh2En = Button(panel, label="中 > 英")
  btnEn2Zh = Button(panel, label="英 > 中")
  btnSettings = Button(panel, label="设置")
  btnTrans = Button(panel, label="翻译")
  resultBox = TextCtrl(panel, style=wTeMultiLine or wTeRich or wTeReadOnly)

  frame.wEvent_Size do ():
    layoutMain()
  frame.wEvent_Destroy do ():
    # 兜底：任何销毁路径都保存一次配置
    saveExitState()
  frame.wEvent_Close do (event: wEvent):
    if g_exiting: return
    if g_set.closeExit:
      quitApp()
    else:
      event.veto()
      frame.hide()
  frame.wEvent_HotKey do (event: wEvent):
    case event.getHotkeyId()
    of HkToggle: toggleMain()
    of HkHistUp: histJump(-1)
    of HkHistDown: histJump(1)
    of HkCopy: copyWord(true)
    else: discard
  frame.wEvent_Timer do (event: wEvent):
    # 88 = 标题反馈到期，恢复原标题
    if event.timerId == 88:
      frame.stopTimer(88)
      frame.title = AppTitle
  frame.wEvent_TrayLeftUp do ():
    toggleMain()
  frame.wEvent_TrayRightUp do ():
    showTrayMenu()
  frame.connect(UINT(WmAppTranslated)) do (event: wEvent):
    onTranslated(cast[cstring](event.getlParam()))
  frame.connect(UINT(WmAppUpdate)) do (event: wEvent):
    onUpdateMessage(int event.getwParam(), event.getlParam())

  btnZh2En.wEvent_Button do ():
    doTranslate(FY_ZH2EN)
  btnEn2Zh.wEvent_Button do ():
    doTranslate(FY_EN2ZH)
  btnTrans.wEvent_Button do ():
    doTranslate(FY_AUTO)
  btnSettings.wEvent_Button do ():
    openSettings()
  input.wEvent_TextEnter do ():
    doTranslate(FY_AUTO)
  input.wEvent_LeftDoubleClick do (event: wEvent):
    let ctrlDown = (GetKeyState(VK_CONTROL) and 0x8000) != 0
    copyWord(ctrlDown)
  # 结果区也支持：双击复制原文/英文，Ctrl+双击按命名格式复制
  resultBox.wEvent_LeftDoubleClick do (event: wEvent):
    let ctrlDown = (GetKeyState(VK_CONTROL) and 0x8000) != 0
    copyWord(ctrlDown)

  layoutMain()
  frame.setTrayIcon(Icon("", 0), AppTitle & "（双击显示/隐藏）")

  # 设置窗口（子控件包在 dpiAutoScale 里按 1:1 物理尺寸渲染）
  setFrame = Frame(title=AppTitle & " - 设置",
                   size=(int(560.0 * g_dpiScale), int(420.0 * g_dpiScale)),
                   style=wCaption or wSystemMenu or wMinimizeBox)
  setFrame.icon = Icon("", 0)
  setPanel = Panel(setFrame)
  setFrame.dpiAutoScale:
    let nb = NoteBook(setPanel, pos=(8, 8), size=(544, 340))
    page1 = nb.addPage(text="通用设置")
    page2 = nb.addPage(text="快捷按键")
    page3 = nb.addPage(text="AI 翻译")

    chkTop = CheckBox(page1, label="软件窗口永远在最上面", pos=(20, 14))
    chkCloseExit = CheckBox(page1, label="点击右上角关闭为退出程序（不勾选=隐藏到托盘）", pos=(20, 44))
    chkAutoStart = CheckBox(page1, label="开机自动启动快译NG", pos=(20, 74))
    chkAutoHide = CheckBox(page1, label="开机启动后隐藏界面", pos=(40, 102))
    chkSummon = CheckBox(page1, label="热键召唤窗口时出现在鼠标旁边", pos=(20, 132))

    discard StaticText(page1, label="缓存目录（历史记录存放位置，可修改）", pos=(20, 162))
    edtCache = TextCtrl(page1, pos=(20, 186), size=(430, 26))
    btnOpenCache = Button(page1, label="查看目录", pos=(460, 184), size=(76, 28))

    discard StaticText(page1, label="复制单词命名格式（Ctrl+双击输入框或热键复制时生效）", pos=(20, 224))
    cmbFmt = ComboBox(page1, style=wCbReadOnly, pos=(20, 248), size=(140, 26))

    btnCheckUpdate = Button(page1, label="检查更新", pos=(20, 288), size=(92, 32))
    lblUpdateStatus = StaticText(page1, label="当前版本 v" & AppVersion,
                                 pos=(124, 296), size=(300, 24))
    btnSaveSet1 = Button(page1, label="保存设置", pos=(436, 288), size=(92, 32))

    discard StaticText(page2, label="显示/隐藏快译", pos=(20, 22))
    hkToggleCtrl = HotkeyCtrl(page2, pos=(180, 16), size=(220, 28))
    discard StaticText(page2, label="上一个历史记录", pos=(20, 62))
    hkUpCtrl = HotkeyCtrl(page2, pos=(180, 56), size=(220, 28))
    discard StaticText(page2, label="下一个历史记录", pos=(20, 102))
    hkDownCtrl = HotkeyCtrl(page2, pos=(180, 96), size=(220, 28))
    discard StaticText(page2, label="复制单词", pos=(20, 142))
    hkCopyCtrl = HotkeyCtrl(page2, pos=(180, 136), size=(220, 28))
    discard StaticText(page2,
      label="点击输入框后直接按下组合键（Ctrl/Shift/Alt 任意组合 + 普通键）",
      pos=(20, 180), size=(500, 26))

    btnResetHot = Button(page2, label="恢复默认设置", pos=(180, 214), size=(120, 30))
    btnSaveSet2 = Button(page2, label="保存设置", pos=(436, 288), size=(92, 32))

    # AI 翻译页
    chkAi = CheckBox(page3, label="使用 AI 接口翻译（OpenAI 格式；未启用或密钥为空时用必应免费）",
                     pos=(20, 16))
    cmbAiPreset = ComboBox(page3, style=wCbReadOnly, pos=(360, 12), size=(160, 26))
    discard StaticText(page3, label="接口地址（如 https://api.deepseek.com，可带 /v1）", pos=(20, 54))
    edtAiUrl = TextCtrl(page3, pos=(20, 76), size=(500, 26))
    discard StaticText(page3, label="API 密钥（sk-…）", pos=(20, 114))
    edtAiKey = TextCtrl(page3, pos=(20, 136), size=(500, 26))
    discard StaticText(page3, label="模型（如 deepseek-chat）", pos=(20, 174))
    edtAiModel = TextCtrl(page3, pos=(20, 196), size=(500, 26))
    discard StaticText(page3,
      label="支持 DeepSeek / OpenAI / Moonshot / 本地 Ollama 等一切 OpenAI 格式接口；",
      pos=(20, 234), size=(500, 22))
    discard StaticText(page3, label="AI 失败时自动回退必应免费翻译；单词词典查询始终走必应。",
      pos=(20, 256), size=(500, 22))
    btnSaveSet3 = Button(page3, label="保存设置", pos=(436, 288), size=(92, 32))

  cmbFmt.append(FmtNames)
  cmbFmt.setSelection(g_set.wordFmt)
  for p in AiPresets:
    cmbAiPreset.append(if p[0].len == 0: "选择预设…" else: p[0])
  cmbAiPreset.setSelection(0)

  setFrame.wEvent_Close do (event: wEvent):
    event.veto()
    setFrame.hide()
  btnSaveSet1.wEvent_Button do ():
    saveSettings()
  btnSaveSet2.wEvent_Button do ():
    saveSettings()
  btnSaveSet3.wEvent_Button do ():
    saveSettings()
  cmbAiPreset.wEvent_ComboBox do ():
    # 预设填充地址与模型（第 0 项"选择预设…"不动）
    let i = cmbAiPreset.getSelection()
    if i > 0 and i < AiPresets.len:
      edtAiUrl.setValue(AiPresets[i][1])
      edtAiModel.setValue(AiPresets[i][2])
      cmbAiPreset.setSelection(0)
  btnCheckUpdate.wEvent_Button do ():
    if g_checkingUpdate: return
    g_checkingUpdate = true
    btnCheckUpdate.disable()
    lblUpdateStatus.label = "正在检查更新……"
    let info = cast[ptr UpdateInfo](alloc0(sizeof(UpdateInfo)))
    discard CreateThread(nil, 0, updateCheckWorker, cast[LPVOID](info), 0, nil)
  btnOpenCache.wEvent_Button do ():
    createDir(g_set.cacheDir)
    discard ShellExecuteW(0, "open", g_set.cacheDir.newWideCString(), nil, nil,
                          SW_SHOWNORMAL)
  btnResetHot.wEvent_Button do ():
    hkToggleCtrl.setHotkey(origToWin(DefToggle[0]), DefToggle[1])
    hkUpCtrl.setHotkey(origToWin(DefHistUp[0]), DefHistUp[1])
    hkDownCtrl.setHotkey(origToWin(DefHistDown[0]), DefHistDown[1])
    hkCopyCtrl.setHotkey(origToWin(DefCopy[0]), DefCopy[1])

  # 应用启动状态
  let hwnd = cast[HWND](frame.getHandle())
  g_hwndMain = hwnd
  if g_set.topmost:
    discard SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE or SWP_NOSIZE)
  registerHotkeys()

  # 窗口位置：记住的上次位置有效则原位显示，否则屏幕居中
  let px = cfgInt("通用设置", "窗口X", -30000)
  let py = cfgInt("通用设置", "窗口Y", -30000)
  if px > -20000 and py > -20000:
    frame.position = (px, py)
  else:
    frame.center()

  let startHidden = ("-hide" in commandLineParams()) or
                    (cfgBool("通用设置", "开机启动", false) and g_set.autoHide)
  if startHidden:
    frame.hide()
  else:
    frame.show()
    input.setFocus()

  # 测试钩子：-testtraymenu 启动 2 秒后把鼠标移到屏幕中部并弹托盘菜单
  if "-testtraymenu" in commandLineParams():
    proc testLog(msg: string) =
      let f = open(g_appDir / "test.log", fmAppend)
      f.writeLine(msg)
      f.close()
    frame.wEvent_Timer do (event: wEvent):
      frame.stopTimer(event.timerId)
      testLog("timer fired")
      discard SetCursorPos(800, 400)
      sleep(150)
      let mp = wGetMousePosition()
      testLog("mouse=" & $mp.x & "," & $mp.y)
      testLog("before menu")
      showTrayMenu()
      testLog("after menu id")
    frame.startTimer(2.0, 77)

  # 测试钩子：-testopensettings 启动 1.5 秒后自动打开设置窗口
  if "-testopensettings" in commandLineParams():
    frame.wEvent_Timer do (event: wEvent):
      if event.timerId == 66:
        frame.stopTimer(66)
        openSettings()
    frame.startTimer(1.5, 66)

  app.mainLoop()

when isMainModule:
  main()
