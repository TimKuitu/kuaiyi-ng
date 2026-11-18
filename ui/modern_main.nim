## 快译NG 现代化主窗口自绘模块（方案二：GDI+ 抗锯齿圆角 + 无边框深色风格）
## 配色与视觉语言与 snip_ui.nim 保持一致：#181A20 底色 / #22262E 卡片 / #3B82F6 强调蓝

import std/[strutils, math]
import winim/lean
import wNim/[wApp, wFrame, wPanel, wTextCtrl, wTypes]

const
  GdiplusDll = "gdiplus.dll"
  DwmapiDll = "dwmapi.dll"
  ComctlDll = "comctl32.dll"

  DwmwaUseImmersiveDarkMode = DWORD(20)
  DwmwaWindowCornerPreference = DWORD(33)
  DwmwcpRound = 2'i32

  WmNcCalcSize = UINT(0x0083)
  WmNcHitTest = UINT(0x0084)
  WmNcPaint = UINT(0x0085)
  WmNcActivate = UINT(0x0086)
  WmMouseWheel = UINT(0x020A)
  WmMouseLeave = UINT(0x02A3)
  EmSetBkgndColor = UINT(WM_USER + 67)
  EmSetCharFormat = UINT(WM_USER + 68)
  ScfAll = 0x0004
  CfmColor = 0x40000000'i32

  HtError = -2
  HtTransparent = -1
  HtClient = 1
  HtCaption = 2
  HtLeft = 10
  HtRight = 11
  HtTop = 12
  HtTopLeft = 13
  HtTopRight = 14
  HtBottom = 15
  HtBottomLeft = 16
  HtBottomRight = 17

  TmeLeave = DWORD(0x00000002)

  # 控件 ID
  IdNone = 0
  IdPin = 1
  IdMin = 2
  IdClose = 3
  IdZh2En = 4
  IdEn2Zh = 5
  IdSettings = 6
  IdStyle = 7
  IdKind = 8
  IdTrans = 9

# ---- GDI+ Flat API 绑定 ----
type
  GpStatus = cint
  GpGraphics = pointer
  GpBrush = pointer
  GpPen = pointer
  GpPath = pointer
  ARGB = uint32

  GdiplusStartupInput = object
    GdiplusVersion: uint32
    DebugEventCallback: pointer
    SuppressBackgroundThread: int32
    SuppressExternalCodecs: int32

  ModCharFormatW = object
    cbSize: UINT
    dwMask: DWORD
    dwEffects: DWORD
    yHeight: LONG
    yOffset: LONG
    crTextColor: COLORREF
    bCharSet: BYTE
    bPitchAndFamily: BYTE
    szFaceName: array[32, WCHAR]

  ModTrackMouseEvent = object
    cbSize: DWORD
    dwFlags: DWORD
    hwndTrack: HWND
    dwHoverTime: DWORD

  SubclassProc = proc (hWnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM,
                       uIdSubclass: UINT_PTR, dwRefData: DWORD_PTR): LRESULT {.stdcall.}

proc GdiplusStartup(token: ptr pointer, input: ptr GdiplusStartupInput,
                    output: pointer): GpStatus {.stdcall, importc, dynlib: GdiplusDll.}
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
proc ModSetWindowSubclass(hWnd: HWND, pfn: SubclassProc, id: UINT_PTR,
                          dwRef: DWORD_PTR): WINBOOL {.stdcall, importc: "SetWindowSubclass", dynlib: ComctlDll.}
proc ModDefSubclassProc(hWnd: HWND, uMsg: UINT, wParam: WPARAM,
                        lParam: LPARAM): LRESULT {.stdcall, importc: "DefSubclassProc", dynlib: ComctlDll.}
proc TrackMouseEvent(lpEventTrack: ptr ModTrackMouseEvent): WINBOOL {.stdcall, importc, dynlib: "user32.dll".}

# ---- 回调与状态 ----
type
  ModernBtn = object
    id: int
    rect: RECT
    labelText: string

  ModernHooks* = object
    onZh2En*: proc ()
    onEn2Zh*: proc ()
    onSettings*: proc ()
    onTrans*: proc ()
    onStyleSelect*: proc (idx: int)
    onKindSelect*: proc (idx: int)
    onToggleTop*: proc ()
    onMinimize*: proc ()
    onClose*: proc ()
    isTranslating*: proc (): bool
    isTopMost*: proc (): bool
    getStyleIdx*: proc (): int
    getKindIdx*: proc (): int
    styleNames*: seq[string]
    kindNames*: seq[string]

var
  g_gpToken: pointer
  g_gpReady = false

  g_hooks: ModernHooks
  g_hwndFrame: HWND
  g_hwndPanel: HWND
  g_hwndInput: HWND
  g_hwndResult: HWND

  g_titleStr = "快译NG"
  g_hintStr = ""
  g_scale = 1.0
  g_winW = 550
  g_winH = 380

  g_titleRc: RECT
  g_inCardRc: RECT
  g_resCardRc: RECT
  g_btns: array[9, ModernBtn] # IdPin..IdTrans (index 0..8 -> id 1..9)

  g_hoverId = IdNone
  g_downId = IdNone
  g_tracking = false
  g_inputFocused = false
  g_isDark = true

  # 弹出下拉菜单
  g_popClassReg = false
  g_hwndPop: HWND
  g_popKind = 0         # IdStyle or IdKind
  g_popItems: seq[string]
  g_popSelected = 0
  g_popHover = -1
  g_popItemH = 30
  g_popW = 140
  g_popH = 80

