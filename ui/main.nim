## 快译NG —— Bing 免费翻译（C 核心 + wNim 界面）
## 中英互译、历史记录、复制单词、全局热键、托盘；缓存目录可改、Bing 免费翻译接口。

import std/[os, strutils, sets, tables, algorithm, math]
import wNim
import winim/lean
import fastcore
import naming_reducer

# winim/lean 未包含 shellapi，这里单独引入打开目录所需的函数
proc ShellExecuteW*(hwnd: HWND, lpOperation, lpFile, lpParameters,
                    lpDirectory: LPCWSTR, nShow: cint): HANDLE
    {.stdcall, dynlib: "shell32", importc: "ShellExecuteW".}

const
  AppTitle = "快译NG"
  AppVersion = "2.1.1"
  # wNim 自定义事件占用 WM_APP+0x10..0xE8，自定义消息必须避开（曾因冲突随机崩溃）
  WmAppTranslated = WM_APP + 0x0200
  WmAppUpdate = WM_APP + 0x0201

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

  # 命名风格下拉（两项）。存配置沿用旧 FY_FMT_* 代码（3=驼峰 4=snake），
  # 老配置里的 0/1/2/5 读取时归到驼峰，不用迁移
  StyleNames = ["snake_case", "驼峰法"]
  StyleCodes = [FY_FMT_SNAKE, FY_FMT_CAMEL]
  # 命名对象下拉，选项序 = naming_reducer.NameKind 枚举序，直接下标配对
  KindNames = ["局部变量", "全局变量", "静态变量", "指针变量",
               "结构体类型", "宏/常量", "函数"]

  # 字体缩放可选档位（百分比）
  UiScalePresets = ["75", "100", "110", "125", "150", "175", "200"]
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
    cacheDir: string
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
    maxWords: int           # 命名缩减预算（config-only，无界面控件）
    maxChars: int
    winW: int               # 主窗口外框尺寸（像素；0 = 默认自适应高度）
    winH: int
    uiScale: int            # 字体缩放（百分比，作用于设置窗口与翻译窗口）
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
  g_set.cacheDir = cfgStr("通用设置", "缓存目录", g_appDir)
  if g_set.cacheDir.len == 0: g_set.cacheDir = g_appDir
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
  edtBaiduAppId, edtBaiduKey, edtBaiduUrl: wTextCtrl
  btnZh2En, btnEn2Zh, btnTrans, btnSettings: wButton
  btnOpenCache, btnSaveSet1, btnSaveSet2, btnSaveSet3, btnResetHot: wButton
  btnCheckUpdate: wButton
  lblUpdateStatus: wStaticText
  edtWinW, edtWinH: wTextCtrl
  btnResetSize: wButton
  cmbUiScale: wComboBox
  lblScaleHint: wStaticText
  cmbStyle: wComboBox      # 主窗口按钮行：命名风格下拉（snake_case/驼峰法）
  cmbKind: wComboBox       # 命名对象下拉（局部变量…函数），选中即生效
  cmbAiPreset: wComboBox
  chkTop, chkCloseExit, chkAutoStart, chkAutoHide, chkSummon, chkAi, chkBaidu: wCheckBox
  chkSummonClear: wCheckBox
  hkToggleCtrl, hkUpCtrl, hkDownCtrl, hkCopyCtrl: wHotkeyCtrl
  g_checkingUpdate = false

# ---------------- 自定义缩略 / 忽略 / 常用词（设置页最后三页） ----------------
# 三页都显示「内置 + 自定义」的合并列表，内置行也能改、能删：
#   缩略页删行 = 该词不再缩写（文件里记 `原词=-`）
#   忽略页删行 = 该内置泛词保留（文件里记 `+原词`）
#   常用词页改英文 = 覆盖内置映射（串口=serial port），删行 = 停用该映射（文件里记 `中文=-`），
#                   也可加全新词条（气泵=pump）；自定义英文词自动享受常用词待遇
# 每行控件懒创建、只隐藏不销毁（运行期销毁 wNim 控件不安全），行槽上限 AbbrMaxRows。
const
  AbbrTop = 116         # 行区起始 y（逻辑像素；上方两行提示各占 50）
  AbbrRowH = 32         # 行距
  AbbrViewRows = 10     # 每页可见行数
  AbbrMaxRows = 240     # 行槽上限（缩略约 60 + 忽略约 65 + 常用词约 100，其余留给自定义）

type
  AbbrRowCtl = object
    word, abbr: wTextCtrl
    arrow: wStaticText
    del: wButton

  IgnoreRowCtl = object
    word: wTextCtrl
    del: wButton

  PrefRowCtl = object
    zh, en: wTextCtrl
    arrow: wStaticText
    del: wButton

var
  nbSet: wNoteBook
  pageAbbr, pageIgnore, pagePreferred: wPanel
  abbrRows: seq[AbbrRowCtl]
  ignoreRows: seq[IgnoreRowCtl]
  prefRows: seq[PrefRowCtl]
  abbrCount = 0           # 当前显示行数（含末尾空行）
  ignoreCount = 0
  prefCount = 0
  abbrScroll = 0          # 顶部行索引
  ignoreScroll = 0
  prefScroll = 0
  sbAbbr, sbIgnore, sbPref: wScrollBar
  btnAddAbbr, btnAddIgnore, btnSaveAbbr, btnSaveIgnore: wButton
  btnAddPref, btnSavePref: wButton
  lblAbbrStatus, lblIgnoreStatus, lblPrefStatus: wStaticText
  g_abbrOverflow = false  # 文件条数超过行槽上限：禁止保存，避免静默截断
  g_ignOverflow = false
  g_prefOverflow = false

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

proc scrollAbbrPage(notches: int)
proc scrollIgnorePage(notches: int)
proc scrollPrefPage(notches: int)
proc renderPref()

proc wheelSubclass(hWnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM,
                   uIdSubclass: UINT_PTR, dwRefData: DWORD_PTR): LRESULT {.stdcall.} =
  if uMsg == WmMouseWheel:
    var d = int((wParam shr 16) and WPARAM(0xFFFF))
    if d >= 0x8000: d -= 0x10000      # 高位是带符号的滚动增量，一格 120
    let n = d div 120
    if n != 0:
      if dwRefData == DWORD_PTR(WheelTagAbbr): scrollAbbrPage(n)
      elif dwRefData == DWORD_PTR(WheelTagIgnore): scrollIgnorePage(n)
      elif dwRefData == DWORD_PTR(WheelTagPreferred): scrollPrefPage(n)
      return 0
  result = DefSubclassProc(hWnd, uMsg, wParam, lParam)

proc hookWheel(w: wWindow, tag: int) =
  if w != nil:
    discard SetWindowSubclass(w.getHandle(), wheelSubclass,
                              UINT_PTR(WheelSubId), DWORD_PTR(tag))

# —— 缩略词页 ——
proc renderAbbr()
proc deleteAbbrRow(i: int)

proc bindAbbrDelete(btn: wButton, idx: int) =
  # 单独一个 proc 建闭包：idx 落在本次调用的环境里。循环里直接捕获会共享同一个变量
  btn.wEvent_Button do ():
    deleteAbbrRow(idx)

proc ensureAbbrRows(n: int) =
  while abbrRows.len < n and abbrRows.len < AbbrMaxRows:
    var r: AbbrRowCtl
    r.word = TextCtrl(pageAbbr, pos=(0, 0), size=(10, 10))
    r.arrow = StaticText(pageAbbr, label="→", pos=(0, 0), size=(10, 10))
    r.abbr = TextCtrl(pageAbbr, pos=(0, 0), size=(10, 10))
    r.del = Button(pageAbbr, label="删除", pos=(0, 0), size=(10, 10))
    bindAbbrDelete(r.del, abbrRows.len)
    hookWheel(r.word, WheelTagAbbr)
    hookWheel(r.arrow, WheelTagAbbr)
    hookWheel(r.abbr, WheelTagAbbr)
    hookWheel(r.del, WheelTagAbbr)
    r.word.hide()
    r.arrow.hide()
    r.abbr.hide()
    r.del.hide()
    abbrRows.add(r)

