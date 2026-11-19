## 快译NG 现代化设置窗口模块（参照 Snipaste 左侧导航 + 子标签 + 丝滑过渡动效）
## 纯 Win32 GDI / GDI+ / AlphaBlend 实现，零外部依赖，支持深浅主题与 75%~200% 缩放

import std/[strutils, math, tables]
import winim/lean
import wNim/[wApp, wFrame, wPanel, wButton, wCheckBox, wTextCtrl, wComboBox, wTypes]

const
  SetWinW* = 740
  SetWinH* = 590
  SetTitleH* = 40
  SetNavW* = 156
  SetSubBarH* = 44
  SetPageX* = SetNavW
  SetPageY* = SetTitleH + SetSubBarH   # 84
  SetPageW* = SetWinW - SetNavW        # 584
  SetPageH* = SetWinH - SetPageY       # 506
  SetPadL* = 20
  SetContW* = SetPageW - SetPadL * 2   # 544

  GdiplusDll = "gdiplus.dll"
  DwmapiDll = "dwmapi.dll"
  ComctlDll = "comctl32.dll"
  UxthemeDll = "uxtheme.dll"
  MsimgDll = "msimg32.dll"
  WinmmDll = "winmm.dll"

  DwmwaUseImmersiveDarkMode = DWORD(20)
  DwmwaWindowCornerPreference = DWORD(33)
  DwmwcpRound = 2'i32

  WmNcCalcSize = UINT(0x0083)
  WmNcHitTest = UINT(0x0084)
  WmNcPaint = UINT(0x0085)
  WmNcActivate = UINT(0x0086)
  WmMouseLeave = UINT(0x02A3)
  WmPrint = UINT(0x0317)
  EmSetMargins = UINT(0x00D3)
  EcLeftMargin = 0x0001
  EcRightMargin = 0x0002

  PrfClient = 0x00000004
  PrfChildren = 0x00000010
  PrfEraseBkgnd = 0x00000008
  PrfNonClient = 0x00000002

  HtClient = 1
  HtCaption = 2
  HtTransparent = -1
  TmeLeave = DWORD(0x00000002)

  # 动画定时器 ID（使用 Win32 SetTimer + timeBeginPeriod(1)，单位毫秒）
  AnimTimerId = UINT_PTR(0x4D99)
  AnimIntervalMs = UINT(8)    # ~120fps 顺滑刷新

  # Tooltip 常量
  TtsAlwaysTip = 0x01'i32
  TtsNoPrefix = 0x02'i32
  TtmAddToolW = UINT(WM_USER + 50)
  TtmSetMaxTipWidth = UINT(WM_USER + 24)
  TtmSetDelayTime = UINT(WM_USER + 3)
  TtdtInitial = 3
  TtfIdishwnd = UINT(0x0001)
  TtfSubclass = UINT(0x0010)

type
  GpStatus = cint
  GpGraphics = pointer
  GpBrush = pointer
  GpPen = pointer
  GpPath = pointer
  ARGB = uint32

  ModBtnKind* = enum
    mbNormal    ## 常规灰蓝按钮
    mbPrimary   ## 主操作蓝按钮 (#3B82F6)
    mbDanger    ## 删除按钮（hover 变红）

  SetTextRole* = enum
    trNormal    ## 普通文字
    trHint      ## 次要说明
    trTitle     ## 分组标题
    trDanger    ## 错误/冲突警告（红色）
    trSuccess   ## 成功状态（绿色）

  SetCardRect* = tuple[x, y, w, h: int]

  NavCatDef* = object
    name*: string
    icon*: uint16          ## Segoe MDL2 Assets / Segoe Fluent Icons 码点
    subtitle*: string      ## 单页大类的副标题
    subNames*: seq[string] ## 子标签名（长度 <= 1 时顶部显示大类标题）
    pageIndices*: seq[int] ## 对应的全局页面索引

  SetTrackMouseEvent = object
    cbSize: DWORD
    dwFlags: DWORD
    hwndTrack: HWND
    dwHoverTime: DWORD

  BlendFunc = object
    blendOp: uint8
    blendFlags: uint8
    sourceConstantAlpha: uint8
    alphaFormat: uint8

  ToolInfoW = object
    cbSize: UINT
    uFlags: UINT
    hwnd: HWND
    uId: UINT_PTR
    rect: RECT
    hinst: HINSTANCE
    lpszText: LPWSTR
    lParam: LPARAM
    lpReserved: pointer

  SetSubclassProc = proc (hWnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM,
                          uIdSubclass: UINT_PTR, dwRefData: DWORD_PTR): LRESULT {.stdcall.}

  PageMeta = ref object
    hwnd: HWND
    cards: seq[SetCardRect]
    edits: seq[tuple[h: HWND, inCard: bool]]

  BtnMeta = ref object
    kind: ModBtnKind
    inCard: bool
    hover: bool
    down: bool
    tracking: bool
    hoverT: float          ## 悬停渐变动效进度 0..1

  ChkMeta = ref object
    inCard: bool
    hover: bool
    tracking: bool
    animPos: float         ## 滑动开关位置 0.0(关)..1.0(开)
    targetPos: float

  StaticMeta = object
    inCard: bool
    role: SetTextRole

  TransDir = enum
    tdNone, tdDown, tdUp, tdRight, tdLeft

proc GdipCreateFromHDC(hdc: HDC, graphics: ptr GpGraphics): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipDeleteGraphics(graphics: GpGraphics): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipSetSmoothingMode(graphics: GpGraphics, mode: cint): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipCreateSolidFill(color: ARGB, brush: ptr GpBrush): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipDeleteBrush(brush: GpBrush): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipCreatePen1(color: ARGB, width: cfloat, unit: cint, pen: ptr GpPen): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipDeletePen(pen: GpPen): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipCreatePath(fillMode: cint, path: ptr GpPath): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipDeletePath(path: GpPath): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipAddPathArc(path: GpPath, x, y, w, h, startAngle, sweepAngle: cfloat): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipClosePathFigure(path: GpPath): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipFillPath(graphics: GpGraphics, brush: GpBrush, path: GpPath): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipDrawPath(graphics: GpGraphics, pen: GpPen, path: GpPath): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipDrawLine(graphics: GpGraphics, pen: GpPen, x1, y1, x2, y2: cfloat): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
proc GdipFillEllipse(graphics: GpGraphics, brush: GpBrush, x, y, w, h: cfloat): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}

proc DwmSetWindowAttribute(hwnd: HWND, dwAttribute: DWORD, pvAttribute: pointer,
                           cbAttribute: DWORD): HRESULT {.stdcall, importc, dynlib: DwmapiDll.}
proc SetSetWindowSubclass(hWnd: HWND, pfn: SetSubclassProc, id: UINT_PTR,
                          dwRef: DWORD_PTR): WINBOOL {.stdcall, importc: "SetWindowSubclass", dynlib: ComctlDll.}
proc SetDefSubclassProc(hWnd: HWND, uMsg: UINT, wParam: WPARAM,
                        lParam: LPARAM): LRESULT {.stdcall, importc: "DefSubclassProc", dynlib: ComctlDll.}
proc SetTrackMouse(lpEventTrack: ptr SetTrackMouseEvent): WINBOOL {.stdcall, importc: "TrackMouseEvent", dynlib: "user32.dll".}
proc SetWindowTheme(hwnd: HWND, pszSubAppName: LPCWSTR, pszSubIdList: LPCWSTR): HRESULT {.stdcall, importc, dynlib: UxthemeDll.}
proc GdiAlphaBlend(hdcDest: HDC, xoriginDest, yoriginDest, wDest, hDest: cint,
                   hdcSrc: HDC, xoriginSrc, yoriginSrc, wSrc, hSrc: cint,
                   ftn: BlendFunc): WINBOOL {.stdcall, importc: "AlphaBlend", dynlib: MsimgDll.}
proc timeBeginPeriod(uPeriod: UINT): UINT {.stdcall, importc, dynlib: WinmmDll.}
proc timeEndPeriod(uPeriod: UINT): UINT {.stdcall, importc, dynlib: WinmmDll.}

# 默认 7 大类结构（与 main.nim 的 12 个子页一一对应）
const
  DefaultNavCats* = [
    NavCatDef(name: "常规", icon: 0xE713'u16, subtitle: "",
              subNames: @["基本", "界面", "行为"], pageIndices: @[0, 1, 2]),
    NavCatDef(name: "翻译", icon: 0xE8C1'u16, subtitle: "AI → 百度 → 必应 自动回退",
              subNames: @[], pageIndices: @[3]),
    NavCatDef(name: "命名", icon: 0xE8EC'u16, subtitle: "",
              subNames: @["缩减预算", "缩略词", "忽略词", "常用词"], pageIndices: @[4, 5, 6, 7]),
    NavCatDef(name: "截屏", icon: 0xE722'u16, subtitle: "F1 截屏 · F3 屏幕贴图 · 截屏翻译",
              subNames: @[], pageIndices: @[8]),
    NavCatDef(name: "控制", icon: 0xE765'u16, subtitle: "全局快捷键与冲突检测",
              subNames: @[], pageIndices: @[9]),
    NavCatDef(name: "存储", icon: 0xE8B7'u16, subtitle: "缓存目录与查询历史",
              subNames: @[], pageIndices: @[10]),
    NavCatDef(name: "关于", icon: 0xE946'u16, subtitle: "版本信息与检查更新",
              subNames: @[], pageIndices: @[11])
  ]

var
  g_setScale = 1.0
  g_setDark = true
  g_animEnabled = true
  g_hwndSetFrame: HWND
  g_hwndSetPanel: HWND
  g_hwndTooltip: HWND
  g_pages: seq[wPanel] = @[]
  g_curCat = 0
  g_curSub = 0
  g_curPage = 0
  g_lastSubPerCat: array[7, int]
  g_onPageChange: proc (pageIdx: int)
  g_onSetClose: proc ()
  g_onSetMin: proc ()
  g_onEditBlur: proc (h: HWND)

  # 顶部与侧边栏几何
  g_minRc, g_closeRc: RECT
  g_navRcs: array[7, RECT]
  g_subRcs: seq[RECT] = @[]

  # 命中编码：-1=无, 100=最小化, 101=关闭, 0..6=左侧大类, 200..209=顶部子标签
  g_topHover = -1
  g_topDown = -1
  g_topTracking = false

  # 动画定时器状态
  g_animRunning = false
  g_lastTick: uint64 = 0

  # 左侧导航滑块与悬停渐变
  g_navIndY = 0.0
  g_navIndTargetY = 0.0
  g_navHoverT: array[7, float]

  # 子标签滑块与悬停渐变
  g_subIndX = 0.0
  g_subIndW = 0.0
  g_subIndTargetX = 0.0
  g_subIndTargetW = 0.0
  g_subHoverT: array[8, float]

  # 窗口打开淡入动画
  g_openAnimActive = false
  g_openAnimStart: uint64 = 0
  g_openTargetX, g_openTargetY: cint

  # 页面交叉淡入+位移动画
  g_transActive = false
  g_transStart: uint64 = 0
  g_transDir = tdNone
  g_transOldBm: HBITMAP = 0
  g_transNewBm: HBITMAP = 0
  g_transW, g_transH: cint
  g_transTargetPage = 0

  # 底部 "✓ 已保存" Toast 动画
  g_toastText = ""
  g_toastIsError = false
  g_toastActive = false
  g_toastStart: uint64 = 0

  # 画刷 & Chrome 字体缓存（避免每帧 WM_PAINT 重复 CreateFontW）
  g_brBg: HBRUSH
  g_brNav: HBRUSH
  g_brCard: HBRUSH
  g_brEditBg: HBRUSH
  g_brEditCardBg: HBRUSH
  g_hfTitle, g_hfIcon, g_hfNav, g_hfSub, g_hfHead, g_hfSubDesc, g_hfToast: HFONT
  g_hfCachedScale = -1.0

  # 元数据表（按 HWND 索引）
  g_pageMetas = initTable[HWND, PageMeta]()
  g_btnMetas = initTable[HWND, BtnMeta]()
  g_chkMetas = initTable[HWND, ChkMeta]()
  g_staticMetas = initTable[HWND, StaticMeta]()
  g_editMeta = initTable[HWND, bool]()      # hwnd -> inCard
  g_editErr = initTable[HWND, bool]()       # hwnd -> 是否红框错误态（如热键冲突）
  g_sysThemeControls: seq[tuple[h: HWND, darkTheme, lightTheme: string]] = @[]