proc argb(r, g, b: uint8, a: uint8 = 255): ARGB {.inline.} =
  (ARGB(a) shl 24) or (ARGB(r) shl 16) or (ARGB(g) shl 8) or ARGB(b)

proc loSigned(v: int64): int {.inline.} =
  int(cast[int16](uint16(uint64(v) and 0xFFFF'u64)))

proc hiSigned(v: int64): int {.inline.} =
  int(cast[int16](uint16((uint64(v) shr 16) and 0xFFFF'u64)))

proc S(x: int): int {.inline.} =
  max(1, int(round(float(x) * g_scale)))

proc ensureGdiplus() =
  if g_gpReady: return
  var inp: GdiplusStartupInput
  inp.GdiplusVersion = 1
  if GdiplusStartup(addr g_gpToken, addr inp, nil) == 0:
    g_gpReady = true

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

proc repaintMain*() =
  if g_hwndPanel != 0:
    discard InvalidateRect(g_hwndPanel, nil, FALSE)

proc modernTitleHeight*(scale: float): int =
  max(28, int(round(36.0 * scale)))

proc modernStyleDarkEdit*(h: HWND, isResult: bool) =
  if h == 0: return
  # 剥离 Win32 3D 凹陷边框
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
  # 卡片底色（深色 #22262E / 浅色 #FFFFFF）
  let bgCol = if g_isDark: RGB(34, 38, 46) else: RGB(255, 255, 255)
  discard SendMessageW(h, EmSetBkgndColor, 0, LPARAM(bgCol))
  var cf: ModCharFormatW
  zeroMem(addr cf, sizeof(ModCharFormatW))
  cf.cbSize = UINT(sizeof(ModCharFormatW))
  cf.dwMask = DWORD(CfmColor)
  cf.crTextColor = if g_isDark:
                     (if isResult: RGB(224, 232, 245) else: RGB(240, 244, 250))
                   else:
                     (if isResult: RGB(30, 41, 59) else: RGB(15, 23, 42))
  discard SendMessageW(h, EmSetCharFormat, WPARAM(ScfAll), cast[LPARAM](addr cf))

proc modernIsDark*(): bool = g_isDark

proc modernSetDark*(dark: bool) =
  g_isDark = dark
  if g_hwndFrame != 0:
    var d = if g_isDark: 1'i32 else: 0'i32
    discard DwmSetWindowAttribute(g_hwndFrame, DwmwaUseImmersiveDarkMode, addr d, DWORD(sizeof(d)))
  if g_hwndInput != 0:
    modernStyleDarkEdit(g_hwndInput, false)
  if g_hwndResult != 0:
    modernStyleDarkEdit(g_hwndResult, true)
  if g_hwndPop != 0:
    discard InvalidateRect(g_hwndPop, nil, FALSE)
  repaintMain()

proc modernSetTitle*(title: string) =
  g_titleStr = title
  repaintMain()

proc modernSetHint*(hint: string) =
  g_hintStr = hint
  repaintMain()

proc modernRefreshState*() =
  repaintMain()

proc ptInRect(rc: RECT, x, y: int): bool {.inline.} =
  x >= int(rc.left) and x < int(rc.right) and y >= int(rc.top) and y < int(rc.bottom)

proc hitTestControl(x, y: int): int =
  for i in 0 .. g_btns.high:
    if ptInRect(g_btns[i].rect, x, y):
      return g_btns[i].id
  result = IdNone

proc modernLayoutMain*(w, h: int, scale: float, rowH: int) =
  g_scale = scale
  g_winW = w
  g_winH = h

  let titleH = modernTitleHeight(scale)
  let padX = S(10)
  let gap = S(8)
  let cardPadX = S(10)
  let cardPadY = S(7)

  g_titleRc = RECT(left: 0, top: 0, right: LONG(w), bottom: LONG(titleH))

  # 标题栏右侧 3 个图标按钮：置顶、最小化、关闭
  let tbW = S(32)
  let tbH = S(26)
  let tbY = (titleH - tbH) div 2
  let tbGap = S(4)
  let closeX = w - padX - tbW
  let minX = closeX - tbGap - tbW
  let pinX = minX - tbGap - tbW
  g_btns[0] = ModernBtn(id: IdPin, rect: RECT(left: LONG(pinX), top: LONG(tbY), right: LONG(pinX + tbW), bottom: LONG(tbY + tbH)))
  g_btns[1] = ModernBtn(id: IdMin, rect: RECT(left: LONG(minX), top: LONG(tbY), right: LONG(minX + tbW), bottom: LONG(tbY + tbH)))
  g_btns[2] = ModernBtn(id: IdClose, rect: RECT(left: LONG(closeX), top: LONG(tbY), right: LONG(closeX + tbW), bottom: LONG(tbY + tbH)))

  # 输入区与结果区平分剩余高度
  let fixedH = titleH + gap * 3 + rowH
  var extra = h - (fixedH + S(96) + S(96))
  if extra < 0: extra = 0
  let inH = S(96) + extra div 2
  let resH = S(96) + (extra - extra div 2)

  let cardW = max(100, w - padX * 2)
  let inTop = titleH
  g_inCardRc = RECT(left: LONG(padX), top: LONG(inTop), right: LONG(padX + cardW), bottom: LONG(inTop + inH))
  if g_hwndInput != 0:
    discard MoveWindow(g_hwndInput, cint(padX + cardPadX), cint(inTop + cardPadY),
                       cint(max(20, cardW - cardPadX * 2)), cint(max(20, inH - cardPadY * 2)), TRUE)

  # 中间工具栏 6 个控件
  let rowY = inTop + inH + gap
  let bGap = S(5)
  let wZh = S(74)
  let wEn = S(74)
  let wSet = S(54)
  let wStyle = S(118)
  let wKind = S(116)
  var x = padX
  g_btns[3] = ModernBtn(id: IdZh2En, rect: RECT(left: LONG(x), top: LONG(rowY), right: LONG(x + wZh), bottom: LONG(rowY + rowH)), labelText: "中 > 英")
  x += wZh + bGap
  g_btns[4] = ModernBtn(id: IdEn2Zh, rect: RECT(left: LONG(x), top: LONG(rowY), right: LONG(x + wEn), bottom: LONG(rowY + rowH)), labelText: "英 > 中")
  x += wEn + bGap
  g_btns[5] = ModernBtn(id: IdSettings, rect: RECT(left: LONG(x), top: LONG(rowY), right: LONG(x + wSet), bottom: LONG(rowY + rowH)), labelText: "设置")
  x += wSet + bGap
  g_btns[6] = ModernBtn(id: IdStyle, rect: RECT(left: LONG(x), top: LONG(rowY), right: LONG(x + wStyle), bottom: LONG(rowY + rowH)))
  x += wStyle + bGap
  g_btns[7] = ModernBtn(id: IdKind, rect: RECT(left: LONG(x), top: LONG(rowY), right: LONG(x + wKind), bottom: LONG(rowY + rowH)))
  x += wKind + bGap
  let transRight = w - padX
  g_btns[8] = ModernBtn(id: IdTrans, rect: RECT(left: LONG(x), top: LONG(rowY), right: LONG(max(x + S(52), transRight)), bottom: LONG(rowY + rowH)))

  # 结果卡片
  let resTop = rowY + rowH + gap
  g_resCardRc = RECT(left: LONG(padX), top: LONG(resTop), right: LONG(padX + cardW), bottom: LONG(resTop + resH))
  if g_hwndResult != 0:
    discard MoveWindow(g_hwndResult, cint(padX + cardPadX), cint(resTop + cardPadY),
                       cint(max(20, cardW - cardPadX * 2)), cint(max(20, resH - cardPadY * 2)), TRUE)

  repaintMain()

# ---- 弹出下拉菜单 ----
proc closePopup() =
  if g_hwndPop != 0:
    DestroyWindow(g_hwndPop)
    g_hwndPop = 0
  g_popKind = 0
  g_popHover = -1

proc paintPopup(hwnd: HWND) =
  var ps: PAINTSTRUCT
  let hdc = BeginPaint(hwnd, addr ps)
  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(g_popW), cint(g_popH))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  var fullRc = RECT(left: 0, top: 0, right: LONG(g_popW), bottom: LONG(g_popH))
  let bg = CreateSolidBrush(if g_isDark: RGB(24, 26, 32) else: RGB(243, 245, 249))
  FillRect(hdcBuf, addr fullRc, bg)
  DeleteObject(bg)

  ensureGdiplus()
  var g: GpGraphics = nil
  if g_gpReady and GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
    discard GdipSetSmoothingMode(g, 4)
    let cardRc = RECT(left: 1, top: 1, right: LONG(g_popW - 1), bottom: LONG(g_popH - 1))
    fillRoundRect(g, cardRc, float(S(6)), if g_isDark: argb(34, 38, 46) else: argb(255, 255, 255))
    drawRoundRect(g, cardRc, float(S(6)), if g_isDark: argb(62, 72, 90) else: argb(203, 213, 225), 1.0'f32)

    let pad = S(4)
    for i in 0 ..< g_popItems.len:
      let iy = pad + i * g_popItemH
      let itemRc = RECT(left: LONG(pad), top: LONG(iy), right: LONG(g_popW - pad), bottom: LONG(iy + g_popItemH - S(2)))
      if i == g_popHover:
        fillRoundRect(g, itemRc, float(S(5)), argb(59, 130, 246))
      elif i == g_popSelected:
        fillRoundRect(g, itemRc, float(S(5)), if g_isDark: argb(46, 54, 68) else: argb(239, 246, 255))

      if i == g_popSelected:
        var brDot: GpBrush = nil
        let dotCol = if i == g_popHover: argb(255, 255, 255) else: argb(59, 130, 246)
        if GdipCreateSolidFill(dotCol, addr brDot) == 0 and brDot != nil:
          let ds = cfloat(S(6))
          discard GdipFillEllipse(g, brDot, cfloat(pad + S(8)), cfloat(iy) + cfloat(g_popItemH - S(2)) * 0.5'f32 - ds * 0.5'f32, ds, ds)
          discard GdipDeleteBrush(brDot)
    discard GdipDeleteGraphics(g)

  let fontPx = max(12, int(round(14.0 * g_scale)))
  let hf = CreateFontW(cint(-fontPx), 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                       0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldF = SelectObject(hdcBuf, hf)
  SetBkMode(hdcBuf, TRANSPARENT)
  let pad = S(4)
  for i in 0 ..< g_popItems.len:
    let iy = pad + i * g_popItemH
    var tr = RECT(left: LONG(pad + S(20)), top: LONG(iy), right: LONG(g_popW - pad - S(6)), bottom: LONG(iy + g_popItemH - S(2)))
    SetTextColor(hdcBuf, if i == g_popHover: RGB(255, 255, 255)
                         elif i == g_popSelected: (if g_isDark: RGB(147, 197, 253) else: RGB(37, 99, 235))
                         else: (if g_isDark: RGB(224, 232, 245) else: RGB(30, 41, 59)))
    DrawTextW(hdcBuf, newWideCString(g_popItems[i]), -1, addr tr,
              DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
  SelectObject(hdcBuf, oldF)
  DeleteObject(hf)

  BitBlt(hdc, 0, 0, cint(g_popW), cint(g_popH), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
  EndPaint(hwnd, addr ps)

proc showPopupMenu(kind: int, anchorRc: RECT, items: seq[string], curIdx: int) =
  if g_hwndPop != 0 and g_popKind == kind:
    closePopup()
    return
  closePopup()
  if items.len == 0: return

  g_popKind = kind
  g_popItems = items
  g_popSelected = curIdx
  g_popHover = curIdx
  g_popItemH = S(30)
  let pad = S(4)
  g_popW = max(int(anchorRc.right - anchorRc.left), S(128))
  g_popH = pad * 2 + items.len * g_popItemH

  var pt = POINT(x: anchorRc.left, y: anchorRc.bottom + LONG(S(4)))
  ClientToScreen(g_hwndPanel, addr pt)

  # 确保不超出屏幕底边
  var mi: MONITORINFO
  mi.cbSize = DWORD(sizeof(MONITORINFO))
  let mon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)
  if mon != 0 and GetMonitorInfoW(mon, addr mi) != 0:
    if int(pt.y) + g_popH > int(mi.rcWork.bottom):
      var topPt = POINT(x: anchorRc.left, y: anchorRc.top - LONG(S(4)))
      ClientToScreen(g_hwndPanel, addr topPt)
      pt.y = topPt.y - LONG(g_popH)

  let clsName = newWideCString("KuaiYiNGModernPopup")
  if not g_popClassReg:
    var wc: WNDCLASSEXW
    wc.cbSize = UINT(sizeof(WNDCLASSEXW))
    wc.style = CS_HREDRAW or CS_VREDRAW or CS_DROPSHADOW
    wc.lpfnWndProc = proc (hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM): LRESULT {.stdcall.} =
      case msg
      of WM_ERASEBKGND:
        return 1
      of WM_PAINT:
        paintPopup(hwnd)
        return 0
      of WM_MOUSEMOVE:
        let y = hiSigned(int64(lp))
        let idx = (y - S(4)) div max(1, g_popItemH)
        let nh = if idx >= 0 and idx < g_popItems.len: idx else: -1
        if nh != g_popHover:
          g_popHover = nh
          discard InvalidateRect(hwnd, nil, FALSE)
        return 0
      of WM_LBUTTONUP:
        let y = hiSigned(int64(lp))
        let idx = (y - S(4)) div max(1, g_popItemH)
        let k = g_popKind
        closePopup()
        if idx >= 0 and idx < g_popItems.len:
          if k == IdStyle and g_hooks.onStyleSelect != nil:
            g_hooks.onStyleSelect(idx)
          elif k == IdKind and g_hooks.onKindSelect != nil:
            g_hooks.onKindSelect(idx)
          repaintMain()
        return 0
      of WM_KEYDOWN:
        if wp == WPARAM(VK_ESCAPE):
          closePopup()
          return 0
      of WM_ACTIVATE:
        if LOWORD(wp) == WA_INACTIVE:
          closePopup()
          return 0
      else: discard
      result = DefWindowProcW(hwnd, msg, wp, lp)
    wc.hInstance = GetModuleHandleW(nil)
    wc.hCursor = LoadCursorW(0, IDC_ARROW)
    wc.lpszClassName = clsName
    discard RegisterClassExW(addr wc)
    g_popClassReg = true

  g_hwndPop = CreateWindowExW(
    WS_EX_TOPMOST or WS_EX_TOOLWINDOW,
    clsName, newWideCString(""),
    WS_POPUP,
    cint(pt.x), cint(pt.y), cint(g_popW), cint(g_popH),
    g_hwndFrame, 0, GetModuleHandleW(nil), nil
  )
  if g_hwndPop != 0:
    var corner = DwmwcpRound
    discard DwmSetWindowAttribute(g_hwndPop, DwmwaWindowCornerPreference, addr corner, DWORD(sizeof(corner)))
    ShowWindow(g_hwndPop, SW_SHOW)
    SetFocus(g_hwndPop)

# ---- 绘制主面板 ----
proc drawPinIcon(g: GpGraphics, rc: RECT, active: bool) =
  let cx = cfloat(rc.left + rc.right) * 0.5'f32
  let cy = cfloat(rc.top + rc.bottom) * 0.5'f32
  let col = if active: argb(255, 255, 255)
            elif g_isDark: argb(170, 182, 200)
            else: argb(100, 116, 139)
  var pen: GpPen = nil
  if GdipCreatePen1(col, cfloat(max(1.3, 1.4 * g_scale)), 2, addr pen) == 0 and pen != nil:
    let u = cfloat(S(4))
    # 图钉帽 + 针
    discard GdipDrawLine(g, pen, cx - u * 0.7'f32, cy - u * 0.9'f32, cx + u * 0.7'f32, cy - u * 0.9'f32)
    discard GdipDrawLine(g, pen, cx - u * 0.5'f32, cy - u * 0.9'f32, cx - u * 0.5'f32, cy + u * 0.1'f32)
    discard GdipDrawLine(g, pen, cx + u * 0.5'f32, cy - u * 0.9'f32, cx + u * 0.5'f32, cy + u * 0.1'f32)
    discard GdipDrawLine(g, pen, cx - u * 0.9'f32, cy + u * 0.1'f32, cx + u * 0.9'f32, cy + u * 0.1'f32)
    discard GdipDrawLine(g, pen, cx, cy + u * 0.1'f32, cx, cy + u * 1.1'f32)
    discard GdipDeletePen(pen)

proc drawMinIcon(g: GpGraphics, rc: RECT) =
  let cx = cfloat(rc.left + rc.right) * 0.5'f32
  let cy = cfloat(rc.top + rc.bottom) * 0.5'f32
  let col = if g_isDark: argb(190, 200, 215) else: argb(71, 85, 105)
  var pen: GpPen = nil
  if GdipCreatePen1(col, cfloat(max(1.4, 1.5 * g_scale)), 2, addr pen) == 0 and pen != nil:
    let u = cfloat(S(5))
    discard GdipDrawLine(g, pen, cx - u, cy, cx + u, cy)
    discard GdipDeletePen(pen)

proc drawCloseIcon(g: GpGraphics, rc: RECT, hover: bool) =
  let cx = cfloat(rc.left + rc.right) * 0.5'f32
  let cy = cfloat(rc.top + rc.bottom) * 0.5'f32
  let col = if hover: argb(255, 255, 255)
            elif g_isDark: argb(190, 200, 215)
            else: argb(71, 85, 105)
  var pen: GpPen = nil
  if GdipCreatePen1(col, cfloat(max(1.4, 1.5 * g_scale)), 2, addr pen) == 0 and pen != nil:
    let u = cfloat(S(4))
    discard GdipDrawLine(g, pen, cx - u, cy - u, cx + u, cy + u)
    discard GdipDrawLine(g, pen, cx + u, cy - u, cx - u, cy + u)
    discard GdipDeletePen(pen)

proc drawArrowDown(g: GpGraphics, rc: RECT) =
  let ax = cfloat(rc.right - S(13))
  let ay = cfloat(rc.top + rc.bottom) * 0.5'f32 - cfloat(S(1))
  let col = if g_isDark: argb(175, 190, 210) else: argb(100, 116, 139)
  var pen: GpPen = nil
  if GdipCreatePen1(col, cfloat(max(1.3, 1.4 * g_scale)), 2, addr pen) == 0 and pen != nil:
    let u = cfloat(S(3))
    discard GdipDrawLine(g, pen, ax - u, ay - u * 0.5'f32, ax, ay + u * 0.6'f32)
    discard GdipDrawLine(g, pen, ax, ay + u * 0.6'f32, ax + u, ay - u * 0.5'f32)
    discard GdipDeletePen(pen)

proc paintMainPanel(hwnd: HWND) =
  var ps: PAINTSTRUCT
  let hdc = BeginPaint(hwnd, addr ps)
  if g_winW <= 0 or g_winH <= 0:
    EndPaint(hwnd, addr ps)
    return

  let hdcBuf = CreateCompatibleDC(hdc)
  let hbmBuf = CreateCompatibleBitmap(hdc, cint(g_winW), cint(g_winH))
  let oldBm = SelectObject(hdcBuf, hbmBuf)

  # 主背景（深色 #181A20 / 浅色 #F3F5F9）
  var fullRc = RECT(left: 0, top: 0, right: LONG(g_winW), bottom: LONG(g_winH))
  let bg = CreateSolidBrush(if g_isDark: RGB(24, 26, 32) else: RGB(243, 245, 249))
  FillRect(hdcBuf, addr fullRc, bg)
  DeleteObject(bg)

  ensureGdiplus()
  let trans = if g_hooks.isTranslating != nil: g_hooks.isTranslating() else: false
  let topMost = if g_hooks.isTopMost != nil: g_hooks.isTopMost() else: false

  var g: GpGraphics = nil
  if g_gpReady and GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
    discard GdipSetSmoothingMode(g, 4)

    # 窗口微弱外框线
    drawRoundRect(g, RECT(left: 0, top: 0, right: LONG(g_winW), bottom: LONG(g_winH)),
                  float(S(8)), if g_isDark: argb(46, 52, 64) else: argb(203, 213, 225), 1.0'f32)

    # 标题栏左侧小蓝点标识
    var brDot: GpBrush = nil
    if GdipCreateSolidFill(argb(59, 130, 246), addr brDot) == 0 and brDot != nil:
      let ds = cfloat(S(7))
      let dy = cfloat(g_titleRc.bottom) * 0.5'f32 - ds * 0.5'f32
      discard GdipFillEllipse(g, brDot, cfloat(S(12)), dy, ds, ds)
      discard GdipDeleteBrush(brDot)

    # 标题栏右侧 3 个图标按钮
    let topHoverBg = if g_isDark: argb(54, 60, 72) else: argb(226, 232, 240)
    for i in 0 .. 2:
      let b = g_btns[i]
      let hover = (g_hoverId == b.id)
      let down = (g_downId == b.id)
      if b.id == IdPin:
        if topMost:
          fillRoundRect(g, b.rect, float(S(5)), if hover: argb(79, 145, 250) else: argb(59, 130, 246))
        elif hover or down:
          fillRoundRect(g, b.rect, float(S(5)), topHoverBg)
        drawPinIcon(g, b.rect, topMost)
      elif b.id == IdMin:
        if hover or down:
          fillRoundRect(g, b.rect, float(S(5)), topHoverBg)
        drawMinIcon(g, b.rect)
      elif b.id == IdClose:
        if hover or down:
          fillRoundRect(g, b.rect, float(S(5)), argb(200, 62, 62))
        drawCloseIcon(g, b.rect, hover or down)

    # 输入卡片 & 结果卡片
    let cardBg = if g_isDark: argb(34, 38, 46) else: argb(255, 255, 255)
    let cardBorder = if g_isDark: argb(48, 54, 66) else: argb(226, 232, 240)
    let inBorder = if g_inputFocused: argb(59, 130, 246) else: cardBorder
    fillRoundRect(g, g_inCardRc, float(S(8)), cardBg)
    drawRoundRect(g, g_inCardRc, float(S(8)), inBorder, 1.0'f32)

    fillRoundRect(g, g_resCardRc, float(S(8)), cardBg)
    drawRoundRect(g, g_resCardRc, float(S(8)), cardBorder, 1.0'f32)

    # 工具栏 6 个自绘圆角按钮/下拉
    let rBtn = float(S(6))
    for i in 3 .. 8:
      let b = g_btns[i]
      let hover = (g_hoverId == b.id)
      let down = (g_downId == b.id)
      let disabled = trans and (b.id == IdZh2En or b.id == IdEn2Zh)

      var fillCol = if g_isDark: argb(54, 60, 72) else: argb(255, 255, 255)
      var borderCol = if g_isDark: argb(68, 76, 92) else: argb(203, 213, 225)
      if b.id == IdTrans:
        if trans:
          fillCol = if hover: argb(218, 74, 74) else: argb(200, 62, 62)
          borderCol = fillCol
        else:
          fillCol = if down: argb(42, 108, 220) elif hover: argb(79, 145, 250) else: argb(59, 130, 246)
          borderCol = fillCol
      elif disabled:
        fillCol = if g_isDark: argb(40, 44, 54) else: argb(241, 245, 249)
        borderCol = if g_isDark: argb(48, 54, 66) else: argb(226, 232, 240)
      elif (b.id == IdStyle or b.id == IdKind) and g_hwndPop != 0 and g_popKind == b.id:
        fillCol = if g_isDark: argb(62, 70, 84) else: argb(239, 246, 255)
        borderCol = argb(59, 130, 246)
      elif down:
        fillCol = if g_isDark: argb(45, 50, 60) else: argb(226, 232, 240)
      elif hover:
        fillCol = if g_isDark: argb(68, 76, 92) else: argb(241, 245, 249)
        borderCol = if g_isDark: argb(88, 100, 120) else: argb(148, 163, 184)

      fillRoundRect(g, b.rect, rBtn, fillCol)
      if b.id != IdTrans:
        drawRoundRect(g, b.rect, rBtn, borderCol, 1.0'f32)

      if b.id == IdStyle or b.id == IdKind:
        drawArrowDown(g, b.rect)

    discard GdipDeleteGraphics(g)

  # 标题栏文字 + 反馈提示
  let titlePx = max(12, int(round(14.0 * g_scale)))
  let hfTitle = CreateFontW(cint(-titlePx), 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                            0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldF = SelectObject(hdcBuf, hfTitle)
  SetBkMode(hdcBuf, TRANSPARENT)
  SetTextColor(hdcBuf, if g_isDark: RGB(230, 236, 246) else: RGB(30, 41, 59))
  var trTitle = RECT(left: LONG(S(24)), top: 0, right: LONG(S(110)), bottom: g_titleRc.bottom)
  DrawTextW(hdcBuf, newWideCString(g_titleStr), -1, addr trTitle,
            DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
  SelectObject(hdcBuf, oldF)
  DeleteObject(hfTitle)

  if g_hintStr.len > 0:
    let hintPx = max(11, int(round(13.0 * g_scale)))
    let hfHint = CreateFontW(cint(-hintPx), 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                             0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
    let oldH = SelectObject(hdcBuf, hfHint)
    SetTextColor(hdcBuf, if g_isDark: RGB(147, 197, 253) else: RGB(37, 99, 235))
    var trHint = RECT(left: LONG(S(92)), top: 0, right: g_btns[0].rect.left - LONG(S(8)), bottom: g_titleRc.bottom)
    DrawTextW(hdcBuf, newWideCString(g_hintStr), -1, addr trHint,
              DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_END_ELLIPSIS or DT_NOPREFIX)
    SelectObject(hdcBuf, oldH)
    DeleteObject(hfHint)

  # 按钮文字
  let btnPx = max(12, int(round(14.0 * g_scale)))
  let hfBtn = CreateFontW(cint(-btnPx), 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                          0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldB = SelectObject(hdcBuf, hfBtn)
  let sIdx = if g_hooks.getStyleIdx != nil: g_hooks.getStyleIdx() else: 0
  let kIdx = if g_hooks.getKindIdx != nil: g_hooks.getKindIdx() else: 0
  for i in 3 .. 8:
    let b = g_btns[i]
    var txt = b.labelText
    if b.id == IdTrans:
      txt = if trans: "取消" else: "翻译"
    elif b.id == IdStyle and sIdx >= 0 and sIdx < g_hooks.styleNames.len:
      txt = g_hooks.styleNames[sIdx]
    elif b.id == IdKind and kIdx >= 0 and kIdx < g_hooks.kindNames.len:
      txt = g_hooks.kindNames[kIdx]

    let disabled = trans and (b.id == IdZh2En or b.id == IdEn2Zh)
    SetTextColor(hdcBuf, if disabled: (if g_isDark: RGB(120, 128, 142) else: RGB(148, 163, 184))
                         elif b.id == IdTrans: RGB(255, 255, 255)
                         else: (if g_isDark: RGB(240, 244, 250) else: RGB(30, 41, 59)))

    var tr = b.rect
    if b.id == IdStyle or b.id == IdKind:
      tr.left += LONG(S(10))
      tr.right -= LONG(S(20))
      DrawTextW(hdcBuf, newWideCString(txt), -1, addr tr,
                DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_END_ELLIPSIS or DT_NOPREFIX)
    else:
      DrawTextW(hdcBuf, newWideCString(txt), -1, addr tr,
                DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)

  SelectObject(hdcBuf, oldB)
  DeleteObject(hfBtn)

  BitBlt(hdc, 0, 0, cint(g_winW), cint(g_winH), hdcBuf, 0, 0, SRCCOPY)
  SelectObject(hdcBuf, oldBm)
  DeleteObject(hbmBuf)
  DeleteDC(hdcBuf)
  EndPaint(hwnd, addr ps)

# ---- 窗口子类化（无边框缩放 + 拖拽 + 自绘交互） ----
proc frameHitTest(hwnd: HWND, lp: LPARAM): LRESULT =
  var rc: RECT
  discard GetWindowRect(hwnd, addr rc)
  let x = loSigned(int64(lp))
  let y = hiSigned(int64(lp))
  let b = S(6)
  let left = x < int(rc.left) + b
  let right = x >= int(rc.right) - b
  let top = y < int(rc.top) + b
  let bottom = y >= int(rc.bottom) - b
  if top and left: return HtTopLeft
  if top and right: return HtTopRight
  if bottom and left: return HtBottomLeft
  if bottom and right: return HtBottomRight
  if left: return HtLeft
  if right: return HtRight
  if top: return HtTop
  if bottom: return HtBottom

  var pt = POINT(x: LONG(x), y: LONG(y))
  ScreenToClient(g_hwndPanel, addr pt)
  if int(pt.y) < int(g_titleRc.bottom) and hitTestControl(int(pt.x), int(pt.y)) == IdNone:
    return HtCaption
  result = HtClient

proc handleWheelAtCursor(wp: WPARAM, lp: LPARAM): bool =
  var pt = POINT(x: LONG(loSigned(int64(lp))), y: LONG(hiSigned(int64(lp))))
  ScreenToClient(g_hwndPanel, addr pt)
  let hit = hitTestControl(int(pt.x), int(pt.y))
  if hit != IdStyle and hit != IdKind:
    return false
  let delta = hiSigned(int64(wp))
  if delta == 0: return true
  let step = if delta > 0: -1 else: 1
  if hit == IdStyle and g_hooks.styleNames.len > 0:
    let cur = if g_hooks.getStyleIdx != nil: g_hooks.getStyleIdx() else: 0
    let nxt = clamp(cur + step, 0, g_hooks.styleNames.high)
    if nxt != cur and g_hooks.onStyleSelect != nil:
      g_hooks.onStyleSelect(nxt)
      if g_hwndPop != 0 and g_popKind == IdStyle:
        g_popSelected = nxt
        discard InvalidateRect(g_hwndPop, nil, FALSE)
      repaintMain()
  elif hit == IdKind and g_hooks.kindNames.len > 0:
    let cur = if g_hooks.getKindIdx != nil: g_hooks.getKindIdx() else: 0
    let nxt = clamp(cur + step, 0, g_hooks.kindNames.high)
    if nxt != cur and g_hooks.onKindSelect != nil:
      g_hooks.onKindSelect(nxt)
      if g_hwndPop != 0 and g_popKind == IdKind:
        g_popSelected = nxt
        discard InvalidateRect(g_hwndPop, nil, FALSE)
      repaintMain()
  result = true

proc frameSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                       uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  case uMsg
  of WmNcCalcSize:
    if wp != 0: return 0
  of WmNcPaint:
    # 阻止系统绘制非客户区白框
    return 0
  of WmNcActivate:
    # 失焦/激活时不让 DefWindowProcW 画系统边框，同时避免激活态重绘闪烁
    return 1
  of WmNcHitTest:
    return frameHitTest(hWnd, lp)
  of WM_ERASEBKGND:
    return 1
  else: discard
  result = ModDefSubclassProc(hWnd, uMsg, wp, lp)

proc panelSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                       uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  case uMsg
  of WmNcHitTest:
    # 让边框和标题栏空白区穿透给父窗口 frame 处理拖拽/缩放
    var pt = POINT(x: LONG(loSigned(int64(lp))), y: LONG(hiSigned(int64(lp))))
    ScreenToClient(hWnd, addr pt)
    let b = S(6)
    if int(pt.x) < b or int(pt.x) >= g_winW - b or int(pt.y) < b or int(pt.y) >= g_winH - b:
      return HtTransparent
    if int(pt.y) < int(g_titleRc.bottom) and hitTestControl(int(pt.x), int(pt.y)) == IdNone:
      return HtTransparent
    return HtClient
  of WM_ERASEBKGND:
    return 1
  of WM_PAINT:
    paintMainPanel(hWnd)
    return 0
  of WM_MOUSEMOVE:
    if not g_tracking:
      var tme: ModTrackMouseEvent
      tme.cbSize = DWORD(sizeof(ModTrackMouseEvent))
      tme.dwFlags = TmeLeave
      tme.hwndTrack = hWnd
      discard TrackMouseEvent(addr tme)
      g_tracking = true
    let x = loSigned(int64(lp))
    let y = hiSigned(int64(lp))
    let nh = hitTestControl(x, y)
    if nh != g_hoverId:
      g_hoverId = nh
      repaintMain()
    return 0
  of WmMouseLeave:
    g_tracking = false
    if g_hoverId != IdNone or g_downId != IdNone:
      g_hoverId = IdNone
      g_downId = IdNone
      repaintMain()
    return 0
  of WM_LBUTTONDOWN:
    let x = loSigned(int64(lp))
    let y = hiSigned(int64(lp))
    let hit = hitTestControl(x, y)
    g_downId = hit
    if hit != IdNone:
      SetCapture(hWnd)
      repaintMain()
    return 0
  of WM_LBUTTONUP:
    let wasDown = g_downId
    g_downId = IdNone
    if GetCapture() == hWnd:
      ReleaseCapture()
    let x = loSigned(int64(lp))
    let y = hiSigned(int64(lp))
    let hit = hitTestControl(x, y)
    repaintMain()
    if wasDown != IdNone and hit == wasDown:
      case hit
      of IdPin:
        if g_hooks.onToggleTop != nil: g_hooks.onToggleTop()
      of IdMin:
        if g_hooks.onMinimize != nil: g_hooks.onMinimize()
      of IdClose:
        if g_hooks.onClose != nil: g_hooks.onClose()
      of IdZh2En:
        let trans = if g_hooks.isTranslating != nil: g_hooks.isTranslating() else: false
        if not trans and g_hooks.onZh2En != nil: g_hooks.onZh2En()
      of IdEn2Zh:
        let trans = if g_hooks.isTranslating != nil: g_hooks.isTranslating() else: false
        if not trans and g_hooks.onEn2Zh != nil: g_hooks.onEn2Zh()
      of IdSettings:
        if g_hooks.onSettings != nil: g_hooks.onSettings()
      of IdTrans:
        if g_hooks.onTrans != nil: g_hooks.onTrans()
      of IdStyle:
        let idx = if g_hooks.getStyleIdx != nil: g_hooks.getStyleIdx() else: 0
        showPopupMenu(IdStyle, g_btns[6].rect, g_hooks.styleNames, idx)
      of IdKind:
        let idx = if g_hooks.getKindIdx != nil: g_hooks.getKindIdx() else: 0
        showPopupMenu(IdKind, g_btns[7].rect, g_hooks.kindNames, idx)
      else: discard
    return 0
  of WmMouseWheel:
    if handleWheelAtCursor(wp, lp):
      return 0
  else: discard
  result = ModDefSubclassProc(hWnd, uMsg, wp, lp)

proc inputSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                       uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  if uMsg == WM_SETFOCUS:
    g_inputFocused = true
    repaintMain()
  elif uMsg == WM_KILLFOCUS:
    g_inputFocused = false
    repaintMain()
  elif uMsg == WmMouseWheel:
    if handleWheelAtCursor(wp, lp):
      return 0
  result = ModDefSubclassProc(hWnd, uMsg, wp, lp)

proc resultSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                        uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  if uMsg == WmMouseWheel:
    if handleWheelAtCursor(wp, lp):
      return 0
  result = ModDefSubclassProc(hWnd, uMsg, wp, lp)

proc initModernMain*(frame: wFrame, panel: wPanel, input, resultBox: wTextCtrl,
                     hooks: ModernHooks) =
  g_hooks = hooks
  g_hwndFrame = cast[HWND](frame.getHandle())
  g_hwndPanel = cast[HWND](panel.getHandle())
  g_hwndInput = cast[HWND](input.getHandle())
  g_hwndResult = cast[HWND](resultBox.getHandle())

  # 去掉系统标题栏与边框，保留原生 WS_THICKFRAME 支持拖拽改大小与 Win11 圆角阴影
  var style = GetWindowLongPtrW(g_hwndFrame, GWL_STYLE)
  style = (style and (not (WS_CAPTION or WS_BORDER))) or WS_THICKFRAME or WS_MINIMIZEBOX or WS_SYSMENU or WS_CLIPCHILDREN
  discard SetWindowLongPtrW(g_hwndFrame, GWL_STYLE, style)

  let pStyle = GetWindowLongPtrW(g_hwndPanel, GWL_STYLE)
  discard SetWindowLongPtrW(g_hwndPanel, GWL_STYLE, pStyle or WS_CLIPCHILDREN)

  var dark = if g_isDark: 1'i32 else: 0'i32
  discard DwmSetWindowAttribute(g_hwndFrame, DwmwaUseImmersiveDarkMode, addr dark, DWORD(sizeof(dark)))
  var corner = DwmwcpRound
  discard DwmSetWindowAttribute(g_hwndFrame, DwmwaWindowCornerPreference, addr corner, DWORD(sizeof(corner)))

  discard SetWindowPos(g_hwndFrame, 0, 0, 0, 0, 0,
                       SWP_NOMOVE or SWP_NOSIZE or SWP_NOZORDER or SWP_FRAMECHANGED)

  discard ModSetWindowSubclass(g_hwndFrame, frameSubclassProc, 0x4D01, 0)
  discard ModSetWindowSubclass(g_hwndPanel, panelSubclassProc, 0x4D02, 0)
  discard ModSetWindowSubclass(g_hwndInput, inputSubclassProc, 0x4D03, 0)
  discard ModSetWindowSubclass(g_hwndResult, resultSubclassProc, 0x4D04, 0)

  modernStyleDarkEdit(g_hwndInput, false)
  modernStyleDarkEdit(g_hwndResult, true)
