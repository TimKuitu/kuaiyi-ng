## 快译NG —— Bing 免费翻译（C 核心 + wNim 界面）
## 中英互译、历史记录、复制单词、全局热键、托盘；缓存目录可改、Bing 免费翻译接口。

import std/[os, strutils, sets, tables, algorithm, math]
import wNim
import winim/lean
import fastcore
import naming_reducer
import dict_list
import snip_ui
import modern_main
import modern_settings

# winim/lean 未包含 shellapi，这里单独引入打开目录所需的函数
proc ShellExecuteW*(hwnd: HWND, lpOperation, lpFile, lpParameters,
                    lpDirectory: LPCWSTR, nShow: cint): HANDLE
    {.stdcall, dynlib: "shell32", importc: "ShellExecuteW".}

const
  AppTitle = "快译NG"
  AppVersion = "2.6.1"
  # wNim 自定义事件占用 WM_APP+0x10..0xE8，自定义消息必须避开（曾因冲突随机崩溃）
  WmAppTranslated = WM_APP + 0x0200
  WmAppUpdate = WM_APP + 0x0201

  # 全局热键 ID
  HkToggle = 1
  HkHistUp = 2
  HkHistDown = 3
  HkCopy = 4
  HkSnip = 5
  HkCapture = 6
  HkPin = 7

  # 热键默认值（功能键 1=Ctrl 2=Shift 4=Alt 按位相加，键码=Windows 虚拟键码）
  DefToggle = (5, 70)
  DefHistUp = (5, 38)
  DefHistDown = (5, 40)
  DefCopy = (5, 67)
  DefSnip = (5, 68)     # Ctrl+Alt+D
  DefCapture = (0, 0x70) # F1
  DefPin = (0, 0x72)     # F3

  # 命名风格下拉（两项）。存配置沿用旧 FY_FMT_* 代码（3=驼峰 4=snake），
  # 老配置里的 0/1/2/5 读取时归到驼峰，不用迁移
  StyleNames = ["snake_case", "驼峰法"]
  StyleCodes = [FY_FMT_SNAKE, FY_FMT_CAMEL]
  # 命名对象下拉，选项序 = naming_reducer.NameKind 枚举序，直接下标配对
  KindNames = ["局部变量", "全局变量", "静态变量", "指针变量",
               "结构体类型", "宏/常量", "函数"]

  # 字体缩放可选档位（百分比）
  UiScalePresets = ["75", "100", "110", "125", "150", "175", "200"]
  ThemePresets = ["跟随系统", "白色", "黑色"]
  MinFontPx = 9.0          # 字号下限，最小档（75%）也有 13px

  RowHeightLogical = 32   # 按钮行高（100% 时的像素，随「字体缩放」放大）

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
    summonClear: bool       # 热键召唤窗口时清空上次的输入和结果
    inputDblClear: bool     # 双击输入区＝清空输入区（关=双击生成命名并复制）
    cacheDir: string
    snipSaveDir: string     # 截图默认保存目录（Ctrl+Shift+S）
    wordFmt: int           # 命名风格（FY_FMT_SNAKE / FY_FMT_CAMEL 两档）
    nameKind: int          # 命名对象（NameKind 枚举序，0=局部变量 … 6=函数）
    aiEnabled: bool         # AI 接口翻译
    aiUrl: string
    aiKey: string
    aiModel: string
    baiduEnabled: bool      # 百度翻译 API
    baiduAppId: string
    baiduKey: string
    baiduUrl: string        # 可选：接口地址（默认官方）
    hkToggle: (int, int)
    hkHistUp: (int, int)
    hkHistDown: (int, int)
    hkCopy: (int, int)
    hkSnip: (int, int)
    hkCapture: (int, int)
    hkPin: (int, int)
    maxWords: int           # 命名缩减预算（config-only，无界面控件）
    maxChars: int
    winW: int               # 主窗口外框尺寸（像素；0 = 默认自适应高度）
    winH: int
    uiScale: int            # 字体缩放（百分比，作用于设置窗口与翻译窗口）
    themeMode: int          # 界面主题：0=跟随系统, 1=白色, 2=黑色
    animEnabled: bool       # 设置界面过渡与悬停动效开关
    setW: int               # 旧版记录过设置窗口尺寸，现在窗口固定尺寸，不再使用
    setH: int

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
  g_uiScale = 1.0             # 界面缩放（来自「字体缩放」档位）
  g_setFont: HFONT = HFONT(0) # 设置窗口自己的字体（随窗口大小变）
  g_setDesignScale = 1.0      # 设置页设计坐标当前已乘过的系数
  g_uiFont: HFONT = HFONT(0)  # 统一界面字体
  g_defWinW = 550             # 默认窗口尺寸（逻辑像素，宽到能放下按钮行两个下拉）
  g_defWinH = 377
  g_lastToEn = false      # 最近一次翻译的目标是否为英文（是则结果区是要复制的英文）
  g_resultValid = false   # 结果区当前是否有有效内容
  g_testTranslate = false # -testtranslate 钩子：自动翻译一次后写 test.log 退出
  g_hwndMain: HWND        # 主窗口句柄（工作线程里 PostMessage 用，避免碰 frame 对象）

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
  g_set.summonClear = cfgBool("通用设置", "召唤清空输入", false)
  g_set.inputDblClear = cfgBool("通用设置", "双击清空输入", false)
  g_set.cacheDir = cfgStr("通用设置", "缓存目录", g_appDir)
  if g_set.cacheDir.len == 0: g_set.cacheDir = g_appDir
  g_set.snipSaveDir = cfgStr("通用设置", "截图保存目录", g_appDir / "screenshots")
  if g_set.snipSaveDir.len == 0: g_set.snipSaveDir = g_appDir / "screenshots"
  g_set.wordFmt = cfgInt("通用设置", "命名格式", FY_FMT_CAMEL)
  # 风格下拉只剩 snake/camel 两档，旧配置里的其它值归到驼峰
  if g_set.wordFmt != FY_FMT_SNAKE and g_set.wordFmt != FY_FMT_CAMEL:
    g_set.wordFmt = FY_FMT_CAMEL
  g_set.nameKind = cfgInt("通用设置", "命名对象", 0)
  if g_set.nameKind < 0 or g_set.nameKind > 6: g_set.nameKind = 0
  g_set.aiEnabled = cfgBool("AI翻译", "启用", false)
  g_set.aiUrl = cfgStr("AI翻译", "地址", "")
  g_set.aiKey = cfgStr("AI翻译", "密钥", "")
  g_set.aiModel = cfgStr("AI翻译", "模型", "deepseek-chat")
  g_set.baiduEnabled = cfgBool("百度翻译", "启用", false)
  g_set.baiduAppId = cfgStr("百度翻译", "AppID", "")
  g_set.baiduKey = cfgStr("百度翻译", "密钥", "")
  g_set.baiduUrl = cfgStr("百度翻译", "地址", "")

  proc hk(key1, key2: string, def: (int, int)): (int, int) =
    (cfgInt("快捷按键", key1, def[0]), cfgInt("快捷按键", key2, def[1]))

  g_set.hkToggle = hk("隐藏界面1", "隐藏界面2", DefToggle)
  g_set.hkHistUp = hk("上历史1", "上历史2", DefHistUp)
  g_set.hkHistDown = hk("下历史1", "下历史2", DefHistDown)
  g_set.hkCopy = hk("复制1", "复制2", DefCopy)
  g_set.hkSnip = hk("截屏翻译1", "截屏翻译2", DefSnip)
  g_set.hkCapture = hk("截屏1", "截屏2", DefCapture)
  g_set.hkPin = hk("贴图1", "贴图2", DefPin)

  # 命名缩减预算（[复制单词] 段，手动编辑 config 可调）
  g_set.maxWords = cfgInt("复制单词", "最大词数", 4)
  g_set.maxChars = cfgInt("复制单词", "最大字符数", 40)
  if g_set.maxWords < 1: g_set.maxWords = 1
  if g_set.maxChars < 4: g_set.maxChars = 4
  g_set.winW = cfgInt("通用设置", "窗口宽", 0)
  g_set.winH = cfgInt("通用设置", "窗口高", 0)
  g_set.setW = cfgInt("通用设置", "设置宽", 0)
  g_set.setH = cfgInt("通用设置", "设置高", 0)
  g_set.uiScale = cfgInt("通用设置", "界面缩放", 100)
  if g_set.uiScale < 75: g_set.uiScale = 75      # 50% 及以下已取消，最小 75%
  if g_set.uiScale > 200: g_set.uiScale = 200
  g_set.themeMode = cfgInt("通用设置", "主题模式", 0)
  if g_set.themeMode < 0 or g_set.themeMode > 2: g_set.themeMode = 0
  g_set.animEnabled = cfgBool("通用设置", "界面动效", true)
  modernSetAnimEnabled(g_set.animEnabled)

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
  panel, setPanel: wPanel
  pageGenBasic, pageGenUi, pageGenBehavior: wPanel
  pageTrans, pageNamingBudget: wPanel
  pageSnip, pageControl, pageStorage, pageAbout: wPanel
  input, resultBox, edtCache, edtSnipSaveDir: wTextCtrl
  edtAiUrl, edtAiKey, edtAiModel: wTextCtrl
  edtBaiduAppId, edtBaiduKey, edtBaiduUrl: wTextCtrl
  edtMaxWords, edtMaxChars: wTextCtrl
  btnOpenCache, btnBrowseSnipDir, btnOpenSnipDir, btnResetHot: wButton
  btnCheckUpdate, btnOpenRepo, btnTrySnip: wButton
  btnTryCapture, btnTryPin, btnClosePins: wButton
  btnToggleAiKey, btnToggleBaiduKey: wButton
  g_aiKeyShown = false
  g_baiduKeyShown = false
  g_loadingSettings = false
  g_syncingHotkeys = false
  g_transSeq = 0
  lblUpdateStatus: wStaticText
  lblHkStatus: array[7, wStaticText]
  lblSnipHkStatus: array[3, wStaticText]
  g_hkOk: array[7, bool] = [true, true, true, true, true, true, true]
  edtWinW, edtWinH: wTextCtrl
  btnResetSize: wButton
  cmbUiScale: wComboBox
  cmbTheme: wComboBox
  cmbAiPreset: wComboBox
  chkTop, chkCloseExit, chkAutoStart, chkAutoHide, chkSummon, chkAi, chkBaidu: wCheckBox
  chkSummonClear, chkDblClear, chkAnim: wCheckBox
  hkToggleCtrl, hkUpCtrl, hkDownCtrl, hkCopyCtrl, hkSnipCtrl, hkCaptureCtrl, hkPinCtrl: wHotkeyCtrl
  hkSnipCapture2, hkSnipPin2, hkSnipTrans2: wHotkeyCtrl
  g_checkingUpdate = false

# ---------------- 自定义缩略 / 忽略 / 常用词（设置页最后三页） ----------------
# 三页都显示「内置 + 自定义」的合并列表，内置行也能改、能删：
#   缩略页删行 = 该词不再缩写（文件里记 `原词=-`）
#   忽略页删行 = 该内置泛词保留（文件里记 `+原词`）
#   常用词页改英文 = 覆盖内置映射（串口=serial port），删行 = 停用该映射（文件里记 `中文=-`），
#                   也可加全新词条（气泵=pump）；自定义英文词自动享受常用词待遇
# 词库列表由 dict_list.nim 统一提供（固定 10 槽位虚拟滚动 + 搜索过滤，无条数上限）。
var
  pageAbbr, pageIgnore, pagePreferred: wPanel
  dlAbbr, dlIgnore, dlPreferred: DictList
  btnAddAbbr, btnAddIgnore, btnSaveAbbr, btnSaveIgnore: wButton
  btnAddPref, btnSavePref: wButton
  lblAbbrStatus, lblIgnoreStatus, lblPrefStatus: wStaticText

proc rowH(): int {.inline.} =
  # 按钮行高：逻辑 32px 按 DPI 放大，跟字号一起变
  max(10, int(float(RowHeightLogical) * g_uiScale))

# —— 统一界面字体 ——
# 字号只跟「字体缩放」走（g_fontScale = 0.9 × 档位）；窗口与控件尺寸固定不变，
# 所以放大字号时字会变大但窗口不动，缩小时反之。
const
  UiFontFace = "Microsoft YaHei UI"
  # 富文本字符格式（winim/lean 没带 richedit.h，这里只补用到的最小定义）
  EmSetCharFormat = WM_USER + 68
  ScfAll = 0x0004
  CfmSize = 0x80000000'i32
  CfmFace = 0x20000000'i32
  UiFontPx = 18.0            # 100% 缩放时的字号（像素）

type CharFormatW = object
  cbSize: UINT
  dwMask: DWORD
  dwEffects: DWORD
  yHeight: LONG
  yOffset: LONG
  crTextColor: COLORREF
  bCharSet: BYTE
  bPitchAndFamily: BYTE
  szFaceName: array[32, WCHAR]

proc makeFontAt(px: float): HFONT =
  var lf: LOGFONTW
  zeroMem(addr lf, sizeof(LOGFONTW))
  lf.lfHeight = -int(max(MinFontPx, px))
  if lf.lfHeight > -5: lf.lfHeight = -5
  lf.lfWeight = FW_NORMAL
  lf.lfCharSet = DEFAULT_CHARSET
  lf.lfQuality = CLEARTYPE_QUALITY
  lf.lfPitchAndFamily = BYTE(DEFAULT_PITCH or FF_DONTCARE)
  for i in 0 ..< UiFontFace.len: lf.lfFaceName[i] = WCHAR(ord(UiFontFace[i]))
  CreateFontIndirectW(addr lf)

proc fontEnumProc2(h: HWND, lp: LPARAM): WINBOOL {.stdcall.} =
  SendMessageW(h, WM_SETFONT, WPARAM(lp), LPARAM(1))
  result = TRUE

proc makeUiFont(): HFONT =
  makeFontAt(UiFontPx * g_uiScale)

proc applyFont(w: wWindow, f: HFONT) =
  ## 只换字体、不动控件几何（设置窗口缩放时用）
  if w == nil or f == HFONT(0): return
  let h = cast[HWND](w.getHandle())
  SendMessageW(h, WM_SETFONT, WPARAM(f), LPARAM(1))
  discard EnumChildWindows(h, fontEnumProc2, LPARAM(f))

proc richFont(h: HWND) =
  ## 富文本控件（输入框/结果框）不吃 WM_SETFONT 的字号，要显式设字符格式。
  ## yHeight 单位是 twips：像素 * 72/96 * 20 = 像素 * 15。
  var cls: array[64, WCHAR]
  let n = GetClassNameW(h, cast[LPWSTR](addr cls[0]), 64)
  if n <= 0: return
  var cn = newString(n)
  for i in 0 ..< n: cn[i] = char(cls[i])
  if not cn.toLowerAscii().startsWith("richedit"): return
  var cf: CharFormatW
  zeroMem(addr cf, sizeof(CharFormatW))
  cf.cbSize = UINT(sizeof(CharFormatW))
  cf.dwMask = cast[DWORD](CfmSize) or cast[DWORD](CfmFace)
  cf.yHeight = LONG(int(max(MinFontPx, UiFontPx * g_uiScale)) * 15)
  for i in 0 ..< UiFontFace.len: cf.szFaceName[i] = WCHAR(ord(UiFontFace[i]))
  discard SendMessageW(h, EmSetCharFormat, WPARAM(ScfAll), cast[LPARAM](addr cf))