proc argb(r, g, b: uint8, a: uint8 = 255): ARGB {.inline.} =
  (ARGB(a) shl 24) or (ARGB(r) shl 16) or (ARGB(g) shl 8) or ARGB(b)

proc lerpU8(a, b: uint8, t: float): uint8 {.inline.} =
  let v = float(a) + (float(b) - float(a)) * clamp(t, 0.0, 1.0)
  uint8(clamp(int(round(v)), 0, 255))

proc lerpArgb(c1, c2: ARGB, t: float): ARGB {.inline.} =
  let a1 = uint8((c1 shr 24) and 0xFF)
  let r1 = uint8((c1 shr 16) and 0xFF)
  let g1 = uint8((c1 shr 8) and 0xFF)
  let b1 = uint8(c1 and 0xFF)
  let a2 = uint8((c2 shr 24) and 0xFF)
  let r2 = uint8((c2 shr 16) and 0xFF)
  let g2 = uint8((c2 shr 8) and 0xFF)
  let b2 = uint8(c2 and 0xFF)
  argb(lerpU8(r1, r2, t), lerpU8(g1, g2, t), lerpU8(b1, b2, t), lerpU8(a1, a2, t))

proc easeOutCubic(t: float): float {.inline.} =
  let u = 1.0 - clamp(t, 0.0, 1.0)
  1.0 - u * u * u