proc collectAbbrSlots(): seq[(string, string)] =
  result = @[]
  for i in 0 ..< abbrCount:
    result.add((abbrRows[i].word.getValue().strip(),
                abbrRows[i].abbr.getValue().strip()))

proc renderAbbr() =
  ## 只摆可见行（滚动窗口），宽高按页面实际宽度换算
  ensureAbbrRows(abbrCount)
  if pageAbbr.isNil: return
  var pw = int(float(pageAbbr.getSize().width) / g_uiScale)
  if pw < 200: pw = 520            # 页面还没布局时兜底
  let maxS = max(0, abbrCount - AbbrViewRows)
  if abbrScroll > maxS: abbrScroll = maxS
  if abbrScroll < 0: abbrScroll = 0
  let left = 24
  let right = pw - 24 - 18 - 4     # 右侧让出滚动条
  let delW = 56
  let arrowW = 20
  let gap = 8
  let delX = right - delW
  let half = (delX - left - arrowW - 3 * gap) div 2
  for i in 0 ..< abbrRows.len:
    let slot = i - abbrScroll
    if i < abbrCount and slot >= 0 and slot < AbbrViewRows:
      let y = AbbrTop + slot * AbbrRowH
      abbrRows[i].word.setSize(dpx(left), dpx(y), dpx(half), dpx(24))
      abbrRows[i].arrow.setSize(dpx(left + half + gap), dpx(y + 3), dpx(arrowW), dpx(18))
      abbrRows[i].abbr.setSize(dpx(left + half + gap + arrowW + gap), dpx(y),
                               dpx(half), dpx(24))
      abbrRows[i].del.setSize(dpx(delX), dpx(y), dpx(delW), dpx(24))
      abbrRows[i].word.show()
      abbrRows[i].arrow.show()
      abbrRows[i].abbr.show()
      abbrRows[i].del.show()
    else:
      abbrRows[i].word.hide()
      abbrRows[i].arrow.hide()
      abbrRows[i].abbr.hide()
      abbrRows[i].del.hide()
  sbAbbr.setSize(dpx(pw - 24 - 18), dpx(AbbrTop), dpx(18),
                 dpx(AbbrViewRows * AbbrRowH - 8))
  sbAbbr.setScrollbar(abbrScroll, AbbrViewRows, max(abbrCount - 1, 1))
  if abbrCount > AbbrViewRows: sbAbbr.enable() else: sbAbbr.disable()

proc abbrMerge(): seq[(string, string)] =
  ## 内置 + 自定义合并：自定义覆盖内置，"-" = 禁用（不出行），按原词排序
  var t = initTable[string, string]()
  for (w, a) in builtinAbbreviations(): t[w] = a
  for (w, a) in customAbbrEntries():
    if a == "-": t.del(w)
    else: t[w] = a
  result = @[]
  for k, v in t: result.add((k, v))
  result.sort(proc (x, y: (string, string)): int = cmp(x[0], y[0]))

proc abbrSummary(total: int): string =
  let builtin = builtinAbbreviations().len
  let custom = customAbbrPairs().len
  let disabled = customAbbrEntries().len - custom
  result = "共 " & $total & " 条：内置 " & $builtin & "，自定义 " & $custom
  if disabled > 0: result.add("，禁用 " & $disabled)

proc loadAbbrRows(preserveScroll = false) =
  ## 打开设置窗口 / 保存后按生效表回填（内置 + 自定义，未保存的编辑丢弃）
  let pairs = abbrMerge()
  g_abbrOverflow = pairs.len > AbbrMaxRows
  abbrCount = max(1, min(pairs.len, AbbrMaxRows))
  if not preserveScroll: abbrScroll = 0
  ensureAbbrRows(abbrCount)
  for i in 0 ..< abbrRows.len:
    if i < pairs.len and i < abbrCount:
      abbrRows[i].word.setValue(pairs[i][0])
      abbrRows[i].abbr.setValue(pairs[i][1])
    else:
      abbrRows[i].word.setValue("")
      abbrRows[i].abbr.setValue("")
  renderAbbr()
  if g_abbrOverflow:
    lblAbbrStatus.label = "共 " & $pairs.len & " 条，超过界面上限 " & $AbbrMaxRows
  else:
    lblAbbrStatus.label = abbrSummary(pairs.len)

proc deleteAbbrRow(i: int) =
  if i < 0 or i >= abbrCount: return
  var slots = collectAbbrSlots()
  if i >= slots.len: return
  slots.delete(i)
  abbrCount = max(1, slots.len)
  ensureAbbrRows(abbrCount)
  for k in 0 ..< abbrRows.len:
    if k < slots.len:
      abbrRows[k].word.setValue(slots[k][0])
      abbrRows[k].abbr.setValue(slots[k][1])
    else:
      abbrRows[k].word.setValue("")
      abbrRows[k].abbr.setValue("")
  renderAbbr()

proc addAbbrRow() =
  if abbrCount >= AbbrMaxRows:
    lblAbbrStatus.label = "最多 " & $AbbrMaxRows & " 条，先删掉不用的行"
    return
  abbrCount += 1
  ensureAbbrRows(abbrCount)
  abbrScroll = max(0, abbrCount - AbbrViewRows)
  renderAbbr()
  abbrRows[abbrCount - 1].word.setFocus()

proc saveAbbrRows(): bool =
  if g_abbrOverflow:
    warnBox("合并后的词条超过界面上限 " & $AbbrMaxRows &
            " 条，界面保存会丢数据。\n请先把 custom_abbr.txt 删到 " &
            $AbbrMaxRows & " 条以内，再重开设置窗口。")
    return false
  var rows: seq[(string, string)] = @[]
  var seen = initHashSet[string]()
  var err = ""
  for i in 0 ..< abbrCount:
    let w = abbrRows[i].word.getValue().strip().toLowerAscii()
    let a = abbrRows[i].abbr.getValue().strip().toLowerAscii()
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

# —— 忽略词页 ——
proc renderIgnore()
proc deleteIgnoreRow(i: int)

proc bindIgnoreDelete(btn: wButton, idx: int) =
  btn.wEvent_Button do ():
    deleteIgnoreRow(idx)

proc ensureIgnoreRows(n: int) =
  while ignoreRows.len < n and ignoreRows.len < AbbrMaxRows:
    var r: IgnoreRowCtl
    r.word = TextCtrl(pageIgnore, pos=(0, 0), size=(10, 10))
    r.del = Button(pageIgnore, label="删除", pos=(0, 0), size=(10, 10))
    bindIgnoreDelete(r.del, ignoreRows.len)
    hookWheel(r.word, WheelTagIgnore)
    hookWheel(r.del, WheelTagIgnore)
    r.word.hide()
    r.del.hide()
    ignoreRows.add(r)

proc collectIgnoreSlots(): seq[string] =
  result = @[]
  for i in 0 ..< ignoreCount: result.add(ignoreRows[i].word.getValue().strip())

proc renderIgnore() =
  ensureIgnoreRows(ignoreCount)
  if pageIgnore.isNil: return
  var pw = int(float(pageIgnore.getSize().width) / g_uiScale)
  if pw < 200: pw = 520
  let maxS = max(0, ignoreCount - AbbrViewRows)
  if ignoreScroll > maxS: ignoreScroll = maxS
  if ignoreScroll < 0: ignoreScroll = 0
  let left = 24
  let right = pw - 24 - 18 - 4
  let delW = 56
  let gap = 8
  let delX = right - delW
  let wordW = delX - left - gap
  for i in 0 ..< ignoreRows.len:
    let slot = i - ignoreScroll
    if i < ignoreCount and slot >= 0 and slot < AbbrViewRows:
      let y = AbbrTop + slot * AbbrRowH
      ignoreRows[i].word.setSize(dpx(left), dpx(y), dpx(wordW), dpx(24))
      ignoreRows[i].del.setSize(dpx(delX), dpx(y), dpx(delW), dpx(24))
      ignoreRows[i].word.show()
      ignoreRows[i].del.show()
    else:
      ignoreRows[i].word.hide()
      ignoreRows[i].del.hide()
  sbIgnore.setSize(dpx(pw - 24 - 18), dpx(AbbrTop), dpx(18),
                   dpx(AbbrViewRows * AbbrRowH - 8))
  sbIgnore.setScrollbar(ignoreScroll, AbbrViewRows, max(ignoreCount - 1, 1))
  if ignoreCount > AbbrViewRows: sbIgnore.enable() else: sbIgnore.disable()

