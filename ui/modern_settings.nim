## 快译NG 现代化设置窗口模块（GDI+ 无边框深色风格，与 modern_main.nim / snip_ui.nim 统一）
## 配色：#181A20 主背景 / #22262E 卡片 / #3B82F6 强调蓝

import std/[strutils, math, tables]
import winim/lean
import wNim/[wApp, wFrame, wPanel, wButton, wCheckBox, wTextCtrl, wComboBox, wTypes]

const
  SetWinW* = 600
  SetWinH* = 636
  SetTitleH* = 38
  SetTabBarY* = 44
  SetTabBarH* = 36
  SetPageY* = 88
  SetPageH* = 548
  SetPadL* = 24
  SetContW* = 552

  GdiplusDll = "gdiplus.dll"
  DwmapiDll = "dwmapi.dll"
  ComctlDll = "comctl32.dll"
  UxthemeDll = "uxtheme.dll"

  DwmwaUseImmersiveDarkMode = DWORD(20)
  DwmwaWindowCornerPreference = DWORD(33)
  DwmwcpRound = 2'i32

  WmNcCalcSize = UINT(0x0083)
  WmNcHitTest = UINT(0x0084)
  WmNcPaint = UINT(0x0085)
  WmNcActivate = UINT(0x0086)
  WmMouseLeave = UINT(0x02A3)
  EmSetMargins = UINT(0x00D3)
  EcLeftMargin = 0x0001
  EcRightMargin = 0x0002

  HtClient = 1
  HtCaption = 2
  HtTransparent = -1
  TmeLeave = DWORD(0x00000002)

  TabNames = ["通用设置", "快捷按键", "翻译接口", "缩略词", "忽略词", "常用词"]

type
  GpStatus = cint
  GpGraphics = pointer
  GpBrush = pointer
  GpPen = pointer
  GpPath = pointer
  ARGB = uint32

  ModBtnKind* = enum
    mbNormal    ## 常规灰蓝按钮 (#363C48)
    mbPrimary   ## 主操作蓝按钮 (#3B82F6)
    mbDanger    ## 删除按钮（hover 变红）

  SetTextRole* = enum
    trNormal    ## 普通文字 (#E2E8F0)
    trHint      ## 次要说明 (#94A3B8)
    trTitle     ## 分组标题 (#93C5FD)

  SetCardRect* = tuple[x, y, w, h: int]

  SetTrackMouseEvent = object
    cbSize: DWORD
    dwFlags: DWORD
    hwndTrack: HWND
    dwHoverTime: DWORD

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

  ChkMeta = ref object
    inCard: bool
    hover: bool
    tracking: bool

  StaticMeta = object
    inCard: bool
    role: SetTextRole

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

var
  g_setScale = 1.0
  g_setDark = true
  g_hwndSetFrame: HWND
  g_hwndSetPanel: HWND
  g_pages: seq[wPanel] = @[]
  g_curTab = 0
  g_onTabChange: proc (idx: int)
  g_onSetClose: proc ()
  g_onSetMin: proc ()

  # 顶部自绘按钮与页签状态
  g_minRc, g_closeRc: RECT
  g_tabBarRc: RECT
  g_tabRcs: array[6, RECT]
  g_topHover = -1        # -1=none, 100=min, 101=close, 0..5=tab
  g_topDown = -1
  g_topTracking = false

  # 画刷缓存
  g_brBg: HBRUSH
  g_brCard: HBRUSH
  g_brEditBg: HBRUSH
  g_brEditCardBg: HBRUSH

  # 元数据表（按 HWND 索引）
  g_pageMetas = initTable[HWND, PageMeta]()
  g_btnMetas = initTable[HWND, BtnMeta]()
  g_chkMetas = initTable[HWND, ChkMeta]()
  g_staticMetas = initTable[HWND, StaticMeta]()
  g_editMeta = initTable[HWND, bool]()   # hwnd -> inCard
  g_sysThemeControls: seq[tuple[h: HWND, darkTheme, lightTheme: string]] = @[]

proc argb(r, g, b: uint8, a: uint8 = 255): ARGB {.inline.} =
  (ARGB(a) shl 24) or (ARGB(r) shl 16) or (ARGB(g) shl 8) or ARGB(b)