proc loSigned(v: int64): int {.inline.} =
  int(cast[int16](uint16(uint64(v) and 0xFFFF'u64)))

proc hiSigned(v: int64): int {.inline.} =
  int(cast[int16](uint16((uint64(v) shr 16) and 0xFFFF'u64)))

proc S(x: int): int {.inline.} =
  max(1, int(round(float(x) * g_setScale)))

proc freeBrushes() =
  if g_brBg != 0: DeleteObject(g_brBg); g_brBg = 0
  if g_brNav != 0: DeleteObject(g_brNav); g_brNav = 0
  if g_brCard != 0: DeleteObject(g_brCard); g_brCard = 0
  if g_brEditBg != 0: DeleteObject(g_brEditBg); g_brEditBg = 0
  if g_brEditCardBg != 0: DeleteObject(g_brEditCardBg); g_brEditCardBg = 0

proc ensureBrushes() =
  if g_setDark:
    if g_brBg == 0: g_brBg = CreateSolidBrush(RGB(24, 26, 32))
    if g_brNav == 0: g_brNav = CreateSolidBrush(RGB(20, 22, 28))
    if g_brCard == 0: g_brCard = CreateSolidBrush(RGB(34, 38, 46))
    if g_brEditBg == 0: g_brEditBg = CreateSolidBrush(RGB(27, 30, 38))
    if g_brEditCardBg == 0: g_brEditCardBg = CreateSolidBrush(RGB(24, 27, 34))
  else:
    if g_brBg == 0: g_brBg = CreateSolidBrush(RGB(244, 246, 250))
    if g_brNav == 0: g_brNav = CreateSolidBrush(RGB(236, 240, 246))
    if g_brCard == 0: g_brCard = CreateSolidBrush(RGB(255, 255, 255))
    if g_brEditBg == 0: g_brEditBg = CreateSolidBrush(RGB(255, 255, 255))
    if g_brEditCardBg == 0: g_brEditCardBg = CreateSolidBrush(RGB(248, 250, 252))

proc freeChromeFonts() =
  for hf in [g_hfTitle, g_hfIcon, g_hfNav, g_hfSub, g_hfHead, g_hfSubDesc, g_hfToast]:
    if hf != 0: DeleteObject(hf)
  g_hfTitle = 0; g_hfIcon = 0; g_hfNav = 0; g_hfSub = 0
  g_hfHead = 0; g_hfSubDesc = 0; g_hfToast = 0
  g_hfCachedScale = -1.0

proc ensureChromeFonts() =
  if abs(g_hfCachedScale - g_setScale) < 0.005 and g_hfTitle != 0: return
  freeChromeFonts()
  g_hfCachedScale = g_setScale
  let titlePx = max(12, int(round(14.5 * g_setScale)))
  g_hfTitle = CreateFontW(cint(-titlePx), 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                          0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let iconPx = max(12, int(round(15.0 * g_setScale)))
  g_hfIcon = CreateFontW(cint(-iconPx), 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                         0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Segoe MDL2 Assets"))
  let navPx = max(12, int(round(14.5 * g_setScale)))
  g_hfNav = CreateFontW(cint(-navPx), 0, 0, 0, FW_MEDIUM, 0, 0, 0, DEFAULT_CHARSET,
                        0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let subPx = max(12, int(round(14.5 * g_setScale)))
  g_hfSub = CreateFontW(cint(-subPx), 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                        0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let headPx = max(13, int(round(16.0 * g_setScale)))
  g_hfHead = CreateFontW(cint(-headPx), 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                         0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let descPx = max(11, int(round(12.5 * g_setScale)))
  g_hfSubDesc = CreateFontW(cint(-descPx), 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                            0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  g_hfToast = CreateFontW(cint(-descPx), 0, 0, 0, FW_MEDIUM, 0, 0, 0, DEFAULT_CHARSET,
                          0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))

proc freeTransBitmaps() =
  if g_transOldBm != 0: DeleteObject(g_transOldBm); g_transOldBm = 0
  if g_transNewBm != 0: DeleteObject(g_transNewBm); g_transNewBm = 0

proc nowMs(): uint64 {.inline.} =
  uint64(GetTickCount64())

proc startAnimLoop() =
  if g_hwndSetPanel == 0: return
  if not g_animRunning:
    g_animRunning = true
    g_lastTick = nowMs()
    discard timeBeginPeriod(1)
    discard SetTimer(g_hwndSetPanel, AnimTimerId, AnimIntervalMs, nil)

proc stopAnimLoop() =
  if g_animRunning and g_hwndSetPanel != 0:
    discard KillTimer(g_hwndSetPanel, AnimTimerId)
    discard timeEndPeriod(1)
    g_animRunning = false

proc makeRoundPath(x, y, w, h, r: cfloat): GpPath =
  var path: GpPath = nil
  if GdipCreatePath(0, addr path) != 0 or path == nil: return nil
  let d = min(r * 2.0'f32, min(w, h))
  if d <= 0.5'f32:
    discard GdipAddPathArc(path, x, y, 1.0'f32, 1.0'f32, 180.0'f32, 90.0'f32)
    discard GdipClosePathFigure(path)
    return path
  discard GdipAddPathArc(path, x, y, d, d, 180.0'f32, 90.0'f32)
  discard GdipAddPathArc(path, x + w - d, y, d, d, 270.0'f32, 90.0'f32)
  discard GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0'f32, 90.0'f32)
  discard GdipAddPathArc(path, x, y + h - d, d, d, 90.0'f32, 90.0'f32)
  discard GdipClosePathFigure(path)
  result = path

proc fillRoundRect(g: GpGraphics, rc: RECT, r: float, col: ARGB) =
  let path = makeRoundPath(cfloat(rc.left), cfloat(rc.top),
                           cfloat(rc.right - rc.left), cfloat(rc.bottom - rc.top), cfloat(r))
  if path == nil: return
  var br: GpBrush = nil
  if GdipCreateSolidFill(col, addr br) == 0 and br != nil:
    discard GdipFillPath(g, br, path)
    discard GdipDeleteBrush(br)
  discard GdipDeletePath(path)

proc drawRoundRect(g: GpGraphics, rc: RECT, r: float, col: ARGB, width = 1.0'f32) =
  let path = makeRoundPath(cfloat(rc.left) + 0.5'f32, cfloat(rc.top) + 0.5'f32,
                           cfloat(rc.right - rc.left) - 1.0'f32,
                           cfloat(rc.bottom - rc.top) - 1.0'f32, cfloat(r))
  if path == nil: return
  var pen: GpPen = nil
  if GdipCreatePen1(col, width, 2, addr pen) == 0 and pen != nil:
    discard GdipDrawPath(g, pen, path)
    discard GdipDeletePen(pen)
  discard GdipDeletePath(path)

proc ptInRect(rc: RECT, x, y: int): bool {.inline.} =
  x >= int(rc.left) and x < int(rc.right) and y >= int(rc.top) and y < int(rc.bottom)

proc layoutSubTabs() =
  let cat = DefaultNavCats[g_curCat]
  g_subRcs.setLen(0)
  if cat.subNames.len <= 1: return
  let startX = S(SetPageX + SetPadL)
  let topY = S(SetTitleH + 6)
  let itemH = S(SetSubBarH - 12)
  let gap = S(8)
  var x = startX
  for name in cat.subNames:
    # 按汉字数估算紧凑宽度
    let runes = name.len div 3
    let w = S(max(62, runes * 16 + 26))
    g_subRcs.add(RECT(left: LONG(x), top: LONG(topY), right: LONG(x + w), bottom: LONG(topY + itemH)))
    x += w + gap

proc layoutChrome(scale: float) =
  g_setScale = scale
  let w = S(SetWinW)
  let titleH = S(SetTitleH)
  let tbW = S(34)
  let tbH = S(26)
  let tbY = (titleH - tbH) div 2
  let closeX = w - S(10) - tbW
  let minX = closeX - S(4) - tbW
  g_closeRc = RECT(left: LONG(closeX), top: LONG(tbY), right: LONG(closeX + tbW), bottom: LONG(tbY + tbH))
  g_minRc = RECT(left: LONG(minX), top: LONG(tbY), right: LONG(minX + tbW), bottom: LONG(tbY + tbH))

  let navPadX = S(10)
  let navW = S(SetNavW) - navPadX * 2
  let navItemH = S(40)
  let navGap = S(6)
  var y = titleH + S(12)
  for i in 0 .. DefaultNavCats.high:
    g_navRcs[i] = RECT(left: LONG(navPadX), top: LONG(y),
                       right: LONG(navPadX + navW), bottom: LONG(y + navItemH))
    y += navItemH + navGap

  g_navIndTargetY = float(g_navRcs[g_curCat].top)
  g_navIndY = g_navIndTargetY

  layoutSubTabs()
  if g_curSub < g_subRcs.len:
    g_subIndTargetX = float(g_subRcs[g_curSub].left)
    g_subIndTargetW = float(g_subRcs[g_curSub].right - g_subRcs[g_curSub].left)
    g_subIndX = g_subIndTargetX
    g_subIndW = g_subIndTargetW

proc capturePageBitmap(page: wPanel, w, h: cint): HBITMAP =
  if page.isNil or w <= 0 or h <= 0: return 0
  let ph = cast[HWND](page.getHandle())
  if ph == 0: return 0
  let hdcScr = GetDC(0)
  let hdcMem = CreateCompatibleDC(hdcScr)
  var bmi: BITMAPINFO
  bmi.bmiHeader.biSize = DWORD(sizeof(BITMAPINFOHEADER))
  bmi.bmiHeader.biWidth = LONG(w)
  bmi.bmiHeader.biHeight = LONG(-h)
  bmi.bmiHeader.biPlanes = 1
  bmi.bmiHeader.biBitCount = 32
  bmi.bmiHeader.biCompression = BI_RGB
  var bits: pointer = nil
  let hbm = CreateDIBSection(hdcScr, addr bmi, DIB_RGB_COLORS, addr bits, 0, 0)
  ReleaseDC(0, hdcScr)
  if hbm == 0:
    DeleteDC(hdcMem)
    return 0
  let oldBm = SelectObject(hdcMem, hbm)
  ensureBrushes()
  var rc = RECT(left: 0, top: 0, right: LONG(w), bottom: LONG(h))
  FillRect(hdcMem, addr rc, g_brBg)
  let wasVis = IsWindowVisible(ph) != 0
  if not wasVis:
    discard ShowWindow(ph, SW_SHOWNA)
  discard SendMessageW(ph, WmPrint, WPARAM(hdcMem),
                       LPARAM(PrfClient or PrfChildren or PrfEraseBkgnd or PrfNonClient))
  if not wasVis:
    discard ShowWindow(ph, SW_HIDE)
  SelectObject(hdcMem, oldBm)
  DeleteDC(hdcMem)
  result = hbm

proc finishPageTransition() =
  if not g_transActive: return
  g_transActive = false
  freeTransBitmaps()
  for i, p in g_pages:
    if p != nil:
      if i == g_transTargetPage: p.show()
      else: p.hide()
  if g_hwndSetPanel != 0:
    discard InvalidateRect(g_hwndSetPanel, nil, FALSE)

proc findCatSubForPage(pageIdx: int): tuple[cat, sub: int] =
  for ci, cat in DefaultNavCats:
    for si, pi in cat.pageIndices:
      if pi == pageIdx:
        return (ci, si)
  result = (0, 0)

proc modernSelectSettingsPage*(pageIdx: int, animate = true) =
  if pageIdx < 0 or pageIdx >= g_pages.len: return
  let (newCat, newSub) = findCatSubForPage(pageIdx)
  let oldPage = g_curPage
  let oldCat = g_curCat
  let oldSub = g_curSub

  if g_transActive:
    finishPageTransition()

  let samePage = (pageIdx == oldPage)
  g_curCat = newCat
  g_curSub = newSub
  g_curPage = pageIdx
  g_lastSubPerCat[newCat] = newSub

  g_navIndTargetY = float(g_navRcs[g_curCat].top)
  layoutSubTabs()
  if g_curSub < g_subRcs.len:
    g_subIndTargetX = float(g_subRcs[g_curSub].left)
    g_subIndTargetW = float(g_subRcs[g_curSub].right - g_subRcs[g_curSub].left)
    if newCat != oldCat or not animate:
      g_subIndX = g_subIndTargetX
      g_subIndW = g_subIndTargetW
  if not animate:
    g_navIndY = g_navIndTargetY

  if g_onPageChange != nil:
    g_onPageChange(pageIdx)

  let canAnimate = g_animEnabled and animate and (not samePage) and g_hwndSetFrame != 0 and
                   IsWindowVisible(g_hwndSetFrame) != 0 and
                   oldPage >= 0 and oldPage < g_pages.len

  if canAnimate:
    let pw = cint(S(SetPageW))
    let ph = cint(S(SetPageH))
    g_transW = pw
    g_transH = ph
    g_transTargetPage = pageIdx
    g_transOldBm = capturePageBitmap(g_pages[oldPage], pw, ph)
    g_transNewBm = capturePageBitmap(g_pages[pageIdx], pw, ph)
    if g_transOldBm != 0 and g_transNewBm != 0:
      for p in g_pages:
        if p != nil: p.hide()
      if newCat != oldCat:
        g_transDir = if newCat > oldCat: tdDown else: tdUp
      else:
        g_transDir = if newSub > oldSub: tdRight else: tdLeft
      g_transActive = true
      g_transStart = nowMs()
      startAnimLoop()
      if g_hwndSetPanel != 0:
        discard InvalidateRect(g_hwndSetPanel, nil, FALSE)
        discard UpdateWindow(g_hwndSetPanel)
      return
    else:
      freeTransBitmaps()

  if not g_animEnabled or not animate:
    g_navIndY = g_navIndTargetY
    if g_curSub < g_subRcs.len:
      g_subIndX = g_subIndTargetX
      g_subIndW = g_subIndTargetW

  for i, p in g_pages:
    if p != nil:
      if i == pageIdx: p.show()
      else: p.hide()
  if g_animEnabled and animate:
    startAnimLoop()
  if g_hwndSetPanel != 0:
    discard InvalidateRect(g_hwndSetPanel, nil, FALSE)

proc modernSelectSettingsTab*(idx: int) =
  modernSelectSettingsPage(idx, animate = true)

proc modernGetSettingsTab*(): int = g_curPage

proc modernLayoutSettings*(scale: float) =
  if g_transActive:
    finishPageTransition()
  layoutChrome(scale)
  let px = S(SetPageX)
  let py = S(SetPageY)
  let pw = S(SetPageW)
  let ph = S(SetPageH)
  for p in g_pages:
    if p != nil:
      p.setSize(px, py, pw, ph)
      discard InvalidateRect(cast[HWND](p.getHandle()), nil, FALSE)
  if g_hwndSetPanel != 0:
    discard InvalidateRect(g_hwndSetPanel, nil, FALSE)

proc modernShowSavedToast*(msg = "已保存", isError = false) =
  g_toastText = msg
  g_toastIsError = isError
  g_toastActive = true
  g_toastStart = nowMs()
  startAnimLoop()
  if g_hwndSetPanel != 0:
    discard InvalidateRect(g_hwndSetPanel, nil, FALSE)

proc modernShowSettingsAnimated*(setFrame: wFrame) =
  ## 带轻量淡入的窗口打开动效（130ms；关闭动效时直接显示）
  if setFrame.isNil: return
  let h = cast[HWND](setFrame.getHandle())
  if h == 0: return
  if not g_animEnabled:
    setFrame.show()
    return
  let ex = GetWindowLongPtrW(h, GWL_EXSTYLE)
  discard SetWindowLongPtrW(h, GWL_EXSTYLE, ex or WS_EX_LAYERED)
  discard SetLayeredWindowAttributes(h, 0, 30, LWA_ALPHA)
  setFrame.show()
  g_openAnimActive = true
  g_openAnimStart = nowMs()
  startAnimLoop()

proc modernSetAnimEnabled*(enabled: bool) =
  ## 开启/关闭设置界面动效；关闭时立即落定所有进行中的动画
  g_animEnabled = enabled
  if not enabled:
    if g_openAnimActive and g_hwndSetFrame != 0:
      g_openAnimActive = false
      discard SetLayeredWindowAttributes(g_hwndSetFrame, 0, 255, LWA_ALPHA)
      let ex = GetWindowLongPtrW(g_hwndSetFrame, GWL_EXSTYLE)
      discard SetWindowLongPtrW(g_hwndSetFrame, GWL_EXSTYLE, ex and (not WS_EX_LAYERED))
    if g_transActive:
      finishPageTransition()
    g_navIndY = g_navIndTargetY
    g_subIndX = g_subIndTargetX
    g_subIndW = g_subIndTargetW
    for i in 0 .. g_navHoverT.high: g_navHoverT[i] = 0.0
    for i in 0 .. g_subHoverT.high: g_subHoverT[i] = 0.0
    for h, cm in g_chkMetas.pairs:
      cm.animPos = cm.targetPos
      if IsWindowVisible(h) != 0: discard InvalidateRect(h, nil, FALSE)
    for h, bm in g_btnMetas.pairs:
      bm.hoverT = if bm.hover: 1.0 else: 0.0
      if IsWindowVisible(h) != 0: discard InvalidateRect(h, nil, FALSE)
    if g_hwndSetPanel != 0:
      discard InvalidateRect(g_hwndSetPanel, nil, FALSE)

proc modernSetEditError*(h: HWND, isErr: bool) =
  if h == 0: return
  if isErr:
    g_editErr[h] = true
  else:
    g_editErr.del(h)
  let par = GetParent(h)
  if par != 0:
    discard InvalidateRect(par, nil, FALSE)

proc modernSetTooltip*(h: HWND, text: string) =
  ## 为控件挂接原生悬停提示（Snipaste 风格，保持页面干净）
  if h == 0 or text.len == 0 or g_hwndSetFrame == 0: return
  if g_hwndTooltip == 0:
    g_hwndTooltip = CreateWindowExW(
      WS_EX_TOPMOST,
      newWideCString("tooltips_class32"),
      nil,
      WS_POPUP or TtsAlwaysTip or TtsNoPrefix,
      CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT,
      g_hwndSetFrame, 0, GetModuleHandleW(nil), nil
    )
    if g_hwndTooltip != 0:
      discard SendMessageW(g_hwndTooltip, TtmSetMaxTipWidth, 0, LPARAM(320))
      discard SendMessageW(g_hwndTooltip, TtmSetDelayTime, WPARAM(TtdtInitial), LPARAM(350))
  if g_hwndTooltip == 0: return
  var ti: ToolInfoW
  zeroMem(addr ti, sizeof(ToolInfoW))
  ti.cbSize = UINT(sizeof(ToolInfoW))
  ti.uFlags = TtfIdishwnd or TtfSubclass
  ti.hwnd = GetParent(h)
  ti.uId = cast[UINT_PTR](h)
  let wtxt = newWideCString(text)
  ti.lpszText = cast[LPWSTR](wtxt)
  discard SendMessageW(g_hwndTooltip, TtmAddToolW, 0, cast[LPARAM](addr ti))

# ---------------- 动画主循环（timeBeginPeriod(1) + 8ms 同步刷新） ----------------
proc stepAnimations() =
  let now = nowMs()
  let dt = clamp(float(now - g_lastTick) / 1000.0, 0.001, 0.05)
  g_lastTick = now
  var needMore = false
  var chromeDirty = false
  var pageDirty = false

  # 1. 窗口打开轻量淡入 (130ms)
  if g_openAnimActive and g_hwndSetFrame != 0:
    let t = float(now - g_openAnimStart) / 130.0
    if t >= 1.0:
      g_openAnimActive = false
      discard SetLayeredWindowAttributes(g_hwndSetFrame, 0, 255, LWA_ALPHA)
      let ex = GetWindowLongPtrW(g_hwndSetFrame, GWL_EXSTYLE)
      discard SetWindowLongPtrW(g_hwndSetFrame, GWL_EXSTYLE, ex and (not WS_EX_LAYERED))
    else:
      let e = easeOutCubic(t)
      let alpha = uint8(clamp(int(round(30.0 + 225.0 * e)), 30, 255))
      discard SetLayeredWindowAttributes(g_hwndSetFrame, 0, alpha, LWA_ALPHA)
      needMore = true

  # 2. 左侧导航选中条滑动
  let navDiff = g_navIndTargetY - g_navIndY
  if abs(navDiff) > 0.4:
    g_navIndY += navDiff * min(1.0, dt * 22.0)
    chromeDirty = true
    needMore = true
  elif g_navIndY != g_navIndTargetY:
    g_navIndY = g_navIndTargetY
    chromeDirty = true

  # 3. 左侧导航悬停渐变 (110ms)
  for i in 0 .. DefaultNavCats.high:
    let target = if g_topHover == i and i != g_curCat: 1.0 else: 0.0
    if abs(g_navHoverT[i] - target) > 0.01:
      if not g_animEnabled:
        g_navHoverT[i] = target
      else:
        let step = dt / 0.11
        if g_navHoverT[i] < target: g_navHoverT[i] = min(target, g_navHoverT[i] + step)
        else: g_navHoverT[i] = max(target, g_navHoverT[i] - step)
        needMore = true
      chromeDirty = true

  # 4. 子标签下划线滑动 & 悬停渐变
  if g_subRcs.len > 1:
    let dx = g_subIndTargetX - g_subIndX
    let dw = g_subIndTargetW - g_subIndW
    if abs(dx) > 0.4 or abs(dw) > 0.4:
      g_subIndX += dx * min(1.0, dt * 24.0)
      g_subIndW += dw * min(1.0, dt * 24.0)
      chromeDirty = true
      needMore = true
    elif g_subIndX != g_subIndTargetX or g_subIndW != g_subIndTargetW:
      g_subIndX = g_subIndTargetX
      g_subIndW = g_subIndTargetW
      chromeDirty = true

    for i in 0 ..< min(g_subRcs.len, g_subHoverT.len):
      let target = if g_topHover == 200 + i and i != g_curSub: 1.0 else: 0.0
      if abs(g_subHoverT[i] - target) > 0.01:
        if not g_animEnabled:
          g_subHoverT[i] = target
        else:
          let step = dt / 0.11
          if g_subHoverT[i] < target: g_subHoverT[i] = min(target, g_subHoverT[i] + step)
          else: g_subHoverT[i] = max(target, g_subHoverT[i] - step)
          needMore = true
        chromeDirty = true

  # 5. 页面交叉淡入 + 滑动过渡 (170ms)
  if g_transActive:
    let t = float(now - g_transStart) / 170.0
    if t >= 1.0:
      finishPageTransition()
    else:
      pageDirty = true
      needMore = true

  # 6. Toast 淡入/停留/淡出 (160 + 1100 + 260 = 1520ms)
  if g_toastActive:
    let elapsed = int(now - g_toastStart)
    if elapsed >= 1520:
      g_toastActive = false
      chromeDirty = true
    else:
      if not g_animEnabled:
        needMore = true
      else:
        if elapsed <= 180 or elapsed >= 1240:
          chromeDirty = true
        needMore = true

  # 7. Toggle Switch 滑动 (130ms)
  for h, cm in g_chkMetas.pairs:
    if abs(cm.animPos - cm.targetPos) > 0.01:
      if not g_animEnabled:
        cm.animPos = cm.targetPos
      else:
        let step = dt / 0.13
        if cm.animPos < cm.targetPos: cm.animPos = min(cm.targetPos, cm.animPos + step)
        else: cm.animPos = max(cm.targetPos, cm.animPos - step)
        needMore = true
      if IsWindowVisible(h) != 0:
        discard InvalidateRect(h, nil, FALSE)
        discard UpdateWindow(h)

  # 8. 按钮悬停渐变 (110ms)
  for h, bm in g_btnMetas.pairs:
    let target = if bm.hover: 1.0 else: 0.0
    if abs(bm.hoverT - target) > 0.01:
      if not g_animEnabled:
        bm.hoverT = target
      else:
        let step = dt / 0.11
        if bm.hoverT < target: bm.hoverT = min(target, bm.hoverT + step)
        else: bm.hoverT = max(target, bm.hoverT - step)
        needMore = true
      if IsWindowVisible(h) != 0:
        discard InvalidateRect(h, nil, FALSE)
        discard UpdateWindow(h)

  if g_hwndSetPanel != 0:
    if chromeDirty:
      discard InvalidateRect(g_hwndSetPanel, nil, FALSE)
      discard UpdateWindow(g_hwndSetPanel)
    elif pageDirty:
      var prc = RECT(left: LONG(S(SetPageX)), top: LONG(S(SetPageY)),
                     right: LONG(S(SetWinW)), bottom: LONG(S(SetWinH)))
      discard InvalidateRect(g_hwndSetPanel, addr prc, FALSE)
      discard UpdateWindow(g_hwndSetPanel)

  if not needMore:
    stopAnimLoop()

# ---------------- 主容器绘制（标题栏 + 左侧导航 + 顶部子标签 + 页面过渡 + Toast） ----------------
proc paintSetPanel(hwnd: HWND) =
  var ps: PAINTSTRUCT
  let hdc = BeginPaint(hwnd, addr ps)
  var rc: RECT
  discard GetClientRect(hwnd, addr rc)
  let w = int(rc.right - rc.left)
  let h = int(rc.bottom - rc.top)
  if w <= 0 or h <= 0:
    EndPaint(hwnd, addr ps)
    return

  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(w), cint(h))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  ensureBrushes()
  FillRect(hdcBuf, addr rc, g_brBg)

  let navW = S(SetNavW)
  let titleH = S(SetTitleH)
  let subH = S(SetSubBarH)

  # 左侧导航栏底色
  var navBgRc = RECT(left: 0, top: 0, right: LONG(navW), bottom: LONG(h))
  FillRect(hdcBuf, addr navBgRc, g_brNav)

  var g: GpGraphics = nil
  if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
    discard GdipSetSmoothingMode(g, 4)

    # 左侧导航与右侧内容区的分割线 & 顶部栏底部分割线
    let divCol = if g_setDark: argb(42, 48, 60) else: argb(218, 224, 234)
    var penDiv: GpPen = nil
    if GdipCreatePen1(divCol, 1.0'f32, 2, addr penDiv) == 0 and penDiv != nil:
      discard GdipDrawLine(g, penDiv, cfloat(navW) - 0.5'f32, 0.0'f32, cfloat(navW) - 0.5'f32, cfloat(h))
      discard GdipDrawLine(g, penDiv, cfloat(navW), cfloat(titleH + subH) - 0.5'f32,
                           cfloat(w), cfloat(titleH + subH) - 0.5'f32)
      discard GdipDeletePen(penDiv)

    # 标题栏左侧蓝点
    var brDot: GpBrush = nil
    if GdipCreateSolidFill(argb(59, 130, 246), addr brDot) == 0 and brDot != nil:
      let ds = cfloat(S(8))
      let dy = cfloat(titleH) * 0.5'f32 - ds * 0.5'f32
      discard GdipFillEllipse(g, brDot, cfloat(S(16)), dy, ds, ds)
      discard GdipDeleteBrush(brDot)

    # 左侧导航悬停胶囊（平滑渐变）
    for i in 0 .. DefaultNavCats.high:
      if g_navHoverT[i] > 0.01 and i != g_curCat:
        let a = uint8(round(g_navHoverT[i] * (if g_setDark: 255.0 else: 220.0)))
        let hCol = if g_setDark: argb(38, 44, 56, a) else: argb(222, 230, 242, a)
        fillRoundRect(g, g_navRcs[i], float(S(8)), hCol)

    # 左侧导航当前选中胶囊（随 g_navIndY 丝滑滑动）
    let selH = int(g_navRcs[0].bottom - g_navRcs[0].top)
    let selRc = RECT(left: g_navRcs[0].left,
                     top: LONG(round(g_navIndY)),
                     right: g_navRcs[0].right,
                     bottom: LONG(round(g_navIndY)) + LONG(selH))
    let selBg = if g_setDark: argb(42, 54, 78) else: argb(219, 234, 254)
    fillRoundRect(g, selRc, float(S(8)), selBg)
    # 左侧蓝色圆角指示竖条
    let indPad = S(9)
    let indRc = RECT(left: selRc.left + LONG(S(2)),
                     top: selRc.top + LONG(indPad),
                     right: selRc.left + LONG(S(6)),
                     bottom: selRc.bottom - LONG(indPad))
    fillRoundRect(g, indRc, float(S(2)), argb(59, 130, 246))

    # 顶部子标签（悬停胶囊 + 底部滑动蓝色下划线）
    if g_subRcs.len > 1:
      for i in 0 ..< g_subRcs.len:
        if g_subHoverT[i] > 0.01 and i != g_curSub:
          let a = uint8(round(g_subHoverT[i] * 255.0))
          let hCol = if g_setDark: argb(38, 44, 56, a) else: argb(226, 232, 240, a)
          fillRoundRect(g, g_subRcs[i], float(S(6)), hCol)
      let barH = S(3)
      let barY = titleH + subH - barH - 1
      let lineRc = RECT(left: LONG(round(g_subIndX)) + LONG(S(8)),
                        top: LONG(barY),
                        right: LONG(round(g_subIndX + g_subIndW)) - LONG(S(8)),
                        bottom: LONG(barY + barH))
      fillRoundRect(g, lineRc, float(S(2)), argb(59, 130, 246))

    # 最小化 / 关闭按钮
    if g_topHover == 100 or g_topDown == 100:
      fillRoundRect(g, g_minRc, float(S(5)), if g_setDark: argb(54, 60, 72) else: argb(226, 232, 240))
    var penMin: GpPen = nil
    let minCol = if g_setDark: argb(190, 200, 215) else: argb(71, 85, 105)
    if GdipCreatePen1(minCol, cfloat(max(1.4, 1.5 * g_setScale)), 2, addr penMin) == 0 and penMin != nil:
      let cx = cfloat(g_minRc.left + g_minRc.right) * 0.5'f32
      let cy = cfloat(g_minRc.top + g_minRc.bottom) * 0.5'f32
      let u = cfloat(S(5))
      discard GdipDrawLine(g, penMin, cx - u, cy, cx + u, cy)
      discard GdipDeletePen(penMin)

    let closeHot = (g_topHover == 101 or g_topDown == 101)
    if closeHot:
      fillRoundRect(g, g_closeRc, float(S(5)), argb(200, 62, 62))
    var penClose: GpPen = nil
    let closeCol = if closeHot: argb(255, 255, 255)
                   elif g_setDark: argb(190, 200, 215)
                   else: argb(71, 85, 105)
    if GdipCreatePen1(closeCol, cfloat(max(1.4, 1.5 * g_setScale)), 2, addr penClose) == 0 and penClose != nil:
      let cx = cfloat(g_closeRc.left + g_closeRc.right) * 0.5'f32
      let cy = cfloat(g_closeRc.top + g_closeRc.bottom) * 0.5'f32
      let u = cfloat(S(4))
      discard GdipDrawLine(g, penClose, cx - u, cy - u, cx + u, cy + u)
      discard GdipDrawLine(g, penClose, cx + u, cy - u, cx - u, cy + u)
      discard GdipDeletePen(penClose)

    # 窗口外轮廓线
    drawRoundRect(g, RECT(left: 0, top: 0, right: LONG(w), bottom: LONG(h)),
                  float(S(8)), if g_setDark: argb(46, 52, 64) else: argb(203, 213, 225), 1.0'f32)

    discard GdipDeleteGraphics(g)

  # 使用缓存字体绘制（零 CreateFontW 开销）
  ensureChromeFonts()
  SetBkMode(hdcBuf, TRANSPARENT)
  let oldF = SelectObject(hdcBuf, g_hfTitle)
  SetTextColor(hdcBuf, if g_setDark: RGB(230, 236, 246) else: RGB(30, 41, 59))
  var trTitle = RECT(left: LONG(S(30)), top: 0, right: LONG(navW - S(6)), bottom: LONG(titleH))
  DrawTextW(hdcBuf, newWideCString("快译NG 设置"), -1, addr trTitle,
            DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
  SelectObject(hdcBuf, oldF)

  # 左侧导航图标（Segoe MDL2 Assets，Win10/11 系统自带）+ 分类名
  for i, cat in DefaultNavCats:
    let isSel = (i == g_curCat)
    let isHot = (g_topHover == i)
    let col = if isSel: (if g_setDark: RGB(147, 197, 253) else: RGB(37, 99, 235))
              elif isHot: (if g_setDark: RGB(236, 242, 252) else: RGB(15, 23, 42))
              else: (if g_setDark: RGB(156, 168, 188) else: RGB(71, 85, 105))
    SetTextColor(hdcBuf, col)
    let oldI = SelectObject(hdcBuf, g_hfIcon)
    var glyph: array[2, WCHAR] = [WCHAR(cat.icon), WCHAR(0)]
    var irc = RECT(left: g_navRcs[i].left + LONG(S(14)), top: g_navRcs[i].top,
                   right: g_navRcs[i].left + LONG(S(36)), bottom: g_navRcs[i].bottom)
    DrawTextW(hdcBuf, cast[LPCWSTR](addr glyph[0]), 1, addr irc,
              DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    SelectObject(hdcBuf, oldI)

    let oldN = SelectObject(hdcBuf, g_hfNav)
    var nrc = RECT(left: g_navRcs[i].left + LONG(S(42)), top: g_navRcs[i].top,
                   right: g_navRcs[i].right - LONG(S(8)), bottom: g_navRcs[i].bottom)
    DrawTextW(hdcBuf, newWideCString(cat.name), -1, addr nrc,
              DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    SelectObject(hdcBuf, oldN)

  # 顶部子标签文字 或 单页大类标题 + 副标题
  let curCatDef = DefaultNavCats[g_curCat]
  if curCatDef.subNames.len > 1:
    let oldS = SelectObject(hdcBuf, g_hfSub)
    for i, sname in curCatDef.subNames:
      let col = if i == g_curSub: (if g_setDark: RGB(147, 197, 253) else: RGB(37, 99, 235))
                elif g_topHover == 200 + i: (if g_setDark: RGB(230, 238, 248) else: RGB(15, 23, 42))
                else: (if g_setDark: RGB(148, 163, 184) else: RGB(100, 116, 139))
      SetTextColor(hdcBuf, col)
      var sr = g_subRcs[i]
      DrawTextW(hdcBuf, newWideCString(sname), -1, addr sr,
                DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    SelectObject(hdcBuf, oldS)
  else:
    let oldH = SelectObject(hdcBuf, g_hfHead)
    SetTextColor(hdcBuf, if g_setDark: RGB(236, 242, 252) else: RGB(15, 23, 42))
    var hrc = RECT(left: LONG(S(SetPageX + SetPadL)), top: LONG(titleH),
                   right: LONG(S(SetPageX + SetPadL + 120)), bottom: LONG(titleH + subH))
    DrawTextW(hdcBuf, newWideCString(curCatDef.name), -1, addr hrc,
              DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    SelectObject(hdcBuf, oldH)

    if curCatDef.subtitle.len > 0:
      let oldS = SelectObject(hdcBuf, g_hfSubDesc)
      SetTextColor(hdcBuf, if g_setDark: RGB(148, 163, 184) else: RGB(100, 116, 139))
      var src = RECT(left: LONG(S(SetPageX + SetPadL + 64)), top: LONG(titleH + S(2)),
                     right: LONG(w - S(20)), bottom: LONG(titleH + subH))
      DrawTextW(hdcBuf, newWideCString(curCatDef.subtitle), -1, addr src,
                DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
      SelectObject(hdcBuf, oldS)

  # 左侧导航底部「✓ 已保存」淡入淡出 Toast
  if g_toastActive and g_toastText.len > 0:
    let elapsed = int(nowMs() - g_toastStart)
    var alpha = 1.0
    if g_animEnabled:
      if elapsed < 160: alpha = easeOutCubic(float(elapsed) / 160.0)
      elif elapsed > 1260: alpha = 1.0 - easeOutCubic(float(elapsed - 1260) / 260.0)
      alpha = clamp(alpha, 0.0, 1.0)
    if alpha > 0.02:
      let tPad = S(12)
      let tH = S(28)
      let tRc = RECT(left: LONG(tPad), top: LONG(h - tPad - tH),
                     right: LONG(navW - tPad), bottom: LONG(h - tPad))
      var g2: GpGraphics = nil
      if GdipCreateFromHDC(hdcBuf, addr g2) == 0 and g2 != nil:
        discard GdipSetSmoothingMode(g2, 4)
        let aBg = uint8(round(alpha * (if g_setDark: 48.0 else: 40.0)))
        let aBd = uint8(round(alpha * 150.0))
        let bgCol = if g_toastIsError: argb(239, 68, 68, aBg) else: argb(34, 197, 94, aBg)
        let bdCol = if g_toastIsError: argb(239, 68, 68, aBd) else: argb(34, 197, 94, aBd)
        fillRoundRect(g2, tRc, float(S(6)), bgCol)
        drawRoundRect(g2, tRc, float(S(6)), bdCol, 1.0'f32)
        discard GdipDeleteGraphics(g2)
      let oldT = SelectObject(hdcBuf, g_hfToast)
      let txtCol = if g_toastIsError: (if g_setDark: RGB(248, 113, 113) else: RGB(220, 38, 38))
                   else: (if g_setDark: RGB(74, 222, 128) else: RGB(22, 163, 74))
      SetTextColor(hdcBuf, txtCol)
      var tr = tRc
      let prefix = if g_toastIsError: "⚠ " else: "✓ "
      DrawTextW(hdcBuf, newWideCString(prefix & g_toastText), -1, addr tr,
                DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
      SelectObject(hdcBuf, oldT)

  # 页面切换交叉淡入 + 位移滑动过渡层 (170ms, 12px)
  if g_transActive and g_transOldBm != 0 and g_transNewBm != 0:
    let t = clamp(float(nowMs() - g_transStart) / 170.0, 0.0, 1.0)
    let e = easeOutCubic(t)
    let px = cint(S(SetPageX))
    let py = cint(S(SetPageY))
    let pw = g_transW
    let ph = g_transH
    let slidePx = float(S(12))

    var oldDx, oldDy, newDx, newDy: cint
    case g_transDir
    of tdDown:
      oldDy = cint(round(-slidePx * 0.5 * e))
      newDy = cint(round(slidePx * (1.0 - e)))
    of tdUp:
      oldDy = cint(round(slidePx * 0.5 * e))
      newDy = cint(round(-slidePx * (1.0 - e)))
    of tdRight:
      oldDx = cint(round(-slidePx * 0.5 * e))
      newDx = cint(round(slidePx * (1.0 - e)))
    of tdLeft:
      oldDx = cint(round(slidePx * 0.5 * e))
      newDx = cint(round(-slidePx * (1.0 - e)))
    of tdNone: discard

    discard SaveDC(hdcBuf)
    discard IntersectClipRect(hdcBuf, px, py, px + pw, py + ph)

    let hdcSrc = CreateCompatibleDC(hdc)
    var bf: BlendFunc
    bf.blendOp = 0 # AC_SRC_OVER
    bf.blendFlags = 0
    bf.alphaFormat = 0

    let oldAlpha = uint8(clamp(int(round(255.0 * (1.0 - e))), 0, 255))
    if oldAlpha > 2:
      let prev = SelectObject(hdcSrc, g_transOldBm)
      bf.sourceConstantAlpha = oldAlpha
      discard GdiAlphaBlend(hdcBuf, px + oldDx, py + oldDy, pw, ph, hdcSrc, 0, 0, pw, ph, bf)
      SelectObject(hdcSrc, prev)

    let newAlpha = uint8(clamp(int(round(255.0 * e)), 0, 255))
    if newAlpha > 2:
      let prev = SelectObject(hdcSrc, g_transNewBm)
      bf.sourceConstantAlpha = newAlpha
      discard GdiAlphaBlend(hdcBuf, px + newDx, py + newDy, pw, ph, hdcSrc, 0, 0, pw, ph, bf)
      SelectObject(hdcSrc, prev)

    DeleteDC(hdcSrc)
    discard RestoreDC(hdcBuf, -1)

  BitBlt(hdc, 0, 0, cint(w), cint(h), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
  EndPaint(hwnd, addr ps)

proc topHitTest(x, y: int): int =
  if ptInRect(g_minRc, x, y): return 100
  if ptInRect(g_closeRc, x, y): return 101
  for i in 0 .. DefaultNavCats.high:
    if ptInRect(g_navRcs[i], x, y): return i
  for i in 0 ..< g_subRcs.len:
    if ptInRect(g_subRcs[i], x, y): return 200 + i
  result = -1

proc setFrameSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                          uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  case uMsg
  of WmNcCalcSize:
    if wp != 0: return 0
  of WmNcPaint:
    return 0
  of WmNcActivate:
    return 1
  of WmNcHitTest:
    let x = loSigned(int64(lp))
    let y = hiSigned(int64(lp))
    var pt = POINT(x: LONG(x), y: LONG(y))
    ScreenToClient(g_hwndSetPanel, addr pt)
    if int(pt.y) >= 0 and int(pt.y) < S(SetTitleH) and topHitTest(int(pt.x), int(pt.y)) == -1:
      return HtCaption
    return HtClient
  of WM_ERASEBKGND:
    return 1
  else: discard
  result = SetDefSubclassProc(hWnd, uMsg, wp, lp)

proc setPanelSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                          uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  case uMsg
  of WM_TIMER:
    if wp == WPARAM(AnimTimerId):
      stepAnimations()
      return 0
  of WmNcHitTest:
    var pt = POINT(x: LONG(loSigned(int64(lp))), y: LONG(hiSigned(int64(lp))))
    ScreenToClient(hWnd, addr pt)
    if int(pt.y) >= 0 and int(pt.y) < S(SetTitleH) and topHitTest(int(pt.x), int(pt.y)) == -1:
      return HtTransparent
    return HtClient
  of WM_ERASEBKGND:
    return 1
  of WM_PAINT:
    paintSetPanel(hWnd)
    return 0
  of WM_MOUSEMOVE:
    if not g_topTracking:
      var tme: SetTrackMouseEvent
      tme.cbSize = DWORD(sizeof(SetTrackMouseEvent))
      tme.dwFlags = TmeLeave
      tme.hwndTrack = hWnd
      discard SetTrackMouse(addr tme)
      g_topTracking = true
    let nh = topHitTest(loSigned(int64(lp)), hiSigned(int64(lp)))
    if nh != g_topHover:
      g_topHover = nh
      if g_animEnabled:
        startAnimLoop()
      else:
        for i in 0 .. g_navHoverT.high:
          g_navHoverT[i] = if nh == i and i != g_curCat: 1.0 else: 0.0
        for i in 0 .. g_subHoverT.high:
          g_subHoverT[i] = if nh == 200 + i and i != g_curSub: 1.0 else: 0.0
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WmMouseLeave:
    g_topTracking = false
    if g_topHover != -1 or g_topDown != -1:
      g_topHover = -1
      g_topDown = -1
      if g_animEnabled:
        startAnimLoop()
      else:
        for i in 0 .. g_navHoverT.high: g_navHoverT[i] = 0.0
        for i in 0 .. g_subHoverT.high: g_subHoverT[i] = 0.0
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WM_LBUTTONDOWN:
    let hit = topHitTest(loSigned(int64(lp)), hiSigned(int64(lp)))
    g_topDown = hit
    if hit != -1:
      SetCapture(hWnd)
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WM_LBUTTONUP:
    let was = g_topDown
    g_topDown = -1
    if GetCapture() == hWnd: ReleaseCapture()
    let hit = topHitTest(loSigned(int64(lp)), hiSigned(int64(lp)))
    discard InvalidateRect(hWnd, nil, FALSE)
    if was != -1 and hit == was:
      if hit == 100 and g_onSetMin != nil:
        g_onSetMin()
      elif hit == 101 and g_onSetClose != nil:
        g_onSetClose()
      elif hit >= 0 and hit <= DefaultNavCats.high:
        let sub = clamp(g_lastSubPerCat[hit], 0, DefaultNavCats[hit].pageIndices.high)
        modernSelectSettingsPage(DefaultNavCats[hit].pageIndices[sub], animate = true)
      elif hit >= 200 and hit < 200 + DefaultNavCats[g_curCat].pageIndices.len:
        let sub = hit - 200
        modernSelectSettingsPage(DefaultNavCats[g_curCat].pageIndices[sub], animate = true)
    return 0
  else: discard
  result = SetDefSubclassProc(hWnd, uMsg, wp, lp)

# ---- 子页面背景、卡片绘制 & 输入框圆角边框 & CTLCOLOR ----
proc paintPagePanel(hwnd: HWND, targetHdc: HDC = 0) =
  var ps: PAINTSTRUCT
  let hdc = if targetHdc != 0: targetHdc else: BeginPaint(hwnd, addr ps)
  var rc: RECT
  discard GetClientRect(hwnd, addr rc)
  let w = int(rc.right - rc.left)
  let h = int(rc.bottom - rc.top)
  if w <= 0 or h <= 0:
    if targetHdc == 0: EndPaint(hwnd, addr ps)
    return

  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(w), cint(h))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  ensureBrushes()
  FillRect(hdcBuf, addr rc, g_brBg)

  let meta = g_pageMetas.getOrDefault(hwnd, nil)
  if meta != nil:
    var g: GpGraphics = nil
    if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
      discard GdipSetSmoothingMode(g, 4)

      # 右侧与底部外框线保持连续
      var penSide: GpPen = nil
      let sideCol = if g_setDark: argb(46, 52, 64) else: argb(203, 213, 225)
      if GdipCreatePen1(sideCol, 1.0'f32, 2, addr penSide) == 0 and penSide != nil:
        discard GdipDrawLine(g, penSide, cfloat(w) - 0.5'f32, 0.0'f32, cfloat(w) - 0.5'f32, cfloat(h))
        discard GdipDrawLine(g, penSide, 0.0'f32, cfloat(h) - 0.5'f32, cfloat(w), cfloat(h) - 0.5'f32)
        discard GdipDeletePen(penSide)

      # 卡片背景 + 边框
      let cardBg = if g_setDark: argb(34, 38, 46) else: argb(255, 255, 255)
      let cardBorder = if g_setDark: argb(48, 54, 66) else: argb(226, 232, 240)
      for c in meta.cards:
        let crc = RECT(left: LONG(S(c.x)), top: LONG(S(c.y)),
                       right: LONG(S(c.x + c.w)), bottom: LONG(S(c.y + c.h)))
        fillRoundRect(g, crc, float(S(8)), cardBg)
        drawRoundRect(g, crc, float(S(8)), cardBorder, 1.0'f32)

      # 为本页所有可见 Edit / Hotkey 框绘制圆角边框（聚焦蓝 / 冲突红）
      let focusH = GetFocus()
      let editNormalBorder = if g_setDark: argb(58, 66, 82) else: argb(203, 213, 225)
      for e in meta.edits:
        if IsWindowVisible(e.h) != 0:
          var er: RECT
          if GetWindowRect(e.h, addr er) != 0:
            var pt1 = POINT(x: er.left, y: er.top)
            var pt2 = POINT(x: er.right, y: er.bottom)
            ScreenToClient(hwnd, addr pt1)
            ScreenToClient(hwnd, addr pt2)
            let pad = 1
            let boxRc = RECT(left: pt1.x - pad, top: pt1.y - pad,
                             right: pt2.x + pad, bottom: pt2.y + pad)
            let isErr = g_editErr.getOrDefault(e.h, false)
            let focused = (focusH == e.h)
            let bcol = if isErr: argb(239, 68, 68)
                       elif focused: argb(59, 130, 246)
                       else: editNormalBorder
            drawRoundRect(g, boxRc, float(S(5)), bcol, if isErr or focused: 1.5'f32 else: 1.0'f32)

      discard GdipDeleteGraphics(g)

  BitBlt(hdc, 0, 0, cint(w), cint(h), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
  if targetHdc == 0:
    EndPaint(hwnd, addr ps)

proc pageSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                      uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  ensureBrushes()
  case uMsg
  of WM_ERASEBKGND:
    return 1
  of WM_PAINT:
    paintPagePanel(hWnd)
    return 0
  of WM_PRINTCLIENT:
    paintPagePanel(hWnd, cast[HDC](wp))
    return 0
  of WM_CTLCOLORSTATIC:
    let hdc = cast[HDC](wp)
    let hCtl = cast[HWND](lp)
    let sm = g_staticMetas.getOrDefault(hCtl, StaticMeta(inCard: false, role: trNormal))
    if g_setDark:
      case sm.role
      of trTitle: SetTextColor(hdc, RGB(147, 197, 253))
      of trHint: SetTextColor(hdc, RGB(148, 163, 184))
      of trNormal: SetTextColor(hdc, RGB(226, 232, 240))
      of trDanger: SetTextColor(hdc, RGB(248, 113, 113))
      of trSuccess: SetTextColor(hdc, RGB(74, 222, 128))
    else:
      case sm.role
      of trTitle: SetTextColor(hdc, RGB(37, 99, 235))
      of trHint: SetTextColor(hdc, RGB(100, 116, 139))
      of trNormal: SetTextColor(hdc, RGB(30, 41, 59))
      of trDanger: SetTextColor(hdc, RGB(220, 38, 38))
      of trSuccess: SetTextColor(hdc, RGB(22, 163, 74))
    if sm.inCard:
      SetBkColor(hdc, if g_setDark: RGB(34, 38, 46) else: RGB(255, 255, 255))
      return cast[LRESULT](g_brCard)
    else:
      SetBkColor(hdc, if g_setDark: RGB(24, 26, 32) else: RGB(244, 246, 250))
      return cast[LRESULT](g_brBg)
  of WM_CTLCOLOREDIT:
    let hdc = cast[HDC](wp)
    let hCtl = cast[HWND](lp)
    let inCard = g_editMeta.getOrDefault(hCtl, false)
    SetTextColor(hdc, if g_setDark: RGB(240, 244, 250) else: RGB(15, 23, 42))
    if inCard:
      SetBkColor(hdc, if g_setDark: RGB(24, 27, 34) else: RGB(248, 250, 252))
      return cast[LRESULT](g_brEditCardBg)
    else:
      SetBkColor(hdc, if g_setDark: RGB(27, 30, 38) else: RGB(255, 255, 255))
      return cast[LRESULT](g_brEditBg)
  of WM_CTLCOLORLISTBOX:
    let hdc = cast[HDC](wp)
    SetTextColor(hdc, if g_setDark: RGB(240, 244, 250) else: RGB(15, 23, 42))
    SetBkColor(hdc, if g_setDark: RGB(34, 38, 46) else: RGB(255, 255, 255))
    return cast[LRESULT](g_brCard)
  else: discard
  result = SetDefSubclassProc(hWnd, uMsg, wp, lp)

# ---- 自绘按钮（带 120ms 悬停平滑渐变） ----
proc getWindowTextWide(h: HWND): seq[WCHAR] =
  let n = GetWindowTextLengthW(h)
  if n <= 0: return @[]
  result = newSeq[WCHAR](n + 1)
  let got = GetWindowTextW(h, addr result[0], cint(n + 1))
  if got <= 0: result.setLen(0)

proc paintModernButton(hWnd: HWND, bm: BtnMeta, targetHdc: HDC = 0) =
  var ps: PAINTSTRUCT
  let hdc = if targetHdc != 0: targetHdc else: BeginPaint(hWnd, addr ps)
  var rc: RECT
  discard GetClientRect(hWnd, addr rc)
  let w = int(rc.right - rc.left)
  let h = int(rc.bottom - rc.top)
  if w <= 0 or h <= 0:
    if targetHdc == 0: EndPaint(hWnd, addr ps)
    return

  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(w), cint(h))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  ensureBrushes()
  FillRect(hdcBuf, addr rc, if bm.inCard: g_brCard else: g_brBg)

  let enabled = IsWindowEnabled(hWnd) != 0
  let ht = bm.hoverT
  var fillCol = if g_setDark: argb(54, 60, 72) else: argb(255, 255, 255)
  var borderCol = if g_setDark: argb(68, 76, 92) else: argb(203, 213, 225)
  var textCol = if g_setDark: RGB(240, 244, 250) else: RGB(30, 41, 59)

  if not enabled:
    fillCol = if g_setDark: argb(38, 42, 52) else: argb(241, 245, 249)
    borderCol = if g_setDark: argb(48, 54, 66) else: argb(226, 232, 240)
    textCol = if g_setDark: RGB(120, 128, 142) else: RGB(148, 163, 184)
  else:
    case bm.kind
    of mbPrimary:
      fillCol = if bm.down: argb(42, 108, 220)
                else: lerpArgb(argb(59, 130, 246), argb(82, 148, 252), ht)
      borderCol = fillCol
      textCol = RGB(255, 255, 255)
    of mbDanger:
      if bm.down:
        fillCol = argb(170, 50, 50)
        borderCol = fillCol
        textCol = RGB(255, 255, 255)
      else:
        let baseFill = if g_setDark: argb(45, 50, 62) else: argb(255, 255, 255)
        let baseBd = if g_setDark: argb(64, 72, 88) else: argb(203, 213, 225)
        fillCol = lerpArgb(baseFill, argb(200, 62, 62), ht)
        borderCol = lerpArgb(baseBd, argb(200, 62, 62), ht)
        if ht > 0.5: textCol = RGB(255, 255, 255)
        else: textCol = if g_setDark: RGB(210, 218, 232) else: RGB(71, 85, 105)
    of mbNormal:
      if bm.down:
        fillCol = if g_setDark: argb(45, 50, 60) else: argb(226, 232, 240)
        borderCol = if g_setDark: argb(72, 82, 98) else: argb(148, 163, 184)
      else:
        let baseFill = if g_setDark: argb(54, 60, 72) else: argb(255, 255, 255)
        let hotFill = if g_setDark: argb(68, 76, 92) else: argb(241, 245, 249)
        let baseBd = if g_setDark: argb(68, 76, 92) else: argb(203, 213, 225)
        let hotBd = if g_setDark: argb(92, 104, 126) else: argb(148, 163, 184)
        fillCol = lerpArgb(baseFill, hotFill, ht)
        borderCol = lerpArgb(baseBd, hotBd, ht)

  var g: GpGraphics = nil
  if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
    discard GdipSetSmoothingMode(g, 4)
    let r = float(S(6))
    fillRoundRect(g, rc, r, fillCol)
    if bm.kind != mbPrimary:
      drawRoundRect(g, rc, r, borderCol, 1.0'f32)
    discard GdipDeleteGraphics(g)

  let txt = getWindowTextWide(hWnd)
  if txt.len > 0:
    let hf = cast[HFONT](SendMessageW(hWnd, WM_GETFONT, 0, 0))
    let oldF = if hf != 0: SelectObject(hdcBuf, hf) else: HGDIOBJ(0)
    SetBkMode(hdcBuf, TRANSPARENT)
    SetTextColor(hdcBuf, textCol)
    var tr = rc
    DrawTextW(hdcBuf, cast[LPCWSTR](unsafeAddr txt[0]), -1, addr tr,
              DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    if oldF != 0: SelectObject(hdcBuf, oldF)

  BitBlt(hdc, 0, 0, cint(w), cint(h), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
  if targetHdc == 0:
    EndPaint(hWnd, addr ps)

proc btnSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                     uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  let bm = g_btnMetas.getOrDefault(hWnd, nil)
  if bm == nil: return SetDefSubclassProc(hWnd, uMsg, wp, lp)
  case uMsg
  of WM_ERASEBKGND:
    return 1
  of WM_PAINT:
    paintModernButton(hWnd, bm)
    return 0
  of WM_PRINTCLIENT:
    paintModernButton(hWnd, bm, cast[HDC](wp))
    return 0
  of WM_MOUSEMOVE:
    if not bm.tracking:
      var tme: SetTrackMouseEvent
      tme.cbSize = DWORD(sizeof(SetTrackMouseEvent))
      tme.dwFlags = TmeLeave
      tme.hwndTrack = hWnd
      discard SetTrackMouse(addr tme)
      bm.tracking = true
    if not bm.hover:
      bm.hover = true
      if g_animEnabled:
        startAnimLoop()
      else:
        bm.hoverT = 1.0
        discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WmMouseLeave:
    bm.tracking = false
    bm.hover = false
    bm.down = false
    if g_animEnabled:
      startAnimLoop()
    else:
      bm.hoverT = 0.0
    discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WM_LBUTTONDOWN:
    if IsWindowEnabled(hWnd) != 0:
      bm.down = true
      SetCapture(hWnd)
      SetFocus(hWnd)
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WM_LBUTTONUP:
    let wasDown = bm.down
    bm.down = false
    if GetCapture() == hWnd: ReleaseCapture()
    var rc: RECT
    discard GetClientRect(hWnd, addr rc)
    let x = loSigned(int64(lp))
    let y = hiSigned(int64(lp))
    discard InvalidateRect(hWnd, nil, FALSE)
    if wasDown and IsWindowEnabled(hWnd) != 0 and ptInRect(rc, x, y):
      let par = GetParent(hWnd)
      let id = GetDlgCtrlID(hWnd)
      if par != 0:
        discard SendMessageW(par, WM_COMMAND, WPARAM((BN_CLICKED shl 16) or (id and 0xFFFF)), cast[LPARAM](hWnd))
    return 0
  of WM_ENABLE, WM_SETTEXT, WM_SETFONT:
    result = SetDefSubclassProc(hWnd, uMsg, wp, lp)
    discard InvalidateRect(hWnd, nil, FALSE)
    return result
  else: discard
  result = SetDefSubclassProc(hWnd, uMsg, wp, lp)

proc modernStyleButton*(btn: wButton, kind: ModBtnKind = mbNormal, inCard = false) =
  if btn.isNil: return
  let h = cast[HWND](btn.getHandle())
  if h == 0: return
  if not g_btnMetas.hasKey(h):
    g_btnMetas[h] = BtnMeta(kind: kind, inCard: inCard)
    discard SetSetWindowSubclass(h, btnSubclassProc, 0x4D21, 0)
  else:
    g_btnMetas[h].kind = kind
    g_btnMetas[h].inCard = inCard
    discard InvalidateRect(h, nil, FALSE)

# ---- 自绘 iOS 风格滑动开关（Toggle Switch，150ms 平滑滑动） ----
proc paintModernCheck(hWnd: HWND, cm: ChkMeta, targetHdc: HDC = 0) =
  var ps: PAINTSTRUCT
  let hdc = if targetHdc != 0: targetHdc else: BeginPaint(hWnd, addr ps)
  var rc: RECT
  discard GetClientRect(hWnd, addr rc)
  let w = int(rc.right - rc.left)
  let h = int(rc.bottom - rc.top)
  if w <= 0 or h <= 0:
    if targetHdc == 0: EndPaint(hWnd, addr ps)
    return

  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(w), cint(h))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  ensureBrushes()
  FillRect(hdcBuf, addr rc, if cm.inCard: g_brCard else: g_brBg)

  let pos = clamp(cm.animPos, 0.0, 1.0)
  let swW = S(36)
  let swH = S(20)
  let swY = (h - swH) div 2
  let trackRc = RECT(left: 0, top: LONG(swY), right: LONG(swW), bottom: LONG(swY + swH))

  var g: GpGraphics = nil
  if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
    discard GdipSetSmoothingMode(g, 4)
    let offTrack = if g_setDark: (if cm.hover: argb(72, 82, 98) else: argb(56, 64, 78))
                   else: (if cm.hover: argb(176, 188, 206) else: argb(203, 213, 225))
    let onTrack = if cm.hover: argb(79, 145, 250) else: argb(59, 130, 246)
    let trackCol = lerpArgb(offTrack, onTrack, pos)
    fillRoundRect(g, trackRc, float(swH) * 0.5, trackCol)

    # 白色滑块圆点
    let pad = max(2.0, float(S(3)))
    let knobD = float(swH) - pad * 2.0
    let travel = float(swW) - pad * 2.0 - knobD
    let kx = pad + travel * pos
    let ky = float(swY) + pad
    var brKnob: GpBrush = nil
    if GdipCreateSolidFill(argb(255, 255, 255), addr brKnob) == 0 and brKnob != nil:
      discard GdipFillEllipse(g, brKnob, cfloat(kx), cfloat(ky), cfloat(knobD), cfloat(knobD))
      discard GdipDeleteBrush(brKnob)
    discard GdipDeleteGraphics(g)

  let txt = getWindowTextWide(hWnd)
  if txt.len > 0:
    let hf = cast[HFONT](SendMessageW(hWnd, WM_GETFONT, 0, 0))
    let oldF = if hf != 0: SelectObject(hdcBuf, hf) else: HGDIOBJ(0)
    SetBkMode(hdcBuf, TRANSPARENT)
    SetTextColor(hdcBuf, if cm.hover: (if g_setDark: RGB(248, 250, 252) else: RGB(15, 23, 42))
                         else: (if g_setDark: RGB(226, 232, 240) else: RGB(30, 41, 59)))
    var tr = RECT(left: LONG(swW + S(10)), top: 0, right: LONG(w), bottom: LONG(h))
    DrawTextW(hdcBuf, cast[LPCWSTR](unsafeAddr txt[0]), -1, addr tr,
              DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    if oldF != 0: SelectObject(hdcBuf, oldF)

  BitBlt(hdc, 0, 0, cint(w), cint(h), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
  if targetHdc == 0:
    EndPaint(hWnd, addr ps)

proc chkSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                     uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  let cm = g_chkMetas.getOrDefault(hWnd, nil)
  if cm == nil: return SetDefSubclassProc(hWnd, uMsg, wp, lp)
  case uMsg
  of WM_ERASEBKGND:
    return 1
  of WM_PAINT:
    paintModernCheck(hWnd, cm)
    return 0
  of WM_PRINTCLIENT:
    paintModernCheck(hWnd, cm, cast[HDC](wp))
    return 0
  of WM_MOUSEMOVE:
    if not cm.tracking:
      var tme: SetTrackMouseEvent
      tme.cbSize = DWORD(sizeof(SetTrackMouseEvent))
      tme.dwFlags = TmeLeave
      tme.hwndTrack = hWnd
      discard SetTrackMouse(addr tme)
      cm.tracking = true
    if not cm.hover:
      cm.hover = true
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WmMouseLeave:
    cm.tracking = false
    if cm.hover:
      cm.hover = false
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WM_LBUTTONDOWN:
    SetFocus(hWnd)
    return 0
  of WM_LBUTTONUP:
    var rc: RECT
    discard GetClientRect(hWnd, addr rc)
    if ptInRect(rc, loSigned(int64(lp)), hiSigned(int64(lp))):
      let cur = SendMessageW(hWnd, BM_GETCHECK, 0, 0)
      let nxt = if cur == BST_CHECKED: BST_UNCHECKED else: BST_CHECKED
      discard SendMessageW(hWnd, BM_SETCHECK, WPARAM(nxt), 0)
      cm.targetPos = if nxt == BST_CHECKED: 1.0 else: 0.0
      if g_animEnabled:
        startAnimLoop()
      else:
        cm.animPos = cm.targetPos
        discard InvalidateRect(hWnd, nil, FALSE)
      let par = GetParent(hWnd)
      let id = GetDlgCtrlID(hWnd)
      if par != 0:
        discard SendMessageW(par, WM_COMMAND, WPARAM((BN_CLICKED shl 16) or (id and 0xFFFF)), cast[LPARAM](hWnd))
    return 0
  of BM_SETCHECK:
    result = SetDefSubclassProc(hWnd, uMsg, wp, lp)
    let target = if wp == WPARAM(BST_CHECKED): 1.0 else: 0.0
    cm.targetPos = target
    if not g_animEnabled or g_hwndSetFrame == 0 or IsWindowVisible(g_hwndSetFrame) == 0:
      cm.animPos = target
    else:
      startAnimLoop()
    discard InvalidateRect(hWnd, nil, FALSE)
    return result
  of WM_SETFONT:
    result = SetDefSubclassProc(hWnd, uMsg, wp, lp)
    discard InvalidateRect(hWnd, nil, FALSE)
    return result
  else: discard
  result = SetDefSubclassProc(hWnd, uMsg, wp, lp)

proc modernStyleCheck*(chk: wCheckBox, inCard = true) =
  if chk.isNil: return
  let h = cast[HWND](chk.getHandle())
  if h == 0: return
  if not g_chkMetas.hasKey(h):
    let checked = SendMessageW(h, BM_GETCHECK, 0, 0) == BST_CHECKED
    let p = if checked: 1.0 else: 0.0
    g_chkMetas[h] = ChkMeta(inCard: inCard, animPos: p, targetPos: p)
    discard SetSetWindowSubclass(h, chkSubclassProc, 0x4D22, 0)

# ---- 输入框去 3D 边框 + 内边距 + 聚焦刷新 + 失焦即保存回调 ----
proc editSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                      uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  if uMsg == WM_SETFOCUS or uMsg == WM_KILLFOCUS:
    let par = GetParent(hWnd)
    if par != 0: discard InvalidateRect(par, nil, FALSE)
    if uMsg == WM_KILLFOCUS and g_onEditBlur != nil:
      g_onEditBlur(hWnd)
  result = SetDefSubclassProc(hWnd, uMsg, wp, lp)

proc modernStyleEdit*(edt: wTextCtrl, inCard = false) =
  if edt.isNil: return
  let h = cast[HWND](edt.getHandle())
  if h == 0: return
  let ex = GetWindowLongPtrW(h, GWL_EXSTYLE)
  if (ex and WS_EX_CLIENTEDGE) != 0:
    discard SetWindowLongPtrW(h, GWL_EXSTYLE, ex and (not WS_EX_CLIENTEDGE))
    discard SetWindowPos(h, 0, 0, 0, 0, 0,
                         SWP_NOMOVE or SWP_NOSIZE or SWP_NOZORDER or SWP_FRAMECHANGED)
  let st = GetWindowLongPtrW(h, GWL_STYLE)
  if (st and WS_BORDER) != 0:
    discard SetWindowLongPtrW(h, GWL_STYLE, st and (not WS_BORDER))
    discard SetWindowPos(h, 0, 0, 0, 0, 0,
                         SWP_NOMOVE or SWP_NOSIZE or SWP_NOZORDER or SWP_FRAMECHANGED)
  discard SendMessageW(h, EmSetMargins, WPARAM(EcLeftMargin or EcRightMargin), LPARAM((7 shl 16) or 7))

  if not g_editMeta.hasKey(h):
    g_editMeta[h] = inCard
    discard SetSetWindowSubclass(h, editSubclassProc, 0x4D23, 0)
    let par = GetParent(h)
    if g_pageMetas.hasKey(par):
      g_pageMetas[par].edits.add((h, inCard))

proc modernMarkStatic*(h: HWND, inCard = false, role: SetTextRole = trNormal) =
  if h != 0:
    g_staticMetas[h] = StaticMeta(inCard: inCard, role: role)

proc applyAppThemeMode(dark: bool) =
  let hUx = LoadLibraryW(newWideCString("uxtheme.dll"))
  if hUx == 0: return
  let pSetMode = GetProcAddress(hUx, cast[LPCSTR](135))
  if pSetMode != nil:
    type SetPrefModeFn = proc (mode: cint): cint {.stdcall.}
    discard cast[SetPrefModeFn](pSetMode)(if dark: 2 else: 0)
  let pFlush = GetProcAddress(hUx, cast[LPCSTR](136))
  if pFlush != nil:
    type FlushFn = proc () {.stdcall.}
    cast[FlushFn](pFlush)()

proc modernStyleDarkSystem*(h: HWND, theme = "DarkMode_Explorer") =
  applyAppThemeMode(g_setDark)
  if h == 0: return
  let lightTheme = if theme.contains("CFD"): "CFD" else: "Explorer"
  var found = false
  for item in g_sysThemeControls:
    if item.h == h:
      found = true
      break
  if not found:
    g_sysThemeControls.add((h, theme, lightTheme))
  let targetTheme = if g_setDark: theme else: lightTheme
  discard SetWindowTheme(h, newWideCString(targetTheme), nil)

proc modernSetSettingsDark*(dark: bool) =
  g_setDark = dark
  applyAppThemeMode(dark)
  freeBrushes()
  if g_hwndSetFrame != 0:
    var d = if dark: 1'i32 else: 0'i32
    discard DwmSetWindowAttribute(g_hwndSetFrame, DwmwaUseImmersiveDarkMode, addr d, DWORD(sizeof(d)))
  for item in g_sysThemeControls:
    if item.h != 0 and IsWindow(item.h) != 0:
      let t = if dark: item.darkTheme else: item.lightTheme
      discard SetWindowTheme(item.h, newWideCString(t), nil)
      discard InvalidateRect(item.h, nil, TRUE)
  if g_hwndSetPanel != 0:
    discard RedrawWindow(g_hwndSetPanel, nil, 0, RDW_INVALIDATE or RDW_ERASE or RDW_ALLCHILDREN)
  for p in g_pages:
    if p != nil:
      let ph = cast[HWND](p.getHandle())
      if ph != 0:
        discard RedrawWindow(ph, nil, 0, RDW_INVALIDATE or RDW_ERASE or RDW_ALLCHILDREN)

proc modernStyleHotkeyHwnd*(h: HWND, inCard = true) =
  if h == 0: return
  let ex = GetWindowLongPtrW(h, GWL_EXSTYLE)
  if (ex and WS_EX_CLIENTEDGE) != 0:
    discard SetWindowLongPtrW(h, GWL_EXSTYLE, ex and (not WS_EX_CLIENTEDGE))
    discard SetWindowPos(h, 0, 0, 0, 0, 0,
                         SWP_NOMOVE or SWP_NOSIZE or SWP_NOZORDER or SWP_FRAMECHANGED)
  let st = GetWindowLongPtrW(h, GWL_STYLE)
  if (st and WS_BORDER) != 0:
    discard SetWindowLongPtrW(h, GWL_STYLE, st and (not WS_BORDER))
    discard SetWindowPos(h, 0, 0, 0, 0, 0,
                         SWP_NOMOVE or SWP_NOSIZE or SWP_NOZORDER or SWP_FRAMECHANGED)
  if not g_editMeta.hasKey(h):
    g_editMeta[h] = inCard
    discard SetSetWindowSubclass(h, editSubclassProc, 0x4D24, 0)
    let par = GetParent(h)
    if g_pageMetas.hasKey(par):
      g_pageMetas[par].edits.add((h, inCard))

proc modernRegisterPage*(page: wPanel, cards: seq[SetCardRect] = @[]) =
  if page.isNil: return
  let h = cast[HWND](page.getHandle())
  if h == 0: return
  let pStyle = GetWindowLongPtrW(h, GWL_STYLE)
  discard SetWindowLongPtrW(h, GWL_STYLE, pStyle or WS_CLIPCHILDREN)
  g_pageMetas[h] = PageMeta(hwnd: h, cards: cards, edits: @[])
  discard SetSetWindowSubclass(h, pageSubclassProc, 0x4D20, 0)

proc initModernSettings*(setFrame: wFrame, setPanel: wPanel, pages: seq[wPanel],
                         scale: float,
                         onTabChange: proc (idx: int),
                         onMin: proc (),
                         onClose: proc (),
                         onEditBlur: proc (h: HWND) = nil) =
  applyAppThemeMode(g_setDark)
  g_hwndSetFrame = cast[HWND](setFrame.getHandle())
  g_hwndSetPanel = cast[HWND](setPanel.getHandle())
  g_pages = pages
  g_onPageChange = onTabChange
  g_onSetMin = onMin
  g_onSetClose = onClose
  g_onEditBlur = onEditBlur

  var style = GetWindowLongPtrW(g_hwndSetFrame, GWL_STYLE)
  style = (style and (not (WS_CAPTION or WS_BORDER or WS_THICKFRAME or WS_MAXIMIZEBOX))) or
          WS_SYSMENU or WS_MINIMIZEBOX or WS_CLIPCHILDREN
  discard SetWindowLongPtrW(g_hwndSetFrame, GWL_STYLE, style)

  let pStyle = GetWindowLongPtrW(g_hwndSetPanel, GWL_STYLE)
  discard SetWindowLongPtrW(g_hwndSetPanel, GWL_STYLE, pStyle or WS_CLIPCHILDREN)

  var dark = if g_setDark: 1'i32 else: 0'i32
  discard DwmSetWindowAttribute(g_hwndSetFrame, DwmwaUseImmersiveDarkMode, addr dark, DWORD(sizeof(dark)))
  var corner = DwmwcpRound
  discard DwmSetWindowAttribute(g_hwndSetFrame, DwmwaWindowCornerPreference, addr corner, DWORD(sizeof(corner)))

  discard SetWindowPos(g_hwndSetFrame, 0, 0, 0, 0, 0,
                       SWP_NOMOVE or SWP_NOSIZE or SWP_NOZORDER or SWP_FRAMECHANGED)

  discard SetSetWindowSubclass(g_hwndSetFrame, setFrameSubclassProc, 0x4D11, 0)
  discard SetSetWindowSubclass(g_hwndSetPanel, setPanelSubclassProc, 0x4D12, 0)

  modernLayoutSettings(scale)
  modernSelectSettingsPage(0, animate = false)