proc fontEnumProc(h: HWND, lp: LPARAM): WINBOOL {.stdcall.} =
  SendMessageW(h, WM_SETFONT, WPARAM(lp), LPARAM(1))
  richFont(h)
  result = TRUE

proc applyUiFont(w: wWindow) =
  # 递归套用统一字体；行控件是懒创建的，openSettings 里会再套一次
  if g_uiFont == HFONT(0) or w == nil: return
  let h = w.getHandle()
  SendMessageW(h, WM_SETFONT, WPARAM(g_uiFont), LPARAM(1))
  richFont(h)
  discard EnumChildWindows(h, fontEnumProc, LPARAM(g_uiFont))

proc dpx(v: int): int {.inline.} =
  ## 逻辑像素 → 物理像素（wNim 的 pos/size 收物理像素）
  int(float(v) * g_uiScale)

proc isPlainWord(s: string): bool =
  ## 缩略/忽略词只支持单个英文单词或数字（引擎按词查表，短语永远匹配不到）
  if s.len == 0: return false
  for c in s:
    if not (c in {'a'..'z', 'A'..'Z', '0'..'9'}): return false
  true

proc warnBox(msg: string) =
  discard MessageBoxW(cast[HWND](setFrame.getHandle()), msg.newWideCString(),
                      AppTitle.newWideCString(), MB_OK or MB_ICONWARNING)

# —— 鼠标滚轮 ——
# wNim 不派发滚轮事件，且 WM_MOUSEWHEEL 会先落到光标下的 Edit/Button 上被吃掉，
# 所以给行控件和页面本体各挂一层 comctl32 subclass，转成列表滚动（一格 3 行）。
const
  WmMouseWheel = 0x020A
  WheelSubId = 0x4B59        # 本程序的 subclass id（comctl32 按窗口区分）
  # 注意：这两个 tag 必须用常量名传参。老版本用过 dpiAutoScale 宏（会把块里的整数
  # 字面量整体乘缩放系数），直接写 hookWheel(pageAbbr, 1) 在非 100% 下会变成 2。
  WheelTagAbbr = 1
  WheelTagIgnore = 2
  WheelTagPreferred = 3

type WheelProc = proc (hWnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM,
                       uIdSubclass: UINT_PTR, dwRefData: DWORD_PTR): LRESULT {.stdcall.}

proc SetWindowSubclass(hWnd: HWND, pfn: WheelProc, id: UINT_PTR,
                       refData: DWORD_PTR): WINBOOL
    {.stdcall, dynlib: "comctl32", importc.}
proc DefSubclassProc(hWnd: HWND, uMsg: UINT, wParam: WPARAM,
                     lParam: LPARAM): LRESULT
    {.stdcall, dynlib: "comctl32", importc.}

proc wheelSubclass(hWnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM,
                   uIdSubclass: UINT_PTR, dwRefData: DWORD_PTR): LRESULT {.stdcall.} =
  if uMsg == WmMouseWheel:
    var d = int((wParam shr 16) and WPARAM(0xFFFF))
    if d >= 0x8000: d -= 0x10000      # 高位是带符号的滚动增量，一格 120
    let n = d div 120
    if n != 0:
      if dwRefData == DWORD_PTR(WheelTagAbbr) and dlAbbr != nil: dlAbbr.scrollBy(n)
      elif dwRefData == DWORD_PTR(WheelTagIgnore) and dlIgnore != nil: dlIgnore.scrollBy(n)
      elif dwRefData == DWORD_PTR(WheelTagPreferred) and dlPreferred != nil: dlPreferred.scrollBy(n)
      return 0
  result = DefSubclassProc(hWnd, uMsg, wParam, lParam)

proc hookWheel(w: wWindow, tag: int) =
  if w != nil:
    discard SetWindowSubclass(w.getHandle(), wheelSubclass,
                              UINT_PTR(WheelSubId), DWORD_PTR(tag))

# —— 缩略词页（业务逻辑；UI 槽位与搜索由 dict_list.nim 统一托管）——
proc abbrMerge(): seq[DictItem] =
  ## 内置 + 自定义合并：自定义覆盖内置，"-" = 禁用（不出行），按原词排序
  var t = initTable[string, string]()
  for (w, a) in builtinAbbreviations(): t[w] = a
  for (w, a) in customAbbrEntries():
    if a == "-": t.del(w)
    else: t[w] = a
  result = @[]
  for k, v in t: result.add((k, v))
  result.sort(proc (x, y: DictItem): int = cmp(x.key, y.key))

proc abbrSummary(total: int): string =
  let builtin = builtinAbbreviations().len
  let custom = customAbbrPairs().len
  let disabled = customAbbrEntries().len - custom
  result = "共 " & $total & " 条：内置 " & $builtin & "，自定义 " & $custom
  if disabled > 0: result.add("，禁用 " & $disabled)

proc loadAbbrRows(preserveScroll = false) =
  ## 打开设置窗口 / 保存后按生效表回填（内置 + 自定义，未保存的编辑丢弃）
  let pairs = abbrMerge()
  if dlAbbr != nil: dlAbbr.setItems(pairs, g_uiScale, preserveScroll)
  if lblAbbrStatus != nil: lblAbbrStatus.label = abbrSummary(pairs.len)

proc saveAbbrRows(): bool =
  if dlAbbr.isNil: return false
  var rows: seq[(string, string)] = @[]
  var seen = initHashSet[string]()
  var err = ""
  for i, it in dlAbbr.items:
    let w = it.key.strip().toLowerAscii()
    let a = it.val.strip().toLowerAscii()
    if w.len == 0 and a.len == 0: continue
    if w.len == 0 or a.len == 0:
      err = "第 " & $(i + 1) & " 行只填了一个框，原词和缩写都要填。"
    elif not isPlainWord(w):
      err = "第 " & $(i + 1) & " 行的原词只能是英文字母/数字（空格、中文不支持）。"
    elif not isPlainWord(a):
      err = "第 " & $(i + 1) & " 行的缩写只能是英文字母/数字。"
    elif a.len > 12:
      err = "第 " & $(i + 1) & " 行的缩写太长（最多 12 个字符）。"
    elif seen.contains(w):
      err = "「" & w & "」重复了（第 " & $(i + 1) & " 行），同一原词只留一条。"
    if err.len > 0: break
    seen.incl(w)
    rows.add((w, a))
  if err.len > 0:
    warnBox(err)
    lblAbbrStatus.label = "未保存：" & err
    return false

  # 只写与内置不同的条目；被删掉的内置行写成 "-"（= 该词不再缩写）
  let builtin = builtinAbbreviations()
  var bmap = initTable[string, string]()
  for (w, a) in builtin: bmap[w] = a
  var lines = @["# 快译NG 自定义缩写：每行 原词=缩写（设置页保存时自动重写）",
                "# 原词=- 表示禁用内置缩写；只记录与内置不同的条目"]
  for (w, a) in rows:
    if bmap.hasKey(w) and bmap[w] == a: continue
    lines.add(w & "=" & a)
  for (w, a) in builtin:
    if not seen.contains(w): lines.add(w & "=-")
  let r = saveCustomAbbreviations(g_appDir / "custom_abbr.txt",
                                  lines.join("\n") & "\n")
  if r.badLine > 0:
    warnBox("保存失败：第 " & $r.badLine & " 行格式不对。")
    return false
  loadAbbrRows(preserveScroll = true)
  let dis = customAbbrEntries().len - customAbbrPairs().len
  var msg = "已保存 " & $rows.len & " 条，立即生效"
  if dis > 0: msg.add("（禁用 " & $dis & "）")
  lblAbbrStatus.label = msg
  fy_log_write("设置", "自定义缩写保存 %d 条 禁用 %d 条",
               cint(rows.len), cint(dis))
  true

# —— 忽略词页（业务逻辑；UI 槽位与搜索由 dict_list.nim 统一托管）——
proc ignoreMerge(): seq[DictItem] =
  ## 内置泛词 + 自定义忽略，去掉被显式保留（+word）的，按字母序
  let keep = toHashSet(customKeepWords())
  var seen = initHashSet[string]()
  var words: seq[string] = @[]
  for w in builtinStopWords():
    if w in keep or seen.contains(w): continue
    seen.incl(w); words.add(w)
  for w in customIgnoreWords():
    if w in keep or seen.contains(w): continue
    seen.incl(w); words.add(w)
  words.sort()
  result = @[]
  for w in words: result.add((w, ""))

proc ignoreSummary(total: int): string =
  let builtin = builtinStopWords().len
  let custom = customIgnoreWords().len
  let kept = customKeepWords().len
  result = "共 " & $total & " 条：内置 " & $builtin & "，自定义 " & $custom
  if kept > 0: result.add("，保留 " & $kept)

proc loadIgnoreRows(preserveScroll = false) =
  ## 打开设置窗口 / 保存后按生效表回填（内置泛词 + 自定义忽略）
  let words = ignoreMerge()
  if dlIgnore != nil: dlIgnore.setItems(words, g_uiScale, preserveScroll)
  if lblIgnoreStatus != nil: lblIgnoreStatus.label = ignoreSummary(words.len)

proc saveIgnoreRows(): bool =
  if dlIgnore.isNil: return false
  var words: seq[string] = @[]
  var seen = initHashSet[string]()
  var err = ""
  for i, it in dlIgnore.items:
    let w = it.key.strip().toLowerAscii()
    if w.len == 0: continue
    if not isPlainWord(w):
      err = "第 " & $(i + 1) & " 行只能是英文字母/数字（中文词请先在缩略页映射）。"
      break
    if seen.contains(w): continue      # 重复的忽略词无副作用，保存时去重
    seen.incl(w)
    words.add(w)
  if err.len > 0:
    warnBox(err)
    lblIgnoreStatus.label = "未保存：" & err
    return false

  # 内置泛词不用写；被删掉的内置行写 "+原词" = 保留；其余是自定义忽略词
  let builtin = builtinStopWords()
  let bset = toHashSet(builtin)
  var lines = @["# 快译NG 自定义忽略词：每行一个英文单词（设置页保存时自动重写）",
                "# +原词 表示保留该词（内置泛词里本来会被删掉）"]
  for w in words:
    if not bset.contains(w): lines.add(w)
  for w in builtin:
    if not seen.contains(w): lines.add("+" & w)
  let n = saveCustomIgnore(g_appDir / "custom_ignore.txt",
                           lines.join("\n") & "\n")
  loadIgnoreRows(preserveScroll = true)
  let kept = customKeepWords().len
  var msg = "已保存 " & $words.len & " 条，立即生效"
  if kept > 0: msg.add("（保留 " & $kept & "）")
  lblIgnoreStatus.label = msg
  fy_log_write("设置", "自定义忽略词保存 %d 条 保留 %d 条",
               cint(n), cint(kept))
  true

# —— 常用词页（业务逻辑；UI 槽位与搜索由 dict_list.nim 统一托管）——
proc prefMergeItems(): seq[DictItem] =
  result = @[]
  for (zh, en) in preferredMerge(): result.add((zh, en))

proc prefSummary(total: int): string =
  let builtin = builtinPreferredPairs().len
  let custom = customPreferredPairs().len
  let disabled = customPreferredEntries().len - custom
  result = "共 " & $total & " 条：内置 " & $builtin & "，自定义 " & $custom
  if disabled > 0: result.add("，停用 " & $disabled)

proc loadPrefRows(preserveScroll = false) =
  ## 打开设置窗口 / 保存后按生效表回填（内置 + 自定义，未保存的编辑丢弃）
  let pairs = prefMergeItems()
  if dlPreferred != nil: dlPreferred.setItems(pairs, g_uiScale, preserveScroll)
  if lblPrefStatus != nil: lblPrefStatus.label = prefSummary(pairs.len)

proc zhRuneLen(s: string): int =
  # UTF-8 字数（不含续字节）；main.nim 不引 unicode，这里按字节判
  for c in s:
    if (ord(c) and 0xC0) != 0x80: inc result

proc zhHasCjk(s: string): bool =
  # 是否含非 ASCII 字符（按字节判，有即视作中文词）
  for c in s:
    if ord(c) >= 0x80: return true
  false

proc savePrefRows(): bool =
  if dlPreferred.isNil: return false
  var rows: seq[(string, string)] = @[]
  var seen = initHashSet[string]()
  var err = ""
  for i, it in dlPreferred.items:
    let zh = it.key.strip()
    let en = it.val.strip()
    if zh.len == 0 and en.len == 0: continue
    if zh.len == 0 or en.len == 0:
      err = "第 " & $(i + 1) & " 行只填了一个框，中文和英文都要填。"
    elif zh.find('=') >= 0:
      err = "第 " & $(i + 1) & " 行的中文里不能有 =。"
    elif zhRuneLen(zh) > 8:
      err = "第 " & $(i + 1) & " 行的中文太长（最多 8 个字）。"
    elif not zhHasCjk(zh):
      err = "第 " & $(i + 1) & " 行的中文要填中文词（纯英文匹配不到词表）。"
    elif builtinZhKind(zh) == 2:
      err = "「" & zh & "」是内置普通词条，不归常用词页管（本页只管词库用词）。"
    elif en != "-":
      # 英文可以是多词（空格分隔），每个词纯字母/数字
      var words = 0
      for tok in en.split(' '):
        if tok.len == 0: continue
        inc words
        if not isPlainWord(tok):
          err = "第 " & $(i + 1) & " 行的英文只能是字母/数字（可多词，空格分隔）。"
          break
        if tok.len > 20:
          err = "第 " & $(i + 1) & " 行的英文单词太长（" & tok & "）。"
          break
      if err.len == 0 and words == 0:
        err = "第 " & $(i + 1) & " 行的英文要填内容。"
      elif err.len == 0 and words > 4:
        err = "第 " & $(i + 1) & " 行的英文最多 4 个词。"
    elif en == "-" and builtinZhKind(zh) != 1:
      err = "「" & zh & "」不是内置词库词条，用不着停用（直接删行即可）。"
    if seen.contains(zh) and err.len == 0:
      err = "「" & zh & "」重复了（第 " & $(i + 1) & " 行），同一中文只留一条。"
    if err.len > 0: break
    seen.incl(zh)
    rows.add((zh, en))
  if err.len > 0:
    warnBox(err)
    lblPrefStatus.label = "未保存：" & err
    return false

  # 只写与内置不同的条目；被删掉的内置行写成 中文=-（= 该映射停用）
  let builtin = builtinPreferredPairs()
  var bmap = initTable[string, string]()
  for (zh, en) in builtin: bmap[zh] = en
  var lines = @["# 快译NG 自定义常用词：每行 中文=英文（设置页保存时自动重写）",
                "# 英文可多词（空格分隔）；中文=- 表示停用内置映射；只记录与内置不同的条目"]
  for (zh, en) in rows:
    if bmap.hasKey(zh) and bmap[zh] == en: continue
    lines.add(zh & "=" & en)
  for (zh, en) in builtin:
    if not seen.contains(zh): lines.add(zh & "=-")
  let r = saveCustomPreferred(g_appDir / "custom_preferred.txt",
                              lines.join("\n") & "\n")
  if r.badLine > 0:
    warnBox("保存失败：第 " & $r.badLine & " 行格式不对。")
    return false
  loadPrefRows(preserveScroll = true)
  let dis = customPreferredEntries().len - customPreferredPairs().len
  var msg = "已保存 " & $rows.len & " 条，立即生效"
  if dis > 0: msg.add("（停用 " & $dis & "）")
  lblPrefStatus.label = msg
  fy_log_write("设置", "自定义常用词保存 %d 条 停用 %d 条",
               cint(rows.len), cint(dis))
  true