# —— 滚轮滚动（一格 3 行；正增量＝向上滚＝行号减小）——
proc scrollAbbrPage(notches: int) =
  if abbrCount <= AbbrViewRows: return
  var s = abbrScroll - notches * 3
  if s < 0: s = 0
  let maxS = abbrCount - AbbrViewRows
  if s > maxS: s = maxS
  if s == abbrScroll: return
  abbrScroll = s
  renderAbbr()

proc scrollIgnorePage(notches: int) =
  if ignoreCount <= AbbrViewRows: return
  var s = ignoreScroll - notches * 3
  if s < 0: s = 0
  let maxS = ignoreCount - AbbrViewRows
  if s > maxS: s = maxS
  if s == ignoreScroll: return
  ignoreScroll = s
  renderIgnore()

proc scrollPrefPage(notches: int) =
  if prefCount <= AbbrViewRows: return
  var s = prefScroll - notches * 3
  if s < 0: s = 0
  let maxS = prefCount - AbbrViewRows
  if s > maxS: s = maxS
  if s == prefScroll: return
  prefScroll = s
  renderPref()

proc ignoreMerge(): seq[string] =
  ## 内置泛词 + 自定义忽略，去掉被显式保留（+word）的，按字母序
  let keep = toHashSet(customKeepWords())
  var seen = initHashSet[string]()
  result = @[]
  for w in builtinStopWords():
    if w in keep or seen.contains(w): continue
    seen.incl(w); result.add(w)
  for w in customIgnoreWords():
    if w in keep or seen.contains(w): continue
    seen.incl(w); result.add(w)
  result.sort()

proc ignoreSummary(total: int): string =
  let builtin = builtinStopWords().len
  let custom = customIgnoreWords().len
  let kept = customKeepWords().len
  result = "共 " & $total & " 条：内置 " & $builtin & "，自定义 " & $custom
  if kept > 0: result.add("，保留 " & $kept)

proc loadIgnoreRows(preserveScroll = false) =
  ## 打开设置窗口 / 保存后按生效表回填（内置泛词 + 自定义忽略）
  let words = ignoreMerge()
  g_ignOverflow = words.len > AbbrMaxRows
  ignoreCount = max(1, min(words.len, AbbrMaxRows))
  if not preserveScroll: ignoreScroll = 0
  ensureIgnoreRows(ignoreCount)
  for i in 0 ..< ignoreRows.len:
    if i < words.len and i < ignoreCount: ignoreRows[i].word.setValue(words[i])
    else: ignoreRows[i].word.setValue("")
  renderIgnore()
  if g_ignOverflow:
    lblIgnoreStatus.label = "共 " & $words.len & " 条，超过界面上限 " & $AbbrMaxRows
  else:
    lblIgnoreStatus.label = ignoreSummary(words.len)

proc deleteIgnoreRow(i: int) =
  if i < 0 or i >= ignoreCount: return
  var vals = collectIgnoreSlots()
  if i >= vals.len: return
  vals.delete(i)
  ignoreCount = max(1, vals.len)
  ensureIgnoreRows(ignoreCount)
  for k in 0 ..< ignoreRows.len:
    if k < vals.len: ignoreRows[k].word.setValue(vals[k])
    else: ignoreRows[k].word.setValue("")
  renderIgnore()

proc addIgnoreRow() =
  if ignoreCount >= AbbrMaxRows:
    lblIgnoreStatus.label = "最多 " & $AbbrMaxRows & " 条，先删掉不用的行"
    return
  ignoreCount += 1
  ensureIgnoreRows(ignoreCount)
  ignoreScroll = max(0, ignoreCount - AbbrViewRows)
  renderIgnore()
  ignoreRows[ignoreCount - 1].word.setFocus()

proc saveIgnoreRows(): bool =
  if g_ignOverflow:
    warnBox("合并后的词条超过界面上限 " & $AbbrMaxRows &
            " 条，界面保存会丢数据。\n请先把 custom_ignore.txt 删到 " &
            $AbbrMaxRows & " 条以内，再重开设置窗口。")
    return false
  var words: seq[string] = @[]
  var seen = initHashSet[string]()
  var err = ""
  for i in 0 ..< ignoreCount:
    let w = ignoreRows[i].word.getValue().strip().toLowerAscii()
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

# —— 常用词页（中文 → 英文词库；机制与缩略词页相同）——
# renderPref 的前向声明在滚轮小节（scrollPrefPage 先用到）
proc deletePrefRow(i: int)

proc bindPrefDelete(btn: wButton, idx: int) =
  btn.wEvent_Button do ():
    deletePrefRow(idx)

proc ensurePrefRows(n: int) =
  while prefRows.len < n and prefRows.len < AbbrMaxRows:
    var r: PrefRowCtl
    r.zh = TextCtrl(pagePreferred, pos=(0, 0), size=(10, 10))
    r.arrow = StaticText(pagePreferred, label="→", pos=(0, 0), size=(10, 10))
    r.en = TextCtrl(pagePreferred, pos=(0, 0), size=(10, 10))
    r.del = Button(pagePreferred, label="删除", pos=(0, 0), size=(10, 10))
    bindPrefDelete(r.del, prefRows.len)
    hookWheel(r.zh, WheelTagPreferred)
    hookWheel(r.arrow, WheelTagPreferred)
    hookWheel(r.en, WheelTagPreferred)
    hookWheel(r.del, WheelTagPreferred)
    r.zh.hide()
    r.arrow.hide()
    r.en.hide()
    r.del.hide()
    prefRows.add(r)

proc collectPrefSlots(): seq[(string, string)] =
  result = @[]
  for i in 0 ..< prefCount:
    result.add((prefRows[i].zh.getValue().strip(),
                prefRows[i].en.getValue().strip()))

proc renderPref() =
  ensurePrefRows(prefCount)
  if pagePreferred.isNil: return
  var pw = int(float(pagePreferred.getSize().width) / g_uiScale)
  if pw < 200: pw = 520            # 页面还没布局时兜底
  let maxS = max(0, prefCount - AbbrViewRows)
  if prefScroll > maxS: prefScroll = maxS
  if prefScroll < 0: prefScroll = 0
  let left = 24
  let right = pw - 24 - 18 - 4     # 右侧让出滚动条
  let delW = 56
  let arrowW = 20
  let gap = 8
  let delX = right - delW
  let half = (delX - left - arrowW - 3 * gap) div 2
  for i in 0 ..< prefRows.len:
    let slot = i - prefScroll
    if i < prefCount and slot >= 0 and slot < AbbrViewRows:
      let y = AbbrTop + slot * AbbrRowH
      prefRows[i].zh.setSize(dpx(left), dpx(y), dpx(half), dpx(24))
      prefRows[i].arrow.setSize(dpx(left + half + gap), dpx(y + 3), dpx(arrowW), dpx(18))
      prefRows[i].en.setSize(dpx(left + half + gap + arrowW + gap), dpx(y),
                             dpx(half), dpx(24))
      prefRows[i].del.setSize(dpx(delX), dpx(y), dpx(delW), dpx(24))
      prefRows[i].zh.show()
      prefRows[i].arrow.show()
      prefRows[i].en.show()
      prefRows[i].del.show()
    else:
      prefRows[i].zh.hide()
      prefRows[i].arrow.hide()
      prefRows[i].en.hide()
      prefRows[i].del.hide()
  sbPref.setSize(dpx(pw - 24 - 18), dpx(AbbrTop), dpx(18),
                 dpx(AbbrViewRows * AbbrRowH - 8))
  sbPref.setScrollbar(prefScroll, AbbrViewRows, max(prefCount - 1, 1))
  if prefCount > AbbrViewRows: sbPref.enable() else: sbPref.disable()