proc loSigned(v: int64): int {.inline.} =
  int(cast[int16](uint16(uint64(v) and 0xFFFF'u64)))

proc hiSigned(v: int64): int {.inline.} =
  int(cast[int16](uint16((uint64(v) shr 16) and 0xFFFF'u64)))

proc S(x: int): int {.inline.} =
  max(1, int(round(float(x) * g_setScale)))

proc freeBrushes() =
  if g_brBg != 0: DeleteObject(g_brBg); g_brBg = 0
  if g_brCard != 0: DeleteObject(g_brCard); g_brCard = 0
  if g_brEditBg != 0: DeleteObject(g_brEditBg); g_brEditBg = 0
  if g_brEditCardBg != 0: DeleteObject(g_brEditCardBg); g_brEditCardBg = 0

proc ensureBrushes() =
  if g_setDark:
    if g_brBg == 0: g_brBg = CreateSolidBrush(RGB(24, 26, 32))
    if g_brCard == 0: g_brCard = CreateSolidBrush(RGB(34, 38, 46))
    if g_brEditBg == 0: g_brEditBg = CreateSolidBrush(RGB(27, 30, 38))
    if g_brEditCardBg == 0: g_brEditCardBg = CreateSolidBrush(RGB(24, 27, 34))
  else:
    if g_brBg == 0: g_brBg = CreateSolidBrush(RGB(243, 245, 249))
    if g_brCard == 0: g_brCard = CreateSolidBrush(RGB(255, 255, 255))
    if g_brEditBg == 0: g_brEditBg = CreateSolidBrush(RGB(255, 255, 255))
    if g_brEditCardBg == 0: g_brEditCardBg = CreateSolidBrush(RGB(248, 250, 252))

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

proc layoutTopBar(scale: float) =
  g_setScale = scale
  let w = S(SetWinW)
  let titleH = S(SetTitleH)
  let tbW = S(32)
  let tbH = S(26)
  let tbY = (titleH - tbH) div 2
  let closeX = w - S(10) - tbW
  let minX = closeX - S(4) - tbW
  g_closeRc = RECT(left: LONG(closeX), top: LONG(tbY), right: LONG(closeX + tbW), bottom: LONG(tbY + tbH))
  g_minRc = RECT(left: LONG(minX), top: LONG(tbY), right: LONG(minX + tbW), bottom: LONG(tbY + tbH))

  let barX = S(16)
  let barW = w - barX * 2
  let barY = S(SetTabBarY)
  let barH = S(SetTabBarH)
  g_tabBarRc = RECT(left: LONG(barX), top: LONG(barY), right: LONG(barX + barW), bottom: LONG(barY + barH))

  let pad = S(4)
  let gap = S(4)
  let innerW = barW - pad * 2 - gap * 5
  let itemW = innerW div 6
  var x = barX + pad
  for i in 0 .. 5:
    let rw = if i == 5: (barX + barW - pad - x) else: itemW
    g_tabRcs[i] = RECT(left: LONG(x), top: LONG(barY + pad),
                       right: LONG(x + rw), bottom: LONG(barY + barH - pad))
    x += rw + gap

proc modernLayoutSettings*(scale: float) =
  layoutTopBar(scale)
  let pw = S(SetWinW)
  let py = S(SetPageY)
  let ph = S(SetPageH)
  for p in g_pages:
    if p != nil:
      p.setSize(0, py, pw, ph)
      discard InvalidateRect(cast[HWND](p.getHandle()), nil, FALSE)
  if g_hwndSetPanel != 0:
    discard InvalidateRect(g_hwndSetPanel, nil, FALSE)

proc modernSelectSettingsTab*(idx: int) =
  if idx < 0 or idx >= g_pages.len: return
  g_curTab = idx
  for i, p in g_pages:
    if p != nil:
      if i == idx: p.show()
      else: p.hide()
  if g_onTabChange != nil:
    g_onTabChange(idx)
  if g_hwndSetPanel != 0:
    discard InvalidateRect(g_hwndSetPanel, nil, FALSE)

proc modernGetSettingsTab*(): int = g_curTab

# ---- 顶部标题栏 + Tab 栏绘制 ----
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

  var g: GpGraphics = nil
  if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
    discard GdipSetSmoothingMode(g, 4)

    # 窗口微弱外轮廓线
    drawRoundRect(g, RECT(left: 0, top: 0, right: LONG(w), bottom: LONG(h)),
                  float(S(8)), if g_setDark: argb(46, 52, 64) else: argb(203, 213, 225), 1.0'f32)

    # 标题栏左侧蓝点
    var brDot: GpBrush = nil
    if GdipCreateSolidFill(argb(59, 130, 246), addr brDot) == 0 and brDot != nil:
      let ds = cfloat(S(7))
      let dy = cfloat(S(SetTitleH)) * 0.5'f32 - ds * 0.5'f32
      discard GdipFillEllipse(g, brDot, cfloat(S(14)), dy, ds, ds)
      discard GdipDeleteBrush(brDot)

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

    # 分段式 Tab 导航容器
    fillRoundRect(g, g_tabBarRc, float(S(8)), if g_setDark: argb(34, 38, 46) else: argb(255, 255, 255))
    drawRoundRect(g, g_tabBarRc, float(S(8)), if g_setDark: argb(48, 54, 66) else: argb(226, 232, 240), 1.0'f32)

    for i in 0 .. 5:
      if i == g_curTab:
        fillRoundRect(g, g_tabRcs[i], float(S(6)), argb(59, 130, 246))
      elif g_topHover == i:
        fillRoundRect(g, g_tabRcs[i], float(S(6)), if g_setDark: argb(48, 55, 68) else: argb(241, 245, 249))

    discard GdipDeleteGraphics(g)

  # 标题文字
  let titlePx = max(12, int(round(14.0 * g_setScale)))
  let hfTitle = CreateFontW(cint(-titlePx), 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                            0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldF = SelectObject(hdcBuf, hfTitle)
  SetBkMode(hdcBuf, TRANSPARENT)
  SetTextColor(hdcBuf, if g_setDark: RGB(230, 236, 246) else: RGB(30, 41, 59))
  var trTitle = RECT(left: LONG(S(26)), top: 0, right: LONG(S(240)), bottom: LONG(S(SetTitleH)))
  DrawTextW(hdcBuf, newWideCString("快译NG · 设置"), -1, addr trTitle,
            DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
  SelectObject(hdcBuf, oldF)
  DeleteObject(hfTitle)

  # Tab 文字
  let tabPx = max(12, int(round(14.0 * g_setScale)))
  let hfTab = CreateFontW(cint(-tabPx), 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                          0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldT = SelectObject(hdcBuf, hfTab)
  for i in 0 .. 5:
    SetTextColor(hdcBuf, if i == g_curTab: RGB(255, 255, 255)
                         elif g_topHover == i: (if g_setDark: RGB(224, 232, 245) else: RGB(15, 23, 42))
                         else: (if g_setDark: RGB(148, 163, 184) else: RGB(100, 116, 139)))
    var tr = g_tabRcs[i]
    DrawTextW(hdcBuf, newWideCString(TabNames[i]), -1, addr tr,
              DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
  SelectObject(hdcBuf, oldT)
  DeleteObject(hfTab)

  BitBlt(hdc, 0, 0, cint(w), cint(h), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
  EndPaint(hwnd, addr ps)

proc topHitTest(x, y: int): int =
  if ptInRect(g_minRc, x, y): return 100
  if ptInRect(g_closeRc, x, y): return 101
  for i in 0 .. 5:
    if ptInRect(g_tabRcs[i], x, y): return i
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
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WmMouseLeave:
    g_topTracking = false
    if g_topHover != -1 or g_topDown != -1:
      g_topHover = -1
      g_topDown = -1
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
      elif hit >= 0 and hit <= 5:
        modernSelectSettingsTab(hit)
    return 0
  else: discard
  result = SetDefSubclassProc(hWnd, uMsg, wp, lp)

# ---- 子页面背景、卡片绘制 & 输入框圆角边框 & CTLCOLOR ----
proc paintPagePanel(hwnd: HWND) =
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

  let meta = g_pageMetas.getOrDefault(hwnd, nil)
  if meta != nil:
    var g: GpGraphics = nil
    if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
      discard GdipSetSmoothingMode(g, 4)

      # 左右边缘保持与外框一致的微弱边线
      var penSide: GpPen = nil
      let sideCol = if g_setDark: argb(46, 52, 64) else: argb(203, 213, 225)
      if GdipCreatePen1(sideCol, 1.0'f32, 2, addr penSide) == 0 and penSide != nil:
        discard GdipDrawLine(g, penSide, 0.5'f32, 0.0'f32, 0.5'f32, cfloat(h))
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

      # 为本页所有可见 Edit 输入框绘制圆角边框（聚焦高亮蓝）
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
            let focused = (focusH == e.h)
            let bcol = if focused: argb(59, 130, 246) else: editNormalBorder
            drawRoundRect(g, boxRc, float(S(5)), bcol, 1.0'f32)

      discard GdipDeleteGraphics(g)

  BitBlt(hdc, 0, 0, cint(w), cint(h), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
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
  of WM_CTLCOLORSTATIC:
    let hdc = cast[HDC](wp)
    let hCtl = cast[HWND](lp)
    let sm = g_staticMetas.getOrDefault(hCtl, StaticMeta(inCard: false, role: trNormal))
    if g_setDark:
      case sm.role
      of trTitle: SetTextColor(hdc, RGB(147, 197, 253))
      of trHint: SetTextColor(hdc, RGB(148, 163, 184))
      of trNormal: SetTextColor(hdc, RGB(226, 232, 240))
    else:
      case sm.role
      of trTitle: SetTextColor(hdc, RGB(37, 99, 235))
      of trHint: SetTextColor(hdc, RGB(100, 116, 139))
      of trNormal: SetTextColor(hdc, RGB(30, 41, 59))
    if sm.inCard:
      SetBkColor(hdc, if g_setDark: RGB(34, 38, 46) else: RGB(255, 255, 255))
      return cast[LRESULT](g_brCard)
    else:
      SetBkColor(hdc, if g_setDark: RGB(24, 26, 32) else: RGB(243, 245, 249))
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

# ---- 自绘按钮 ----
proc getWindowTextWide(h: HWND): seq[WCHAR] =
  let n = GetWindowTextLengthW(h)
  if n <= 0: return @[]
  result = newSeq[WCHAR](n + 1)
  let got = GetWindowTextW(h, addr result[0], cint(n + 1))
  if got <= 0: result.setLen(0)

proc paintModernButton(hWnd: HWND, bm: BtnMeta) =
  var ps: PAINTSTRUCT
  let hdc = BeginPaint(hWnd, addr ps)
  var rc: RECT
  discard GetClientRect(hWnd, addr rc)
  let w = int(rc.right - rc.left)
  let h = int(rc.bottom - rc.top)
  if w <= 0 or h <= 0:
    EndPaint(hWnd, addr ps)
    return

  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(w), cint(h))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  # 先填按钮背景所在底色（默认与 #181A20/#22262E 接近，圆角外角由按钮所在父底色兜底；用父 DC 或 #181A20）
  ensureBrushes()
  FillRect(hdcBuf, addr rc, if bm.inCard: g_brCard else: g_brBg)

  let enabled = IsWindowEnabled(hWnd) != 0
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
                elif bm.hover: argb(79, 145, 250)
                else: argb(59, 130, 246)
      borderCol = fillCol
      textCol = RGB(255, 255, 255)
    of mbDanger:
      if bm.down:
        fillCol = argb(170, 50, 50)
        borderCol = fillCol
        textCol = RGB(255, 255, 255)
      elif bm.hover:
        fillCol = argb(200, 62, 62)
        borderCol = fillCol
        textCol = RGB(255, 255, 255)
      else:
        fillCol = if g_setDark: argb(45, 50, 62) else: argb(255, 255, 255)
        borderCol = if g_setDark: argb(64, 72, 88) else: argb(203, 213, 225)
        textCol = if g_setDark: RGB(210, 218, 232) else: RGB(71, 85, 105)
    of mbNormal:
      if bm.down:
        fillCol = if g_setDark: argb(45, 50, 60) else: argb(226, 232, 240)
        borderCol = if g_setDark: argb(72, 82, 98) else: argb(148, 163, 184)
      elif bm.hover:
        fillCol = if g_setDark: argb(68, 76, 92) else: argb(241, 245, 249)
        borderCol = if g_setDark: argb(88, 100, 120) else: argb(148, 163, 184)

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
      discard InvalidateRect(hWnd, nil, FALSE)
    return 0
  of WmMouseLeave:
    bm.tracking = false
    bm.hover = false
    bm.down = false
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

# ---- 自绘复选框 ----
proc paintModernCheck(hWnd: HWND, cm: ChkMeta) =
  var ps: PAINTSTRUCT
  let hdc = BeginPaint(hWnd, addr ps)
  var rc: RECT
  discard GetClientRect(hWnd, addr rc)
  let w = int(rc.right - rc.left)
  let h = int(rc.bottom - rc.top)
  if w <= 0 or h <= 0:
    EndPaint(hWnd, addr ps)
    return

  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(w), cint(h))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  ensureBrushes()
  FillRect(hdcBuf, addr rc, if cm.inCard: g_brCard else: g_brBg)

  let checked = SendMessageW(hWnd, BM_GETCHECK, 0, 0) == BST_CHECKED
  let boxS = S(16)
  let boxY = (h - boxS) div 2
  let boxRc = RECT(left: 0, top: LONG(boxY), right: LONG(boxS), bottom: LONG(boxY + boxS))

  var g: GpGraphics = nil
  if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
    discard GdipSetSmoothingMode(g, 4)
    if checked:
      let col = if cm.hover: argb(79, 145, 250) else: argb(59, 130, 246)
      fillRoundRect(g, boxRc, float(S(4)), col)
      var pen: GpPen = nil
      if GdipCreatePen1(argb(255, 255, 255), cfloat(max(1.6, 1.8 * g_setScale)), 2, addr pen) == 0 and pen != nil:
        let x0 = cfloat(boxRc.left) + cfloat(boxS) * 0.22'f32
        let y0 = cfloat(boxRc.top) + cfloat(boxS) * 0.52'f32
        let x1 = cfloat(boxRc.left) + cfloat(boxS) * 0.43'f32
        let y1 = cfloat(boxRc.top) + cfloat(boxS) * 0.74'f32
        let x2 = cfloat(boxRc.left) + cfloat(boxS) * 0.78'f32
        let y2 = cfloat(boxRc.top) + cfloat(boxS) * 0.28'f32
        discard GdipDrawLine(g, pen, x0, y0, x1, y1)
        discard GdipDrawLine(g, pen, x1, y1, x2, y2)
        discard GdipDeletePen(pen)
    else:
      fillRoundRect(g, boxRc, float(S(4)), if g_setDark: argb(24, 27, 34) else: argb(255, 255, 255))
      let bcol = if cm.hover: (if g_setDark: argb(96, 112, 140) else: argb(59, 130, 246))
                 else: (if g_setDark: argb(68, 76, 92) else: argb(148, 163, 184))
      drawRoundRect(g, boxRc, float(S(4)), bcol, 1.0'f32)
    discard GdipDeleteGraphics(g)

  let txt = getWindowTextWide(hWnd)
  if txt.len > 0:
    let hf = cast[HFONT](SendMessageW(hWnd, WM_GETFONT, 0, 0))
    let oldF = if hf != 0: SelectObject(hdcBuf, hf) else: HGDIOBJ(0)
    SetBkMode(hdcBuf, TRANSPARENT)
    SetTextColor(hdcBuf, if cm.hover: (if g_setDark: RGB(248, 250, 252) else: RGB(15, 23, 42))
                         else: (if g_setDark: RGB(226, 232, 240) else: RGB(30, 41, 59)))
    var tr = RECT(left: LONG(boxS + S(8)), top: 0, right: LONG(w), bottom: LONG(h))
    DrawTextW(hdcBuf, cast[LPCWSTR](unsafeAddr txt[0]), -1, addr tr,
              DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    if oldF != 0: SelectObject(hdcBuf, oldF)

  BitBlt(hdc, 0, 0, cint(w), cint(h), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
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
      let par = GetParent(hWnd)
      let id = GetDlgCtrlID(hWnd)
      if par != 0:
        discard SendMessageW(par, WM_COMMAND, WPARAM((BN_CLICKED shl 16) or (id and 0xFFFF)), cast[LPARAM](hWnd))
    return 0
  of BM_SETCHECK, WM_SETFONT:
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
    g_chkMetas[h] = ChkMeta(inCard: inCard)
    discard SetSetWindowSubclass(h, chkSubclassProc, 0x4D22, 0)

# ---- 输入框去 3D 边框 + 内边距 + 聚焦刷新 ----
proc editSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                      uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  if uMsg == WM_SETFOCUS or uMsg == WM_KILLFOCUS:
    let par = GetParent(hWnd)
    if par != 0: discard InvalidateRect(par, nil, FALSE)
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
  ## 启用 Win10 1903+ / Win11 暗色/浅色策略，使 DarkMode_Explorer / DarkMode_CFD 动态切换生效
  let hUx = LoadLibraryW(newWideCString("uxtheme.dll"))
  if hUx == 0: return
  let pSetMode = GetProcAddress(hUx, cast[LPCSTR](135))
  if pSetMode != nil:
    type SetPrefModeFn = proc (mode: cint): cint {.stdcall.}
    discard cast[SetPrefModeFn](pSetMode)(if dark: 2 else: 0) # 2=ForceDark, 0=Default(Light)
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
                         onClose: proc ()) =
  applyAppThemeMode(g_setDark)
  g_hwndSetFrame = cast[HWND](setFrame.getHandle())
  g_hwndSetPanel = cast[HWND](setPanel.getHandle())
  g_pages = pages
  g_onTabChange = onTabChange
  g_onSetMin = onMin
  g_onSetClose = onClose

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
  modernSelectSettingsTab(0)