# ---------------- 剪贴板 ----------------
proc copyText(s: string) =
  if s.len == 0: return
  discard wSetClipboard(DataObject(s))

proc flashTitle(msg: string) =
  # 短暂在标题栏显示反馈，2 秒后自动恢复
  frame.title = AppTitle & "  " & msg
  modernSetHint(msg)
  frame.startTimer(2.0, 88)

# ---------------- 翻译（后台线程） ----------------
type TranslateJob = object
  seq: int
  text: cstring
  mode: cint
  useAi: bool
  aiUrl: cstring
  aiKey: cstring
  aiModel: cstring
  useBaidu: bool
  baiduAppId: cstring
  baiduKey: cstring
  baiduUrl: cstring

proc translateWorker(param: LPVOID): DWORD {.stdcall.} =
  # 本线程由 CreateThread 创建，不经 Nim 运行时托管——
  # 只调用 C 核心（回退链在 C 内完成），不触碰 Nim GC。
  # 窗口句柄用 g_hwndMain（纯值拷贝），不在裸线程里调 frame 的方法
  let job = cast[ptr TranslateJob](param)
  let seq = job.seq
  let res = fy_translate_chain(job.text, job.mode,
                               job.aiUrl, job.aiKey, job.aiModel,
                               job.baiduAppId, job.baiduKey, job.baiduUrl)
  fy_free(job.text)
  fy_free(job.aiUrl)
  fy_free(job.aiKey)
  fy_free(job.aiModel)
  fy_free(job.baiduAppId)
  fy_free(job.baiduKey)
  fy_free(job.baiduUrl)
  dealloc(job)
  PostMessageW(g_hwndMain, UINT(WmAppTranslated), WPARAM(seq), cast[LPARAM](res))
  return 0

proc cancelTranslate() =
  ## 软取消当前等待：递增序号使后台旧回调结果到达时自动丢弃，立即恢复按钮
  if not g_translating: return
  inc g_transSeq
  g_translating = false
  modernRefreshState()
  resultBox.setValue("已取消本次翻译")
  fy_log_write("翻译", "用户取消等待 seq=%d", cint(g_transSeq))

proc doTranslate(mode: int) =
  if g_translating or frame.isNil: return
  let text = input.getValue().strip()
  if text.len == 0: return
  inc g_transSeq
  g_translating = true
  g_resultValid = false
  # 记录本次翻译目标：中→英时结果区是英文（复制目标）
  g_lastToEn = (mode == FY_ZH2EN) or (mode == FY_AUTO and nimHasCjk(text))
  resultBox.setValue("正在翻译……")
  modernRefreshState()
  # 中文输入不用 AI，直接走 百度 → 必应 二级链（useAi 置假时 C 核心自动跳过 AI 段）
  let useAi = (not nimHasCjk(text)) and g_set.aiEnabled and
              g_set.aiKey.len > 0 and g_set.aiUrl.len > 0
  let useBaidu = g_set.baiduEnabled and g_set.baiduAppId.len > 0 and
                 g_set.baiduKey.len > 0
  fy_log_write("翻译", "开始 seq=%d mode=%d 文本%u字节 预览=%.48s AI=%d 百度=%d",
               cint(g_transSeq), cint(mode), cuint(text.len), text.cstring,
               cint(ord(useAi)), cint(ord(useBaidu)))
  let job = cast[ptr TranslateJob](alloc0(sizeof(TranslateJob)))
  job.seq = g_transSeq
  job.text = fy_strdup(text.cstring)
  job.mode = cint(mode)
  job.useAi = useAi
  job.aiUrl = fy_strdup(if useAi: g_set.aiUrl else: "")
  job.aiKey = fy_strdup(if useAi: g_set.aiKey else: "")
  job.aiModel = fy_strdup(if useAi: g_set.aiModel else: "")
  job.useBaidu = useBaidu
  job.baiduAppId = fy_strdup(if useBaidu: g_set.baiduAppId else: "")
  job.baiduKey = fy_strdup(if useBaidu: g_set.baiduKey else: "")
  job.baiduUrl = fy_strdup(if useBaidu: g_set.baiduUrl else: "")
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
    # NSIS 规定 /D= 放最后且不加引号，确保自动更新直接定位到当前安装目录
    let updParam = "/D=" & getAppDir()
    discard ShellExecuteW(0, "open", newWideCString($cast[cstring](addr info.path)),
                          newWideCString(updParam), nil, SW_SHOWNORMAL)
    dealloc(info)
  of UpdStageFail:
    let info = cast[ptr UpdateInfo](lp)
    lblUpdateStatus.label = "下载失败：" & $cast[cstring](addr info.err)
    dealloc(info)
  else:
    discard

proc quitApp()   # 前置声明：-testtranslate 钩子在 onTranslated 里提前用到

proc onTranslated(seq: int, res: cstring) =
  if seq != g_transSeq:
    if not res.isNil: fy_free(cast[pointer](res))
    return
  g_translating = false
  modernRefreshState()
  if res.isNil:
    resultBox.setValue("翻译失败\n" & $fy_last_error())
    g_resultValid = false
    fy_log_write("翻译", "失败")
    if g_testTranslate:
      let f = open(g_appDir / "test.log", fmAppend)
      f.writeLine("TEST-RESULT: FAIL " & $fy_last_error())
      f.close()
      quitApp()
    return
  resultBox.setValue($res)
  g_resultValid = true
  fy_free(cast[pointer](res))
  let eng = $fy_last_engine()
  if eng.len > 0: flashTitle("（" & eng & "翻译）")
  let text = input.getValue().strip()
  if text.len != 0:
    fy_hist_push(g_hist, text.cstring)
    g_histIdx = -1
  fy_log_write("翻译", "完成 引擎=%s 结果%u字节", eng.cstring, cuint(resultBox.getValue().len))
  if g_testTranslate:
    let f = open(g_appDir / "test.log", fmAppend)
    f.writeLine("TEST-RESULT: OK " & resultBox.getValue())
    f.close()
    quitApp()

# ---------------- 布局 ----------------
proc layoutMain() =
  let cs = frame.getClientSize()
  let w = cs.width
  let h = cs.height
  panel.setSize((0, 0), (w, h))
  modernLayoutMain(w, h, g_uiScale, rowH())

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

proc toggleMain(fromHotkey = false) =
  let hwnd = cast[HWND](frame.getHandle())
  if IsWindowVisible(hwnd) != 0 and IsIconic(hwnd) == 0:
    frame.hide()
  else:
    if g_set.summonNearMouse:
      moveToMouse()
    if fromHotkey and g_set.summonClear:
      input.setValue("")          # 召唤即清空上次输入和结果，直接敲新内容
      resultBox.setValue("")
      g_resultValid = false
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

# ---------------- 命名复制（双击/热键直接出标识符） ----------------
proc namingSourceText(): string =
  # 命名素材：优先输入框原文（中文/英文短语都能拆），输入框空了才取结果区首行
  # （自动跳过 AI/百度失败回退到必应时添加的“（…已用必应翻译）”提示前缀行）
  result = input.getValue().strip()
  if result.len == 0 and g_lastToEn and g_resultValid:
    for line in resultBox.getValue().splitLines():
      let s = line.strip()
      if s.len == 0 or s == "正在翻译……": continue
      if s.startsWith("（") and s.contains("已用必应翻译）"): continue
      return s

proc namingCopy() =
  # 不用先翻译：按「风格 + 对象」两个下拉直接把短语缩成标识符并复制
  # 名字同时回显到结果区（g_lastToEn 置位，再双击结果区也能拿到同一个名字）
  let text = namingSourceText()
  if text.len == 0: return
  let style = if g_set.wordFmt == FY_FMT_SNAKE: nsSnake else: nsCamel
  let kind = NameKind(g_set.nameKind)   # 枚举序即下拉选项序，loadSettings 已钳制
  let r = reduceName(text, style, kind,
                     maxWords = g_set.maxWords, maxChars = g_set.maxChars)
  if r.ok:
    resultBox.setValue(r.name)
    g_resultValid = true
    g_lastToEn = true
    copyText(r.name)
    fy_log_write("命名", "%s -> %s", text.cstring, r.name.cstring)
    flashTitle("（已复制：" & r.name & "）")
  else:
    fy_log_write("命名", "%s -> 失败(%s)", text.cstring, r.reason.cstring)
    flashTitle("（无法缩减：" & r.reason & "）")

# ---------------- 全局热键注册 ----------------
proc hotkeyLabel(hk: (int, int)): string =
  if hk[1] == 0: return ""
  result = wHotkeyToString((int(origToWin(hk[0])), hk[1])).replace(" + ", "+")

proc refreshHotkeyLabels() =
  let cap = hotkeyLabel(g_set.hkCapture)
  let pin = hotkeyLabel(g_set.hkPin)
  let snip = hotkeyLabel(g_set.hkSnip)
  if btnTryCapture != nil:
    btnTryCapture.label = "立即截屏" & (if cap.len > 0: " (" & cap & ")" else: "")
  if btnTryPin != nil:
    btnTryPin.label = "剪贴板贴图" & (if pin.len > 0: " (" & pin & ")" else: "")
  if btnTrySnip != nil:
    btnTrySnip.label = "直接截屏翻译" & (if snip.len > 0: " (" & snip & ")" else: "")

proc syncHotkeyStatusUi() =
  let ctrls = [hkToggleCtrl, hkUpCtrl, hkDownCtrl, hkCopyCtrl, hkSnipCtrl, hkCaptureCtrl, hkPinCtrl]
  let defs = [g_set.hkToggle, g_set.hkHistUp, g_set.hkHistDown, g_set.hkCopy, g_set.hkSnip,
              g_set.hkCapture, g_set.hkPin]
  for i in 0 .. 6:
    if ctrls[i].isNil or lblHkStatus[i].isNil: continue
    let h = cast[HWND](ctrls[i].getHandle())
    if defs[i][1] == 0:
      modernSetEditError(h, false)
      modernMarkStatic(cast[HWND](lblHkStatus[i].getHandle()), inCard=true, role=trHint)
      lblHkStatus[i].label = "未设置"
    elif g_hkOk[i]:
      modernSetEditError(h, false)
      modernMarkStatic(cast[HWND](lblHkStatus[i].getHandle()), inCard=true, role=trSuccess)
      lblHkStatus[i].label = "✓ 生效"
    else:
      modernSetEditError(h, true)
      modernMarkStatic(cast[HWND](lblHkStatus[i].getHandle()), inCard=true, role=trDanger)
      lblHkStatus[i].label = "⚠ 已被占用"
    discard InvalidateRect(cast[HWND](lblHkStatus[i].getHandle()), nil, TRUE)

  let snipCtrls = [hkSnipCapture2, hkSnipPin2, hkSnipTrans2]
  let snipIdx = [5, 6, 4]
  for j in 0 .. 2:
    if snipCtrls[j].isNil or lblSnipHkStatus[j].isNil: continue
    let idx = snipIdx[j]
    let h = cast[HWND](snipCtrls[j].getHandle())
    if defs[idx][1] == 0:
      modernSetEditError(h, false)
      modernMarkStatic(cast[HWND](lblSnipHkStatus[j].getHandle()), inCard=true, role=trHint)
      lblSnipHkStatus[j].label = "未设置"
    elif g_hkOk[idx]:
      modernSetEditError(h, false)
      modernMarkStatic(cast[HWND](lblSnipHkStatus[j].getHandle()), inCard=true, role=trSuccess)
      lblSnipHkStatus[j].label = "✓ 生效"
    else:
      modernSetEditError(h, true)
      modernMarkStatic(cast[HWND](lblSnipHkStatus[j].getHandle()), inCard=true, role=trDanger)
      lblSnipHkStatus[j].label = "⚠ 已被占用"
    discard InvalidateRect(cast[HWND](lblSnipHkStatus[j].getHandle()), nil, TRUE)

proc registerHotkeys(): int {.discardable.} =
  let hwnd = cast[HWND](frame.getHandle())
  discard UnregisterHotKey(hwnd, HkToggle)
  discard UnregisterHotKey(hwnd, HkHistUp)
  discard UnregisterHotKey(hwnd, HkHistDown)
  discard UnregisterHotKey(hwnd, HkCopy)
  discard UnregisterHotKey(hwnd, HkSnip)
  discard UnregisterHotKey(hwnd, HkCapture)
  discard UnregisterHotKey(hwnd, HkPin)
  let hks = [(HkToggle, g_set.hkToggle), (HkHistUp, g_set.hkHistUp),
             (HkHistDown, g_set.hkHistDown), (HkCopy, g_set.hkCopy),
             (HkSnip, g_set.hkSnip), (HkCapture, g_set.hkCapture),
             (HkPin, g_set.hkPin)]
  var log = ""
  var fails = 0
  for idx, pair in hks:
    g_hkOk[idx] = true
    if pair[1][1] != 0:
      let ok = RegisterHotKey(hwnd, UINT(pair[0]), origToWin(pair[1][0]),
                              UINT(pair[1][1]))
      if ok == 0:
        g_hkOk[idx] = false
        inc fails
        log.add("RegisterHotKey id=" & $pair[0] & " mods=" & $pair[1][0] &
                " vk=" & $pair[1][1] & " 失败 err=" & $GetLastError() & "\n")
  if log.len > 0:
    let f = open(g_appDir / "hotkey.log", fmAppend)
    f.writeLine("--- tick=" & $GetTickCount64())
    f.write(log)
    f.close()
    fy_log_write("热键", "注册失败 %d 个（详见 hotkey.log）", cint(fails))
  syncHotkeyStatusUi()
  result = fails

# ---------------- 设置窗口 ----------------
proc syncSizeFields() =
  # 把主窗口当前外框尺寸（像素）回填到设置页
  let sz = frame.getSize()
  edtWinW.setValue($sz.width)
  edtWinH.setValue($sz.height)

# —— 设置窗口缩放 ——
# 窗口拖多大，里面的字号和按钮就等比例多大；按钮的相对位置不变（整体等比缩放，
# 所以按钮的宽高比也不变）。行控件（缩略词/忽略词）由 render* 重排。
var g_setDesign: seq[tuple[h: HWND, x, y, w, hh: int]]
var g_setZoom = 1.0        # 设置页当前实际应用的系数
var g_pageHwnds: seq[HWND] = @[]

proc isSettingsPage(h: HWND): bool =
  for p in g_pageHwnds:
    if p == h: return true

proc setDesignProc(h: HWND, lp: LPARAM): WINBOOL {.stdcall.} =
  if isSettingsPage(h): return TRUE          # 页面本身由笔记本自动撑满
  var r: RECT
  if GetWindowRect(h, addr r) == 0: return TRUE
  let par = GetParent(h)
  var pt: POINT
  pt.x = r.left
  pt.y = r.top
  if par != HWND(0): discard ScreenToClient(par, addr pt)
  g_setDesign.add((h, int(pt.x), int(pt.y), int(r.right - r.left), int(r.bottom - r.top)))
  result = TRUE