proc prefSummary(total: int): string =
  let builtin = builtinPreferredPairs().len
  let custom = customPreferredPairs().len
  let disabled = customPreferredEntries().len - custom
  result = "共 " & $total & " 条：内置 " & $builtin & "，自定义 " & $custom
  if disabled > 0: result.add("，停用 " & $disabled)

proc loadPrefRows(preserveScroll = false) =
  ## 打开设置窗口 / 保存后按生效表回填（内置 + 自定义，未保存的编辑丢弃）
  let pairs = preferredMerge()
  g_prefOverflow = pairs.len > AbbrMaxRows
  prefCount = max(1, min(pairs.len, AbbrMaxRows))
  if not preserveScroll: prefScroll = 0
  ensurePrefRows(prefCount)
  for i in 0 ..< prefRows.len:
    if i < pairs.len and i < prefCount:
      prefRows[i].zh.setValue(pairs[i][0])
      prefRows[i].en.setValue(pairs[i][1])
    else:
      prefRows[i].zh.setValue("")
      prefRows[i].en.setValue("")
  renderPref()
  if g_prefOverflow:
    lblPrefStatus.label = "共 " & $pairs.len & " 条，超过界面上限 " & $AbbrMaxRows
  else:
    lblPrefStatus.label = prefSummary(pairs.len)

proc deletePrefRow(i: int) =
  if i < 0 or i >= prefCount: return
  var slots = collectPrefSlots()
  if i >= slots.len: return
  slots.delete(i)
  prefCount = max(1, slots.len)
  ensurePrefRows(prefCount)
  for k in 0 ..< prefRows.len:
    if k < slots.len:
      prefRows[k].zh.setValue(slots[k][0])
      prefRows[k].en.setValue(slots[k][1])
    else:
      prefRows[k].zh.setValue("")
      prefRows[k].en.setValue("")
  renderPref()

proc addPrefRow() =
  if prefCount >= AbbrMaxRows:
    lblPrefStatus.label = "最多 " & $AbbrMaxRows & " 条，先删掉不用的行"
    return
  prefCount += 1
  ensurePrefRows(prefCount)
  prefScroll = max(0, prefCount - AbbrViewRows)
  renderPref()
  prefRows[prefCount - 1].zh.setFocus()

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
  if g_prefOverflow:
    warnBox("合并后的词条超过界面上限 " & $AbbrMaxRows &
            " 条，界面保存会丢数据。\n请先把 custom_preferred.txt 删到 " &
            $AbbrMaxRows & " 条以内，再重开设置窗口。")
    return false
  var rows: seq[(string, string)] = @[]
  var seen = initHashSet[string]()
  var err = ""
  for i in 0 ..< prefCount:
    let zh = prefRows[i].zh.getValue().strip()
    let en = prefRows[i].en.getValue().strip()
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
  useBaidu: bool
  baiduAppId: cstring
  baiduKey: cstring
  baiduUrl: cstring

proc translateWorker(param: LPVOID): DWORD {.stdcall.} =
  # 本线程由 CreateThread 创建，不经 Nim 运行时托管——
  # 只调用 C 核心（回退链在 C 内完成），不触碰 Nim GC。
  # 窗口句柄用 g_hwndMain（纯值拷贝），不在裸线程里调 frame 的方法
  let job = cast[ptr TranslateJob](param)
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
  PostMessageW(g_hwndMain, UINT(WmAppTranslated), 0, cast[LPARAM](res))
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
  # 中文输入不用 AI，直接走 百度 → 必应 二级链（useAi 置假时 C 核心自动跳过 AI 段）
  let useAi = (not nimHasCjk(text)) and g_set.aiEnabled and
              g_set.aiKey.len > 0 and g_set.aiUrl.len > 0
  let useBaidu = g_set.baiduEnabled and g_set.baiduAppId.len > 0 and
                 g_set.baiduKey.len > 0
  fy_log_write("翻译", "开始 mode=%d 文本%u字节 预览=%.48s AI=%d 百度=%d",
               cint(mode), cuint(text.len), text.cstring,
               cint(ord(useAi)), cint(ord(useBaidu)))
  let job = cast[ptr TranslateJob](alloc0(sizeof(TranslateJob)))
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
    discard ShellExecuteW(0, "open", newWideCString($cast[cstring](addr info.path)),
                          nil, nil, SW_SHOWNORMAL)
    dealloc(info)
  of UpdStageFail:
    let info = cast[ptr UpdateInfo](lp)
    lblUpdateStatus.label = "下载失败：" & $cast[cstring](addr info.err)
    dealloc(info)
  else:
    discard

proc quitApp()   # 前置声明：-testtranslate 钩子在 onTranslated 里提前用到

proc onTranslated(res: cstring) =
  g_translating = false
  btnZh2En.enable(); btnEn2Zh.enable(); btnTrans.enable()
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
  let text = input.getValue().strip()
  if text.len != 0:
    fy_hist_push(g_hist, text.cstring)
    g_histIdx = -1
  fy_log_write("翻译", "完成 结果%u字节", cuint(resultBox.getValue().len))
  if g_testTranslate:
    let f = open(g_appDir / "test.log", fmAppend)
    f.writeLine("TEST-RESULT: OK " & resultBox.getValue())
    f.close()
    quitApp()

# ---------------- 布局 ----------------
proc layoutMain() =
  # pos/size 都是像素；整体按 g_uiScale（字体缩放）等比换算，别混用。
  template S(x: int): int = int(float(x) * g_uiScale)
  let cs = frame.getClientSize()
  let w = cs.width
  let h = cs.height
  let gap = S(8)
  let rh = rowH()
  panel.setSize((0, 0), (w, h))
  # 输入区与结果区默认各 96 逻辑像素；窗口拉高时余量平分，两者永远等高
  let fixedH = gap * 4 + rh                 # 上下边距 + 两个间距 + 按钮行
  var extra = h - (fixedH + S(96) + S(96))
  if extra < 0: extra = 0
  let inH = S(96) + extra div 2
  let resH = S(96) + (extra - extra div 2)
  input.setSize((gap, gap), (w - gap * 2, inH))
  # 按钮行：行高随 DPI 缩放，和字号同步
  # 宽度按实测文字宽定的（snake_case=94px、结构体类型=90px @YaHei 18px）：
  # 下拉留 text+30，按钮留 text+18；翻译靠右吃剩余，窗口最小宽保证整行放得下
  let rowY = gap + inH + gap
  btnZh2En.setSize((S(8), rowY), (S(78), rh))
  btnEn2Zh.setSize((S(90), rowY), (S(78), rh))
  btnSettings.setSize((S(172), rowY), (S(54), rh))
  cmbStyle.setSize((S(230), rowY), (S(122), rh))
  cmbKind.setSize((S(356), rowY), (S(118), rh))
  btnTrans.setSize((S(478), rowY), (w - S(486), rh))
  let resultTop = rowY + rh + gap
  resultBox.setSize((gap, resultTop), (w - gap * 2, resH))