proc recordSettingsDesign(f: float) =
  g_setDesign.setLen(0)
  if setPanel != nil:
    discard EnumChildWindows(cast[HWND](setPanel.getHandle()), setDesignProc, LPARAM(0))
  g_setZoom = f

proc defSettingsSize(): tuple[w, h: int] =
  (max(480, int(float(SetWinW) * g_uiScale)), max(380, int(float(SetWinH) * g_uiScale)))

proc nearSize(actual: tuple[w, h: int], expect: tuple[w, h: int], tol = 4): bool =
  abs(actual.w - expect.w) <= tol and abs(actual.h - expect.h) <= tol

proc relayoutSettings(target: float) =
  if setFrame.isNil or g_setDesign.len == 0: return
  modernLayoutSettings(target)
  let k = target / g_setZoom
  if abs(k - 1.0) >= 0.005:
    for e in g_setDesign:
      discard SetWindowPos(e.h, HWND(0),
        int(round(float(e.x) * k)), int(round(float(e.y) * k)),
        int(round(float(e.w) * k)), int(round(float(e.hh) * k)),
        SWP_NOZORDER or SWP_NOACTIVATE)
  g_setZoom = target
  let old = g_setFont
  g_setFont = makeFontAt(UiFontPx * target)
  applyFont(setFrame, g_setFont)
  if setPanel != nil: discard InvalidateRect(cast[HWND](setPanel.getHandle()), nil, FALSE)
  if old != HFONT(0): discard DeleteObject(old)

proc refreshSettings(target: float) =
  ## 应用新的缩放：先缩放静态控件，再按同一系数重排行控件
  relayoutSettings(target)
  let saved = g_uiScale
  g_uiScale = target
  loadAbbrRows()
  loadIgnoreRows()
  loadPrefRows()
  g_uiScale = saved
  recordSettingsDesign(target)
  applyFont(setFrame, g_setFont)
  if setPanel != nil: discard InvalidateRect(cast[HWND](setPanel.getHandle()), nil, FALSE)

proc computeDefWinSize(): tuple[w, h: int] =
  ## 默认窗口尺寸：宽 550（按钮行六控件）、高度刚好放下 标题栏/输入框/按钮行/结果框
  let contentH = modernTitleHeight(g_uiScale) + dpx(96) + dpx(8) + rowH() + dpx(8) + dpx(96) + dpx(10)
  result.w = max(546, int(550.0 * g_uiScale))
  result.h = max(200, contentH)

proc applyWinSize(w, h: int) =
  var wl = float(w)
  var hl = float(h)
  if wl < 546.0: wl = 546.0   # 按钮行放两个下拉后的最窄需求
  if hl < 260.0: hl = 260.0
  frame.size = (int(wl), int(hl))

const EmSetPasswordChar = UINT(0x00CC)

proc setKeyMask(edt: wTextCtrl, btn: wButton, show: bool) =
  ## 切换密钥输入框的明文/掩码显示
  if edt != nil:
    let h = cast[HWND](edt.getHandle())
    discard SendMessageW(h, EmSetPasswordChar,
                         if show: WPARAM(0) else: WPARAM(0x25CF), 0)
    discard InvalidateRect(h, nil, TRUE)
  if btn != nil:
    btn.label = if show: "隐藏" else: "显示"

proc isSystemDarkTheme(): bool =
  ## 读取 Windows 系统应用主题（AppsUseLightTheme=0 为深色，1 为浅色）
  var key: HKEY
  if RegOpenKeyExW(HKEY_CURRENT_USER,
                   r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize".newWideCString(),
                   0, KEY_QUERY_VALUE, addr key) == ERROR_SUCCESS:
    var val: DWORD = 1
    var cb: DWORD = DWORD(sizeof(val))
    var typ: DWORD = 0
    let rc = RegQueryValueExW(key, "AppsUseLightTheme".newWideCString(), nil,
                              addr typ, cast[ptr BYTE](addr val), addr cb)
    RegCloseKey(key)
    if rc == ERROR_SUCCESS and typ == REG_DWORD:
      return val == 0
  result = false

proc resolveThemeDark(mode: int): bool =
  case mode
  of 1: false
  of 2: true
  else: isSystemDarkTheme()

proc applyTheme(mode: int) =
  let dark = resolveThemeDark(mode)
  modernSetDark(dark)
  modernSetSettingsDark(dark)

const DebounceTimerId = UINT_PTR(0x4D88)

proc openSettings() =
  g_loadingSettings = true
  g_aiKeyShown = false
  g_baiduKeyShown = false
  setKeyMask(edtAiKey, btnToggleAiKey, false)
  setKeyMask(edtBaiduKey, btnToggleBaiduKey, false)
  chkTop.setValue(g_set.topmost)
  chkCloseExit.setValue(g_set.closeExit)
  chkAutoStart.setValue(g_set.autoStart)
  chkAutoHide.setValue(g_set.autoHide)
  chkSummon.setValue(g_set.summonNearMouse)
  chkSummonClear.setValue(g_set.summonClear)
  chkDblClear.setValue(g_set.inputDblClear)
  chkAnim.setValue(g_set.animEnabled)
  chkAi.setValue(g_set.aiEnabled)
  edtAiUrl.setValue(g_set.aiUrl)
  edtAiKey.setValue(g_set.aiKey)
  edtAiModel.setValue(g_set.aiModel)
  chkBaidu.setValue(g_set.baiduEnabled)
  edtBaiduAppId.setValue(g_set.baiduAppId)
  edtBaiduKey.setValue(g_set.baiduKey)
  edtBaiduUrl.setValue(g_set.baiduUrl)
  edtMaxWords.setValue($g_set.maxWords)
  edtMaxChars.setValue($g_set.maxChars)
  lblUpdateStatus.label = "当前版本 v" & AppVersion
  edtCache.setValue(g_set.cacheDir)
  edtSnipSaveDir.setValue(g_set.snipSaveDir)
  hkToggleCtrl.changeHotkey(int(origToWin(g_set.hkToggle[0])), g_set.hkToggle[1])
  hkUpCtrl.changeHotkey(int(origToWin(g_set.hkHistUp[0])), g_set.hkHistUp[1])
  hkDownCtrl.changeHotkey(int(origToWin(g_set.hkHistDown[0])), g_set.hkHistDown[1])
  hkCopyCtrl.changeHotkey(int(origToWin(g_set.hkCopy[0])), g_set.hkCopy[1])
  hkSnipCtrl.changeHotkey(int(origToWin(g_set.hkSnip[0])), g_set.hkSnip[1])
  hkCaptureCtrl.changeHotkey(int(origToWin(g_set.hkCapture[0])), g_set.hkCapture[1])
  hkPinCtrl.changeHotkey(int(origToWin(g_set.hkPin[0])), g_set.hkPin[1])
  hkSnipCapture2.changeHotkey(int(origToWin(g_set.hkCapture[0])), g_set.hkCapture[1])
  hkSnipPin2.changeHotkey(int(origToWin(g_set.hkPin[0])), g_set.hkPin[1])
  hkSnipTrans2.changeHotkey(int(origToWin(g_set.hkSnip[0])), g_set.hkSnip[1])
  syncHotkeyStatusUi()
  refreshHotkeyLabels()
  loadAbbrRows()
  loadIgnoreRows()
  loadPrefRows()
  applyUiFont(setFrame)     # 行控件是懒创建的，这里补一次统一字体
  if abs(g_setZoom - g_uiScale) >= 0.01: refreshSettings(g_uiScale)
  else: recordSettingsDesign(g_uiScale)
  var pidx = 0
  for i, pct in UiScalePresets:
    if pct == $g_set.uiScale: pidx = i
  cmbUiScale.setSelection(pidx)
  cmbTheme.setSelection(g_set.themeMode)
  syncSizeFields()
  g_loadingSettings = false
  setFrame.center()
  modernShowSettingsAnimated(setFrame)
  forceForeground(cast[HWND](setFrame.getHandle()))

proc applyGeneralChecks() =
  if g_loadingSettings: return
  g_set.topmost = chkTop.getValue()
  g_set.closeExit = chkCloseExit.getValue()
  g_set.autoStart = chkAutoStart.getValue()
  g_set.autoHide = chkAutoHide.getValue()
  g_set.summonNearMouse = chkSummon.getValue()
  g_set.summonClear = chkSummonClear.getValue()
  g_set.inputDblClear = chkDblClear.getValue()
  fy_cfg_set(g_cfg, "通用设置", "窗口置顶", if g_set.topmost: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "关闭退出", if g_set.closeExit: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "开机启动", if g_set.autoStart: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "开机隐藏", if g_set.autoHide: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "鼠标召唤", if g_set.summonNearMouse: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "召唤清空输入", if g_set.summonClear: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "双击清空输入", if g_set.inputDblClear: "1" else: "0")
  discard fy_cfg_save(g_cfg)
  setAutoStart(g_set.autoStart, g_set.autoHide)
  let hwnd = cast[HWND](frame.getHandle())
  discard SetWindowPos(hwnd, if g_set.topmost: HWND_TOPMOST else: HWND_NOTOPMOST,
                       0, 0, 0, 0, SWP_NOMOVE or SWP_NOSIZE)
  modernShowSavedToast("已保存")

proc applyUiScaleChange() =
  if g_loadingSettings: return
  var newScale = g_set.uiScale
  let sel = cmbUiScale.getSelection()
  if sel >= 0 and sel < UiScalePresets.len:
    try: newScale = parseInt(UiScalePresets[sel])
    except ValueError: return
  if newScale == g_set.uiScale: return
  let oldPct = g_set.uiScale
  g_set.uiScale = newScale
  g_uiScale = float(newScale) / 100.0
  let ratio = float(newScale) / float(max(1, oldPct))
  let oldDef = (g_defWinW, g_defWinH)
  let def = computeDefWinSize()
  let cur = frame.getSize()
  g_uiFont = makeUiFont()
  applyUiFont(frame)
  modernStyleDarkEdit(cast[HWND](input.getHandle()), false)
  modernStyleDarkEdit(cast[HWND](resultBox.getHandle()), true)
  if nearSize((int(cur.width), int(cur.height)), oldDef):
    frame.size = (def.w, def.h)
  else:
    frame.size = (max(def.w, int(round(float(cur.width) * ratio))),
                  max(def.h, int(round(float(cur.height) * ratio))))
  g_defWinW = def.w
  g_defWinH = def.h
  layoutMain()
  g_loadingSettings = true
  syncSizeFields()
  g_loadingSettings = false
  if not setFrame.isNil and setFrame.getHandle() != HWND(0):
    let sc = setFrame.getSize()
    let oldSetDef = (max(480, int(float(SetWinW) * float(oldPct) / 100.0)),
                     max(380, int(float(SetWinH) * float(oldPct) / 100.0)))
    let nd = defSettingsSize()
    var nw = nd.w
    var nh = nd.h
    if not nearSize((int(sc.width), int(sc.height)), oldSetDef):
      nw = max(nd.w, int(round(float(sc.width) * ratio)))
      nh = max(nd.h, int(round(float(sc.height) * ratio)))
    setFrame.minSize = (nw, nh)
    setFrame.maxSize = (nw, nh)
    setFrame.size = (nw, nh)
    refreshSettings(g_uiScale)
  let wsz = frame.getSize()
  g_set.winW = int(wsz.width)
  g_set.winH = int(wsz.height)
  fy_cfg_set(g_cfg, "通用设置", "界面缩放", $g_set.uiScale)
  fy_cfg_set(g_cfg, "通用设置", "窗口宽", $g_set.winW)
  fy_cfg_set(g_cfg, "通用设置", "窗口高", $g_set.winH)
  discard fy_cfg_save(g_cfg)
  modernShowSavedToast("缩放已应用")

proc applyThemeChange() =
  if g_loadingSettings: return
  let tSel = cmbTheme.getSelection()
  if tSel < 0 or tSel > 2 or tSel == g_set.themeMode: return
  g_set.themeMode = tSel
  fy_cfg_set(g_cfg, "通用设置", "主题模式", $g_set.themeMode)
  discard fy_cfg_save(g_cfg)
  applyTheme(g_set.themeMode)
  modernShowSavedToast("主题已切换")

proc applyAnimSetting() =
  if g_loadingSettings: return
  let en = chkAnim.getValue()
  if en == g_set.animEnabled: return
  g_set.animEnabled = en
  modernSetAnimEnabled(en)
  fy_cfg_set(g_cfg, "通用设置", "界面动效", if en: "1" else: "0")
  discard fy_cfg_save(g_cfg)
  modernShowSavedToast(if en: "动效已开启" else: "动效已关闭")

proc applyWinSizeInputs() =
  if g_loadingSettings: return
  var tw = 0
  var th = 0
  try: tw = parseInt(edtWinW.getValue().strip())
  except ValueError: tw = 0
  try: th = parseInt(edtWinH.getValue().strip())
  except ValueError: th = 0
  if tw < 420 or th < 260: return
  applyWinSize(tw, th)
  let wsz = frame.getSize()
  if int(wsz.width) == g_set.winW and int(wsz.height) == g_set.winH: return
  g_set.winW = int(wsz.width)
  g_set.winH = int(wsz.height)
  fy_cfg_set(g_cfg, "通用设置", "窗口宽", $g_set.winW)
  fy_cfg_set(g_cfg, "通用设置", "窗口高", $g_set.winH)
  discard fy_cfg_save(g_cfg)
  modernShowSavedToast("窗口尺寸已保存")

proc applyTransSettings() =
  if g_loadingSettings: return
  let nAiEn = chkAi.getValue()
  let nAiUrl = edtAiUrl.getValue().strip()
  let nAiKey = edtAiKey.getValue().strip()
  var nAiModel = edtAiModel.getValue().strip()
  if nAiModel.len == 0: nAiModel = "deepseek-chat"
  let nBdEn = chkBaidu.getValue()
  let nBdId = edtBaiduAppId.getValue().strip()
  let nBdKey = edtBaiduKey.getValue().strip()
  let nBdUrl = edtBaiduUrl.getValue().strip()
  if nAiEn == g_set.aiEnabled and nAiUrl == g_set.aiUrl and nAiKey == g_set.aiKey and
     nAiModel == g_set.aiModel and nBdEn == g_set.baiduEnabled and
     nBdId == g_set.baiduAppId and nBdKey == g_set.baiduKey and nBdUrl == g_set.baiduUrl:
    return
  g_set.aiEnabled = nAiEn
  g_set.aiUrl = nAiUrl
  g_set.aiKey = nAiKey
  g_set.aiModel = nAiModel
  g_set.baiduEnabled = nBdEn
  g_set.baiduAppId = nBdId
  g_set.baiduKey = nBdKey
  g_set.baiduUrl = nBdUrl
  fy_cfg_set(g_cfg, "AI翻译", "启用", if g_set.aiEnabled: "1" else: "0")
  fy_cfg_set(g_cfg, "AI翻译", "地址", g_set.aiUrl)
  fy_cfg_set(g_cfg, "AI翻译", "密钥", g_set.aiKey)
  fy_cfg_set(g_cfg, "AI翻译", "模型", g_set.aiModel)
  fy_cfg_set(g_cfg, "百度翻译", "启用", if g_set.baiduEnabled: "1" else: "0")
  fy_cfg_set(g_cfg, "百度翻译", "AppID", g_set.baiduAppId)
  fy_cfg_set(g_cfg, "百度翻译", "密钥", g_set.baiduKey)
  fy_cfg_set(g_cfg, "百度翻译", "地址", g_set.baiduUrl)
  discard fy_cfg_save(g_cfg)
  fy_log_write("设置", "即时保存 AI=%d 百度=%d",
               cint(ord(g_set.aiEnabled)), cint(ord(g_set.baiduEnabled)))
  modernShowSavedToast("已保存")

proc applyNamingBudget() =
  if g_loadingSettings: return
  var mw = g_set.maxWords
  var mc = g_set.maxChars
  try: mw = clamp(parseInt(edtMaxWords.getValue().strip()), 1, 16)
  except ValueError: discard
  try: mc = clamp(parseInt(edtMaxChars.getValue().strip()), 8, 128)
  except ValueError: discard
  if mw == g_set.maxWords and mc == g_set.maxChars: return
  g_set.maxWords = mw
  g_set.maxChars = mc
  fy_cfg_set(g_cfg, "复制单词", "最大词数", $g_set.maxWords)
  fy_cfg_set(g_cfg, "复制单词", "最大字符数", $g_set.maxChars)
  discard fy_cfg_save(g_cfg)
  modernShowSavedToast("已保存")

proc applyStorageSettings() =
  if g_loadingSettings: return
  var dir = edtCache.getValue().strip()
  if dir.len == 0: dir = g_appDir
  if dir == g_set.cacheDir: return
  g_set.cacheDir = dir
  fy_cfg_set(g_cfg, "通用设置", "缓存目录", g_set.cacheDir)
  discard fy_cfg_save(g_cfg)
  fy_hist_close(g_hist)
  g_hist = fy_hist_open(g_set.cacheDir.cstring)
  g_histIdx = -1
  modernShowSavedToast("缓存目录已更新")

proc applySnipSaveDir() =
  if g_loadingSettings: return
  var dir = edtSnipSaveDir.getValue().strip()
  if dir.len == 0: dir = g_appDir / "screenshots"
  if dir == g_set.snipSaveDir: return
  g_set.snipSaveDir = dir
  fy_cfg_set(g_cfg, "通用设置", "截图保存目录", g_set.snipSaveDir)
  discard fy_cfg_save(g_cfg)
  modernShowSavedToast("保存目录已更新")

proc applyHotkeysNow(fromSnipPage = false) =
  if g_loadingSettings or g_syncingHotkeys: return
  g_syncingHotkeys = true
  defer: g_syncingHotkeys = false

  if fromSnipPage:
    let sc = hkSnipCapture2.getHotkey()
    let sp = hkSnipPin2.getHotkey()
    let st = hkSnipTrans2.getHotkey()
    hkCaptureCtrl.changeHotkey(sc.modifiers, sc.keyCode)
    hkPinCtrl.changeHotkey(sp.modifiers, sp.keyCode)
    hkSnipCtrl.changeHotkey(st.modifiers, st.keyCode)
  else:
    let sc = hkCaptureCtrl.getHotkey()
    let sp = hkPinCtrl.getHotkey()
    let st = hkSnipCtrl.getHotkey()
    hkSnipCapture2.changeHotkey(sc.modifiers, sc.keyCode)
    hkSnipPin2.changeHotkey(sp.modifiers, sp.keyCode)
    hkSnipTrans2.changeHotkey(st.modifiers, st.keyCode)

  let hkt = hkToggleCtrl.getHotkey()
  let hku = hkUpCtrl.getHotkey()
  let hkd = hkDownCtrl.getHotkey()
  let hkc = hkCopyCtrl.getHotkey()
  let hks = hkSnipCtrl.getHotkey()
  let hkc2 = hkCaptureCtrl.getHotkey()
  let hkp = hkPinCtrl.getHotkey()
  let nt = (winToOrig(hkt.modifiers), hkt.keyCode)
  let nu = (winToOrig(hku.modifiers), hku.keyCode)
  let nd = (winToOrig(hkd.modifiers), hkd.keyCode)
  let nc = (winToOrig(hkc.modifiers), hkc.keyCode)
  let ns = (winToOrig(hks.modifiers), hks.keyCode)
  let ncap = (winToOrig(hkc2.modifiers), hkc2.keyCode)
  let npin = (winToOrig(hkp.modifiers), hkp.keyCode)
  if nt == g_set.hkToggle and nu == g_set.hkHistUp and nd == g_set.hkHistDown and
     nc == g_set.hkCopy and ns == g_set.hkSnip and
     ncap == g_set.hkCapture and npin == g_set.hkPin:
    return
  g_set.hkToggle = nt
  g_set.hkHistUp = nu
  g_set.hkHistDown = nd
  g_set.hkCopy = nc
  g_set.hkSnip = ns
  g_set.hkCapture = ncap
  g_set.hkPin = npin
  fy_cfg_set(g_cfg, "快捷按键", "隐藏界面1", $g_set.hkToggle[0])
  fy_cfg_set(g_cfg, "快捷按键", "隐藏界面2", $g_set.hkToggle[1])
  fy_cfg_set(g_cfg, "快捷按键", "上历史1", $g_set.hkHistUp[0])
  fy_cfg_set(g_cfg, "快捷按键", "上历史2", $g_set.hkHistUp[1])
  fy_cfg_set(g_cfg, "快捷按键", "下历史1", $g_set.hkHistDown[0])
  fy_cfg_set(g_cfg, "快捷按键", "下历史2", $g_set.hkHistDown[1])
  fy_cfg_set(g_cfg, "快捷按键", "复制1", $g_set.hkCopy[0])
  fy_cfg_set(g_cfg, "快捷按键", "复制2", $g_set.hkCopy[1])
  fy_cfg_set(g_cfg, "快捷按键", "截屏翻译1", $g_set.hkSnip[0])
  fy_cfg_set(g_cfg, "快捷按键", "截屏翻译2", $g_set.hkSnip[1])
  fy_cfg_set(g_cfg, "快捷按键", "截屏1", $g_set.hkCapture[0])
  fy_cfg_set(g_cfg, "快捷按键", "截屏2", $g_set.hkCapture[1])
  fy_cfg_set(g_cfg, "快捷按键", "贴图1", $g_set.hkPin[0])
  fy_cfg_set(g_cfg, "快捷按键", "贴图2", $g_set.hkPin[1])
  discard fy_cfg_save(g_cfg)
  let fails = registerHotkeys()
  refreshHotkeyLabels()
  if fails > 0:
    modernShowSavedToast("热键冲突", isError = true)
  else:
    modernShowSavedToast("热键已生效")

proc commitAllPendingEdits() =
  applyWinSizeInputs()
  applyTransSettings()
  applyNamingBudget()
  applyStorageSettings()
  applySnipSaveDir()
  applyHotkeysNow()

proc scheduleEditDebounce() =
  if g_loadingSettings or setFrame.isNil: return
  let h = cast[HWND](setFrame.getHandle())
  if h != 0:
    discard SetTimer(h, DebounceTimerId, 500, nil)

proc saveExitState() =
  # 退出/销毁时只保存窗口位置；尺寸只在设置页点「保存设置」时落盘
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
  fy_log_write("退出", "正常退出")
  saveExitState()
  fy_cfg_close(g_cfg)
  fy_hist_close(g_hist)
  frame.delete()
  # Nim 运行时退出路径可能死锁（实测会留下无窗口的残留进程占用互斥体），
  # 直接结束进程，确保彻底退出
  ExitProcess(0)

proc buildSnipCfg(): SnipCfg =
  let useAi = g_set.aiEnabled and g_set.aiKey.len > 0 and g_set.aiUrl.len > 0
  let useBaidu = g_set.baiduEnabled and g_set.baiduAppId.len > 0 and g_set.baiduKey.len > 0
  SnipCfg(
    aiUrl: if useAi: g_set.aiUrl else: "",
    aiKey: if useAi: g_set.aiKey else: "",
    aiModel: if useAi: g_set.aiModel else: "",
    baiduAppId: if useBaidu: g_set.baiduAppId else: "",
    baiduKey: if useBaidu: g_set.baiduKey else: "",
    baiduUrl: if useBaidu: g_set.baiduUrl else: "",
    cacheDir: g_set.cacheDir,
    saveDir: g_set.snipSaveDir,
    pinMods: g_set.hkPin[0],
    pinVk: g_set.hkPin[1]
  )

proc triggerCapture() =
  startSnipCapture(g_hwndMain, buildSnipCfg(), proc (s: string) = copyText(s),
                   directTranslate = false, notifyCb = proc (msg: string) = flashTitle(msg))

proc triggerPin() =
  pastePinFromClipboard(g_hwndMain, buildSnipCfg(), proc (s: string) = copyText(s),
                        notifyCb = proc (msg: string) = flashTitle(msg))

proc triggerSnip() =
  startSnipCapture(g_hwndMain, buildSnipCfg(), proc (s: string) = copyText(s),
                   directTranslate = true, notifyCb = proc (msg: string) = flashTitle(msg))