proc fitComboVisibleHeight(cb: wComboBox) =
  # ComboBox 收起时的可视高度由 item 高度决定，setSize 设的是含下拉列表的总高。
  # 发 CB_SETITEMHEIGHT 把可视高度校准到行高（32 物理像素），量两次收敛
  let h = cb.getHandle()
  let want = rowH()
  var itemH = want - 8
  var rc: RECT
  for i in 1 .. 3:
    discard SendMessageW(h, CB_SETITEMHEIGHT, WPARAM(0xFFFFFFFF), itemH)
    discard SetWindowPos(h, HWND(0), 0, 0, 0, 0,
                         SWP_NOMOVE or SWP_NOSIZE or SWP_NOZORDER or
                         SWP_FRAMECHANGED)
    discard GetWindowRect(h, rc)
    let got = int(rc.bottom - rc.top)
    if got == want: break
    itemH += want - got

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
  result = input.getValue().strip()
  if result.len == 0 and g_lastToEn and g_resultValid:
    let first = resultBox.getValue().splitLines()[0].strip()
    if first.len > 0 and first != "正在翻译……":
      result = first

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
proc registerHotkeys() =
  let hwnd = cast[HWND](frame.getHandle())
  discard UnregisterHotKey(hwnd, HkToggle)
  discard UnregisterHotKey(hwnd, HkHistUp)
  discard UnregisterHotKey(hwnd, HkHistDown)
  discard UnregisterHotKey(hwnd, HkCopy)
  let hks = [(HkToggle, g_set.hkToggle), (HkHistUp, g_set.hkHistUp),
             (HkHistDown, g_set.hkHistDown), (HkCopy, g_set.hkCopy)]
  var log = ""
  var fails = 0
  for pair in hks:
    if pair[1][1] != 0:
      let ok = RegisterHotKey(hwnd, UINT(pair[0]), origToWin(pair[1][0]),
                              UINT(pair[1][1]))
      if ok == 0:
        inc fails
        log.add("RegisterHotKey id=" & $pair[0] & " mods=" & $pair[1][0] &
                " vk=" & $pair[1][1] & " 失败 err=" & $GetLastError() & "\n")
  if log.len > 0:
    let f = open(g_appDir / "hotkey.log", fmAppend)
    f.writeLine("--- tick=" & $GetTickCount64())
    f.write(log)
    f.close()
    fy_log_write("热键", "注册失败 %d 个（详见 hotkey.log）", cint(fails))

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
  (max(440, int(544.0 * g_uiScale) + 22), max(340, int(567.0 * g_uiScale) + 56))

proc nearSize(actual: tuple[w, h: int], expect: tuple[w, h: int], tol = 4): bool =
  abs(actual.w - expect.w) <= tol and abs(actual.h - expect.h) <= tol

proc relayoutSettings(target: float) =
  if setFrame.isNil or g_setDesign.len == 0: return
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

proc computeDefWinSize(): tuple[w, h: int] =
  ## 默认窗口尺寸：宽 550（按钮行六控件）、高度刚好放下 输入框/按钮行/结果框（随字体缩放变化）
  let chrome = int(frame.getSize().height) - int(frame.getClientSize().height)
  let contentH = dpx(8) + dpx(96) + dpx(8) + rowH() + dpx(8) + dpx(96) + dpx(8)
  result.w = max(546, int(550.0 * g_uiScale))
  result.h = max(200, contentH + chrome)

proc applyWinSize(w, h: int) =
  var wl = float(w)
  var hl = float(h)
  if wl < 546.0: wl = 546.0   # 按钮行放两个下拉后的最窄需求
  if hl < 260.0: hl = 260.0
  frame.size = (int(wl), int(hl))

proc openSettings() =
  chkTop.setValue(g_set.topmost)
  chkCloseExit.setValue(g_set.closeExit)
  chkAutoStart.setValue(g_set.autoStart)
  chkAutoHide.setValue(g_set.autoHide)
  chkSummon.setValue(g_set.summonNearMouse)
  chkSummonClear.setValue(g_set.summonClear)
  chkAi.setValue(g_set.aiEnabled)
  edtAiUrl.setValue(g_set.aiUrl)
  edtAiKey.setValue(g_set.aiKey)
  edtAiModel.setValue(g_set.aiModel)
  chkBaidu.setValue(g_set.baiduEnabled)
  edtBaiduAppId.setValue(g_set.baiduAppId)
  edtBaiduKey.setValue(g_set.baiduKey)
  edtBaiduUrl.setValue(g_set.baiduUrl)
  lblUpdateStatus.label = "当前版本 v" & AppVersion
  edtCache.setValue(g_set.cacheDir)
  hkToggleCtrl.setHotkey(origToWin(g_set.hkToggle[0]), g_set.hkToggle[1])
  hkUpCtrl.setHotkey(origToWin(g_set.hkHistUp[0]), g_set.hkHistUp[1])
  hkDownCtrl.setHotkey(origToWin(g_set.hkHistDown[0]), g_set.hkHistDown[1])
  hkCopyCtrl.setHotkey(origToWin(g_set.hkCopy[0]), g_set.hkCopy[1])
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
  syncSizeFields()
  setFrame.center()
  setFrame.show()
  forceForeground(cast[HWND](setFrame.getHandle()))