proc showTrayMenu() =
  let menu = Menu()
  let capHk = hotkeyLabel(g_set.hkCapture)
  let pinHk = hotkeyLabel(g_set.hkPin)
  let snipHk = hotkeyLabel(g_set.hkSnip)
  menu.append(4, "截屏" & (if capHk.len > 0: " (" & capHk & ")" else: ""))
  menu.append(5, "贴图" & (if pinHk.len > 0: " (" & pinHk & ")" else: ""))
  menu.append(3, "截屏翻译（英→中）" & (if snipHk.len > 0: " (" & snipHk & ")" else: ""))
  menu.appendSeparator()
  menu.append(1, "软件设置")
  menu.append(2, "退出程序")
  # 不传坐标：wNim 会自动用当前鼠标位置（自己传 GetCursorPos 的屏幕坐标
  # 会被 wNim 当作客户区坐标再偏移一次，菜单会被顶到错误位置）
  let id = frame.popupMenu(menu, flag=wPopMenuReturnId)
  case id
  of 4: triggerCapture()
  of 5: triggerPin()
  of 3: triggerSnip()
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

  # 界面缩放**不跟系统 DPI 走**（方案 A：同一套像素在任何 DPI 的机器上长得一样），
  # 而是由设置页「字体缩放」决定，默认 100%。loadSettings 之后立刻取值。
  g_uiScale = 1.0

  g_appDir = getAppDir() / "fastT"
  createDir(g_appDir)

  # 运行日志：上一次会话自动轮换为 run.last.log，闪退时异常码/地址也追加进同一份日志
  fy_log_open((g_appDir / "run.log").cstring)
  fy_log_install_crash_filter()

  g_cfg = fy_cfg_open((g_appDir / "config").cstring)
  loadSettings()
  g_uiScale = float(g_set.uiScale) / 100.0
  applyTheme(g_set.themeMode)
  fy_log_write("启动", "v%s 缓存=%s AI=%d 地址=%s 百度=%d AppID=%s 地址=%s",
               AppVersion.cstring, g_set.cacheDir.cstring,
               cint(ord(g_set.aiEnabled)), g_set.aiUrl.cstring,
               cint(ord(g_set.baiduEnabled)), g_set.baiduAppId.cstring,
               g_set.baiduUrl.cstring)
  g_hist = fy_hist_open(g_set.cacheDir.cstring)

  # 用户自定义缩略词 / 忽略词（设置页最后两页维护，纯文本文件，重启也生效）
  let nAbbr = loadCustomAbbreviations(g_appDir / "custom_abbr.txt")
  let nIgnore = loadCustomIgnore(g_appDir / "custom_ignore.txt")
  let nPref = loadCustomPreferred(g_appDir / "custom_preferred.txt")
  fy_log_write("启动", "自定义缩写 %d 条 忽略词 %d 条 常用词 %d 条",
               cint(nAbbr), cint(nIgnore), cint(nPref))

  # 主窗口：配置存物理像素，换算成 wNim 逻辑值传入
  # 每次启动固定默认大小；运行期间用户调整的尺寸在隐藏/召唤间自动保留
  # 默认宽 550：按钮行要放下 中英/英中/设置/风格下拉/对象下拉/翻译 六个控件
  frame = Frame(title=AppTitle,
                size=(int(550.0 * g_uiScale), int(377.0 * g_uiScale)))
  frame.icon = Icon("", 0)
  frame.minSize = (546, 200)   # 比按钮行最窄需求略宽，不随档位变（档位随时可改）

  panel = Panel(frame)
  input = TextCtrl(panel, style=wTeMultiLine or wTeRich)
  resultBox = TextCtrl(panel, style=wTeMultiLine or wTeRich or wTeReadOnly)

  initModernMain(frame, panel, input, resultBox, ModernHooks(
    onZh2En: proc () = doTranslate(FY_ZH2EN),
    onEn2Zh: proc () = doTranslate(FY_EN2ZH),
    onSettings: proc () = openSettings(),
    onTrans: proc () =
      if g_translating: cancelTranslate()
      else: doTranslate(FY_AUTO),
    onStyleSelect: proc (idx: int) =
      g_set.wordFmt = StyleCodes[idx]
      fy_cfg_set(g_cfg, "通用设置", "命名格式", $g_set.wordFmt)
      discard fy_cfg_save(g_cfg)
      fy_log_write("设置", "命名风格即时切换=%d", cint(g_set.wordFmt)),
    onKindSelect: proc (idx: int) =
      g_set.nameKind = idx
      fy_cfg_set(g_cfg, "通用设置", "命名对象", $g_set.nameKind)
      discard fy_cfg_save(g_cfg)
      fy_log_write("设置", "命名对象即时切换=%d", cint(g_set.nameKind)),
    onToggleTop: proc () =
      g_set.topMost = not g_set.topMost
      let h = cast[HWND](frame.getHandle())
      discard SetWindowPos(h, if g_set.topMost: HWND_TOPMOST else: HWND_NOTOPMOST,
                           0, 0, 0, 0, SWP_NOMOVE or SWP_NOSIZE or SWP_NOACTIVATE)
      fy_cfg_set(g_cfg, "通用设置", "置顶", if g_set.topMost: "1" else: "0")
      discard fy_cfg_save(g_cfg)
      if not chkTop.isNil: chkTop.setValue(g_set.topMost)
      modernRefreshState(),
    onMinimize: proc () =
      discard ShowWindow(cast[HWND](frame.getHandle()), SW_MINIMIZE),
    onClose: proc () =
      if g_set.closeExit:
        quitApp()
      else:
        frame.hide(),
    isTranslating: proc (): bool = g_translating,
    isTopMost: proc (): bool = g_set.topMost,
    getStyleIdx: proc (): int =
      if g_set.wordFmt == FY_FMT_SNAKE: 0 else: 1,
    getKindIdx: proc (): int = g_set.nameKind,
    styleNames: @StyleNames,
    kindNames: @KindNames,
  ))

  # 窗口高度按内容自适应：边距8 + 输入96 + 间距8 + 按钮行 + 间距8 + 结果96 + 边距8
  # 宽度默认 550；用实测的边框/标题栏差值把客户区顶到刚好放下内容
  block:
    # 默认尺寸：宽 453、高度刚好放下内容；设置页保存过就沿用保存值（像素）
    let def = computeDefWinSize()
    g_defWinW = def.w
    g_defWinH = def.h
    var wl = float(g_defWinW)
    var hl = float(g_defWinH)
    if g_set.winW >= 420 and g_set.winH >= 260:
      wl = float(g_set.winW)
      hl = float(g_set.winH)
    # 换过显示器时保存值可能比当前屏还大，按屏幕兜一层
    let maxW = float(GetSystemMetrics(SM_CXSCREEN)) - 16.0
    let maxH = float(GetSystemMetrics(SM_CYSCREEN)) - 48.0
    if wl > maxW: wl = maxW
    if hl > maxH: hl = maxH
    if wl < float(g_defWinW): wl = float(g_defWinW)
    # 保存值是按旧缩放存的，字体放大后可能装不下 → 至少给到内容高度
    if hl < float(g_defWinH): hl = float(g_defWinH)
    frame.size = (int(wl), int(hl))

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
    if modernIsHotkeyFocused():
      return
    case event.getHotkeyId()
    of HkToggle: toggleMain(true)
    of HkHistUp: histJump(-1)
    of HkHistDown: histJump(1)
    of HkCopy: namingCopy()
    of HkSnip: triggerSnip()
    of HkCapture: triggerCapture()
    of HkPin: triggerPin()
    of HkSnipEsc: cancelSnip()
    else: discard
  frame.wEvent_Timer do (event: wEvent):
    # 88 = 标题反馈到期，恢复原标题
    if event.timerId == 88:
      frame.stopTimer(88)
      frame.title = AppTitle
      modernSetHint("")
  frame.wEvent_TrayLeftUp do ():
    toggleMain()
  frame.wEvent_TrayRightUp do ():
    showTrayMenu()
  frame.connect(UINT(WmAppTranslated)) do (event: wEvent):
    onTranslated(int(event.getwParam()), cast[cstring](event.getlParam()))
  frame.connect(UINT(WmAppUpdate)) do (event: wEvent):
    onUpdateMessage(int event.getwParam(), event.getlParam())
  frame.connect(UINT(WmAppSnipDone)) do (event: wEvent):
    onSnipDone(int(event.getwParam()), cast[ptr FySnipResult](event.getlParam()))
  frame.connect(UINT(WmAppPinTransDone)) do (event: wEvent):
    onPinTransDone(event.getwParam(), event.getlParam())
  frame.connect(WM_SETTINGCHANGE) do (event: wEvent):
    if g_set.themeMode == 0:
      let wantDark = isSystemDarkTheme()
      if wantDark != modernIsDark():
        applyTheme(0)

  input.wEvent_TextEnter do ():
    doTranslate(FY_AUTO)
  # 开关开启：双击输入区＝清空输入区；默认仍按当前「风格+对象」直接生成命名并复制
  input.wEvent_LeftDoubleClick do (event: wEvent):
    if g_set.inputDblClear:
      input.setValue("")
      return
    namingCopy()
  resultBox.wEvent_LeftDoubleClick do (event: wEvent):
    namingCopy()

  g_uiFont = makeUiFont()
  applyUiFont(frame)
  modernStyleDarkEdit(cast[HWND](input.getHandle()), false)
  modernStyleDarkEdit(cast[HWND](resultBox.getHandle()), true)
  layoutMain()
  frame.setTrayIcon(Icon("", 0), AppTitle & "（双击显示/隐藏）")

  # 设置窗口：Snipaste 风格左侧导航 + 子标签 + 丝滑过渡动效
  let dw = max(480, int(float(SetWinW) * g_uiScale))
  let dh = max(380, int(float(SetWinH) * g_uiScale))
  setFrame = Frame(title=AppTitle & " - 设置", size=(dw, dh),
                   style=wSystemMenu or wMinimizeBox)
  setFrame.icon = Icon("", 0)
  setFrame.minSize = (dw, dh)
  setFrame.maxSize = (dw, dh)
  setPanel = Panel(setFrame)
  const PadL = SetPadL                  # 20
  const ContW = SetContW                # 544
  const PageW = SetPageW                # 584
  const PageH = SetPageH                # 506
  const CardPadX = 16
  const InnerW = ContW - CardPadX * 2   # 512

  proc mkPage(): wPanel =
    Panel(setPanel, pos=(dpx(SetPageX), dpx(SetPageY)), size=(dpx(PageW), dpx(PageH)))

  pageGenBasic = mkPage()
  pageGenUi = mkPage()
  pageGenBehavior = mkPage()
  pageTrans = mkPage()
  pageNamingBudget = mkPage()
  pageAbbr = mkPage()
  pageIgnore = mkPage()
  pagePreferred = mkPage()
  pageSnip = mkPage()
  pageControl = mkPage()
  pageStorage = mkPage()
  pageAbout = mkPage()

  modernRegisterPage(pageGenBasic, @[
    (PadL, 14, ContW, 128),
    (PadL, 156, ContW, 128)
  ])
  modernRegisterPage(pageGenUi, @[
    (PadL, 14, ContW, 174),
    (PadL, 202, ContW, 114)
  ])
  modernRegisterPage(pageGenBehavior, @[
    (PadL, 14, ContW, 174)
  ])
  modernRegisterPage(pageTrans, @[
    (PadL, 14, ContW, 142),
    (PadL, 168, ContW, 146),
    (PadL, 326, ContW, 108)
  ])
  modernRegisterPage(pageNamingBudget, @[
    (PadL, 14, ContW, 152),
    (PadL, 180, ContW, 128)
  ])
  for dp in [pageAbbr, pageIgnore, pagePreferred]:
    modernRegisterPage(dp, @[(PadL, 8, ContW, 76)])
  modernRegisterPage(pageSnip, @[
    (PadL, 12, ContW, 148),
    (PadL, 168, ContW, 106),
    (PadL, 282, ContW, 212)
  ])
  modernRegisterPage(pageControl, @[
    (PadL, 12, ContW, 436)
  ])
  modernRegisterPage(pageStorage, @[
    (PadL, 14, ContW, 150)
  ])
  modernRegisterPage(pageAbout, @[
    (PadL, 14, ContW, 186)
  ])

  let allSetPages = @[
    pageGenBasic, pageGenUi, pageGenBehavior,
    pageTrans,
    pageNamingBudget, pageAbbr, pageIgnore, pagePreferred,
    pageSnip, pageControl, pageStorage, pageAbout
  ]
  g_pageHwnds = @[]
  for pp in allSetPages:
    g_pageHwnds.add(cast[HWND](pp.getHandle()))

  let cxIn = dpx(PadL + CardPadX)
  let wIn = dpx(InnerW)

  template secTitle(pg: wPanel, txt: string, y0: int) =
    let lblT = StaticText(pg, label="▍ " & txt,
                          pos=(cxIn, dpx(y0)), size=(dpx(300), dpx(24)))
    modernMarkStatic(cast[HWND](lblT.getHandle()), inCard=true, role=trTitle)

  # ---------- 0: 常规 - 基本 ----------
  secTitle(pageGenBasic, "窗口与退出", 26)
  chkTop = CheckBox(pageGenBasic, label="软件窗口永远在最上面",
                    pos=(cxIn, dpx(58)), size=(wIn, dpx(26)))
  chkCloseExit = CheckBox(pageGenBasic, label="点击关闭直接退出程序",
                          pos=(cxIn, dpx(94)), size=(wIn, dpx(26)))
  secTitle(pageGenBasic, "启动与托盘", 168)
  chkAutoStart = CheckBox(pageGenBasic, label="开机自动启动快译NG",
                          pos=(cxIn, dpx(200)), size=(wIn, dpx(26)))
  chkAutoHide = CheckBox(pageGenBasic, label="开机启动后隐藏到托盘",
                         pos=(cxIn, dpx(236)), size=(wIn, dpx(26)))
  let lblAutoSaveHint = StaticText(pageGenBasic,
    label="所有设置修改后立即生效并自动保存，无需点击保存按钮。",
    pos=(dpx(PadL + 4), dpx(300)), size=(dpx(ContW - 8), dpx(22)))
  modernMarkStatic(cast[HWND](lblAutoSaveHint.getHandle()), inCard=false, role=trHint)

  # ---------- 1: 常规 - 界面 ----------
  secTitle(pageGenUi, "外观与缩放", 26)
  let lblTheme = StaticText(pageGenUi, label="界面主题", pos=(cxIn, dpx(62)), size=(dpx(82), dpx(22)))
  modernMarkStatic(cast[HWND](lblTheme.getHandle()), inCard=true, role=trNormal)
  cmbTheme = ComboBox(pageGenUi, style=wCbReadOnly,
                      pos=(dpx(PadL + CardPadX + 90), dpx(58)), size=(dpx(148), dpx(28)))
  for t in ThemePresets: cmbTheme.append(t)
  modernStyleDarkSystem(cast[HWND](cmbTheme.getHandle()), "DarkMode_CFD")

  let lblScl = StaticText(pageGenUi, label="字体缩放", pos=(cxIn, dpx(104)), size=(dpx(82), dpx(22)))
  modernMarkStatic(cast[HWND](lblScl.getHandle()), inCard=true, role=trNormal)
  cmbUiScale = ComboBox(pageGenUi, style=wCbReadOnly,
                        pos=(dpx(PadL + CardPadX + 90), dpx(100)), size=(dpx(148), dpx(28)))
  for pct in UiScalePresets: cmbUiScale.append(pct & "%")
  modernStyleDarkSystem(cast[HWND](cmbUiScale.getHandle()), "DarkMode_CFD")
  let lblSclTip = StaticText(pageGenUi, label="实时等比缩放翻译窗与设置窗",
                             pos=(dpx(PadL + CardPadX + 252), dpx(104)), size=(dpx(250), dpx(22)))
  modernMarkStatic(cast[HWND](lblSclTip.getHandle()), inCard=true, role=trHint)

  chkAnim = CheckBox(pageGenUi, label="启用界面切换与悬停动效",
                     pos=(cxIn, dpx(144)), size=(wIn, dpx(26)))

  secTitle(pageGenUi, "主窗口尺寸", 214)
  let lblWsz = StaticText(pageGenUi, label="窗口大小", pos=(cxIn, dpx(252)), size=(dpx(82), dpx(22)))
  modernMarkStatic(cast[HWND](lblWsz.getHandle()), inCard=true, role=trNormal)
  edtWinW = TextCtrl(pageGenUi, pos=(dpx(PadL + CardPadX + 90), dpx(248)), size=(dpx(74), dpx(28)))
  let lblMul = StaticText(pageGenUi, label="×", pos=(dpx(PadL + CardPadX + 172), dpx(252)), size=(dpx(16), dpx(22)))
  modernMarkStatic(cast[HWND](lblMul.getHandle()), inCard=true, role=trHint)
  edtWinH = TextCtrl(pageGenUi, pos=(dpx(PadL + CardPadX + 194), dpx(248)), size=(dpx(74), dpx(28)))
  let lblPx = StaticText(pageGenUi, label="像素", pos=(dpx(PadL + CardPadX + 276), dpx(252)), size=(dpx(44), dpx(22)))
  modernMarkStatic(cast[HWND](lblPx.getHandle()), inCard=true, role=trHint)
  btnResetSize = Button(pageGenUi, label="恢复默认", pos=(dpx(PadL + CardPadX + 332), dpx(248)),
                        size=(dpx(96), dpx(28)))
  modernStyleButton(btnResetSize, mbNormal, inCard=true)

  # ---------- 2: 常规 - 行为 ----------
  secTitle(pageGenBehavior, "召唤与输入行为", 26)
  chkSummon = CheckBox(pageGenBehavior, label="热键召唤窗口时出现在鼠标旁边",
                       pos=(cxIn, dpx(60)), size=(wIn, dpx(26)))
  chkSummonClear = CheckBox(pageGenBehavior, label="热键召唤窗口时清空输入区和结果区",
                            pos=(cxIn, dpx(98)), size=(wIn, dpx(26)))
  chkDblClear = CheckBox(pageGenBehavior, label="双击输入区＝清空输入区",
                         pos=(cxIn, dpx(136)), size=(wIn, dpx(26)))

  for c in [chkTop, chkCloseExit, chkDblClear, chkAutoStart, chkAutoHide, chkSummon, chkSummonClear, chkAnim]:
    modernStyleCheck(c, inCard=true)
  for e in [edtWinW, edtWinH]:
    modernStyleEdit(e, inCard=true)

  # ---------- 3: 翻译接口 ----------
  chkAi = CheckBox(pageTrans, label="AI 接口（OpenAI 格式）",
                   pos=(cxIn, dpx(26)), size=(dpx(228), dpx(26)))
  modernStyleCheck(chkAi, inCard=true)
  cmbAiPreset = ComboBox(pageTrans, style=wCbReadOnly, pos=(dpx(PadL + CardPadX + 236), dpx(24)),
                         size=(dpx(142), dpx(28)))
  modernStyleDarkSystem(cast[HWND](cmbAiPreset.getHandle()), "DarkMode_CFD")
  let lblAiTip = StaticText(pageTrans, label="地址可带 /v1", pos=(dpx(PadL + CardPadX + 386), dpx(28)),
                            size=(dpx(126), dpx(22)))
  modernMarkStatic(cast[HWND](lblAiTip.getHandle()), inCard=true, role=trHint)

  edtAiUrl = TextCtrl(pageTrans, pos=(cxIn, dpx(64)), size=(dpx(250), dpx(28)))
  let lblAiKey = StaticText(pageTrans, label="密钥", pos=(dpx(PadL + CardPadX + 258), dpx(68)), size=(dpx(36), dpx(22)))
  modernMarkStatic(cast[HWND](lblAiKey.getHandle()), inCard=true, role=trNormal)
  edtAiKey = TextCtrl(pageTrans, pos=(dpx(PadL + CardPadX + 298), dpx(64)), size=(dpx(150), dpx(28)), style=wTePassword)
  btnToggleAiKey = Button(pageTrans, label="显示", pos=(dpx(PadL + CardPadX + 456), dpx(64)), size=(dpx(56), dpx(28)))

  let lblAiModel = StaticText(pageTrans, label="模型", pos=(cxIn, dpx(108)), size=(dpx(40), dpx(22)))
  modernMarkStatic(cast[HWND](lblAiModel.getHandle()), inCard=true, role=trNormal)
  edtAiModel = TextCtrl(pageTrans, pos=(dpx(PadL + CardPadX + 46), dpx(104)), size=(dpx(204), dpx(28)))

  chkBaidu = CheckBox(pageTrans, label="百度翻译开放平台（fanyi-api.baidu.com）",
                      pos=(cxIn, dpx(180)), size=(wIn, dpx(26)))
  modernStyleCheck(chkBaidu, inCard=true)
  let lblBId = StaticText(pageTrans, label="AppID", pos=(cxIn, dpx(220)), size=(dpx(56), dpx(22)))
  modernMarkStatic(cast[HWND](lblBId.getHandle()), inCard=true, role=trNormal)
  edtBaiduAppId = TextCtrl(pageTrans, pos=(dpx(PadL + CardPadX + 60), dpx(216)), size=(dpx(150), dpx(28)))
  let lblBKey = StaticText(pageTrans, label="密钥", pos=(dpx(PadL + CardPadX + 218), dpx(220)), size=(dpx(38), dpx(22)))
  modernMarkStatic(cast[HWND](lblBKey.getHandle()), inCard=true, role=trNormal)
  edtBaiduKey = TextCtrl(pageTrans, pos=(dpx(PadL + CardPadX + 260), dpx(216)), size=(dpx(188), dpx(28)), style=wTePassword)
  btnToggleBaiduKey = Button(pageTrans, label="显示", pos=(dpx(PadL + CardPadX + 456), dpx(216)), size=(dpx(56), dpx(28)))

  let lblBUrl = StaticText(pageTrans, label="接口地址（可选，默认官方）", pos=(cxIn, dpx(260)),
                           size=(dpx(228), dpx(22)))
  modernMarkStatic(cast[HWND](lblBUrl.getHandle()), inCard=true, role=trHint)
  edtBaiduUrl = TextCtrl(pageTrans, pos=(dpx(PadL + CardPadX + 234), dpx(256)), size=(dpx(InnerW - 234), dpx(28)))

  for e in [edtAiUrl, edtAiKey, edtAiModel, edtBaiduAppId, edtBaiduKey, edtBaiduUrl]:
    modernStyleEdit(e, inCard=true)
  modernStyleButton(btnToggleAiKey, mbNormal, inCard=true)
  modernStyleButton(btnToggleBaiduKey, mbNormal, inCard=true)

  let lblNote1 = StaticText(pageTrans,
    label="翻译顺序：AI → 百度 → 必应免费（未启用或失败自动回退）。",
    pos=(cxIn, dpx(338)), size=(wIn, dpx(22)))
  let lblNote2 = StaticText(pageTrans,
    label="单词词典始终走必应；百度免费版每秒 1 次、每月 5 万字符。",
    pos=(cxIn, dpx(366)), size=(wIn, dpx(22)))
  let lblNote3 = StaticText(pageTrans,
    label="Ollama 本地无需密钥；百度需开通“通用文本翻译”权限。",
    pos=(cxIn, dpx(394)), size=(wIn, dpx(22)))
  for ln in [lblNote1, lblNote2, lblNote3]:
    modernMarkStatic(cast[HWND](ln.getHandle()), inCard=true, role=trHint)

  # ---------- 4: 命名 - 缩减预算 ----------
  secTitle(pageNamingBudget, "命名缩减预算", 26)
  let lblMaxW = StaticText(pageNamingBudget, label="最大词数", pos=(cxIn, dpx(64)), size=(dpx(96), dpx(22)))
  modernMarkStatic(cast[HWND](lblMaxW.getHandle()), inCard=true, role=trNormal)
  edtMaxWords = TextCtrl(pageNamingBudget, pos=(dpx(PadL + CardPadX + 104), dpx(60)), size=(dpx(72), dpx(28)))
  let lblMaxWTip = StaticText(pageNamingBudget, label="默认 4，超出自动按优先级裁剪（1~16）",
                              pos=(dpx(PadL + CardPadX + 190), dpx(64)), size=(dpx(316), dpx(22)))
  modernMarkStatic(cast[HWND](lblMaxWTip.getHandle()), inCard=true, role=trHint)

  let lblMaxC = StaticText(pageNamingBudget, label="最大字符数", pos=(cxIn, dpx(106)), size=(dpx(96), dpx(22)))
  modernMarkStatic(cast[HWND](lblMaxC.getHandle()), inCard=true, role=trNormal)
  edtMaxChars = TextCtrl(pageNamingBudget, pos=(dpx(PadL + CardPadX + 104), dpx(102)), size=(dpx(72), dpx(28)))
  let lblMaxCTip = StaticText(pageNamingBudget, label="默认 40，超长时优先触发词尾缩写（8~128）",
                              pos=(dpx(PadL + CardPadX + 190), dpx(106)), size=(dpx(316), dpx(22)))
  modernMarkStatic(cast[HWND](lblMaxCTip.getHandle()), inCard=true, role=trHint)
  for e in [edtMaxWords, edtMaxChars]:
    modernStyleEdit(e, inCard=true)

  secTitle(pageNamingBudget, "规则优先级说明", 192)
  let lblNm1 = StaticText(pageNamingBudget,
    label="优先级：自定义忽略词 ＞ 自定义缩略词 ＞ 常用词库 ＞ 内置规则",
    pos=(cxIn, dpx(226)), size=(wIn, dpx(22)))
  let lblNm2 = StaticText(pageNamingBudget,
    label="命名风格（snake_case / 驼峰法）与命名对象请在主窗口工具栏直接切换。",
    pos=(cxIn, dpx(256)), size=(wIn, dpx(22)))
  for ln in [lblNm1, lblNm2]:
    modernMarkStatic(cast[HWND](ln.getHandle()), inCard=true, role=trHint)

  # ---------- 5/6/7: 命名 - 缩略词 / 忽略词 / 常用词 ----------
  dlAbbr = newDictList(
    pageAbbr, dlTwoCol, WheelTagAbbr, g_uiScale, hookWheel,
    "原词 → 缩写，例 Initialization → Init（内置已列出）",
    "可改可删：删行＝该词不再缩写；空白行忽略，点击「应用修改」生效。",
    btnAddAbbr, btnSaveAbbr, lblAbbrStatus)

  dlIgnore = newDictList(
    pageIgnore, dlOneCol, WheelTagIgnore, g_uiScale, hookWheel,
    "每行一个英文单词，生成命名时直接删掉（内置泛词已列出）",
    "删掉某行＝保留该词；中文先经内置词表转英文再判断，点击「应用修改」生效。",
    btnAddIgnore, btnSaveIgnore, lblIgnoreStatus)

  dlPreferred = newDictList(
    pagePreferred, dlTwoCol, WheelTagPreferred, g_uiScale, hookWheel,
    "每行一条：中文 → 英文词库，例 串口 → uart（内置已列出）",
    "改英文＝覆盖内置映射，删行＝停用；可加全新词条，点击「应用修改」生效。",
    btnAddPref, btnSavePref, lblPrefStatus)

  # ---------- 8: 截屏与贴图（快捷键 · 默认保存位置 · 标注说明） ----------
  secTitle(pageSnip, "截屏与贴图快捷键", 22)
  let snipHkSpecs = [
    ("区域截屏（含工具栏）", 50),
    ("贴图到屏幕", 84),
    ("直接截屏翻译", 118)
  ]
  for i, hs in snipHkSpecs:
    let lbl = StaticText(pageSnip, label=hs[0], pos=(cxIn, dpx(hs[1] + 4)), size=(dpx(166), dpx(22)))
    modernMarkStatic(cast[HWND](lbl.getHandle()), inCard=true, role=trNormal)
    lblSnipHkStatus[i] = StaticText(pageSnip, label="",
                                    pos=(dpx(PadL + CardPadX + 388), dpx(hs[1] + 4)),
                                    size=(dpx(120), dpx(22)))
    modernMarkStatic(cast[HWND](lblSnipHkStatus[i].getHandle()), inCard=true, role=trHint)
  hkSnipCapture2 = HotkeyCtrl(pageSnip, pos=(dpx(PadL + CardPadX + 172), dpx(50)), size=(dpx(206), dpx(28)))
  hkSnipPin2 = HotkeyCtrl(pageSnip, pos=(dpx(PadL + CardPadX + 172), dpx(84)), size=(dpx(206), dpx(28)))
  hkSnipTrans2 = HotkeyCtrl(pageSnip, pos=(dpx(PadL + CardPadX + 172), dpx(118)), size=(dpx(206), dpx(28)))
  for hk in [hkSnipCapture2, hkSnipPin2, hkSnipTrans2]:
    modernStyleHotkeyHwnd(cast[HWND](hk.getHandle()), inCard=true)

  secTitle(pageSnip, "截图默认保存位置", 178)
  let lblSaveDirTip = StaticText(pageSnip,
    label="Ctrl+Shift+S 直接保存到此目录；Ctrl+S 弹出窗口自选位置；Ctrl+C 复制截图",
    pos=(cxIn, dpx(206)), size=(wIn, dpx(22)))
  modernMarkStatic(cast[HWND](lblSaveDirTip.getHandle()), inCard=true, role=trHint)
  edtSnipSaveDir = TextCtrl(pageSnip, pos=(cxIn, dpx(234)), size=(dpx(InnerW - 196), dpx(28)))
  modernStyleEdit(edtSnipSaveDir, inCard=true)
  btnBrowseSnipDir = Button(pageSnip, label="选择目录",
                            pos=(dpx(PadL + CardPadX + InnerW - 186), dpx(234)),
                            size=(dpx(88), dpx(28)))
  btnOpenSnipDir = Button(pageSnip, label="打开目录",
                          pos=(dpx(PadL + CardPadX + InnerW - 90), dpx(234)),
                          size=(dpx(90), dpx(28)))
  modernStyleButton(btnBrowseSnipDir, mbNormal, inCard=true)
  modernStyleButton(btnOpenSnipDir, mbNormal, inCard=true)

  secTitle(pageSnip, "操作与标注说明", 292)
  let lblSnip1 = StaticText(pageSnip,
    label="• 截屏选区：自动识窗、8向调框、放大镜取色（单按 C 复制色值）",
    pos=(cxIn, dpx(320)), size=(wIn, dpx(22)))
  let lblSnip2 = StaticText(pageSnip,
    label="• 标注工具：框/圆/线/箭头/画笔/荧光笔/序号/文字/马赛克/模糊",
    pos=(cxIn, dpx(344)), size=(wIn, dpx(22)))
  let lblSnip3 = StaticText(pageSnip,
    label="• 快捷操作：Ctrl+C 复制、Ctrl+S 另存为、Ctrl+Shift+S 存到默认目录",
    pos=(cxIn, dpx(368)), size=(wIn, dpx(22)))
  let lblSnip4 = StaticText(pageSnip,
    label="• 屏幕贴图：支持图文贴图、多图置顶、滚轮缩放与透明度调节",
    pos=(cxIn, dpx(392)), size=(wIn, dpx(22)))
  for ls in [lblSnip1, lblSnip2, lblSnip3, lblSnip4]:
    modernMarkStatic(cast[HWND](ls.getHandle()), inCard=true, role=trHint)
  btnTryCapture = Button(pageSnip, label="立即截屏 (F1)",
                         pos=(cxIn, dpx(420)), size=(dpx(200), dpx(30)))
  btnTryPin = Button(pageSnip, label="剪贴板贴图 (F3)",
                     pos=(dpx(PadL + CardPadX + 214), dpx(420)), size=(dpx(200), dpx(30)))
  btnTrySnip = Button(pageSnip, label="直接截屏翻译",
                      pos=(cxIn, dpx(456)), size=(dpx(200), dpx(30)))
  btnClosePins = Button(pageSnip, label="关闭所有贴图",
                        pos=(dpx(PadL + CardPadX + 214), dpx(456)), size=(dpx(200), dpx(30)))
  modernStyleButton(btnTryCapture, mbPrimary, inCard=true)
  modernStyleButton(btnTryPin, mbNormal, inCard=true)
  modernStyleButton(btnTrySnip, mbNormal, inCard=true)
  modernStyleButton(btnClosePins, mbDanger, inCard=true)

  # ---------- 9: 控制（全局快捷键 + 冲突实时检测） ----------
  let hkSpecs = [
    ("显示/隐藏快译", 24),
    ("上一个历史记录", 66),
    ("下一个历史记录", 108),
    ("复制单词", 150),
    ("直接截屏翻译", 192),
    ("截屏（含工具栏）", 234),
    ("贴图到屏幕", 276)
  ]
  for i, hs in hkSpecs:
    let lbl = StaticText(pageControl, label=hs[0], pos=(cxIn, dpx(hs[1] + 4)), size=(dpx(166), dpx(24)))
    modernMarkStatic(cast[HWND](lbl.getHandle()), inCard=true, role=trNormal)
    lblHkStatus[i] = StaticText(pageControl, label="",
                                pos=(dpx(PadL + CardPadX + 388), dpx(hs[1] + 4)),
                                size=(dpx(120), dpx(24)))
    modernMarkStatic(cast[HWND](lblHkStatus[i].getHandle()), inCard=true, role=trHint)
  hkToggleCtrl = HotkeyCtrl(pageControl, pos=(dpx(PadL + CardPadX + 172), dpx(24)), size=(dpx(206), dpx(28)))
  hkUpCtrl = HotkeyCtrl(pageControl, pos=(dpx(PadL + CardPadX + 172), dpx(66)), size=(dpx(206), dpx(28)))
  hkDownCtrl = HotkeyCtrl(pageControl, pos=(dpx(PadL + CardPadX + 172), dpx(108)), size=(dpx(206), dpx(28)))
  hkCopyCtrl = HotkeyCtrl(pageControl, pos=(dpx(PadL + CardPadX + 172), dpx(150)), size=(dpx(206), dpx(28)))
  hkSnipCtrl = HotkeyCtrl(pageControl, pos=(dpx(PadL + CardPadX + 172), dpx(192)), size=(dpx(206), dpx(28)))
  hkCaptureCtrl = HotkeyCtrl(pageControl, pos=(dpx(PadL + CardPadX + 172), dpx(234)), size=(dpx(206), dpx(28)))
  hkPinCtrl = HotkeyCtrl(pageControl, pos=(dpx(PadL + CardPadX + 172), dpx(276)), size=(dpx(206), dpx(28)))
  for hk in [hkToggleCtrl, hkUpCtrl, hkDownCtrl, hkCopyCtrl, hkSnipCtrl, hkCaptureCtrl, hkPinCtrl]:
    modernStyleHotkeyHwnd(cast[HWND](hk.getHandle()), inCard=true)
  let lblHkHint = StaticText(pageControl,
    label="按下组合键立即生效，红框表示已被其他软件占用。",
    pos=(cxIn, dpx(322)), size=(wIn, dpx(24)))
  modernMarkStatic(cast[HWND](lblHkHint.getHandle()), inCard=true, role=trHint)
  btnResetHot = Button(pageControl, label="恢复默认热键", pos=(dpx(PadL + CardPadX + 172), dpx(358)),
                       size=(dpx(128), dpx(32)))
  modernStyleButton(btnResetHot, mbNormal, inCard=true)

  # ---------- 10: 存储 ----------
  secTitle(pageStorage, "缓存目录与查询历史", 26)
  let lblCache = StaticText(pageStorage,
    label="查询历史（上限 200 条）和运行日志保存在此目录，修改后立即生效。",
    pos=(cxIn, dpx(56)), size=(wIn, dpx(22)))
  modernMarkStatic(cast[HWND](lblCache.getHandle()), inCard=true, role=trHint)
  edtCache = TextCtrl(pageStorage, pos=(cxIn, dpx(92)), size=(dpx(InnerW - 108), dpx(28)))
  modernStyleEdit(edtCache, inCard=true)
  btnOpenCache = Button(pageStorage, label="打开目录",
                        pos=(dpx(PadL + CardPadX + InnerW - 96), dpx(92)),
                        size=(dpx(96), dpx(28)))
  modernStyleButton(btnOpenCache, mbNormal, inCard=true)

  # ---------- 11: 关于 ----------
  secTitle(pageAbout, "快译NG", 28)
  lblUpdateStatus = StaticText(pageAbout, label="当前版本 v" & AppVersion,
                               pos=(cxIn, dpx(62)), size=(wIn, dpx(24)))
  modernMarkStatic(cast[HWND](lblUpdateStatus.getHandle()), inCard=true, role=trNormal)
  let lblAboutDesc = StaticText(pageAbout,
    label="轻量级 Windows 单文件翻译与编程命名工具（C 核心 + Nim/wNim 原生界面）。",
    pos=(cxIn, dpx(94)), size=(wIn, dpx(24)))
  modernMarkStatic(cast[HWND](lblAboutDesc.getHandle()), inCard=true, role=trHint)
  btnCheckUpdate = Button(pageAbout, label="检查更新", pos=(cxIn, dpx(142)), size=(dpx(104), dpx(32)))
  btnOpenRepo = Button(pageAbout, label="GitHub 主页", pos=(dpx(PadL + CardPadX + 116), dpx(142)),
                       size=(dpx(116), dpx(32)))
  modernStyleButton(btnCheckUpdate, mbPrimary, inCard=true)
  modernStyleButton(btnOpenRepo, mbNormal, inCard=true)

  for p in AiPresets:
    cmbAiPreset.append(if p[0].len == 0: "选择预设…" else: p[0])
  cmbAiPreset.setSelection(0)

  initModernSettings(
    setFrame, setPanel,
    allSetPages,
    g_uiScale,
    onTabChange = proc (idx: int) =
      if idx == 5 and dlAbbr != nil: dlAbbr.render(g_uiScale)
      elif idx == 6 and dlIgnore != nil: dlIgnore.render(g_uiScale)
      elif idx == 7 and dlPreferred != nil: dlPreferred.render(g_uiScale)
      elif idx == 8:
        syncHotkeyStatusUi()
        refreshHotkeyLabels()
      elif idx == 9: syncHotkeyStatusUi(),
    onMin = proc () = setFrame.minimize(),
    onClose = proc () =
      modernClearSettingsFocus()
      commitAllPendingEdits()
      discard fy_cfg_save(g_cfg)
      setFrame.hide(),
    onEditBlur = proc (h: HWND) =
      if g_loadingSettings: return
      if h == cast[HWND](edtWinW.getHandle()) or h == cast[HWND](edtWinH.getHandle()):
        applyWinSizeInputs()
      elif h == cast[HWND](edtCache.getHandle()):
        applyStorageSettings()
      elif h == cast[HWND](edtSnipSaveDir.getHandle()):
        applySnipSaveDir()
      elif h == cast[HWND](edtMaxWords.getHandle()) or h == cast[HWND](edtMaxChars.getHandle()):
        applyNamingBudget()
      elif h == cast[HWND](edtAiUrl.getHandle()) or h == cast[HWND](edtAiKey.getHandle()) or
           h == cast[HWND](edtAiModel.getHandle()) or h == cast[HWND](edtBaiduAppId.getHandle()) or
           h == cast[HWND](edtBaiduKey.getHandle()) or h == cast[HWND](edtBaiduUrl.getHandle()):
        applyTransSettings()
      elif h == cast[HWND](hkToggleCtrl.getHandle()) or h == cast[HWND](hkUpCtrl.getHandle()) or
           h == cast[HWND](hkDownCtrl.getHandle()) or h == cast[HWND](hkCopyCtrl.getHandle()) or
           h == cast[HWND](hkSnipCtrl.getHandle()) or h == cast[HWND](hkCaptureCtrl.getHandle()) or
           h == cast[HWND](hkPinCtrl.getHandle()):
        applyHotkeysNow(false)
      elif h == cast[HWND](hkSnipCapture2.getHandle()) or h == cast[HWND](hkSnipPin2.getHandle()) or
           h == cast[HWND](hkSnipTrans2.getHandle()):
        applyHotkeysNow(true)
  )

  # 悬停 Tooltip（Snipaste 风格：界面保持简洁，悬停查看详细说明）
  modernSetTooltip(cast[HWND](chkTop.getHandle()), "开启后翻译主窗口始终置顶显示在其他窗口上方")
  modernSetTooltip(cast[HWND](chkCloseExit.getHandle()), "开启：点右上角关闭直接退出；关闭：隐藏到系统托盘")
  modernSetTooltip(cast[HWND](chkAutoStart.getHandle()), "写入当前用户启动项，登录 Windows 时自动运行")
  modernSetTooltip(cast[HWND](chkAutoHide.getHandle()), "仅在开启“开机自动启动”后生效，开机后直接静默隐藏到托盘")
  modernSetTooltip(cast[HWND](chkSummon.getHandle()), "按全局热键显示窗口时，自动定位到当前鼠标光标附近")
  modernSetTooltip(cast[HWND](chkSummonClear.getHandle()), "按全局热键召唤主窗口时，自动清空上次的输入与翻译结果")
  modernSetTooltip(cast[HWND](chkDblClear.getHandle()), "开启：双击输入区清空内容；关闭：双击直接按命名规则生成并复制变量名")
  modernSetTooltip(cast[HWND](cmbUiScale.getHandle()), "独立于系统 DPI，整窗按所选百分比等比缩放")
  modernSetTooltip(cast[HWND](chkAnim.getHandle()), "关闭后切页、开关和悬停将立即响应，无过渡动画")
  modernSetTooltip(cast[HWND](edtCache.getHandle()), "存放查询历史（hist.dat）与运行日志（run.log）的目录")
  modernSetTooltip(cast[HWND](edtSnipSaveDir.getHandle()), "按 Ctrl+Shift+S 时截图将直接保存到此目录")

  # 输入框 500ms 去抖定时器
  setFrame.connect(WM_TIMER) do (event: wEvent):
    if event.getwParam() == WPARAM(DebounceTimerId):
      discard KillTimer(cast[HWND](setFrame.getHandle()), DebounceTimerId)
      commitAllPendingEdits()

  # 热键框变更即刻注册并双向同步（使用 wEvent_HotkeyChanged）
  for hk in [hkToggleCtrl, hkUpCtrl, hkDownCtrl, hkCopyCtrl, hkSnipCtrl, hkCaptureCtrl, hkPinCtrl]:
    hk.wEvent_HotkeyChanged do ():
      applyHotkeysNow(false)
  for hk in [hkSnipCapture2, hkSnipPin2, hkSnipTrans2]:
    hk.wEvent_HotkeyChanged do ():
      applyHotkeysNow(true)

  setFrame.wEvent_Close do (event: wEvent):
    modernClearSettingsFocus()
    commitAllPendingEdits()
    discard fy_cfg_save(g_cfg)
    event.veto()
    setFrame.hide()

  # 即改即生效：复选框 / 下拉框 / 输入框事件
  for chk in [chkTop, chkCloseExit, chkAutoStart, chkAutoHide, chkSummon, chkSummonClear, chkDblClear]:
    chk.wEvent_CheckBox do (): applyGeneralChecks()
  chkAnim.wEvent_CheckBox do (): applyAnimSetting()
  chkAi.wEvent_CheckBox do (): applyTransSettings()
  chkBaidu.wEvent_CheckBox do (): applyTransSettings()
  cmbUiScale.wEvent_ComboBox do (): applyUiScaleChange()
  cmbTheme.wEvent_ComboBox do (): applyThemeChange()
  for edt in [edtWinW, edtWinH, edtCache, edtSnipSaveDir, edtAiUrl, edtAiKey, edtAiModel,
              edtBaiduAppId, edtBaiduKey, edtBaiduUrl, edtMaxWords, edtMaxChars]:
    edt.wEvent_Text do (): scheduleEditDebounce()

  btnToggleAiKey.wEvent_Button do ():
    g_aiKeyShown = not g_aiKeyShown
    setKeyMask(edtAiKey, btnToggleAiKey, g_aiKeyShown)
  btnToggleBaiduKey.wEvent_Button do ():
    g_baiduKeyShown = not g_baiduKeyShown
    setKeyMask(edtBaiduKey, btnToggleBaiduKey, g_baiduKeyShown)
  btnAddAbbr.wEvent_Button do ():
    if dlAbbr != nil: dlAbbr.addRow(g_uiScale)
  btnSaveAbbr.wEvent_Button do ():
    if saveAbbrRows(): modernShowSavedToast("缩略词已应用")
  btnAddIgnore.wEvent_Button do ():
    if dlIgnore != nil: dlIgnore.addRow(g_uiScale)
  btnSaveIgnore.wEvent_Button do ():
    if saveIgnoreRows(): modernShowSavedToast("忽略词已应用")
  btnAddPref.wEvent_Button do ():
    if dlPreferred != nil: dlPreferred.addRow(g_uiScale)
  btnSavePref.wEvent_Button do ():
    if savePrefRows(): modernShowSavedToast("常用词已应用")
  cmbAiPreset.wEvent_ComboBox do ():
    let i = cmbAiPreset.getSelection()
    if i > 0 and i < AiPresets.len:
      edtAiUrl.setValue(AiPresets[i][1])
      edtAiModel.setValue(AiPresets[i][2])
      cmbAiPreset.setSelection(0)
      applyTransSettings()
  btnResetSize.wEvent_Button do ():
    applyWinSize(g_defWinW, g_defWinH)
    g_loadingSettings = true
    syncSizeFields()
    g_loadingSettings = false
    let wsz = frame.getSize()
    g_set.winW = int(wsz.width)
    g_set.winH = int(wsz.height)
    fy_cfg_set(g_cfg, "通用设置", "窗口宽", $g_set.winW)
    fy_cfg_set(g_cfg, "通用设置", "窗口高", $g_set.winH)
    discard fy_cfg_save(g_cfg)
    modernShowSavedToast("已恢复默认尺寸")
  btnBrowseSnipDir.wEvent_Button do ():
    var initDir = edtSnipSaveDir.getValue().strip()
    if initDir.len == 0: initDir = g_set.snipSaveDir
    try:
      if initDir.len > 0 and not dirExists(initDir):
        createDir(initDir)
    except OSError:
      initDir = g_appDir
    let picked = DirDialog(setFrame, message="选择截图默认保存位置", defaultPath=initDir).display()
    if picked.len > 0:
      edtSnipSaveDir.setValue(picked)
      applySnipSaveDir()
  btnOpenSnipDir.wEvent_Button do ():
    var dir = edtSnipSaveDir.getValue().strip()
    if dir.len == 0: dir = g_set.snipSaveDir
    try: createDir(dir)
    except OSError: discard
    discard ShellExecuteW(0, "open", dir.newWideCString(), nil, nil, SW_SHOWNORMAL)
  btnTryCapture.wEvent_Button do ():
    modernClearSettingsFocus()
    setFrame.hide()
    triggerCapture()
  btnTryPin.wEvent_Button do ():
    modernClearSettingsFocus()
    setFrame.hide()
    triggerPin()
  btnTrySnip.wEvent_Button do ():
    modernClearSettingsFocus()
    setFrame.hide()
    triggerSnip()
  btnClosePins.wEvent_Button do ():
    closeAllPins()
    modernShowSavedToast("已关闭所有贴图")
  btnCheckUpdate.wEvent_Button do ():
    if g_checkingUpdate: return
    g_checkingUpdate = true
    btnCheckUpdate.disable()
    lblUpdateStatus.label = "正在检查更新……"
    let info = cast[ptr UpdateInfo](alloc0(sizeof(UpdateInfo)))
    discard CreateThread(nil, 0, updateCheckWorker, cast[LPVOID](info), 0, nil)
  btnOpenRepo.wEvent_Button do ():
    discard ShellExecuteW(0, "open", "https://github.com/TimKuitu/kuaiyi-ng".newWideCString(),
                          nil, nil, SW_SHOWNORMAL)
  btnOpenCache.wEvent_Button do ():
    createDir(g_set.cacheDir)
    discard ShellExecuteW(0, "open", g_set.cacheDir.newWideCString(), nil, nil,
                          SW_SHOWNORMAL)
  btnResetHot.wEvent_Button do ():
    g_loadingSettings = true
    hkToggleCtrl.changeHotkey(int(origToWin(DefToggle[0])), DefToggle[1])
    hkUpCtrl.changeHotkey(int(origToWin(DefHistUp[0])), DefHistUp[1])
    hkDownCtrl.changeHotkey(int(origToWin(DefHistDown[0])), DefHistDown[1])
    hkCopyCtrl.changeHotkey(int(origToWin(DefCopy[0])), DefCopy[1])
    hkSnipCtrl.changeHotkey(int(origToWin(DefSnip[0])), DefSnip[1])
    hkCaptureCtrl.changeHotkey(int(origToWin(DefCapture[0])), DefCapture[1])
    hkPinCtrl.changeHotkey(int(origToWin(DefPin[0])), DefPin[1])
    hkSnipCapture2.changeHotkey(int(origToWin(DefCapture[0])), DefCapture[1])
    hkSnipPin2.changeHotkey(int(origToWin(DefPin[0])), DefPin[1])
    hkSnipTrans2.changeHotkey(int(origToWin(DefSnip[0])), DefSnip[1])
    g_loadingSettings = false
    applyHotkeysNow(false)

  # 应用启动状态
  let hwnd = cast[HWND](frame.getHandle())
  g_hwndMain = hwnd
  if g_set.topmost:
    discard SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE or SWP_NOSIZE)
  registerHotkeys()
  refreshHotkeyLabels()

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

  # 测试钩子：-testtab=3 / -testtab=4 打开设置并切到第 3/4 页（缩略词 / 忽略词）
  # 仅用于截图核对布局与自动化回归，不影响正常使用
  block:
    var wantTab = -1
    for p in commandLineParams():
      if p.startsWith("-testtab="):
        try: wantTab = parseInt(p[9 .. ^1])
        except ValueError: discard
    if wantTab >= 0:
      frame.wEvent_Timer do (event: wEvent):
        if event.timerId == 68:
          frame.stopTimer(68)
          openSettings()
          modernSelectSettingsTab(wantTab)
          if dlAbbr != nil: dlAbbr.render(g_uiScale)
          if dlIgnore != nil: dlIgnore.render(g_uiScale)
          if dlPreferred != nil: dlPreferred.render(g_uiScale)
      frame.startTimer(1.5, 68)

  # 测试钩子：-testtranslate 启动后自动翻译一段固定文本，结果写入 test.log 后退出
  # （配 mock 服务器做端到端验证用，也顺带验证 run.log 全链路埋点）
  if "-testtranslate" in commandLineParams():
    g_testTranslate = true
    frame.startTimer(1.2, 65)
    frame.startTimer(15.0, 69)  # 兜底超时退出，防止网络卡死导致自测进程常驻后台
    frame.wEvent_Timer do (event: wEvent):
      if event.timerId == 65:
        frame.stopTimer(65)
        input.setValue("The quick brown fox jumps over the lazy dog.")
        doTranslate(FY_AUTO)
      elif event.timerId == 69:
        frame.stopTimer(69)
        if not g_exiting:
          let f = open(g_appDir / "test.log", fmAppend)
          f.writeLine("TEST-RESULT: TIMEOUT")
          f.close()
          fy_log_write("翻译", "自测超时退出")
          quitApp()

  try:
    app.mainLoop()
  except Exception as e:
    fy_log_write("异常", "MAINLOOP-EXC: %s", e.msg.cstring)
    let f = open(g_appDir / "crash.log", fmAppend)
    f.writeLine("MAINLOOP-EXC: " & e.msg)
    f.writeLine(getStackTrace())
    f.close()

when isMainModule:
  main()