proc saveSettings() =
  g_set.topmost = chkTop.getValue()
  g_set.closeExit = chkCloseExit.getValue()
  g_set.autoStart = chkAutoStart.getValue()
  g_set.autoHide = chkAutoHide.getValue()
  g_set.summonNearMouse = chkSummon.getValue()
  g_set.summonClear = chkSummonClear.getValue()
  g_set.cacheDir = edtCache.getValue().strip()
  if g_set.cacheDir.len == 0: g_set.cacheDir = g_appDir
  g_set.aiEnabled = chkAi.getValue()
  g_set.aiUrl = edtAiUrl.getValue().strip()
  g_set.aiKey = edtAiKey.getValue().strip()
  g_set.aiModel = edtAiModel.getValue().strip()
  if g_set.aiModel.len == 0: g_set.aiModel = "deepseek-chat"
  g_set.baiduEnabled = chkBaidu.getValue()
  g_set.baiduAppId = edtBaiduAppId.getValue().strip()
  g_set.baiduKey = edtBaiduKey.getValue().strip()
  g_set.baiduUrl = edtBaiduUrl.getValue().strip()

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
  fy_cfg_set(g_cfg, "通用设置", "召唤清空输入", if g_set.summonClear: "1" else: "0")
  fy_cfg_set(g_cfg, "通用设置", "缓存目录", g_set.cacheDir)
  fy_cfg_set(g_cfg, "AI翻译", "启用", if g_set.aiEnabled: "1" else: "0")
  fy_cfg_set(g_cfg, "AI翻译", "地址", g_set.aiUrl)
  fy_cfg_set(g_cfg, "AI翻译", "密钥", g_set.aiKey)
  fy_cfg_set(g_cfg, "AI翻译", "模型", g_set.aiModel)
  fy_cfg_set(g_cfg, "百度翻译", "启用", if g_set.baiduEnabled: "1" else: "0")
  fy_cfg_set(g_cfg, "百度翻译", "AppID", g_set.baiduAppId)
  fy_cfg_set(g_cfg, "百度翻译", "密钥", g_set.baiduKey)
  fy_cfg_set(g_cfg, "百度翻译", "地址", g_set.baiduUrl)
  # 窗口大小：先按输入框里的值（用户可能直接手填）调整，再读实际尺寸落盘
  var tw = 0
  var th = 0
  try: tw = parseInt(edtWinW.getValue().strip())
  except ValueError: tw = 0
  try: th = parseInt(edtWinH.getValue().strip())
  except ValueError: th = 0
  if tw >= 420 and th >= 260: applyWinSize(tw, th)
  # 字体缩放：设置窗口重启后按新比例重建；翻译窗口当场生效
  var newScale = 100
  let sel = cmbUiScale.getSelection()
  if sel >= 0 and sel < UiScalePresets.len:
    try: newScale = parseInt(UiScalePresets[sel])
    except ValueError: newScale = g_set.uiScale
  if newScale != g_set.uiScale:
    let oldPct = g_set.uiScale
    g_set.uiScale = newScale
    g_uiScale = float(newScale) / 100.0
    let ratio = float(newScale) / float(max(1, oldPct))
    # 翻译窗口：按比例缩放当前尺寸（用户自定义的大小也跟着走），再保证装得下内容
    let oldDef = (g_defWinW, g_defWinH)     # 旧档位的默认尺寸，用来判断"有没有自定义过"
    let def = computeDefWinSize()
    let cur = frame.getSize()
    g_uiFont = makeUiFont()
    applyUiFont(frame)
    if nearSize((int(cur.width), int(cur.height)), oldDef):
      frame.size = (def.w, def.h)          # 没自定义过：直接用新档位的默认尺寸
    else:
      frame.size = (max(def.w, int(round(float(cur.width) * ratio))),
                    max(def.h, int(round(float(cur.height) * ratio))))
    g_defWinW = def.w
    g_defWinH = def.h
    layoutMain()
    fitComboVisibleHeight(cmbStyle)
    fitComboVisibleHeight(cmbKind)
    syncSizeFields()
    # 设置窗口：同样按比例缩放，并立刻重排里面的字和控件
    if not setFrame.isNil and setFrame.getHandle() != HWND(0):
      let sc = setFrame.getSize()
      let oldDef = (max(440, int(544.0 * float(oldPct) / 100.0) + 22),
                    max(340, int(534.0 * float(oldPct) / 100.0) + 56))
      let nd = defSettingsSize()
      var nw = nd.w
      var nh = nd.h
      if not nearSize((int(sc.width), int(sc.height)), oldDef):
        nw = max(nd.w, int(round(float(sc.width) * ratio)))
        nh = max(nd.h, int(round(float(sc.height) * ratio)))
      setFrame.minSize = (nw, nh)
      setFrame.maxSize = (nw, nh)
      setFrame.size = (nw, nh)
      refreshSettings(g_uiScale)
    lblScaleHint.label = "已应用 $1%（两个窗口都已缩放）" % [$newScale]
  fy_cfg_set(g_cfg, "通用设置", "界面缩放", $g_set.uiScale)
  # 只在这里落盘（退出时只存位置），下次启动按保存值还原
  let wsz = frame.getSize()
  g_set.winW = int(wsz.width)
  g_set.winH = int(wsz.height)
  fy_cfg_set(g_cfg, "通用设置", "窗口宽", $g_set.winW)
  fy_cfg_set(g_cfg, "通用设置", "窗口高", $g_set.winH)
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

  fy_log_write("设置", "已保存 AI=%d 百度=%d",
               cint(ord(g_set.aiEnabled)), cint(ord(g_set.baiduEnabled)))
  # 保存后设置窗口保持打开（只有点右上角 × 才关），提示框挂在设置窗口上
  discard MessageBoxW(cast[HWND](setFrame.getHandle()), "设置已保存。", AppTitle,
                      MB_OK or MB_ICONINFORMATION)

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
  btnZh2En = Button(panel, label="中 > 英")
  btnEn2Zh = Button(panel, label="英 > 中")
  btnSettings = Button(panel, label="设置")
  cmbStyle = ComboBox(panel, style=wCbReadOnly)
  cmbKind = ComboBox(panel, style=wCbReadOnly)
  btnTrans = Button(panel, label="翻译")
  resultBox = TextCtrl(panel, style=wTeMultiLine or wTeRich or wTeReadOnly)

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
    case event.getHotkeyId()
    of HkToggle: toggleMain(true)
    of HkHistUp: histJump(-1)
    of HkHistDown: histJump(1)
    of HkCopy: namingCopy()
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
  cmbStyle.wEvent_ComboBox do ():
    # 风格/对象两个下拉都即时生效并写配置，不用进设置页保存
    g_set.wordFmt = StyleCodes[cmbStyle.getSelection()]
    fy_cfg_set(g_cfg, "通用设置", "命名格式", $g_set.wordFmt)
    discard fy_cfg_save(g_cfg)
    fy_log_write("设置", "命名风格即时切换=%d", cint(g_set.wordFmt))
  cmbKind.wEvent_ComboBox do ():
    g_set.nameKind = cmbKind.getSelection()
    fy_cfg_set(g_cfg, "通用设置", "命名对象", $g_set.nameKind)
    discard fy_cfg_save(g_cfg)
    fy_log_write("设置", "命名对象即时切换=%d", cint(g_set.nameKind))
  btnTrans.wEvent_Button do ():
    doTranslate(FY_AUTO)
  btnSettings.wEvent_Button do ():
    openSettings()
  input.wEvent_TextEnter do ():
    doTranslate(FY_AUTO)
  # 双击=按当前「风格+对象」直接生成命名并复制（Ctrl 不再区分，老 Ctrl+双击一样走这里）
  input.wEvent_LeftDoubleClick do (event: wEvent):
    namingCopy()
  resultBox.wEvent_LeftDoubleClick do (event: wEvent):
    namingCopy()

  g_uiFont = makeUiFont()
  applyUiFont(frame)
  layoutMain()
  fitComboVisibleHeight(cmbStyle)
  fitComboVisibleHeight(cmbKind)
  frame.setTrayIcon(Icon("", 0), AppTitle & "（双击显示/隐藏）")

  # 设置窗口：按「字体缩放」给默认尺寸建窗（先量客户区当缩放基准），再套用用户保存的大小
  let dw = max(440, int(544.0 * g_uiScale) + 22)
  let dh = max(340, int(567.0 * g_uiScale) + 56)
  # 固定尺寸：不给 wResizeBorder（拖边不能改大小）、不给 wMaximizeBox（无最大化）
  setFrame = Frame(title=AppTitle & " - 设置", size=(dw, dh),
                   style=wCaption or wSystemMenu or wMinimizeBox)
  setFrame.icon = Icon("", 0)
  setFrame.minSize = (dw, dh)      # 固定尺寸：最小 = 最大 = 内容尺寸
  setFrame.maxSize = (dw, dh)
  setPanel = Panel(setFrame)
  const PadL = 24
  const ContW = 496            # 544 - 2*24
  const PageH = 529            # 页面内容高度（设计像素）
  nbSet = NoteBook(setPanel, pos=(dpx(8), dpx(8)), size=(dpx(544), dpx(PageH + 38)))
  page1 = nbSet.addPage(text="通用设置")
  page2 = nbSet.addPage(text="快捷按键")
  page3 = nbSet.addPage(text="翻译接口")
  pageAbbr = nbSet.addPage(text="缩略词")
  pageIgnore = nbSet.addPage(text="忽略词")
  pagePreferred = nbSet.addPage(text="常用词")
  g_pageHwnds = @[]
  for pp in [page1, page2, page3, pageAbbr, pageIgnore, pagePreferred]:
    g_pageHwnds.add(cast[HWND](pp.getHandle()))
  # 页签加宽加高（TCM_SETPADDING），点起来松快
  const TCM_SETPADDING = 0x1300 + 43
  discard SendMessage(cast[HWND](nbSet.getHandle()), TCM_SETPADDING, 0,
                      LPARAM(18 or (8 shl 16)))

  # ---------- 通用设置页：分三组，左右留白 24，组间 22，行距 33 ----------
  var cy = dpx(16)
  let by = dpx(PageH - 48)     # 底部操作条（检查更新 / 保存设置）统一基线

  template setTitle(txt: string) =
    discard StaticText(page1, label="▍" & txt,
                       pos=(dpx(PadL), cy), size=(dpx(280), dpx(26)))
    cy += dpx(26)
  template setDivider() =
    cy += dpx(5)
    discard StaticText(page1, pos=(dpx(PadL), cy), size=(dpx(ContW), dpx(2)),
                       style=0x10)      # SS_ETCHEDHORZ，浅灰分割线
    cy += dpx(17)

  setTitle("基础设置")
  chkTop = CheckBox(page1, label="软件窗口永远在最上面", pos=(dpx(PadL), cy))
  cy += dpx(33)
  chkCloseExit = CheckBox(page1, label="点击右上角关闭为退出程序（不勾选=隐藏到托盘）",
                          pos=(dpx(PadL), cy))
  cy += dpx(33)

  setDivider()
  setTitle("启动设置")
  chkAutoStart = CheckBox(page1, label="开机自动启动快译NG", pos=(dpx(PadL), cy))
  cy += dpx(33)
  chkAutoHide = CheckBox(page1, label="     开机启动后隐藏界面", pos=(dpx(PadL), cy))
  cy += dpx(33)
  chkSummon = CheckBox(page1, label="热键召唤窗口时出现在鼠标旁边", pos=(dpx(PadL), cy))
  cy += dpx(33)
  chkSummonClear = CheckBox(page1, label="热键召唤窗口时清空输入区和结果区", pos=(dpx(PadL), cy))
  cy += dpx(33)

  setDivider()
  setTitle("缓存与窗口")
  discard StaticText(page1, label="缓存目录（历史记录存放位置，可修改）",
                     pos=(dpx(PadL), cy), size=(dpx(ContW), dpx(24)))
  cy += dpx(26)
  edtCache = TextCtrl(page1, pos=(dpx(PadL), cy), size=(dpx(336), dpx(30)))
  btnOpenCache = Button(page1, label="查看目录", pos=(dpx(PadL + 348), cy),
                        size=(dpx(96), dpx(30)))
  cy += dpx(34)

  # 窗口大小：宽 × 高（像素）—— 标签与输入框垂直居中
  discard StaticText(page1, label="窗口大小", pos=(dpx(PadL), cy + 5), size=(dpx(76), dpx(22)))
  edtWinW = TextCtrl(page1, pos=(dpx(PadL + 80), cy), size=(dpx(64), dpx(30)))
  discard StaticText(page1, label="×", pos=(dpx(PadL + 152), cy + 5), size=(dpx(16), dpx(22)))
  edtWinH = TextCtrl(page1, pos=(dpx(PadL + 176), cy), size=(dpx(64), dpx(30)))
  discard StaticText(page1, label="像素", pos=(dpx(PadL + 248), cy + 5), size=(dpx(40), dpx(22)))
  btnResetSize = Button(page1, label="恢复默认", pos=(dpx(PadL + 300), cy),
                        size=(dpx(92), dpx(30)))
  cy += dpx(34)

  # 字体缩放：设置窗口 + 翻译窗口一起放大（不吃系统 DPI，自己说了算）
  discard StaticText(page1, label="字体缩放", pos=(dpx(PadL), cy + 5), size=(dpx(76), dpx(22)))
  cmbUiScale = ComboBox(page1, style=wCbReadOnly,
                        pos=(dpx(PadL + 80), cy), size=(dpx(92), dpx(30)))
  for pct in UiScalePresets: cmbUiScale.append(pct & "%")
  lblScaleHint = StaticText(page1, label="整窗等比缩放，设置窗口固定尺寸",
                            pos=(dpx(PadL + 184), cy + 5), size=(dpx(300), dpx(22)))

  btnCheckUpdate = Button(page1, label="检查更新", pos=(dpx(PadL), by), size=(dpx(92), dpx(32)))
  lblUpdateStatus = StaticText(page1, label="当前版本 v" & AppVersion,
                               pos=(dpx(PadL + 104), by + 6), size=(dpx(240), dpx(22)))
  btnSaveSet1 = Button(page1, label="保存设置", pos=(dpx(544 - PadL - 92), by),
                       size=(dpx(92), dpx(32)))

  # ---------- 快捷按键页 ----------
  discard StaticText(page2, label="显示/隐藏快译", pos=(dpx(PadL), dpx(24)))
  hkToggleCtrl = HotkeyCtrl(page2, pos=(dpx(200), dpx(18)), size=(dpx(220), dpx(32)))
  discard StaticText(page2, label="上一个历史记录", pos=(dpx(PadL), dpx(70)))
  hkUpCtrl = HotkeyCtrl(page2, pos=(dpx(200), dpx(64)), size=(dpx(220), dpx(32)))
  discard StaticText(page2, label="下一个历史记录", pos=(dpx(PadL), dpx(116)))
  hkDownCtrl = HotkeyCtrl(page2, pos=(dpx(200), dpx(110)), size=(dpx(220), dpx(32)))
  discard StaticText(page2, label="复制单词", pos=(dpx(PadL), dpx(162)))
  hkCopyCtrl = HotkeyCtrl(page2, pos=(dpx(200), dpx(156)), size=(dpx(220), dpx(32)))
  discard StaticText(page2,
    label="点击输入框后直接按下组合键（Ctrl/Shift/Alt 任意组合 + 普通键）",
    pos=(dpx(PadL), dpx(208)), size=(dpx(ContW), dpx(24)))
  btnResetHot = Button(page2, label="恢复默认设置", pos=(dpx(200), dpx(250)),
                       size=(dpx(120), dpx(32)))
  btnSaveSet2 = Button(page2, label="保存设置", pos=(dpx(544 - PadL - 92), by),
                       size=(dpx(92), dpx(32)))

  # ---------- 翻译接口页（AI → 百度 → 必应 依次回退） ----------
  chkAi = CheckBox(page3, label="AI 接口（OpenAI 格式）", pos=(dpx(PadL), dpx(14)))
  cmbAiPreset = ComboBox(page3, style=wCbReadOnly, pos=(dpx(210), dpx(10)),
                         size=(dpx(160), dpx(30)))
  discard StaticText(page3, label="地址（可带 /v1）", pos=(dpx(390), dpx(16)),
                     size=(dpx(140), dpx(22)))
  edtAiUrl = TextCtrl(page3, pos=(dpx(PadL), dpx(48)), size=(dpx(300), dpx(30)))
  discard StaticText(page3, label="密钥", pos=(dpx(340), dpx(52)), size=(dpx(40), dpx(22)))
  edtAiKey = TextCtrl(page3, pos=(dpx(384), dpx(48)), size=(dpx(136), dpx(30)))
  discard StaticText(page3, label="模型", pos=(dpx(PadL), dpx(90)), size=(dpx(40), dpx(22)))
  edtAiModel = TextCtrl(page3, pos=(dpx(68), dpx(86)), size=(dpx(200), dpx(30)))

  chkBaidu = CheckBox(page3, label="百度翻译开放平台（免费标准版，fanyi-api.baidu.com 注册）",
                      pos=(dpx(PadL), dpx(134)))
  discard StaticText(page3, label="AppID", pos=(dpx(PadL), dpx(170)), size=(dpx(50), dpx(22)))
  edtBaiduAppId = TextCtrl(page3, pos=(dpx(78), dpx(166)), size=(dpx(200), dpx(30)))
  discard StaticText(page3, label="密钥", pos=(dpx(296), dpx(170)), size=(dpx(40), dpx(22)))
  edtBaiduKey = TextCtrl(page3, pos=(dpx(340), dpx(166)), size=(dpx(180), dpx(30)))
  discard StaticText(page3, label="地址（可选，默认官方接口）", pos=(dpx(PadL), dpx(208)),
                     size=(dpx(200), dpx(22)))
  edtBaiduUrl = TextCtrl(page3, pos=(dpx(232), dpx(204)), size=(dpx(288), dpx(30)))

  discard StaticText(page3,
    label="翻译顺序：AI → 百度 → 必应免费（前面的未启用或失败自动用后面的）。",
    pos=(dpx(PadL), dpx(254)), size=(dpx(ContW), dpx(22)))
  discard StaticText(page3,
    label="单词词典（音标/释义）始终走必应；百度免费版每秒 1 次、每月 5 万字符。",
    pos=(dpx(PadL), dpx(278)), size=(dpx(ContW), dpx(22)))
  discard StaticText(page3,
    label="Ollama 本地无需密钥（密钥随意填）；百度需实名注册后开通“通用文本翻译”。",
    pos=(dpx(PadL), dpx(302)), size=(dpx(ContW), dpx(22)))
  btnSaveSet3 = Button(page3, label="保存设置", pos=(dpx(544 - PadL - 92), by),
                       size=(dpx(92), dpx(32)))

  # 缩略词 / 忽略词两页：行控件由 ensureXxxRows 懒创建，这里只放文案、滚动条和按钮
  let hintAbbr1 = StaticText(pageAbbr,
    label="每行一条：原词 → 缩写，例 Initialization → Init（内置已列出）",
    pos=(dpx(PadL), dpx(10)), size=(dpx(ContW), dpx(50)))
  let hintAbbr2 = StaticText(pageAbbr,
    label="可改可删：删行＝该词不再缩写；空白行忽略，保存后立即生效。",
    pos=(dpx(PadL), dpx(60)), size=(dpx(ContW), dpx(50)))
  sbAbbr = ScrollBar(pageAbbr, pos=(dpx(496), dpx(AbbrTop)),
                     size=(dpx(18), dpx(AbbrViewRows * AbbrRowH - 8)),
                     style=wSbVertical)
  btnAddAbbr = Button(pageAbbr, label="添加一行",
                      pos=(dpx(PadL), by), size=(dpx(92), dpx(32)))
  lblAbbrStatus = StaticText(pageAbbr, label="",
                             pos=(dpx(PadL + 104), by + 6), size=(dpx(300), dpx(22)))
  btnSaveAbbr = Button(pageAbbr, label="保存",
                       pos=(dpx(544 - PadL - 92), by), size=(dpx(92), dpx(32)))
  # 页面上的每一块都挂滚轮：说明文字/状态栏/按钮/滚动条上滚也能滚列表
  hookWheel(hintAbbr1, WheelTagAbbr)
  hookWheel(hintAbbr2, WheelTagAbbr)
  hookWheel(sbAbbr, WheelTagAbbr)
  hookWheel(btnAddAbbr, WheelTagAbbr)
  hookWheel(lblAbbrStatus, WheelTagAbbr)
  hookWheel(btnSaveAbbr, WheelTagAbbr)

  let hintIgn1 = StaticText(pageIgnore,
    label="每行一个英文单词，生成命名时直接删掉（内置泛词已列出）",
    pos=(dpx(PadL), dpx(10)), size=(dpx(ContW), dpx(50)))
  let hintIgn2 = StaticText(pageIgnore,
    label="删掉某行＝保留该词；中文先经内置词表转英文（如 状态 → state）再判断。",
    pos=(dpx(PadL), dpx(60)), size=(dpx(ContW), dpx(50)))
  sbIgnore = ScrollBar(pageIgnore, pos=(dpx(496), dpx(AbbrTop)),
                       size=(dpx(18), dpx(AbbrViewRows * AbbrRowH - 8)),
                       style=wSbVertical)
  btnAddIgnore = Button(pageIgnore, label="添加一行",
                        pos=(dpx(PadL), by), size=(dpx(92), dpx(32)))
  lblIgnoreStatus = StaticText(pageIgnore, label="",
                               pos=(dpx(PadL + 104), by + 6), size=(dpx(300), dpx(22)))
  btnSaveIgnore = Button(pageIgnore, label="保存",
                         pos=(dpx(544 - PadL - 92), by), size=(dpx(92), dpx(32)))
  # 页面本体 + 页面上每一块都挂滚轮：说明文字/状态栏/按钮/滚动条上滚也能滚列表
  hookWheel(pageAbbr, WheelTagAbbr)
  hookWheel(pageIgnore, WheelTagIgnore)
  hookWheel(hintIgn1, WheelTagIgnore)
  hookWheel(hintIgn2, WheelTagIgnore)
  hookWheel(sbIgnore, WheelTagIgnore)
  hookWheel(btnAddIgnore, WheelTagIgnore)
  hookWheel(lblIgnoreStatus, WheelTagIgnore)
  hookWheel(btnSaveIgnore, WheelTagIgnore)

  # 常用词页：与缩略词页同款布局（中文 → 英文词库）
  let hintPref1 = StaticText(pagePreferred,
    label="每行一条：中文 → 英文词库，例 串口 → uart（内置已列出）",
    pos=(dpx(PadL), dpx(10)), size=(dpx(ContW), dpx(50)))
  let hintPref2 = StaticText(pagePreferred,
    label="改英文＝覆盖内置映射（串口 → serial port），删行＝停用该映射；也可加全新词条，保存后立即生效。",
    pos=(dpx(PadL), dpx(60)), size=(dpx(ContW), dpx(50)))
  sbPref = ScrollBar(pagePreferred, pos=(dpx(496), dpx(AbbrTop)),
                     size=(dpx(18), dpx(AbbrViewRows * AbbrRowH - 8)),
                     style=wSbVertical)
  btnAddPref = Button(pagePreferred, label="添加一行",
                      pos=(dpx(PadL), by), size=(dpx(92), dpx(32)))
  lblPrefStatus = StaticText(pagePreferred, label="",
                             pos=(dpx(PadL + 104), by + 6), size=(dpx(300), dpx(22)))
  btnSavePref = Button(pagePreferred, label="保存",
                       pos=(dpx(544 - PadL - 92), by), size=(dpx(92), dpx(32)))
  hookWheel(pagePreferred, WheelTagPreferred)
  hookWheel(hintPref1, WheelTagPreferred)
  hookWheel(hintPref2, WheelTagPreferred)
  hookWheel(sbPref, WheelTagPreferred)
  hookWheel(btnAddPref, WheelTagPreferred)
  hookWheel(lblPrefStatus, WheelTagPreferred)
  hookWheel(btnSavePref, WheelTagPreferred)

  cmbStyle.append(StyleNames)
  cmbStyle.setSelection(if g_set.wordFmt == FY_FMT_SNAKE: 0 else: 1)
  cmbKind.append(KindNames)
  cmbKind.setSelection(g_set.nameKind)
  for p in AiPresets:
    cmbAiPreset.append(if p[0].len == 0: "选择预设…" else: p[0])
  cmbAiPreset.setSelection(0)

  setFrame.wEvent_Close do (event: wEvent):
    # 关掉设置页时记住窗口大小
    discard fy_cfg_save(g_cfg)
    event.veto()
    setFrame.hide()
  btnSaveSet1.wEvent_Button do ():
    saveSettings()
  btnSaveSet2.wEvent_Button do ():
    saveSettings()
  btnSaveSet3.wEvent_Button do ():
    saveSettings()
  btnAddAbbr.wEvent_Button do ():
    addAbbrRow()
  btnSaveAbbr.wEvent_Button do ():
    discard saveAbbrRows()
  sbAbbr.wEvent_ScrollBar do ():
    abbrScroll = sbAbbr.getScrollPos()
    renderAbbr()
  btnAddIgnore.wEvent_Button do ():
    addIgnoreRow()
  btnSaveIgnore.wEvent_Button do ():
    discard saveIgnoreRows()
  sbIgnore.wEvent_ScrollBar do ():
    ignoreScroll = sbIgnore.getScrollPos()
    renderIgnore()
  btnAddPref.wEvent_Button do ():
    addPrefRow()
  btnSavePref.wEvent_Button do ():
    discard savePrefRows()
  sbPref.wEvent_ScrollBar do ():
    prefScroll = sbPref.getScrollPos()
    renderPref()
  cmbAiPreset.wEvent_ComboBox do ():
    # 预设填充地址与模型（第 0 项"选择预设…"不动）
    let i = cmbAiPreset.getSelection()
    if i > 0 and i < AiPresets.len:
      edtAiUrl.setValue(AiPresets[i][1])
      edtAiModel.setValue(AiPresets[i][2])
      cmbAiPreset.setSelection(0)
  btnResetSize.wEvent_Button do ():
    applyWinSize(g_defWinW, g_defWinH)
    syncSizeFields()
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
          nbSet.setSelection(wantTab)
          renderAbbr()
          renderIgnore()
          renderPref()
      frame.startTimer(1.5, 68)

  # 测试钩子：-testtranslate 启动后自动翻译一段固定文本，结果写入 test.log 后退出
  # （配 mock 服务器做端到端验证用，也顺带验证 run.log 全链路埋点）
  if "-testtranslate" in commandLineParams():
    g_testTranslate = true
    frame.startTimer(1.2, 65)
    frame.wEvent_Timer do (event: wEvent):
      if event.timerId == 65:
        frame.stopTimer(65)
        input.setValue("The quick brown fox jumps over the lazy dog.")
        doTranslate(FY_AUTO)

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
