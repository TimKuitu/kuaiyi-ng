## 快译NG 截屏与贴图模块（Snipaste 一期 + 二期标注完整能力 + 截屏翻译）
## 1. 全屏虚拟桌面暗化遮罩：自动窗口探测、框选/8向调整/移动、放大镜取色、10种标注工具+调色盘+撤销重做
## 2. 屏幕多贴图悬浮窗（F3）：支持图文剪贴板贴图、拖拽、滚轮缩放、Ctrl+滚轮调透明度、右键菜单
## 3. 截屏翻译悬浮窗：后台裸线程调用 fy_snip_translate（WinRT OCR + 译文原图覆盖），支持一键贴图

import std/[os, math, strutils, tables]
import winim/lean
import fastcore

const
  WmAppSnipDone* = WM_APP + 0x0202
  WmAppPinTransDone* = WM_APP + 0x0203
  HkSnipEsc* = 99
  ToolbarH = 38
  MinWinW = 450

  # 遮罩主工具栏与二级属性栏尺寸
  OvBarH = 36
  OvSubBarH = 34

  # 主工具栏按钮 ID
  OvBtnNone = 0
  OvBtnRect = 1
  OvBtnEllipse = 2
  OvBtnLine = 3
  OvBtnArrow = 4
  OvBtnPen = 5
  OvBtnMarker = 6
  OvBtnNumber = 7
  OvBtnText = 8
  OvBtnMosaic = 9
  OvBtnBlur = 10
  OvBtnUndo = 11
  OvBtnRedo = 12
  OvBtnTrans = 13
  OvBtnPin = 14
  OvBtnSave = 15
  OvBtnCancel = 16
  OvBtnCopy = 17
  OvBtnQuickSave = 18

  # 二级属性栏命中 ID
  SubHitNone = 0
  SubHitSize0 = 100
  SubHitSize1 = 101
  SubHitSize2 = 102
  SubHitFill = 105
  SubHitColorBase = 200   # 200..207

  # 翻译结果悬浮窗按钮 ID
  BtnToggle = 1
  BtnCopyText = 2
  BtnCopyImg = 3
  BtnPin = 4
  BtnResnip = 5
  BtnClose = 6

  # 贴图右键菜单命令 ID
  PinMenuCopy = 1001
  PinMenuSave = 1002
  PinMenuTrans = 1003
  PinMenuResetZoom = 1004
  PinMenuOp100 = 1005
  PinMenuOp80 = 1006
  PinMenuOp50 = 1007
  PinMenuBorder = 1008
  PinMenuClose = 1009
  PinMenuCloseAll = 1010
  PinMenuQuickSave = 1011
  PinMenuToggleTrans = 1012
  PinMenuCopyTransText = 1013

  PinWmMouseLeave = UINT(0x02A3)
  PinTmeLeave = DWORD(0x00000002)
  PinMfGrayed = UINT(0x00000003)   # MF_GRAYED | MF_DISABLED

  PinHudTimerId = UINT_PTR(0x5011)
  TextEditCtrlId = HMENU(0x5021)

  DwmwaExtendedFrameBounds = DWORD(9)
  DwmwaCloaked = DWORD(14)
  PixelFormat32bppRgb = cint(0x00022009)

  OfnOverwritePrompt = DWORD(0x00000002)
  OfnPathMustExist = DWORD(0x00000800)
  OfnNoChangeDir = DWORD(0x00000008)

  LineCapRound = cint(2)
  LineJoinRound = cint(2)
  SmoothingAntiAlias = cint(4)
  FillModeAlternate = cint(0)

  # 8 色 Snipaste 风格调色盘 (R, G, B)
  AnnotColors = [
    (239'u8, 68'u8, 68'u8),    # 鲜红
    (249'u8, 115'u8, 22'u8),   # 亮橙
    (234'u8, 179'u8, 8'u8),    # 明黄
    (34'u8, 197'u8, 94'u8),    # 翠绿
    (6'u8, 182'u8, 212'u8),    # 青蓝
    (59'u8, 130'u8, 246'u8),   # 科技蓝
    (168'u8, 85'u8, 247'u8),   # 亮紫
    (255'u8, 255'u8, 255'u8)   # 纯白
  ]
  StrokeSizes = [2.0'f32, 4.0'f32, 7.0'f32]
  HighlighterSizes = [14.0'f32, 22.0'f32, 32.0'f32]
  MosaicBlocks = [8, 13, 20]
  MosaicRadii = [11, 18, 28]
  BlurRadii = [4, 7, 11]
  BlurBrushRadii = [12, 20, 30]
  NumberRadii = [12.0, 15.0, 19.0]
  TextFontSizes = [16, 22, 30]

type
  SnipCfg* = object
    aiUrl*: string
    aiKey*: string
    aiModel*: string
    baiduAppId*: string
    baiduKey*: string
    baiduUrl*: string
    cacheDir*: string
    saveDir*: string
    pinMods*: int
    pinVk*: int

  SnipJob = object
    seq: int
    hwndMain: HWND
    pinHwnd: HWND
    bgra: pointer
    w, h: cint
    aiUrl, aiKey, aiModel: cstring
    baiduAppId, baiduKey, baiduUrl: cstring
    cacheDir: cstring

  PinTransPayload = object
    pinHwnd: HWND
    seq: int
    res: ptr FySnipResult

  PinTrackMouseEvent = object
    cbSize: DWORD
    dwFlags: DWORD
    hwndTrack: HWND
    dwHoverTime: DWORD

  ToolBtn = object
    rect: RECT
    labelText: string
    tipText: string
    id: int

  OverlayState = enum
    osHover       # 自动探测鼠标下窗口
    osDragging    # 鼠标拖拽新建选区
    osAdjust      # 选区已就绪，显示 8 向手柄与工具栏
    osMoving      # 拖拽移动选区
    osResizing    # 拖拽 8 向控制手柄缩放选区
    osDrawing     # 正在选区内绘制标注
    osTextEditing # 正在输入文字标注

  AnnotTool = enum
    atNone
    atRect
    atEllipse
    atLine
    atArrow
    atPen
    atMarker
    atNumber
    atText
    atMosaic
    atBlur

  AnnotItem = object
    tool: AnnotTool
    colorIdx: int
    sizeIdx: int
    filled: bool
    p1, p2: POINT
    pts: seq[POINT]
    numVal: int
    text: string

  PinWin = ref object
    hwnd: HWND
    hbm: HBITMAP
    bits: pointer
    hbmTrans: HBITMAP
    bitsTrans: pointer
    showTrans: bool
    translating: bool
    transSeq: int
    zhText: string
    mouseIn: bool
    btnHover: bool
    origW, origH: cint
    dispW, dispH: cint
    zoomScale: float
    alpha: uint8
    showBorder: bool
    hudMsg: string

  GpPointF = object
    x, y: cfloat

  SnipGuid = object
    data1: uint32
    data2: uint16
    data3: uint16
    data4: array[8, uint8]

  SnipGdiplusStartupInput = object
    gdiplusVersion: uint32
    debugEventCallback: pointer
    suppressBackgroundThread: int32
    suppressExternalCodecs: int32

  SnipOpenFileNameW = object
    lStructSize: DWORD
    hwndOwner: HWND
    hInstance: HINSTANCE
    lpstrFilter: LPCWSTR
    lpstrCustomFilter: LPWSTR
    nMaxCustFilter: DWORD
    nFilterIndex: DWORD
    lpstrFile: LPWSTR
    nMaxFile: DWORD
    lpstrFileTitle: LPWSTR
    nMaxFileTitle: DWORD
    lpstrInitialDir: LPCWSTR
    lpstrTitle: LPCWSTR
    Flags: DWORD
    nFileOffset: WORD
    nFileExtension: WORD
    lpstrDefExt: LPCWSTR
    lCustData: LPARAM
    lpfnHook: pointer
    lpTemplateName: LPCWSTR
    pvReserved: pointer
    dwReserved: DWORD
    FlagsEx: DWORD

  TextEditSubProc = proc (hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                          uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.}

const
  PngClsid = SnipGuid(
    data1: 0x557CF406'u32,
    data2: 0x1A04'u16,
    data3: 0x11D3'u16,
    data4: [0x9A'u8, 0x73'u8, 0x00'u8, 0x00'u8, 0xF8'u8, 0x1E'u8, 0xF3'u8, 0x2E'u8]
  )

proc c_malloc(sz: csize_t): pointer {.importc: "malloc", header: "<stdlib.h>".}
proc c_free(p: pointer) {.importc: "free", header: "<stdlib.h>".}

proc DwmGetWindowAttribute(hwnd: HWND, dwAttribute: DWORD, pvAttribute: pointer,
                           cbAttribute: DWORD): HRESULT {.stdcall, importc, dynlib: "dwmapi.dll".}
proc GetSaveFileNameW(lpofn: ptr SnipOpenFileNameW): WINBOOL {.stdcall, importc, dynlib: "comdlg32.dll".}
proc SnipSetWindowSubclass(hWnd: HWND, pfn: TextEditSubProc, id: UINT_PTR,
                           dwRef: DWORD_PTR): WINBOOL {.stdcall, importc: "SetWindowSubclass", dynlib: "comctl32.dll".}
proc SnipDefSubclassProc(hWnd: HWND, uMsg: UINT, wp: WPARAM,
                         lp: LPARAM): LRESULT {.stdcall, importc: "DefSubclassProc", dynlib: "comctl32.dll".}
proc PinTrackMouse(lpEventTrack: ptr PinTrackMouseEvent): WINBOOL {.stdcall, importc: "TrackMouseEvent", dynlib: "user32.dll".}

proc GdiplusStartup(token: ptr pointer, input: ptr SnipGdiplusStartupInput,
                    output: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipCreateBitmapFromScan0(width, height, stride, format: cint,
                               scan0: pointer, bitmap: ptr pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDisposeImage(image: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipSaveImageToFile(image: pointer, filename: LPCWSTR,
                         clsidEncoder: ptr SnipGuid, encoderParams: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipCreateFromHDC(hdc: HDC, graphics: ptr pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDeleteGraphics(graphics: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipSetSmoothingMode(graphics: pointer, mode: cint): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipCreatePen1(color: uint32, width: cfloat, unit: cint, pen: ptr pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDeletePen(pen: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipSetPenStartCap(pen: pointer, cap: cint): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipSetPenEndCap(pen: pointer, cap: cint): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipSetPenLineJoin(pen: pointer, join: cint): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipCreateSolidFill(color: uint32, brush: ptr pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDeleteBrush(brush: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDrawLine(graphics, pen: pointer, x1, y1, x2, y2: cfloat): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDrawLines(graphics, pen: pointer, points: ptr GpPointF, count: cint): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDrawRectangle(graphics, pen: pointer, x, y, w, h: cfloat): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipFillRectangle(graphics, brush: pointer, x, y, w, h: cfloat): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDrawEllipse(graphics, pen: pointer, x, y, w, h: cfloat): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipFillEllipse(graphics, brush: pointer, x, y, w, h: cfloat): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipFillPolygon(graphics, brush: pointer, points: ptr GpPointF, count, fillMode: cint): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDrawPolygon(graphics, pen: pointer, points: ptr GpPointF, count: cint): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipCreatePath(fillMode: cint, path: ptr pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDeletePath(path: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipAddPathArc(path: pointer, x, y, w, h, startAngle, sweepAngle: cfloat): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipClosePathFigure(path: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipFillPath(graphics, brush, path: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}
proc GdipDrawPath(graphics, pen, path: pointer): cint {.stdcall, importc, dynlib: "gdiplus.dll".}

# ---- 全局状态（仅 UI 线程读写） ----
var
  g_gpToken: pointer
  g_gpReady = false

  g_hwndMain: HWND
  g_lastCfg: SnipCfg
  g_copyTextCb: proc (s: string)
  g_notifyCb: proc (msg: string)
  g_overlayClassReg = false
  g_resultClassReg = false
  g_pinClassReg = false

  # 最近一次截图缓存（F3 剪贴板为空时回退使用）
  g_lastSnipBits: pointer = nil
  g_lastSnipW: cint = 0
  g_lastSnipH: cint = 0

  # 遮罩状态
  g_hwndOverlay: HWND
  g_directTranslate = false
  g_vx, g_vy, g_vw, g_vh: cint
  g_hbmOrig, g_hbmDark, g_hbmBack: HBITMAP
  g_bitsOrig, g_bitsBack: pointer
  g_state: OverlayState = osHover
  g_ptMouse: POINT
type
  WindowCandidate = object
    hwnd: HWND
    topRect: RECT
    subRects: seq[RECT]

var
  g_ptStart, g_ptCurr: POINT
  g_selRc: RECT
  g_dragOrigRc: RECT
  g_dragStartPt: POINT
  g_resizeHandle = 0        # 1..8
  g_winRects: seq[RECT] = @[]
  g_winCands: seq[WindowCandidate] = @[]
  g_hoverWinRc: RECT
  g_hasHoverWin = false

  # 主工具栏 & 二级属性栏
  g_ovBtns: array[17, ToolBtn]
  g_ovBarRc: RECT
  g_ovSubBarRc: RECT
  g_ovHoverBtn = 0
  g_subHoverHit = 0
  g_sizeRcs: array[3, RECT]
  g_fillToggleRc: RECT
  g_colorRcs: array[8, RECT]

  # 二期标注状态与撤销重做栈
  g_curTool: AnnotTool = atNone
  g_curColorIdx = 0
  g_curSizeIdx = 1
  g_curFilled = false
  g_nextNumber = 1
  g_annots: seq[AnnotItem] = @[]
  g_redoStack: seq[AnnotItem] = @[]
  g_curAnnot: AnnotItem

  # 内联文字标注编辑框
  g_hwndTextEdit: HWND
  g_textEditFont: HFONT
  g_textEditBrush: HBRUSH
  g_textEditPt: POINT

  # 多贴图实例管理
  g_pins = initTable[HWND, PinWin]()

  # OCR 翻译结果悬浮窗状态
  g_hwndResult: HWND
  g_snipSeq = 0
  g_resW, g_resH: cint
  g_dispW, g_dispH: cint
  g_winW, g_winH: cint
  g_zoomScale = 1.0
  g_hbmResOrig, g_hbmResTrans: HBITMAP
  g_bitsResOrig, g_bitsResTrans: pointer
  g_showOrig = false
  g_translating = false
  g_zhText = ""
  g_statusMsg = ""
  g_btns: array[6, ToolBtn]
  g_hoverBtn = 0

proc startSnipCapture*(hwndMain: HWND, cfg: SnipCfg, copyCb: proc (s: string),
                       directTranslate = false, notifyCb: proc (msg: string) = nil)
proc openResultWindow(screenX, screenY, w, h: cint, cropBits: pointer)
proc createPinWindow(screenX, screenY, w, h: cint, bitsIn: pointer): PinWin

proc argb(r, g, b: uint8, a: uint8 = 255'u8): uint32 {.inline.} =
  (uint32(a) shl 24) or (uint32(r) shl 16) or (uint32(g) shl 8) or uint32(b)

proc colorArgb(idx: int, a: uint8 = 255'u8): uint32 {.inline.} =
  let c = AnnotColors[clamp(idx, 0, AnnotColors.high)]
  argb(c[0], c[1], c[2], a)

proc ensureGdiplus() =
  if g_gpReady: return
  var inp: SnipGdiplusStartupInput
  inp.gdiplusVersion = 1
  if GdiplusStartup(addr g_gpToken, addr inp, nil) == 0:
    g_gpReady = true

proc makeDib32(hdc: HDC, w, h: cint, bitsOut: ptr pointer): HBITMAP =
  var bmi: BITMAPINFO
  bmi.bmiHeader.biSize = DWORD(sizeof(BITMAPINFOHEADER))
  bmi.bmiHeader.biWidth = LONG(w)
  bmi.bmiHeader.biHeight = LONG(-h)   # top-down
  bmi.bmiHeader.biPlanes = 1
  bmi.bmiHeader.biBitCount = 32
  bmi.bmiHeader.biCompression = BI_RGB
  result = CreateDIBSection(hdc, addr bmi, DIB_RGB_COLORS, bitsOut, 0, 0)

proc saveLastSnipCache(bits: pointer, w, h: cint) =
  if bits == nil or w <= 0 or h <= 0: return
  if g_lastSnipBits != nil:
    c_free(g_lastSnipBits)
    g_lastSnipBits = nil
  let sz = int(w) * int(h) * 4
  g_lastSnipBits = c_malloc(csize_t(sz))
  if g_lastSnipBits != nil:
    copyMem(g_lastSnipBits, bits, sz)
    g_lastSnipW = w
    g_lastSnipH = h

proc destroyTextEdit() =
  if g_hwndTextEdit != 0:
    DestroyWindow(g_hwndTextEdit)
    g_hwndTextEdit = 0
  if g_textEditFont != 0:
    DeleteObject(g_textEditFont)
    g_textEditFont = 0
  if g_textEditBrush != 0:
    DeleteObject(g_textEditBrush)
    g_textEditBrush = 0

proc freeOverlayBitmaps() =
  destroyTextEdit()
  if g_hbmOrig != 0: DeleteObject(g_hbmOrig); g_hbmOrig = 0
  if g_hbmDark != 0: DeleteObject(g_hbmDark); g_hbmDark = 0
  if g_hbmBack != 0: DeleteObject(g_hbmBack); g_hbmBack = 0
  g_bitsOrig = nil
  g_bitsBack = nil

proc freeResultBitmaps() =
  if g_hbmResOrig != 0: DeleteObject(g_hbmResOrig); g_hbmResOrig = 0
  if g_hbmResTrans != 0: DeleteObject(g_hbmResTrans); g_hbmResTrans = 0
  g_bitsResOrig = nil
  g_bitsResTrans = nil

proc syncEscHotkey() =
  if g_hwndMain == 0: return
  if g_hwndOverlay != 0 or g_hwndResult != 0:
    discard RegisterHotKey(g_hwndMain, UINT(HkSnipEsc), 0, UINT(VK_ESCAPE))
  else:
    discard UnregisterHotKey(g_hwndMain, HkSnipEsc)

proc closeResultWindow*() =
  if g_hwndResult != 0:
    DestroyWindow(g_hwndResult)
    g_hwndResult = 0
  freeResultBitmaps()
  g_zhText = ""
  g_statusMsg = ""
  syncEscHotkey()

proc normRect(p1, p2: POINT): RECT =
  result.left = min(p1.x, p2.x)
  result.top = min(p1.y, p2.y)
  result.right = max(p1.x, p2.x)
  result.bottom = max(p1.y, p2.y)

proc ptInRect(rc: RECT, x, y: int): bool {.inline.} =
  x >= int(rc.left) and x < int(rc.right) and y >= int(rc.top) and y < int(rc.bottom)

proc copyDibToClipboard(hwnd: HWND, bits: pointer, w, h: cint): bool =
  if bits == nil or w <= 0 or h <= 0: return false
  let imgBytes = int(w) * int(h) * 4
  let totalBytes = sizeof(BITMAPINFOHEADER) + imgBytes
  let hMem = GlobalAlloc(GMEM_MOVEABLE, SIZE_T(totalBytes))
  if hMem == 0: return false
  let pMem = GlobalLock(hMem)
  if pMem == nil:
    GlobalFree(hMem)
    return false
  zeroMem(pMem, sizeof(BITMAPINFOHEADER))
  let bih = cast[ptr BITMAPINFOHEADER](pMem)
  bih.biSize = DWORD(sizeof(BITMAPINFOHEADER))
  bih.biWidth = LONG(w)
  bih.biHeight = LONG(-h)
  bih.biPlanes = 1
  bih.biBitCount = 32
  bih.biCompression = BI_RGB
  bih.biSizeImage = DWORD(imgBytes)
  copyMem(cast[pointer](cast[uint](pMem) + uint(sizeof(BITMAPINFOHEADER))), bits, imgBytes)
  GlobalUnlock(hMem)
  if OpenClipboard(hwnd) != 0:
    EmptyClipboard()
    if SetClipboardData(CF_DIB, hMem) != 0:
      CloseClipboard()
      return true
    CloseClipboard()
  GlobalFree(hMem)
  return false

proc makeSnipFileName(): string =
  var st: SYSTEMTIME
  GetLocalTime(addr st)
  "Snip_" &
    align($st.wYear, 4, '0') &
    align($st.wMonth, 2, '0') &
    align($st.wDay, 2, '0') & "_" &
    align($st.wHour, 2, '0') &
    align($st.wMinute, 2, '0') &
    align($st.wSecond, 2, '0') & ".png"

proc effectiveSaveDir(): string =
  result = g_lastCfg.saveDir.strip()
  if result.len == 0:
    let base = g_lastCfg.cacheDir.strip()
    if base.len > 0:
      result = base / "screenshots"
    else:
      result = getAppDir() / "fastT" / "screenshots"

proc saveBitsToPngFile(path: string, bits: pointer, w, h: cint): bool =
  if bits == nil or w <= 0 or h <= 0 or path.len == 0: return false
  ensureGdiplus()
  if not g_gpReady: return false
  var gpBmp: pointer = nil
  if GdipCreateBitmapFromScan0(w, h, w * 4, PixelFormat32bppRgb, bits, addr gpBmp) != 0 or gpBmp == nil:
    return false
  var clsid = PngClsid
  let stSave = GdipSaveImageToFile(gpBmp, newWideCString(path), addr clsid, nil)
  discard GdipDisposeImage(gpBmp)
  result = (stSave == 0)

proc saveBitsAsPngDefault(bits: pointer, w, h: cint, outPath: var string): bool =
  outPath = ""
  if bits == nil or w <= 0 or h <= 0: return false
  let dir = effectiveSaveDir()
  try:
    createDir(dir)
  except OSError:
    return false
  let baseName = makeSnipFileName()
  let stem = if baseName.endsWith(".png"): baseName[0 .. ^5] else: baseName
  var candidate = dir / (stem & ".png")
  var idx = 1
  while fileExists(candidate) and idx < 1000:
    candidate = dir / (stem & "_" & $idx & ".png")
    inc idx
  if saveBitsToPngFile(candidate, bits, w, h):
    outPath = candidate
    return true
  result = false

proc saveBitsAsPngDialog(owner: HWND, bits: pointer, w, h: cint): bool =
  if bits == nil or w <= 0 or h <= 0: return false
  ensureGdiplus()
  if not g_gpReady: return false

  let defName = makeSnipFileName()
  let initDir = effectiveSaveDir()
  try:
    if initDir.len > 0 and not dirExists(initDir):
      createDir(initDir)
  except OSError:
    discard

  var fileBuf = newSeq[uint16](512)
  let wDef = newWideCString(defName)
  for i in 0 ..< min(defName.len, 500):
    fileBuf[i] = uint16(wDef[i])

  # 显式构建双 '\0' 结尾过滤器，避免 cstring 转码在首 '\0' 截断
  var filterBuf: seq[uint16] = @[]
  for part in ["PNG 图片 (*.png)", "*.png", "所有文件 (*.*)", "*.*"]:
    let wp = newWideCString(part)
    var k = 0
    while uint16(wp[k]) != 0'u16:
      filterBuf.add(uint16(wp[k]))
      inc k
    filterBuf.add(0'u16)
  filterBuf.add(0'u16)

  let wExt = newWideCString("png")
  let wTitle = newWideCString("保存截图")
  let wInitDir = if dirExists(initDir): newWideCString(initDir) else: nil

  var ofn: SnipOpenFileNameW
  zeroMem(addr ofn, sizeof(SnipOpenFileNameW))
  ofn.lStructSize = DWORD(sizeof(SnipOpenFileNameW))
  ofn.hwndOwner = owner
  ofn.lpstrFilter = cast[LPCWSTR](addr filterBuf[0])
  ofn.nFilterIndex = 1
  ofn.lpstrFile = cast[LPWSTR](addr fileBuf[0])
  ofn.nMaxFile = DWORD(fileBuf.len)
  if wInitDir != nil:
    ofn.lpstrInitialDir = cast[LPCWSTR](wInitDir)
  ofn.lpstrDefExt = cast[LPCWSTR](wExt)
  ofn.lpstrTitle = cast[LPCWSTR](wTitle)
  ofn.Flags = OfnOverwritePrompt or OfnPathMustExist or OfnNoChangeDir

  if GetSaveFileNameW(addr ofn) == 0:
    return false

  var path = $cast[WideCString](addr fileBuf[0])
  if path.len == 0: return false
  if not path.toLowerAscii().endsWith(".png"):
    path.add(".png")

  result = saveBitsToPngFile(path, bits, w, h)

# ---------------- 后台 OCR + 翻译裸线程 ----------------
proc snipWorker(param: LPVOID): DWORD {.stdcall.} =
  let job = cast[ptr SnipJob](param)
  let seq = job.seq
  let hwndMain = job.hwndMain
  let res = fy_snip_translate(job.bgra, job.w, job.h,
                              job.aiUrl, job.aiKey, job.aiModel,
                              job.baiduAppId, job.baiduKey, job.baiduUrl,
                              job.cacheDir)
  c_free(job.bgra)
  fy_free(job.aiUrl)
  fy_free(job.aiKey)
  fy_free(job.aiModel)
  fy_free(job.baiduAppId)
  fy_free(job.baiduKey)
  fy_free(job.baiduUrl)
  fy_free(job.cacheDir)
  dealloc(job)
  PostMessageW(hwndMain, UINT(WmAppSnipDone), WPARAM(seq), cast[LPARAM](res))
  return 0

proc snipPinWorker(param: LPVOID): DWORD {.stdcall.} =
  let job = cast[ptr SnipJob](param)
  let seq = job.seq
  let hwndMain = job.hwndMain
  let pinHwnd = job.pinHwnd
  let res = fy_snip_translate(job.bgra, job.w, job.h,
                              job.aiUrl, job.aiKey, job.aiModel,
                              job.baiduAppId, job.baiduKey, job.baiduUrl,
                              job.cacheDir)
  c_free(job.bgra)
  fy_free(job.aiUrl)
  fy_free(job.aiKey)
  fy_free(job.aiModel)
  fy_free(job.baiduAppId)
  fy_free(job.baiduKey)
  fy_free(job.baiduUrl)
  fy_free(job.cacheDir)
  c_free(job)
  let payload = cast[ptr PinTransPayload](c_malloc(csize_t(sizeof(PinTransPayload))))
  if payload != nil:
    payload.pinHwnd = pinHwnd
    payload.seq = seq
    payload.res = res
    if PostMessageW(hwndMain, UINT(WmAppPinTransDone), WPARAM(0), cast[LPARAM](payload)) == 0:
      c_free(payload)
      if res != nil: fy_snip_result_free(res)
  else:
    if res != nil: fy_snip_result_free(res)
  return 0

# ---------------- 多贴图悬浮窗（F3） ----------------
proc activePinBits(pw: PinWin): pointer {.inline.} =
  if pw != nil and pw.showTrans and pw.bitsTrans != nil: pw.bitsTrans
  elif pw != nil: pw.bits
  else: nil

proc activePinHbm(pw: PinWin): HBITMAP {.inline.} =
  if pw != nil and pw.showTrans and pw.hbmTrans != 0: pw.hbmTrans
  elif pw != nil: pw.hbm
  else: 0

proc pinTransBtnRect(pw: PinWin): RECT =
  if pw != nil and pw.dispW >= 64 and pw.dispH >= 40:
    return RECT(left: LONG(pw.dispW - 32), top: 6, right: LONG(pw.dispW - 6), bottom: 30)
  result = RECT(left: 0, top: 0, right: 0, bottom: 0)

proc closePinWindow(pw: PinWin) =
  if pw == nil: return
  let h = pw.hwnd
  if h != 0:
    g_pins.del(h)
    DestroyWindow(h)
    pw.hwnd = 0
  if pw.hbm != 0:
    DeleteObject(pw.hbm)
    pw.hbm = 0
  pw.bits = nil
  if pw.hbmTrans != 0:
    DeleteObject(pw.hbmTrans)
    pw.hbmTrans = 0
  pw.bitsTrans = nil

proc closeAllPins*() =
  var list: seq[PinWin] = @[]
  for _, pw in g_pins.pairs:
    list.add(pw)
  for pw in list:
    closePinWindow(pw)

proc applyPinZoom(pw: PinWin, targetScale: float, anchorMouse: bool) =
  if pw == nil or pw.hwnd == 0 or pw.origW <= 0 or pw.origH <= 0: return
  let minBySize = max(48.0 / float(pw.origW), 32.0 / float(pw.origH))
  let minScale = min(1.0, max(0.15, minBySize))
  let maxBySize = min(4096.0 / float(pw.origW), 4096.0 / float(pw.origH))
  let maxScale = max(1.0, min(5.0, maxBySize))
  let newScale = clamp(targetScale, minScale, maxScale)

  let newW = cint(max(24, int(round(float(pw.origW) * newScale))))
  let newH = cint(max(24, int(round(float(pw.origH) * newScale))))

  var wr: RECT
  discard GetWindowRect(pw.hwnd, addr wr)
  var newLeft = int(wr.left)
  var newTop = int(wr.top)
  if anchorMouse:
    var pt: POINT
    discard GetCursorPos(addr pt)
    let rx = clamp(float(pt.x - int(wr.left)) / float(max(1, int(pw.dispW))), 0.0, 1.0)
    let ry = clamp(float(pt.y - int(wr.top)) / float(max(1, int(pw.dispH))), 0.0, 1.0)
    newLeft = int(pt.x) - int(round(rx * float(newW)))
    newTop = int(pt.y) - int(round(ry * float(newH)))
  else:
    let cx = int(wr.left) + int(pw.dispW) div 2
    let cy = int(wr.top) + int(pw.dispH) div 2
    newLeft = cx - int(newW) div 2
    newTop = cy - int(newH) div 2

  pw.zoomScale = newScale
  pw.dispW = newW
  pw.dispH = newH
  pw.hudMsg = $int(round(newScale * 100.0)) & "%"
  discard SetTimer(pw.hwnd, PinHudTimerId, 850, nil)
  SetWindowPos(pw.hwnd, 0, cint(newLeft), cint(newTop), newW, newH,
               SWP_NOZORDER or SWP_NOACTIVATE)
  InvalidateRect(pw.hwnd, nil, FALSE)

proc applyPinAlpha(pw: PinWin, newAlpha: uint8) =
  if pw == nil or pw.hwnd == 0: return
  pw.alpha = newAlpha
  let ex = GetWindowLongPtrW(pw.hwnd, GWL_EXSTYLE)
  if newAlpha < 255'u8:
    discard SetWindowLongPtrW(pw.hwnd, GWL_EXSTYLE, ex or WS_EX_LAYERED)
    discard SetLayeredWindowAttributes(pw.hwnd, 0, newAlpha, LWA_ALPHA)
  else:
    discard SetLayeredWindowAttributes(pw.hwnd, 0, 255, LWA_ALPHA)
    discard SetWindowLongPtrW(pw.hwnd, GWL_EXSTYLE, ex and (not WS_EX_LAYERED))
  let pct = int(round(float(newAlpha) * 100.0 / 255.0))
  pw.hudMsg = "透明度 " & $pct & "%"
  discard SetTimer(pw.hwnd, PinHudTimerId, 850, nil)
  InvalidateRect(pw.hwnd, nil, FALSE)

proc startPinTranslate(pw: PinWin) =
  if pw == nil or pw.hwnd == 0 or pw.bits == nil or pw.origW <= 0 or pw.origH <= 0: return
  if pw.translating: return
  inc pw.transSeq
  pw.translating = true
  pw.hudMsg = "正在识别翻译…"
  discard KillTimer(pw.hwnd, PinHudTimerId)
  InvalidateRect(pw.hwnd, nil, FALSE)

  let imgBytes = int(pw.origW) * int(pw.origH) * 4
  let bgraCopy = c_malloc(csize_t(imgBytes))
  if bgraCopy == nil:
    pw.translating = false
    pw.hudMsg = "内存不足"
    discard SetTimer(pw.hwnd, PinHudTimerId, 1100, nil)
    InvalidateRect(pw.hwnd, nil, FALSE)
    return
  copyMem(bgraCopy, pw.bits, imgBytes)
  let pIn = cast[ptr UncheckedArray[uint8]](bgraCopy)
  var k = 3
  while k < imgBytes:
    pIn[k] = 255'u8
    k += 4

  let job = cast[ptr SnipJob](c_malloc(csize_t(sizeof(SnipJob))))
  if job == nil:
    c_free(bgraCopy)
    pw.translating = false
    pw.hudMsg = "内存不足"
    discard SetTimer(pw.hwnd, PinHudTimerId, 1100, nil)
    InvalidateRect(pw.hwnd, nil, FALSE)
    return
  zeroMem(job, sizeof(SnipJob))
  job.seq = pw.transSeq
  job.hwndMain = g_hwndMain
  job.pinHwnd = pw.hwnd
  job.bgra = bgraCopy
  job.w = pw.origW
  job.h = pw.origH
  job.aiUrl = fy_strdup(g_lastCfg.aiUrl.cstring)
  job.aiKey = fy_strdup(g_lastCfg.aiKey.cstring)
  job.aiModel = fy_strdup(g_lastCfg.aiModel.cstring)
  job.baiduAppId = fy_strdup(g_lastCfg.baiduAppId.cstring)
  job.baiduKey = fy_strdup(g_lastCfg.baiduKey.cstring)
  job.baiduUrl = fy_strdup(g_lastCfg.baiduUrl.cstring)
  job.cacheDir = fy_strdup(g_lastCfg.cacheDir.cstring)
  let th = CreateThread(nil, 0, snipPinWorker, cast[LPVOID](job), 0, nil)
  if th == 0:
    c_free(job.bgra)
    fy_free(job.aiUrl); fy_free(job.aiKey); fy_free(job.aiModel)
    fy_free(job.baiduAppId); fy_free(job.baiduKey); fy_free(job.baiduUrl)
    fy_free(job.cacheDir)
    c_free(job)
    pw.translating = false
    pw.hudMsg = "启动翻译失败"
    discard SetTimer(pw.hwnd, PinHudTimerId, 1100, nil)
    InvalidateRect(pw.hwnd, nil, FALSE)
  else:
    discard CloseHandle(th)

proc onPinTransDone*(wp: WPARAM, lp: LPARAM) =
  if lp == 0: return
  let payload = cast[ptr PinTransPayload](lp)
  let res = payload.res
  let pinHwnd = payload.pinHwnd
  let seq = payload.seq
  c_free(payload)
  if res == nil: return
  defer: fy_snip_result_free(res)

  let pw = g_pins.getOrDefault(pinHwnd, nil)
  if pw == nil or seq != pw.transSeq or pw.hwnd == 0: return
  pw.translating = false
  if res.ok != 0 and res.bgra_out != nil and res.width == pw.origW and res.height == pw.origH:
    if pw.hbmTrans == 0:
      let hdcScr = GetDC(0)
      pw.hbmTrans = makeDib32(hdcScr, pw.origW, pw.origH, addr pw.bitsTrans)
      ReleaseDC(0, hdcScr)
    if pw.hbmTrans != 0 and pw.bitsTrans != nil:
      let imgBytes = int(pw.origW) * int(pw.origH) * 4
      copyMem(pw.bitsTrans, res.bgra_out, imgBytes)
      let pOut = cast[ptr UncheckedArray[uint8]](pw.bitsTrans)
      var k = 3
      while k < imgBytes:
        pOut[k] = 255'u8
        k += 4
      pw.zhText = if res.zh_text != nil: $res.zh_text else: ""
      pw.showTrans = true
      pw.hudMsg = "翻译完成 (T切原图)"
    else:
      pw.hudMsg = "创建译图失败"
  else:
    let errTxt = if res.err != nil: ($res.err).strip() else: ""
    pw.hudMsg = if errTxt.len > 0: errTxt else: "未识别到英文文本"
  discard SetTimer(pw.hwnd, PinHudTimerId, 1500, nil)
  InvalidateRect(pw.hwnd, nil, FALSE)

proc showPinContextMenu(pw: PinWin) =
  if pw == nil or pw.hwnd == 0: return
  let menu = CreatePopupMenu()
  if menu == 0: return
  if pw.translating:
    discard AppendMenuW(menu, MF_STRING or PinMfGrayed, UINT_PTR(0), newWideCString("正在识别翻译…"))
  elif pw.hbmTrans == 0:
    discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuTrans), newWideCString("翻译贴图 (T)"))
  else:
    discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuToggleTrans),
                        newWideCString(if pw.showTrans: "显示原图 (T)" else: "显示译图 (T)"))
    if pw.zhText.len > 0:
      discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuCopyTransText), newWideCString("复制译文"))
    discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuTrans), newWideCString("重新翻译贴图"))
  discard AppendMenuW(menu, MF_SEPARATOR, 0, nil)
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuCopy), newWideCString("复制图片 (Ctrl+C)"))
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuQuickSave), newWideCString("快速保存到默认位置 (Ctrl+Shift+S)"))
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuSave), newWideCString("另存为 PNG... (Ctrl+S)"))
  discard AppendMenuW(menu, MF_SEPARATOR, 0, nil)
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuResetZoom), newWideCString("重置缩放 (100%)"))
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuOp100), newWideCString("不透明度 100%"))
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuOp80), newWideCString("不透明度 80%"))
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuOp50), newWideCString("不透明度 50%"))
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuBorder),
                      newWideCString(if pw.showBorder: "隐藏边框" else: "显示边框"))
  discard AppendMenuW(menu, MF_SEPARATOR, 0, nil)
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuClose), newWideCString("关闭此贴图"))
  discard AppendMenuW(menu, MF_STRING, UINT_PTR(PinMenuCloseAll), newWideCString("关闭所有贴图"))

  var pt: POINT
  discard GetCursorPos(addr pt)
  SetForegroundWindow(pw.hwnd)
  let cmd = int(TrackPopupMenu(menu, TPM_RETURNCMD or TPM_RIGHTBUTTON, pt.x, pt.y, 0, pw.hwnd, nil))
  discard DestroyMenu(menu)

  case cmd
  of PinMenuCopy:
    if copyDibToClipboard(pw.hwnd, activePinBits(pw), pw.origW, pw.origH):
      pw.hudMsg = "已复制"
      discard SetTimer(pw.hwnd, PinHudTimerId, 850, nil)
      InvalidateRect(pw.hwnd, nil, FALSE)
  of PinMenuQuickSave:
    var savedPath = ""
    if saveBitsAsPngDefault(activePinBits(pw), pw.origW, pw.origH, savedPath):
      pw.hudMsg = "已保存到默认位置"
    else:
      pw.hudMsg = "保存失败"
    discard SetTimer(pw.hwnd, PinHudTimerId, 1100, nil)
    InvalidateRect(pw.hwnd, nil, FALSE)
  of PinMenuSave:
    discard saveBitsAsPngDialog(pw.hwnd, activePinBits(pw), pw.origW, pw.origH)
  of PinMenuTrans:
    startPinTranslate(pw)
  of PinMenuToggleTrans:
    if pw.hbmTrans != 0 and not pw.translating:
      pw.showTrans = not pw.showTrans
      pw.hudMsg = if pw.showTrans: "已切换译图" else: "已切换原图"
      discard SetTimer(pw.hwnd, PinHudTimerId, 950, nil)
      InvalidateRect(pw.hwnd, nil, FALSE)
  of PinMenuCopyTransText:
    if pw.zhText.len > 0 and g_copyTextCb != nil:
      g_copyTextCb(pw.zhText)
      pw.hudMsg = "已复制译文"
      discard SetTimer(pw.hwnd, PinHudTimerId, 950, nil)
      InvalidateRect(pw.hwnd, nil, FALSE)
  of PinMenuResetZoom:
    applyPinZoom(pw, 1.0, false)
  of PinMenuOp100:
    applyPinAlpha(pw, 255'u8)
  of PinMenuOp80:
    applyPinAlpha(pw, 204'u8)
  of PinMenuOp50:
    applyPinAlpha(pw, 128'u8)
  of PinMenuBorder:
    pw.showBorder = not pw.showBorder
    InvalidateRect(pw.hwnd, nil, FALSE)
  of PinMenuClose:
    closePinWindow(pw)
  of PinMenuCloseAll:
    closeAllPins()
  else: discard

proc drawPinTransBtn(hdcBuf: HDC, pw: PinWin, rc: RECT) =
  let x = cfloat(rc.left)
  let y = cfloat(rc.top)
  let w = cfloat(rc.right - rc.left)
  let h = cfloat(rc.bottom - rc.top)
  let r = 5.0'f32

  let bgCol = if pw.btnHover: argb(59, 130, 246, 240'u8)
              elif pw.showTrans: argb(37, 99, 235, 220'u8)
              else: argb(24, 28, 36, 205'u8)
  let bdCol = if pw.btnHover or pw.showTrans: argb(147, 197, 253, 225'u8)
              else: argb(148, 163, 184, 170'u8)

  ensureGdiplus()
  var drawnGp = false
  if g_gpReady:
    var g: pointer = nil
    if GdipCreateFromHDC(hdcBuf, addr g) == 0 and g != nil:
      discard GdipSetSmoothingMode(g, SmoothingAntiAlias)
      var path: pointer = nil
      if GdipCreatePath(0, addr path) == 0 and path != nil:
        let d = min(r * 2.0'f32, min(w, h))
        discard GdipAddPathArc(path, x, y, d, d, 180.0'f32, 90.0'f32)
        discard GdipAddPathArc(path, x + w - d, y, d, d, 270.0'f32, 90.0'f32)
        discard GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0'f32, 90.0'f32)
        discard GdipAddPathArc(path, x, y + h - d, d, d, 90.0'f32, 90.0'f32)
        discard GdipClosePathFigure(path)
        var br: pointer = nil
        if GdipCreateSolidFill(bgCol, addr br) == 0 and br != nil:
          discard GdipFillPath(g, br, path)
          discard GdipDeleteBrush(br)
        var pen: pointer = nil
        if GdipCreatePen1(bdCol, 1.0'f32, 2, addr pen) == 0 and pen != nil:
          discard GdipDrawPath(g, pen, path)
          discard GdipDeletePen(pen)
        discard GdipDeletePath(path)
        drawnGp = true
      discard GdipDeleteGraphics(g)

  if not drawnGp:
    var rFill = rc
    let br = CreateSolidBrush(if pw.btnHover or pw.showTrans: RGB(37, 99, 235) else: RGB(28, 32, 42))
    FillRect(hdcBuf, addr rFill, br)
    DeleteObject(br)

  let txt = if pw.translating: "…"
            elif pw.hbmTrans != 0 and pw.showTrans: "原"
            else: "译"
  let hf = CreateFontW(-12, 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                       0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldF = SelectObject(hdcBuf, hf)
  SetBkMode(hdcBuf, TRANSPARENT)
  SetTextColor(hdcBuf, RGB(244, 248, 255))
  var tr = rc
  DrawTextW(hdcBuf, newWideCString(txt), -1, addr tr,
            DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
  SelectObject(hdcBuf, oldF)
  DeleteObject(hf)

proc pinWndProc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM): LRESULT {.stdcall.} =
  let pw = g_pins.getOrDefault(hwnd, nil)
  if pw == nil:
    return DefWindowProcW(hwnd, msg, wp, lp)

  case msg
  of WM_PAINT:
    var ps: PAINTSTRUCT
    let hdc = BeginPaint(hwnd, addr ps)
    let hdcBuf = CreateCompatibleDC(hdc)
    let hbmBuf = CreateCompatibleBitmap(hdc, pw.dispW, pw.dispH)
    let oldBuf = SelectObject(hdcBuf, hbmBuf)

    let srcHbm = activePinHbm(pw)
    if srcHbm != 0:
      let hdcSrc = CreateCompatibleDC(hdc)
      let oldSrc = SelectObject(hdcSrc, srcHbm)
      if pw.dispW == pw.origW and pw.dispH == pw.origH:
        BitBlt(hdcBuf, 0, 0, pw.dispW, pw.dispH, hdcSrc, 0, 0, SRCCOPY)
      else:
        SetStretchBltMode(hdcBuf, HALFTONE)
        SetBrushOrgEx(hdcBuf, 0, 0, nil)
        StretchBlt(hdcBuf, 0, 0, pw.dispW, pw.dispH, hdcSrc, 0, 0, pw.origW, pw.origH, SRCCOPY)
      SelectObject(hdcSrc, oldSrc)
      DeleteDC(hdcSrc)

    if pw.showBorder:
      let pen = CreatePen(PS_SOLID, 1, RGB(59, 130, 246))
      let oldP = SelectObject(hdcBuf, pen)
      let oldB = SelectObject(hdcBuf, GetStockObject(NULL_BRUSH))
      Rectangle(hdcBuf, 0, 0, pw.dispW, pw.dispH)
      SelectObject(hdcBuf, oldB)
      SelectObject(hdcBuf, oldP)
      DeleteObject(pen)

    let btnRc = pinTransBtnRect(pw)
    let showBtn = (pw.mouseIn or pw.translating) and btnRc.right > btnRc.left

    if pw.hudMsg.len > 0:
      let hf = CreateFontW(-12, 0, 0, 0, FW_SEMIBOLD, 0, 0, 0, DEFAULT_CHARSET,
                           0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
      let oldF = SelectObject(hdcBuf, hf)
      let wHud = newWideCString(pw.hudMsg)
      var calcRc = RECT(left: 0, top: 0, right: 0, bottom: 0)
      discard DrawTextW(hdcBuf, cast[LPCWSTR](wHud), -1, addr calcRc, DT_CALCRECT or DT_SINGLELINE or DT_NOPREFIX)
      let maxHudW = if showBtn: int(pw.dispW) - 42 else: int(pw.dispW) - 12
      let badgeW = min(maxHudW, max(72, int(calcRc.right - calcRc.left) + 20))
      var badge = RECT(left: 6, top: 6, right: LONG(6 + badgeW), bottom: 28)
      if badge.right > badge.left + 30:
        let br = CreateSolidBrush(RGB(20, 24, 32))
        FillRect(hdcBuf, addr badge, br)
        DeleteObject(br)
        SetBkMode(hdcBuf, TRANSPARENT)
        SetTextColor(hdcBuf, RGB(236, 244, 255))
        DrawTextW(hdcBuf, cast[LPCWSTR](wHud), -1, addr badge,
                  DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
      SelectObject(hdcBuf, oldF)
      DeleteObject(hf)

    if showBtn:
      drawPinTransBtn(hdcBuf, pw, btnRc)

    BitBlt(hdc, 0, 0, pw.dispW, pw.dispH, hdcBuf, 0, 0, SRCCOPY)
    SelectObject(hdcBuf, oldBuf)
    DeleteObject(hbmBuf)
    DeleteDC(hdcBuf)
    EndPaint(hwnd, addr ps)
    return 0

  of WM_TIMER:
    if wp == PinHudTimerId:
      discard KillTimer(hwnd, PinHudTimerId)
      pw.hudMsg = ""
      InvalidateRect(hwnd, nil, FALSE)
    return 0

  of WM_MOUSEMOVE:
    let mx = int(cast[int16](lp and 0xFFFF))
    let my = int(cast[int16]((lp shr 16) and 0xFFFF))
    var needRedraw = false
    if not pw.mouseIn:
      var tme: PinTrackMouseEvent
      tme.cbSize = DWORD(sizeof(PinTrackMouseEvent))
      tme.dwFlags = PinTmeLeave
      tme.hwndTrack = hwnd
      discard PinTrackMouse(addr tme)
      pw.mouseIn = true
      needRedraw = true
    let brc = pinTransBtnRect(pw)
    let nh = (brc.right > brc.left) and ptInRect(brc, mx, my)
    if nh != pw.btnHover:
      pw.btnHover = nh
      needRedraw = true
    if needRedraw:
      InvalidateRect(hwnd, nil, FALSE)
    return 0

  of PinWmMouseLeave:
    if pw.mouseIn or pw.btnHover:
      pw.mouseIn = false
      pw.btnHover = false
      InvalidateRect(hwnd, nil, FALSE)
    return 0

  of WM_SETCURSOR:
    if (lp and 0xFFFF) == HTCLIENT and pw.btnHover:
      SetCursor(LoadCursorW(0, IDC_HAND))
      return 1
    return DefWindowProcW(hwnd, msg, wp, lp)

  of WM_LBUTTONDOWN:
    let mx = int(cast[int16](lp and 0xFFFF))
    let my = int(cast[int16]((lp shr 16) and 0xFFFF))
    let brc = pinTransBtnRect(pw)
    if (pw.mouseIn or pw.translating) and brc.right > brc.left and ptInRect(brc, mx, my):
      if not pw.translating:
        if pw.hbmTrans != 0:
          pw.showTrans = not pw.showTrans
          pw.hudMsg = if pw.showTrans: "已切换译图" else: "已切换原图"
          discard SetTimer(hwnd, PinHudTimerId, 950, nil)
          InvalidateRect(hwnd, nil, FALSE)
        else:
          startPinTranslate(pw)
      return 0
    ReleaseCapture()
    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0)
    return 0

  of WM_LBUTTONDBLCLK:
    let mx = int(cast[int16](lp and 0xFFFF))
    let my = int(cast[int16]((lp shr 16) and 0xFFFF))
    let brc = pinTransBtnRect(pw)
    if brc.right > brc.left and ptInRect(brc, mx, my):
      if not pw.translating:
        if pw.hbmTrans != 0:
          pw.showTrans = not pw.showTrans
          pw.hudMsg = if pw.showTrans: "已切换译图" else: "已切换原图"
          discard SetTimer(hwnd, PinHudTimerId, 950, nil)
          InvalidateRect(hwnd, nil, FALSE)
        else:
          startPinTranslate(pw)
      return 0
    closePinWindow(pw)
    return 0

  of WM_RBUTTONUP:
    showPinContextMenu(pw)
    return 0

  of WM_MOUSEWHEEL:
    let delta = int(cast[int16]((wp shr 16) and 0xFFFF))
    if delta != 0:
      let steps = float(delta) / 120.0
      if (GetKeyState(VK_CONTROL) and 0x8000'i16) != 0:
        let cur = int(pw.alpha)
        let nxt = clamp(cur + int(round(steps * 22.0)), 30, 255)
        applyPinAlpha(pw, uint8(nxt))
      else:
        applyPinZoom(pw, pw.zoomScale * pow(1.1, steps), true)
    return 0

  of WM_KEYDOWN:
    let ctrlDown = (GetKeyState(VK_CONTROL) and 0x8000'i16) != 0
    let shiftDown = (GetKeyState(VK_SHIFT) and 0x8000'i16) != 0
    let altDown = (GetKeyState(VK_MENU) and 0x8000'i16) != 0
    if wp == VK_ESCAPE or wp == VK_DELETE:
      closePinWindow(pw)
      return 0
    elif wp == WPARAM('T') and not ctrlDown and not altDown:
      if not pw.translating:
        if pw.hbmTrans != 0:
          pw.showTrans = not pw.showTrans
          pw.hudMsg = if pw.showTrans: "已切换译图" else: "已切换原图"
          discard SetTimer(hwnd, PinHudTimerId, 950, nil)
          InvalidateRect(hwnd, nil, FALSE)
        else:
          startPinTranslate(pw)
      return 0
    elif ctrlDown and wp == WPARAM('C'):
      if copyDibToClipboard(hwnd, activePinBits(pw), pw.origW, pw.origH):
        pw.hudMsg = "已复制"
        discard SetTimer(hwnd, PinHudTimerId, 850, nil)
        InvalidateRect(hwnd, nil, FALSE)
      return 0
    elif ctrlDown and wp == WPARAM('S'):
      if shiftDown:
        var savedPath = ""
        if saveBitsAsPngDefault(activePinBits(pw), pw.origW, pw.origH, savedPath):
          pw.hudMsg = "已保存到默认位置"
        else:
          pw.hudMsg = "保存失败"
        discard SetTimer(hwnd, PinHudTimerId, 1100, nil)
        InvalidateRect(hwnd, nil, FALSE)
      else:
        discard saveBitsAsPngDialog(hwnd, activePinBits(pw), pw.origW, pw.origH)
      return 0
    elif wp == WPARAM('0') or wp == WPARAM('1'):
      applyPinZoom(pw, 1.0, false)
      return 0
    elif wp == VK_SPACE:
      pw.showBorder = not pw.showBorder
      InvalidateRect(hwnd, nil, FALSE)
      return 0
    return 0

  of WM_ERASEBKGND:
    return 1

  else:
    return DefWindowProcW(hwnd, msg, wp, lp)

proc createPinWindow(screenX, screenY, w, h: cint, bitsIn: pointer): PinWin =
  if bitsIn == nil or w <= 0 or h <= 0: return nil
  if not g_pinClassReg:
    var wc: WNDCLASSEXW
    wc.cbSize = UINT(sizeof(WNDCLASSEXW))
    wc.style = CS_HREDRAW or CS_VREDRAW or CS_DBLCLKS or CS_DROPSHADOW
    wc.lpfnWndProc = pinWndProc
    wc.hInstance = GetModuleHandleW(nil)
    wc.hCursor = LoadCursorW(0, IDC_SIZEALL)
    wc.lpszClassName = newWideCString("KuaiYiPinWindow")
    discard RegisterClassExW(addr wc)
    g_pinClassReg = true

  let hdcScr = GetDC(0)
  var bits: pointer = nil
  let hbm = makeDib32(hdcScr, w, h, addr bits)
  ReleaseDC(0, hdcScr)
  if hbm == 0 or bits == nil:
    if hbm != 0: DeleteObject(hbm)
    return nil
  copyMem(bits, bitsIn, int(w) * int(h) * 4)

  var pt = POINT(x: LONG(screenX + w div 2), y: LONG(screenY + h div 2))
  let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)
  var mi: MONITORINFO
  mi.cbSize = DWORD(sizeof(MONITORINFO))
  discard GetMonitorInfoW(hmon, addr mi)
  var wx = int(screenX)
  var wy = int(screenY)
  let dispW = min(int(w), max(120, int(mi.rcWork.right - mi.rcWork.left) - 40))
  let dispH = min(int(h), max(80, int(mi.rcWork.bottom - mi.rcWork.top) - 40))
  let scale = min(float(dispW) / float(w), float(dispH) / float(h))
  let initW = cint(max(1, int(round(float(w) * scale))))
  let initH = cint(max(1, int(round(float(h) * scale))))
  if wx + int(initW) > int(mi.rcWork.right): wx = int(mi.rcWork.right) - int(initW)
  if wy + int(initH) > int(mi.rcWork.bottom): wy = int(mi.rcWork.bottom) - int(initH)
  if wx < int(mi.rcWork.left): wx = int(mi.rcWork.left)
  if wy < int(mi.rcWork.top): wy = int(mi.rcWork.top)

  let pw = PinWin(
    hbm: hbm,
    bits: bits,
    origW: w,
    origH: h,
    dispW: initW,
    dispH: initH,
    zoomScale: scale,
    alpha: 255'u8,
    showBorder: true,
    hudMsg: ""
  )
  let hwnd = CreateWindowExW(
    WS_EX_TOPMOST or WS_EX_TOOLWINDOW,
    newWideCString("KuaiYiPinWindow"),
    newWideCString("快译NG 贴图"),
    WS_POPUP or WS_VISIBLE,
    cint(wx), cint(wy), initW, initH,
    0, 0, GetModuleHandleW(nil), nil
  )
  if hwnd == 0:
    DeleteObject(hbm)
    return nil
  pw.hwnd = hwnd
  g_pins[hwnd] = pw
  SetForegroundWindow(hwnd)
  SetFocus(hwnd)
  result = pw

proc createTextPinAtCursor(txt: string) =
  let clean = txt.strip()
  if clean.len == 0: return
  let hdcScr = GetDC(0)
  let hdcMem = CreateCompatibleDC(hdcScr)
  let hf = CreateFontW(-16, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                       0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldF = SelectObject(hdcMem, hf)
  let wTxt = newWideCString(clean)

  var calcRc = RECT(left: 0, top: 0, right: 440, bottom: 0)
  discard DrawTextW(hdcMem, cast[LPCWSTR](wTxt), -1, addr calcRc,
                    DT_CALCRECT or DT_WORDBREAK or DT_NOPREFIX)
  let padX = 18
  let padY = 14
  let w = cint(clamp(int(calcRc.right) + padX * 2, 120, 480))
  let h = cint(clamp(int(calcRc.bottom) + padY * 2, 48, 720))

  var bits: pointer = nil
  let hbm = makeDib32(hdcScr, w, h, addr bits)
  ReleaseDC(0, hdcScr)
  if hbm != 0 and bits != nil:
    let oldBm = SelectObject(hdcMem, hbm)
    var fullRc = RECT(left: 0, top: 0, right: LONG(w), bottom: LONG(h))
    let bg = CreateSolidBrush(RGB(28, 32, 40))
    FillRect(hdcMem, addr fullRc, bg)
    DeleteObject(bg)

    SetBkMode(hdcMem, TRANSPARENT)
    SetTextColor(hdcMem, RGB(238, 244, 252))
    var txtRc = RECT(left: LONG(padX), top: LONG(padY), right: LONG(w - padX), bottom: LONG(h - padY))
    DrawTextW(hdcMem, cast[LPCWSTR](wTxt), -1, addr txtRc,
              DT_LEFT or DT_TOP or DT_WORDBREAK or DT_NOPREFIX)
    GdiFlush()
    SelectObject(hdcMem, oldBm)

    let imgBytes = int(w) * int(h) * 4
    let pOut = cast[ptr UncheckedArray[uint8]](bits)
    var k = 3
    while k < imgBytes:
      pOut[k] = 255'u8
      k += 4

    var pt: POINT
    discard GetCursorPos(addr pt)
    discard createPinWindow(cint(pt.x - int(w) div 2), cint(pt.y - int(h) div 2), w, h, bits)

  SelectObject(hdcMem, oldF)
  DeleteObject(hf)
  if hbm != 0: DeleteObject(hbm)
  DeleteDC(hdcMem)

# ---------------- 结果悬浮窗 ----------------
proc layoutResultButtons() =
  let y0 = g_dispH + 5
  let bh = ToolbarH - 10
  let pad = 8
  let labels = [
    (BtnToggle, if g_showOrig: "显示译图" else: "对照原图", 70),
    (BtnCopyText, "复制译文", 66),
    (BtnCopyImg, "复制图片", 66),
    (BtnPin, "贴图", 48),
    (BtnResnip, "重选", 48),
    (BtnClose, "关闭", 48)
  ]
  var x = g_winW - pad
  for i in countdown(labels.high, 0):
    let w = labels[i][2]
    x -= w
    g_btns[i] = ToolBtn(
      rect: RECT(left: LONG(x), top: LONG(y0), right: LONG(x + w), bottom: LONG(y0 + bh)),
      labelText: labels[i][1],
      id: labels[i][0]
    )
    x -= 5

proc applyResultZoom(hwnd: HWND, targetScale: float, anchorMouse: bool) =
  if hwnd == 0 or g_resW <= 0 or g_resH <= 0: return
  let minBySize = max(80.0 / float(g_resW), 40.0 / float(g_resH))
  let minScale = min(1.0, max(0.15, minBySize))
  let maxBySize = min(4096.0 / float(g_resW), 4096.0 / float(g_resH))
  let maxScale = max(1.0, min(5.0, maxBySize))
  let newScale = clamp(targetScale, minScale, maxScale)

  let newDispW = max(1, int(round(float(g_resW) * newScale)))
  let newDispH = max(1, int(round(float(g_resH) * newScale)))
  let newWinW = max(newDispW, MinWinW)
  let newWinH = newDispH + ToolbarH

  var wr: RECT
  discard GetWindowRect(hwnd, addr wr)
  var newLeft = int(wr.left)
  var newTop = int(wr.top)

  if anchorMouse:
    var pt: POINT
    discard GetCursorPos(addr pt)
    let oldImgX = (int(g_winW) - int(g_dispW)) div 2
    let rx = clamp(float(pt.x - int(wr.left) - oldImgX) / float(max(1, int(g_dispW))), 0.0, 1.0)
    let ry = clamp(float(pt.y - int(wr.top)) / float(max(1, int(g_dispH))), 0.0, 1.0)
    let newImgX = (newWinW - newDispW) div 2
    newLeft = int(pt.x) - newImgX - int(round(rx * float(newDispW)))
    newTop = int(pt.y) - int(round(ry * float(newDispH)))
  else:
    let cx = int(wr.left) + int(g_winW) div 2
    let cy = int(wr.top) + int(g_winH) div 2
    newLeft = cx - newWinW div 2
    newTop = cy - newWinH div 2

  g_zoomScale = newScale
  g_dispW = cint(newDispW)
  g_dispH = cint(newDispH)
  g_winW = cint(newWinW)
  g_winH = cint(newWinH)
  layoutResultButtons()
  if not g_translating:
    let pct = int(round(newScale * 100.0))
    g_statusMsg = if pct == 100: "完成(空格切原图)"
                  else: "缩放 " & $pct & "%"
  SetWindowPos(hwnd, 0, cint(newLeft), cint(newTop), g_winW, g_winH,
               SWP_NOZORDER or SWP_NOACTIVATE)
  InvalidateRect(hwnd, nil, FALSE)

proc openResultWindow(screenX, screenY, w, h: cint, cropBits: pointer) =
  closeResultWindow()
  inc g_snipSeq
  g_resW = w
  g_resH = h
  g_showOrig = false
  g_translating = true
  g_zhText = ""
  g_statusMsg = "正在识别翻译…"
  g_hoverBtn = 0

  var pt: POINT
  pt.x = LONG(screenX + w div 2)
  pt.y = LONG(screenY + h div 2)
  let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST)
  var mi: MONITORINFO
  mi.cbSize = DWORD(sizeof(MONITORINFO))
  discard GetMonitorInfoW(hmon, addr mi)
  let workW = int(mi.rcWork.right - mi.rcWork.left)
  let workH = int(mi.rcWork.bottom - mi.rcWork.top)
  let maxW = int(float(workW) * 0.82)
  let maxH = int(float(workH) * 0.82) - ToolbarH

  var dw = int(w)
  var dh = int(h)
  if dw > maxW or dh > maxH:
    let scale = min(float(maxW) / float(dw), float(maxH) / float(dh))
    dw = max(1, int(float(dw) * scale))
    dh = max(1, int(float(dh) * scale))
  g_dispW = cint(dw)
  g_dispH = cint(dh)
  g_zoomScale = float(dw) / float(max(1, int(w)))
  g_winW = cint(max(dw, MinWinW))
  g_winH = cint(dh + ToolbarH)

  let hdcScr = GetDC(0)
  g_hbmResOrig = makeDib32(hdcScr, w, h, addr g_bitsResOrig)
  g_hbmResTrans = makeDib32(hdcScr, w, h, addr g_bitsResTrans)
  ReleaseDC(0, hdcScr)
  let imgBytes = int(w) * int(h) * 4
  if g_bitsResOrig != nil: copyMem(g_bitsResOrig, cropBits, imgBytes)
  if g_bitsResTrans != nil: copyMem(g_bitsResTrans, cropBits, imgBytes)

  layoutResultButtons()

  var wx = int(screenX)
  var wy = int(screenY)
  if wx + int(g_winW) > int(mi.rcWork.right): wx = int(mi.rcWork.right) - int(g_winW)
  if wy + int(g_winH) > int(mi.rcWork.bottom): wy = int(mi.rcWork.bottom) - int(g_winH)
  if wx < int(mi.rcWork.left): wx = int(mi.rcWork.left)
  if wy < int(mi.rcWork.top): wy = int(mi.rcWork.top)

  if not g_resultClassReg:
    var wc: WNDCLASSEXW
    wc.cbSize = UINT(sizeof(WNDCLASSEXW))
    wc.style = CS_HREDRAW or CS_VREDRAW
    wc.lpfnWndProc = proc (hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM): LRESULT {.stdcall.} =
      case msg
      of WM_PAINT:
        var ps: PAINTSTRUCT
        let hdc = BeginPaint(hwnd, addr ps)
        let hdcBuf = CreateCompatibleDC(hdc)
        let hbmBuf = CreateCompatibleBitmap(hdc, g_winW, g_winH)
        let oldBuf = SelectObject(hdcBuf, hbmBuf)

        var topRc = RECT(left: 0, top: 0, right: LONG(g_winW), bottom: LONG(g_dispH))
        let bgImg = CreateSolidBrush(RGB(24, 26, 32))
        FillRect(hdcBuf, addr topRc, bgImg)
        DeleteObject(bgImg)

        let srcBm = if g_showOrig or g_hbmResTrans == 0: g_hbmResOrig else: g_hbmResTrans
        if srcBm != 0:
          let hdcSrc = CreateCompatibleDC(hdc)
          let oldSrc = SelectObject(hdcSrc, srcBm)
          let imgX = (g_winW - g_dispW) div 2
          if g_dispW == g_resW and g_dispH == g_resH:
            BitBlt(hdcBuf, imgX, 0, g_dispW, g_dispH, hdcSrc, 0, 0, SRCCOPY)
          else:
            SetStretchBltMode(hdcBuf, HALFTONE)
            SetBrushOrgEx(hdcBuf, 0, 0, nil)
            StretchBlt(hdcBuf, imgX, 0, g_dispW, g_dispH, hdcSrc, 0, 0, g_resW, g_resH, SRCCOPY)
          SelectObject(hdcSrc, oldSrc)
          DeleteDC(hdcSrc)

        var barRc = RECT(left: 0, top: LONG(g_dispH), right: LONG(g_winW), bottom: LONG(g_winH))
        let bgBar = CreateSolidBrush(RGB(34, 38, 46))
        FillRect(hdcBuf, addr barRc, bgBar)
        DeleteObject(bgBar)

        let hf = CreateFontW(-13, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                             0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
        let oldF = SelectObject(hdcBuf, hf)
        SetBkMode(hdcBuf, TRANSPARENT)

        if g_statusMsg.len > 0:
          SetTextColor(hdcBuf, RGB(175, 195, 225))
          var stRc = RECT(left: 10, top: LONG(g_dispH), right: g_btns[0].rect.left - 6, bottom: LONG(g_winH))
          DrawTextW(hdcBuf, newWideCString(g_statusMsg), -1, addr stRc,
                    DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_END_ELLIPSIS or DT_NOPREFIX)

        for i in 0 .. g_btns.high:
          let b = g_btns[i]
          let isHover = (g_hoverBtn == b.id)
          let col = if b.id == BtnClose and isHover: RGB(200, 62, 62)
                    elif isHover: RGB(59, 130, 246)
                    else: RGB(54, 60, 72)
          let br = CreateSolidBrush(col)
          var r = b.rect
          FillRect(hdcBuf, addr r, br)
          DeleteObject(br)
          SetTextColor(hdcBuf, RGB(240, 244, 250))
          DrawTextW(hdcBuf, newWideCString(b.labelText), -1, addr r,
                    DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)

        SelectObject(hdcBuf, oldF)
        DeleteObject(hf)

        let pen = CreatePen(PS_SOLID, 1, RGB(68, 76, 92))
        let oldPen = SelectObject(hdcBuf, pen)
        let oldBr = SelectObject(hdcBuf, GetStockObject(NULL_BRUSH))
        Rectangle(hdcBuf, 0, 0, g_winW, g_winH)
        SelectObject(hdcBuf, oldBr)
        SelectObject(hdcBuf, oldPen)
        DeleteObject(pen)

        BitBlt(hdc, 0, 0, g_winW, g_winH, hdcBuf, 0, 0, SRCCOPY)
        SelectObject(hdcBuf, oldBuf)
        DeleteObject(hbmBuf)
        DeleteDC(hdcBuf)
        EndPaint(hwnd, addr ps)
        return 0

      of WM_MOUSEMOVE:
        let mx = int(cast[int16](lp and 0xFFFF))
        let my = int(cast[int16]((lp shr 16) and 0xFFFF))
        var nh = 0
        for i in 0 .. g_btns.high:
          let r = g_btns[i].rect
          if mx >= r.left and mx < r.right and my >= r.top and my < r.bottom:
            nh = g_btns[i].id
            break
        if nh != g_hoverBtn:
          g_hoverBtn = nh
          InvalidateRect(hwnd, nil, FALSE)
        return 0

      of WM_LBUTTONDOWN:
        let mx = int(cast[int16](lp and 0xFFFF))
        let my = int(cast[int16]((lp shr 16) and 0xFFFF))
        for i in 0 .. g_btns.high:
          let r = g_btns[i].rect
          if mx >= r.left and mx < r.right and my >= r.top and my < r.bottom:
            case g_btns[i].id
            of BtnToggle:
              g_showOrig = not g_showOrig
              layoutResultButtons()
              InvalidateRect(hwnd, nil, FALSE)
            of BtnCopyText:
              if g_zhText.len > 0 and g_copyTextCb != nil:
                g_copyTextCb(g_zhText)
                g_statusMsg = "已复制译文"
                InvalidateRect(hwnd, nil, FALSE)
            of BtnCopyImg:
              let bits = if g_showOrig or g_bitsResTrans == nil: g_bitsResOrig else: g_bitsResTrans
              if copyDibToClipboard(hwnd, bits, g_resW, g_resH):
                g_statusMsg = "已复制图片"
                InvalidateRect(hwnd, nil, FALSE)
            of BtnPin:
              let bits = if g_showOrig or g_bitsResTrans == nil: g_bitsResOrig else: g_bitsResTrans
              if bits != nil:
                var wr: RECT
                discard GetWindowRect(hwnd, addr wr)
                let pw = g_resW
                let ph = g_resH
                let sz = int(pw) * int(ph) * 4
                let tmp = c_malloc(csize_t(sz))
                if tmp != nil:
                  copyMem(tmp, bits, sz)
                  closeResultWindow()
                  discard createPinWindow(cint(wr.left), cint(wr.top), pw, ph, tmp)
                  c_free(tmp)
            of BtnResnip:
              closeResultWindow()
              startSnipCapture(g_hwndMain, g_lastCfg, g_copyTextCb,
                               directTranslate = true, notifyCb = g_notifyCb)
            of BtnClose:
              closeResultWindow()
            else: discard
            return 0
        if my < g_dispH or g_hoverBtn == 0:
          ReleaseCapture()
          SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0)
        return 0

      of WM_MOUSEWHEEL:
        let delta = int(cast[int16]((wp shr 16) and 0xFFFF))
        if delta != 0:
          let steps = float(delta) / 120.0
          applyResultZoom(hwnd, g_zoomScale * pow(1.1, steps), true)
        return 0

      of WM_KEYDOWN:
        let ctrlDown = (GetKeyState(VK_CONTROL) and 0x8000'i16) != 0
        let shiftDown = (GetKeyState(VK_SHIFT) and 0x8000'i16) != 0
        if wp == VK_ESCAPE:
          closeResultWindow()
          return 0
        elif ctrlDown and wp == WPARAM('C'):
          let bits = if g_showOrig or g_bitsResTrans == nil: g_bitsResOrig else: g_bitsResTrans
          if copyDibToClipboard(hwnd, bits, g_resW, g_resH):
            g_statusMsg = "已复制图片"
            InvalidateRect(hwnd, nil, FALSE)
          return 0
        elif ctrlDown and wp == WPARAM('S'):
          let bits = if g_showOrig or g_bitsResTrans == nil: g_bitsResOrig else: g_bitsResTrans
          if shiftDown:
            var savedPath = ""
            if saveBitsAsPngDefault(bits, g_resW, g_resH, savedPath):
              g_statusMsg = "已保存到默认目录"
            else:
              g_statusMsg = "保存失败"
            InvalidateRect(hwnd, nil, FALSE)
          else:
            if saveBitsAsPngDialog(hwnd, bits, g_resW, g_resH):
              g_statusMsg = "已保存图片"
              InvalidateRect(hwnd, nil, FALSE)
          return 0
        elif wp == VK_SPACE:
          g_showOrig = not g_showOrig
          layoutResultButtons()
          InvalidateRect(hwnd, nil, FALSE)
          return 0
        elif wp == WPARAM('0') or wp == WPARAM('1'):
          applyResultZoom(hwnd, 1.0, false)
          return 0
        return 0

      of WM_ERASEBKGND:
        return 1
      else:
        return DefWindowProcW(hwnd, msg, wp, lp)
    wc.hInstance = GetModuleHandleW(nil)
    wc.hCursor = LoadCursorW(0, IDC_ARROW)
    wc.lpszClassName = newWideCString("KuaiYiSnipResult")
    discard RegisterClassExW(addr wc)
    g_resultClassReg = true

  g_hwndResult = CreateWindowExW(
    WS_EX_TOPMOST or WS_EX_TOOLWINDOW,
    newWideCString("KuaiYiSnipResult"),
    newWideCString("快译NG 截屏翻译"),
    WS_POPUP or WS_VISIBLE,
    cint(wx), cint(wy), g_winW, g_winH,
    0, 0, GetModuleHandleW(nil), nil
  )
  SetForegroundWindow(g_hwndResult)
  SetFocus(g_hwndResult)
  syncEscHotkey()

  let job = cast[ptr SnipJob](alloc0(sizeof(SnipJob)))
  job.seq = g_snipSeq
  job.hwndMain = g_hwndMain
  let bgraCopy = c_malloc(csize_t(imgBytes))
  copyMem(bgraCopy, cropBits, imgBytes)
  job.bgra = bgraCopy
  job.w = w
  job.h = h
  job.aiUrl = fy_strdup(g_lastCfg.aiUrl.cstring)
  job.aiKey = fy_strdup(g_lastCfg.aiKey.cstring)
  job.aiModel = fy_strdup(g_lastCfg.aiModel.cstring)
  job.baiduAppId = fy_strdup(g_lastCfg.baiduAppId.cstring)
  job.baiduKey = fy_strdup(g_lastCfg.baiduKey.cstring)
  job.baiduUrl = fy_strdup(g_lastCfg.baiduUrl.cstring)
  job.cacheDir = fy_strdup(g_lastCfg.cacheDir.cstring)
  discard CreateThread(nil, 0, snipWorker, cast[LPVOID](job), 0, nil)

proc onSnipDone*(seq: int, res: ptr FySnipResult) =
  if res == nil: return
  defer: fy_snip_result_free(res)
  if seq != g_snipSeq or g_hwndResult == 0: return
  g_translating = false
  if res.ok != 0 and res.bgra_out != nil and res.width == g_resW and res.height == g_resH:
    let imgBytes = int(g_resW) * int(g_resH) * 4
    if g_bitsResTrans != nil:
      copyMem(g_bitsResTrans, res.bgra_out, imgBytes)
    g_zhText = if res.zh_text != nil: $res.zh_text else: ""
    g_statusMsg = "完成(空格切原图)"
    g_showOrig = false
  else:
    g_statusMsg = if res.err != nil: $res.err else: "未识别到英文文本"
  layoutResultButtons()
  InvalidateRect(g_hwndResult, nil, FALSE)

# ---------------- 二期标注渲染引擎（遮罩预览与导出共用） ----------------
proc distSqToSeg(px, py, x1, y1, x2, y2: float): float {.inline.} =
  let dx = x2 - x1
  let dy = y2 - y1
  let lenSq = dx * dx + dy * dy
  if lenSq <= 0.001:
    let ex = px - x1
    let ey = py - y1
    return ex * ex + ey * ey
  let t = clamp(((px - x1) * dx + (py - y1) * dy) / lenSq, 0.0, 1.0)
  let projX = x1 + t * dx
  let projY = y1 + t * dy
  let ex = px - projX
  let ey = py - projY
  ex * ex + ey * ey

proc pointNearPolyline(px, py: float, pts: seq[POINT], radSq: float): bool =
  if pts.len == 0: return false
  if pts.len == 1:
    let dx = px - float(pts[0].x)
    let dy = py - float(pts[0].y)
    return dx * dx + dy * dy <= radSq
  for i in 0 ..< pts.high:
    if distSqToSeg(px, py, float(pts[i].x), float(pts[i].y),
                   float(pts[i + 1].x), float(pts[i + 1].y)) <= radSq:
      return true
  result = false

proc polylineBounds(pts: seq[POINT], pad: int): RECT =
  if pts.len == 0: return RECT(left: 0, top: 0, right: 0, bottom: 0)
  var minX = int(pts[0].x)
  var maxX = minX
  var minY = int(pts[0].y)
  var maxY = minY
  for p in pts:
    if int(p.x) < minX: minX = int(p.x)
    if int(p.x) > maxX: maxX = int(p.x)
    if int(p.y) < minY: minY = int(p.y)
    if int(p.y) > maxY: maxY = int(p.y)
  RECT(left: LONG(minX - pad), top: LONG(minY - pad),
       right: LONG(maxX + pad + 1), bottom: LONG(maxY + pad + 1))

proc applyMosaicStroke(bitsDst: pointer, bufW, bufH: cint,
                       clipOvRc: RECT, offX, offY: int, item: AnnotItem) =
  if g_bitsOrig == nil or bitsDst == nil or item.pts.len == 0: return
  let blk = MosaicBlocks[clamp(item.sizeIdx, 0, 2)]
  let rad = MosaicRadii[clamp(item.sizeIdx, 0, 2)]
  let radSq = float(rad * rad)
  let bb = polylineBounds(item.pts, rad + blk)
  let l = max(int(clipOvRc.left), int(bb.left))
  let t = max(int(clipOvRc.top), int(bb.top))
  let r = min(int(clipOvRc.right), int(bb.right))
  let b = min(int(clipOvRc.bottom), int(bb.bottom))
  if r <= l or b <= t: return

  let src = cast[ptr UncheckedArray[uint8]](g_bitsOrig)
  let dst = cast[ptr UncheckedArray[uint8]](bitsDst)
  let gx0 = (l div blk) * blk
  let gy0 = (t div blk) * blk

  var gy = gy0
  while gy < b:
    var gx = gx0
    while gx < r:
      let cx = float(gx) + float(blk) * 0.5
      let cy = float(gy) + float(blk) * 0.5
      if pointNearPolyline(cx, cy, item.pts, radSq):
        let cl = max(l, gx)
        let ct = max(t, gy)
        let cr = min(r, gx + blk)
        let cb = min(b, gy + blk)
        if cr > cl and cb > ct:
          var sumB, sumG, sumR, cnt: uint32 = 0
          let step = max(1, blk div 3)
          var sy = ct
          while sy < cb:
            let oy = clamp(sy, 0, int(g_vh) - 1)
            var sx = cl
            while sx < cr:
              let ox = clamp(sx, 0, int(g_vw) - 1)
              let soff = (oy * int(g_vw) + ox) * 4
              sumB += uint32(src[soff])
              sumG += uint32(src[soff + 1])
              sumR += uint32(src[soff + 2])
              inc cnt
              sx += step
            sy += step
          if cnt > 0'u32:
            let avgB = uint8(sumB div cnt)
            let avgG = uint8(sumG div cnt)
            let avgR = uint8(sumR div cnt)
            for py in ct ..< cb:
              let dy = py - offY
              if dy < 0 or dy >= int(bufH): continue
              for px in cl ..< cr:
                let dx = px - offX
                if dx < 0 or dx >= int(bufW): continue
                let doff = (dy * int(bufW) + dx) * 4
                dst[doff] = avgB
                dst[doff + 1] = avgG
                dst[doff + 2] = avgR
                dst[doff + 3] = 255'u8
      gx += blk
    gy += blk

proc applyBlurStroke(bitsDst: pointer, bufW, bufH: cint,
                     clipOvRc: RECT, offX, offY: int, item: AnnotItem) =
  if g_bitsOrig == nil or bitsDst == nil or item.pts.len == 0: return
  let kr = BlurRadii[clamp(item.sizeIdx, 0, 2)]
  let rad = BlurBrushRadii[clamp(item.sizeIdx, 0, 2)]
  let radSq = float(rad * rad)
  let bb = polylineBounds(item.pts, rad)
  let l = max(int(clipOvRc.left), int(bb.left))
  let t = max(int(clipOvRc.top), int(bb.top))
  let r = min(int(clipOvRc.right), int(bb.right))
  let b = min(int(clipOvRc.bottom), int(bb.bottom))
  if r <= l or b <= t: return

  let src = cast[ptr UncheckedArray[uint8]](g_bitsOrig)
  let dst = cast[ptr UncheckedArray[uint8]](bitsDst)
  let step = max(2, kr div 2)

  for py in t ..< b:
    let dy = py - offY
    if dy < 0 or dy >= int(bufH): continue
    for px in l ..< r:
      let dx = px - offX
      if dx < 0 or dx >= int(bufW): continue
      if pointNearPolyline(float(px), float(py), item.pts, radSq):
        var sumB, sumG, sumR, cnt: uint32 = 0
        var ky = -kr
        while ky <= kr:
          let oy = clamp(py + ky, 0, int(g_vh) - 1)
          var kx = -kr
          while kx <= kr:
            let ox = clamp(px + kx, 0, int(g_vw) - 1)
            let soff = (oy * int(g_vw) + ox) * 4
            sumB += uint32(src[soff])
            sumG += uint32(src[soff + 1])
            sumR += uint32(src[soff + 2])
            inc cnt
            kx += step
          ky += step
        if cnt > 0'u32:
          let doff = (dy * int(bufW) + dx) * 4
          dst[doff] = uint8(sumB div cnt)
          dst[doff + 1] = uint8(sumG div cnt)
          dst[doff + 2] = uint8(sumR div cnt)
          dst[doff + 3] = 255'u8

proc drawVectorAnnotation(hdc: HDC, offX, offY: int, item: AnnotItem) =
  ensureGdiplus()
  if not g_gpReady: return

  var g: pointer = nil
  if GdipCreateFromHDC(hdc, addr g) != 0 or g == nil: return
  discard GdipSetSmoothingMode(g, SmoothingAntiAlias)

  let sw = StrokeSizes[clamp(item.sizeIdx, 0, 2)]
  let col = colorArgb(item.colorIdx, 255'u8)

  case item.tool
  of atRect:
    let rc = normRect(item.p1, item.p2)
    let x = cfloat(int(rc.left) - offX)
    let y = cfloat(int(rc.top) - offY)
    let w = cfloat(rc.right - rc.left)
    let h = cfloat(rc.bottom - rc.top)
    if w >= 1.0'f32 and h >= 1.0'f32:
      if item.filled:
        var br: pointer = nil
        if GdipCreateSolidFill(col, addr br) == 0 and br != nil:
          discard GdipFillRectangle(g, br, x, y, w, h)
          discard GdipDeleteBrush(br)
      else:
        var pen: pointer = nil
        if GdipCreatePen1(col, sw, 2, addr pen) == 0 and pen != nil:
          discard GdipSetPenLineJoin(pen, LineJoinRound)
          discard GdipDrawRectangle(g, pen, x, y, w, h)
          discard GdipDeletePen(pen)

  of atEllipse:
    let rc = normRect(item.p1, item.p2)
    let x = cfloat(int(rc.left) - offX)
    let y = cfloat(int(rc.top) - offY)
    let w = cfloat(rc.right - rc.left)
    let h = cfloat(rc.bottom - rc.top)
    if w >= 1.0'f32 and h >= 1.0'f32:
      if item.filled:
        var br: pointer = nil
        if GdipCreateSolidFill(col, addr br) == 0 and br != nil:
          discard GdipFillEllipse(g, br, x, y, w, h)
          discard GdipDeleteBrush(br)
      else:
        var pen: pointer = nil
        if GdipCreatePen1(col, sw, 2, addr pen) == 0 and pen != nil:
          discard GdipDrawEllipse(g, pen, x, y, w, h)
          discard GdipDeletePen(pen)

  of atLine:
    var pen: pointer = nil
    if GdipCreatePen1(col, sw, 2, addr pen) == 0 and pen != nil:
      discard GdipSetPenStartCap(pen, LineCapRound)
      discard GdipSetPenEndCap(pen, LineCapRound)
      discard GdipDrawLine(g, pen,
                           cfloat(int(item.p1.x) - offX), cfloat(int(item.p1.y) - offY),
                           cfloat(int(item.p2.x) - offX), cfloat(int(item.p2.y) - offY))
      discard GdipDeletePen(pen)

  of atArrow:
    let x1 = float(int(item.p1.x) - offX)
    let y1 = float(int(item.p1.y) - offY)
    let x2 = float(int(item.p2.x) - offX)
    let y2 = float(int(item.p2.y) - offY)
    let dx = x2 - x1
    let dy = y2 - y1
    let L = hypot(dx, dy)
    if L >= 2.0:
      let ux = dx / L
      let uy = dy / L
      let nx = -uy
      let ny = ux
      let hl = min(L * 0.48, max(12.0, float(sw) * 3.8))
      let hw = hl * 0.48
      let nw = max(1.0, float(sw) * 0.55)
      let tw = max(0.4, float(sw) * 0.16)
      let bx = x2 - ux * hl
      let by = y2 - uy * hl
      var poly: array[7, GpPointF] = [
        GpPointF(x: cfloat(x1 + nx * tw), y: cfloat(y1 + ny * tw)),
        GpPointF(x: cfloat(bx + nx * nw), y: cfloat(by + ny * nw)),
        GpPointF(x: cfloat(bx + nx * hw), y: cfloat(by + ny * hw)),
        GpPointF(x: cfloat(x2), y: cfloat(y2)),
        GpPointF(x: cfloat(bx - nx * hw), y: cfloat(by - ny * hw)),
        GpPointF(x: cfloat(bx - nx * nw), y: cfloat(by - ny * nw)),
        GpPointF(x: cfloat(x1 - nx * tw), y: cfloat(y1 - ny * tw))
      ]
      var br: pointer = nil
      if GdipCreateSolidFill(col, addr br) == 0 and br != nil:
        discard GdipFillPolygon(g, br, addr poly[0], 7, FillModeAlternate)
        discard GdipDeleteBrush(br)

  of atPen, atMarker:
    if item.pts.len >= 2:
      let width = if item.tool == atMarker: HighlighterSizes[clamp(item.sizeIdx, 0, 2)] else: sw
      let pCol = if item.tool == atMarker: colorArgb(item.colorIdx, 105'u8) else: col
      var pts = newSeq[GpPointF](item.pts.len)
      for i, pt in item.pts:
        pts[i] = GpPointF(x: cfloat(int(pt.x) - offX), y: cfloat(int(pt.y) - offY))
      var pen: pointer = nil
      if GdipCreatePen1(pCol, width, 2, addr pen) == 0 and pen != nil:
        discard GdipSetPenStartCap(pen, LineCapRound)
        discard GdipSetPenEndCap(pen, LineCapRound)
        discard GdipSetPenLineJoin(pen, LineJoinRound)
        discard GdipDrawLines(g, pen, addr pts[0], cint(pts.len))
        discard GdipDeletePen(pen)

  of atNumber:
    let cx = float(int(item.p1.x) - offX)
    let cy = float(int(item.p1.y) - offY)
    let tx = float(int(item.p2.x) - offX)
    let ty = float(int(item.p2.y) - offY)
    let r = NumberRadii[clamp(item.sizeIdx, 0, 2)]
    let dx = tx - cx
    let dy = ty - cy
    let L = hypot(dx, dy)
    var br: pointer = nil
    var penW: pointer = nil
    discard GdipCreateSolidFill(col, addr br)
    discard GdipCreatePen1(argb(255, 255, 255), 1.6'f32, 2, addr penW)
    if L > r + 4.0 and br != nil:
      let ux = dx / L
      let uy = dy / L
      let nx = -uy
      let ny = ux
      let s = r * 0.55
      var tri: array[3, GpPointF] = [
        GpPointF(x: cfloat(cx + nx * s), y: cfloat(cy + ny * s)),
        GpPointF(x: cfloat(tx), y: cfloat(ty)),
        GpPointF(x: cfloat(cx - nx * s), y: cfloat(cy - ny * s))
      ]
      discard GdipFillPolygon(g, br, addr tri[0], 3, FillModeAlternate)
      if penW != nil:
        discard GdipDrawPolygon(g, penW, addr tri[0], 3)
    if br != nil:
      discard GdipFillEllipse(g, br, cfloat(cx - r), cfloat(cy - r), cfloat(r * 2.0), cfloat(r * 2.0))
      discard GdipDeleteBrush(br)
    if penW != nil:
      discard GdipDrawEllipse(g, penW, cfloat(cx - r), cfloat(cy - r), cfloat(r * 2.0), cfloat(r * 2.0))
      discard GdipDeletePen(penW)

    discard GdipDeleteGraphics(g)
    g = nil

    # GDI 绘制清晰居中数字
    let fontPx = cint(round(r * 1.18))
    let hf = CreateFontW(-fontPx, 0, 0, 0, FW_BOLD, 0, 0, 0, DEFAULT_CHARSET,
                         0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
    let oldF = SelectObject(hdc, hf)
    SetBkMode(hdc, TRANSPARENT)
    # 白色或明黄背景用深字，其余用白字
    if item.colorIdx == 7 or item.colorIdx == 2:
      SetTextColor(hdc, RGB(24, 28, 36))
    else:
      SetTextColor(hdc, RGB(255, 255, 255))
    var nrc = RECT(left: LONG(round(cx - r)), top: LONG(round(cy - r)),
                   right: LONG(round(cx + r)), bottom: LONG(round(cy + r)))
    DrawTextW(hdc, newWideCString($item.numVal), -1, addr nrc,
              DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    SelectObject(hdc, oldF)
    DeleteObject(hf)

  of atText:
    discard GdipDeleteGraphics(g)
    g = nil
    if item.text.len > 0:
      let fpx = cint(TextFontSizes[clamp(item.sizeIdx, 0, 2)])
      let hf = CreateFontW(-fpx, 0, 0, 0, FW_MEDIUM, 0, 0, 0, DEFAULT_CHARSET,
                           0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
      let oldF = SelectObject(hdc, hf)
      SetBkMode(hdc, TRANSPARENT)
      let c = AnnotColors[clamp(item.colorIdx, 0, AnnotColors.high)]
      let x = LONG(int(item.p1.x) - offX)
      let y = LONG(int(item.p1.y) - offY)
      let wTxt = newWideCString(item.text)
      # 轻微 1px 阴影增强可读性
      var shRc = RECT(left: x + 1, top: y + 1, right: x + 2000, bottom: y + 2000)
      SetTextColor(hdc, if item.colorIdx == 7: RGB(0, 0, 0) else: RGB(18, 22, 28))
      DrawTextW(hdc, cast[LPCWSTR](wTxt), -1, addr shRc, DT_LEFT or DT_TOP or DT_NOPREFIX)
      var txRc = RECT(left: x, top: y, right: x + 2000, bottom: y + 2000)
      SetTextColor(hdc, RGB(c[0], c[1], c[2]))
      DrawTextW(hdc, cast[LPCWSTR](wTxt), -1, addr txRc, DT_LEFT or DT_TOP or DT_NOPREFIX)
      SelectObject(hdc, oldF)
      DeleteObject(hf)

  else: discard

  if g != nil:
    discard GdipDeleteGraphics(g)

proc renderAllAnnotations(hdc: HDC, bitsDst: pointer, bufW, bufH: cint,
                          clipOvRc: RECT, offX, offY: int, includeCurrent: bool) =
  if g_annots.len == 0 and (not includeCurrent or g_curAnnot.tool == atNone): return
  discard SaveDC(hdc)
  discard IntersectClipRect(hdc,
                            cint(int(clipOvRc.left) - offX), cint(int(clipOvRc.top) - offY),
                            cint(int(clipOvRc.right) - offX), cint(int(clipOvRc.bottom) - offY))

  for item in g_annots:
    if item.tool == atMosaic:
      GdiFlush()
      applyMosaicStroke(bitsDst, bufW, bufH, clipOvRc, offX, offY, item)
    elif item.tool == atBlur:
      GdiFlush()
      applyBlurStroke(bitsDst, bufW, bufH, clipOvRc, offX, offY, item)
    else:
      drawVectorAnnotation(hdc, offX, offY, item)

  if includeCurrent and g_curAnnot.tool != atNone:
    if g_curAnnot.tool == atMosaic:
      GdiFlush()
      applyMosaicStroke(bitsDst, bufW, bufH, clipOvRc, offX, offY, g_curAnnot)
    elif g_curAnnot.tool == atBlur:
      GdiFlush()
      applyBlurStroke(bitsDst, bufW, bufH, clipOvRc, offX, offY, g_curAnnot)
    else:
      drawVectorAnnotation(hdc, offX, offY, g_curAnnot)

  discard RestoreDC(hdc, -1)

# ---------------- 全屏框选遮罩（自动探测 + 8向调整 + 放大镜取色 + 二期标注工具栏） ----------------
proc closeOverlay() =
  if g_state in {osDragging, osMoving, osResizing, osDrawing}:
    ReleaseCapture()
  g_state = osHover
  g_hasHoverWin = false
  g_winRects.setLen(0)
  g_winCands.setLen(0)
  g_annots.setLen(0)
  g_redoStack.setLen(0)
  g_curTool = atNone
  g_curAnnot.tool = atNone
  g_nextNumber = 1
  if g_hwndOverlay != 0:
    DestroyWindow(g_hwndOverlay)
    g_hwndOverlay = 0
  freeOverlayBitmaps()
  syncEscHotkey()

proc cancelSnip*() =
  if g_hwndOverlay != 0:
    if g_state == osTextEditing:
      destroyTextEdit()
      g_state = osAdjust
      SetFocus(g_hwndOverlay)
      InvalidateRect(g_hwndOverlay, nil, FALSE)
    else:
      closeOverlay()
  elif g_hwndResult != 0:
    closeResultWindow()

proc intersectRects(a, b: RECT): RECT {.inline.} =
  result.left = max(a.left, b.left)
  result.top = max(a.top, b.top)
  result.right = max(result.left, min(a.right, b.right))
  result.bottom = max(result.top, min(a.bottom, b.bottom))

type
  ChildEnumData = object
    topRc: RECT
    vx, vy: int
    subList: ptr seq[RECT]

proc childEnumProc(chwnd: HWND, lParam: LPARAM): WINBOOL {.stdcall.} =
  let data = cast[ptr ChildEnumData](lParam)
  if IsWindowVisible(chwnd) == 0: return 1
  let ex = GetWindowLongPtrW(chwnd, GWL_EXSTYLE)
  if (ex and WS_EX_TRANSPARENT) != 0: return 1
  var rc: RECT
  if GetWindowRect(chwnd, addr rc) == 0: return 1
  let cw = rc.right - rc.left
  let ch = rc.bottom - rc.top
  if cw < 24 or ch < 14: return 1
  let local = RECT(
    left: LONG(int(rc.left) - data.vx),
    top: LONG(int(rc.top) - data.vy),
    right: LONG(int(rc.right) - data.vx),
    bottom: LONG(int(rc.bottom) - data.vy)
  )
  let inter = intersectRects(local, data.topRc)
  let iw = inter.right - inter.left
  let ih = inter.bottom - inter.top
  if iw >= 24 and ih >= 14:
    data.subList[].add(inter)
    let topGap = inter.top - data.topRc.top
    let topW = data.topRc.right - data.topRc.left
    if topGap >= 16 and topGap <= 200 and iw * 10 >= topW * 7:
      data.subList[].add(RECT(
        left: data.topRc.left,
        top: data.topRc.top,
        right: data.topRc.right,
        bottom: inter.top
      ))
  if data.subList[].len >= 64: 0 else: 1

proc cleanSubRects(topRc: RECT, list: var seq[RECT]) =
  var cleaned: seq[RECT] = @[]
  let tw = topRc.right - topRc.left
  let th = topRc.bottom - topRc.top
  for r in list:
    let rw = r.right - r.left
    let rh = r.bottom - r.top
    if abs(rw - tw) <= 2 and abs(rh - th) <= 2: continue
    if rw < 24 or rh < 14: continue
    var dup = false
    for c in cleaned:
      if abs(c.left - r.left) <= 2 and abs(c.top - r.top) <= 2 and
         abs(c.right - r.right) <= 2 and abs(c.bottom - r.bottom) <= 2:
        dup = true
        break
    if not dup:
      cleaned.add(r)
  if cleaned.len > 40:
    cleaned.setLen(40)
  list = cleaned

proc enumScreenWindows() =
  g_winRects.setLen(0)
  g_winCands.setLen(0)
  let fgHwnd = GetForegroundWindow()
  let fgRoot = if fgHwnd != 0: GetAncestor(fgHwnd, 2) else: 0
  var topmostCands: seq[WindowCandidate] = @[]
  var normalCands: seq[WindowCandidate] = @[]
  var h = GetTopWindow(0)
  while h != 0:
    if h != g_hwndMain and h != g_hwndOverlay and h != g_hwndResult and
       not g_pins.hasKey(h) and IsWindowVisible(h) != 0 and IsIconic(h) == 0:
      var cloaked: DWORD = 0
      discard DwmGetWindowAttribute(h, DwmwaCloaked, addr cloaked, DWORD(sizeof(cloaked)))
      if cloaked == 0:
        var rc: RECT
        if DwmGetWindowAttribute(h, DwmwaExtendedFrameBounds, addr rc, DWORD(sizeof(rc))) != 0:
          discard GetWindowRect(h, addr rc)
        var l = max(0, int(rc.left) - int(g_vx))
        var t = max(0, int(rc.top) - int(g_vy))
        var r = min(int(g_vw), int(rc.right) - int(g_vx))
        var b = min(int(g_vh), int(rc.bottom) - int(g_vy))
        if r - l >= 16 and b - t >= 16:
          let topRc = RECT(left: LONG(l), top: LONG(t), right: LONG(r), bottom: LONG(b))
          var subs: seq[RECT] = @[]
          var clRc: RECT
          if GetClientRect(h, addr clRc) != 0 and clRc.right > 0 and clRc.bottom > 0:
            var pt = POINT(x: 0, y: 0)
            discard ClientToScreen(h, addr pt)
            let clLocal = RECT(
              left: LONG(int(pt.x) - int(g_vx)),
              top: LONG(int(pt.y) - int(g_vy)),
              right: LONG(int(pt.x) - int(g_vx) + int(clRc.right - clRc.left)),
              bottom: LONG(int(pt.y) - int(g_vy) + int(clRc.bottom - clRc.top))
            )
            let clInter = intersectRects(topRc, clLocal)
            if clInter.top - topRc.top >= 16 and clInter.top - topRc.top <= 200:
              subs.add(RECT(left: topRc.left, top: topRc.top, right: topRc.right, bottom: clInter.top))
            if clInter.right - clInter.left >= 32 and clInter.bottom - clInter.top >= 24:
              subs.add(clInter)

          var enumData = ChildEnumData(
            topRc: topRc,
            vx: int(g_vx),
            vy: int(g_vy),
            subList: addr subs
          )
          discard EnumChildWindows(h, childEnumProc, cast[LPARAM](addr enumData))
          cleanSubRects(topRc, subs)
          let cand = WindowCandidate(hwnd: h, topRect: topRc, subRects: subs)
          let exStyle = GetWindowLongPtr(h, GWL_EXSTYLE)
          if (exStyle and WS_EX_TOPMOST) != 0:
            topmostCands.add(cand)
          else:
            normalCands.add(cand)
    h = GetWindow(h, GW_HWNDNEXT)

  if fgRoot != 0:
    for i in 0 ..< normalCands.len:
      if normalCands[i].hwnd == fgRoot:
        let fgCand = normalCands[i]
        normalCands.delete(i)
        normalCands.insert(fgCand, 0)
        break

  g_winCands = topmostCands & normalCands
  for c in g_winCands:
    g_winRects.add(c.topRect)

proc updateHoverWindow(mx, my: int) =
  g_hasHoverWin = false
  for cand in g_winCands:
    if ptInRect(cand.topRect, mx, my):
      var bestRc = cand.topRect
      var bestArea = int64(cand.topRect.right - cand.topRect.left) * int64(cand.topRect.bottom - cand.topRect.top)
      for sub in cand.subRects:
        if ptInRect(sub, mx, my):
          let w = sub.right - sub.left
          let h = sub.bottom - sub.top
          if w >= 24 and h >= 14:
            let area = int64(w) * int64(h)
            if area < bestArea:
              bestArea = area
              bestRc = sub
      g_hoverWinRc = bestRc
      g_hasHoverWin = true
      return

proc activeSelRect(): RECT =
  if g_state in {osAdjust, osMoving, osResizing, osDrawing, osTextEditing}:
    return g_selRc
  elif g_state == osDragging:
    return normRect(g_ptStart, g_ptCurr)
  elif g_hasHoverWin:
    return g_hoverWinRc
  result = RECT(left: 0, top: 0, right: 0, bottom: 0)

proc btnToTool(id: int): AnnotTool {.inline.} =
  case id
  of OvBtnRect: atRect
  of OvBtnEllipse: atEllipse
  of OvBtnLine: atLine
  of OvBtnArrow: atArrow
  of OvBtnPen: atPen
  of OvBtnMarker: atMarker
  of OvBtnNumber: atNumber
  of OvBtnText: atText
  of OvBtnMosaic: atMosaic
  of OvBtnBlur: atBlur
  else: atNone

proc toolToBtn(t: AnnotTool): int {.inline.} =
  case t
  of atRect: OvBtnRect
  of atEllipse: OvBtnEllipse
  of atLine: OvBtnLine
  of atArrow: OvBtnArrow
  of atPen: OvBtnPen
  of atMarker: OvBtnMarker
  of atNumber: OvBtnNumber
  of atText: OvBtnText
  of atMosaic: OvBtnMosaic
  of atBlur: OvBtnBlur
  else: OvBtnNone

proc layoutOverlayToolbar() =
  let toolSpecs = [
    (OvBtnRect, "□", "矩形 (Ctrl+1)", 28),
    (OvBtnEllipse, "○", "椭圆 (Ctrl+2)", 28),
    (OvBtnLine, "／", "直线 (Ctrl+3)", 28),
    (OvBtnArrow, "↗", "箭头 (Ctrl+4)", 28),
    (OvBtnPen, "✎", "画笔 (Ctrl+5)", 28),
    (OvBtnMarker, "🖍", "荧光笔 (Ctrl+6)", 28),
    (OvBtnNumber, "①", "序号 (Ctrl+7)", 28),
    (OvBtnText, "A", "文字 (Ctrl+8)", 28),
    (OvBtnMosaic, "▦", "马赛克 (Ctrl+9)", 28),
    (OvBtnBlur, "◌", "模糊 (Ctrl+0)", 28),
    (OvBtnUndo, "↶", "撤销 (Ctrl+Z)", 28),
    (OvBtnRedo, "↷", "重做 (Ctrl+Y)", 28),
    (OvBtnTrans, "翻译", "OCR 截屏翻译 (T)", 44),
    (OvBtnPin, "贴图", "固定到屏幕 (F3)", 44),
    (OvBtnSave, "保存", "另存为 (Ctrl+S) / 快存默认目录 (Ctrl+Shift+S)", 44),
    (OvBtnCancel, "取消", "退出截屏 (Esc)", 44),
    (OvBtnCopy, "✓ 复制", "复制截图 (Ctrl+C / Enter)", 58)
  ]
  let gap = 2
  let padX = 7
  var totalW = padX * 2 + 12  # 含两组竖线分隔留空
  for i, s in toolSpecs:
    totalW += s[3]
    if i < toolSpecs.high: totalW += gap

  var bx = int(g_selRc.right) - totalW
  var by = int(g_selRc.bottom) + 8
  let needExtra = if g_curTool != atNone: OvSubBarH + 6 else: 0
  if by + OvBarH + needExtra > int(g_vh) - 6:
    by = int(g_selRc.top) - OvBarH - 8
    if by - needExtra < 6:
      by = int(g_selRc.bottom) - OvBarH - 10
  bx = clamp(bx, 6, max(6, int(g_vw) - totalW - 6))
  by = clamp(by, 6, max(6, int(g_vh) - OvBarH - 6))

  g_ovBarRc = RECT(left: LONG(bx), top: LONG(by), right: LONG(bx + totalW), bottom: LONG(by + OvBarH))
  var x = bx + padX
  let btnY = by + 4
  let btnH = OvBarH - 8
  for i, s in toolSpecs:
    if i == 10 or i == 12:
      x += 6
    let w = s[3]
    g_ovBtns[i] = ToolBtn(
      rect: RECT(left: LONG(x), top: LONG(btnY), right: LONG(x + w), bottom: LONG(btnY + btnH)),
      labelText: s[1],
      tipText: s[2],
      id: s[0]
    )
    x += w + gap

  # 二级属性栏位置（贴在主工具栏下方，超底边则翻到上方）
  if g_curTool != atNone:
    let hasFill = g_curTool in {atRect, atEllipse}
    let hasColor = g_curTool notin {atMosaic, atBlur}
    var subW = 12 + 3 * 24 + 4
    if hasFill: subW += 46
    if hasColor: subW += 8 + 8 * 22
    if not hasColor: subW += 116
    var sx = bx
    var sy = by + OvBarH + 6
    if sy + OvSubBarH > int(g_vh) - 6:
      sy = by - OvSubBarH - 6
    sx = clamp(sx, 6, max(6, int(g_vw) - subW - 6))
    sy = clamp(sy, 6, max(6, int(g_vh) - OvSubBarH - 6))
    g_ovSubBarRc = RECT(left: LONG(sx), top: LONG(sy), right: LONG(sx + subW), bottom: LONG(sy + OvSubBarH))

    var cx = sx + 8
    let cy = sy + 5
    for i in 0 .. 2:
      g_sizeRcs[i] = RECT(left: LONG(cx), top: LONG(cy), right: LONG(cx + 24), bottom: LONG(cy + 24))
      cx += 24
    cx += 6

    if hasFill:
      g_fillToggleRc = RECT(left: LONG(cx), top: LONG(cy), right: LONG(cx + 40), bottom: LONG(cy + 24))
      cx += 46
    else:
      g_fillToggleRc = RECT(left: 0, top: 0, right: 0, bottom: 0)

    if hasColor:
      for i in 0 .. 7:
        g_colorRcs[i] = RECT(left: LONG(cx), top: LONG(cy + 3), right: LONG(cx + 18), bottom: LONG(cy + 21))
        cx += 22
    else:
      for i in 0 .. 7:
        g_colorRcs[i] = RECT(left: 0, top: 0, right: 0, bottom: 0)
  else:
    g_ovSubBarRc = RECT(left: 0, top: 0, right: 0, bottom: 0)

proc getHandleCenters(rc: RECT): array[8, POINT] =
  let mx = (rc.left + rc.right) div 2
  let my = (rc.top + rc.bottom) div 2
  result = [
    POINT(x: rc.left, y: rc.top),     # 1 TL
    POINT(x: mx, y: rc.top),          # 2 T
    POINT(x: rc.right, y: rc.top),    # 3 TR
    POINT(x: rc.right, y: my),        # 4 R
    POINT(x: rc.right, y: rc.bottom), # 5 BR
    POINT(x: mx, y: rc.bottom),       # 6 B
    POINT(x: rc.left, y: rc.bottom),  # 7 BL
    POINT(x: rc.left, y: my)          # 8 L
  ]

proc hitTestHandles(rc: RECT, mx, my: int): int =
  let pts = getHandleCenters(rc)
  for i, p in pts:
    if abs(mx - int(p.x)) <= 6 and abs(my - int(p.y)) <= 6:
      return i + 1
  if ptInRect(rc, mx, my): return 9
  result = 0

proc hitTestOvToolbar(mx, my: int): int =
  if g_state notin {osAdjust, osTextEditing}: return 0
  for b in g_ovBtns:
    if ptInRect(b.rect, mx, my): return b.id
  if ptInRect(g_ovBarRc, mx, my): return -1
  if g_curTool != atNone and ptInRect(g_ovSubBarRc, mx, my):
    for i in 0 .. 2:
      if ptInRect(g_sizeRcs[i], mx, my): return SubHitSize0 + i
    if g_curTool in {atRect, atEllipse} and ptInRect(g_fillToggleRc, mx, my):
      return SubHitFill
    if g_curTool notin {atMosaic, atBlur}:
      for i in 0 .. 7:
        if ptInRect(g_colorRcs[i], mx, my): return SubHitColorBase + i
    return -1
  result = 0

proc commitTextEdit() =
  if g_hwndTextEdit != 0:
    let n = GetWindowTextLengthW(g_hwndTextEdit)
    if n > 0:
      var buf = newSeq[WCHAR](n + 1)
      discard GetWindowTextW(g_hwndTextEdit, addr buf[0], cint(n + 1))
      let txt = ($cast[WideCString](addr buf[0])).strip()
      if txt.len > 0:
        g_annots.add(AnnotItem(
          tool: atText,
          colorIdx: g_curColorIdx,
          sizeIdx: g_curSizeIdx,
          p1: g_textEditPt,
          text: txt
        ))
        g_redoStack.setLen(0)
    destroyTextEdit()
    if g_state == osTextEditing:
      g_state = osAdjust
    if g_hwndOverlay != 0:
      SetFocus(g_hwndOverlay)
      InvalidateRect(g_hwndOverlay, nil, FALSE)

proc hotkeyToTool(wp: WPARAM): AnnotTool {.inline.} =
  case wp
  of WPARAM('1'), VK_NUMPAD1: atRect
  of WPARAM('2'), VK_NUMPAD2: atEllipse
  of WPARAM('3'), VK_NUMPAD3: atLine
  of WPARAM('4'), VK_NUMPAD4: atArrow
  of WPARAM('5'), VK_NUMPAD5: atPen
  of WPARAM('6'), VK_NUMPAD6: atMarker
  of WPARAM('7'), VK_NUMPAD7: atNumber
  of WPARAM('8'), VK_NUMPAD8: atText
  of WPARAM('9'), VK_NUMPAD9: atMosaic
  of WPARAM('0'), VK_NUMPAD0: atBlur
  else: atNone

proc toggleAnnotTool(t: AnnotTool)
proc execOverlayAction(actionId: int)

proc textEditSubProc(hWnd: HWND, uMsg: UINT, wp: WPARAM, lp: LPARAM,
                     uId: UINT_PTR, dwRef: DWORD_PTR): LRESULT {.stdcall.} =
  if uMsg == WM_KEYDOWN:
    let ctrlDown = (GetKeyState(VK_CONTROL) and 0x8000'i16) != 0
    let shiftDown = (GetKeyState(VK_SHIFT) and 0x8000'i16) != 0
    if wp == VK_ESCAPE:
      destroyTextEdit()
      if g_state == osTextEditing: g_state = osAdjust
      if g_hwndOverlay != 0:
        SetFocus(g_hwndOverlay)
        InvalidateRect(g_hwndOverlay, nil, FALSE)
      return 0
    elif wp == VK_RETURN and ctrlDown:
      commitTextEdit()
      return 0
    elif ctrlDown and wp == WPARAM('S'):
      execOverlayAction(if shiftDown: OvBtnQuickSave else: OvBtnSave)
      return 0
    elif ctrlDown:
      let kt = hotkeyToTool(wp)
      if kt != atNone:
        toggleAnnotTool(kt)
        return 0
  result = SnipDefSubclassProc(hWnd, uMsg, wp, lp)

proc toggleAnnotTool(t: AnnotTool) =
  commitTextEdit()
  g_curTool = if g_curTool == t: atNone else: t
  layoutOverlayToolbar()
  if g_hwndOverlay != 0:
    SetFocus(g_hwndOverlay)
    InvalidateRect(g_hwndOverlay, nil, FALSE)

proc startTextEditAt(mx, my: int) =
  commitTextEdit()
  if g_hwndOverlay == 0: return
  let px = clamp(mx, int(g_selRc.left) + 2, max(int(g_selRc.left) + 2, int(g_selRc.right) - 80))
  let py = clamp(my, int(g_selRc.top) + 2, max(int(g_selRc.top) + 2, int(g_selRc.bottom) - 32))
  let ew = min(240, max(80, int(g_selRc.right) - px - 4))
  let eh = min(100, max(32, int(g_selRc.bottom) - py - 4))
  g_textEditPt = POINT(x: LONG(px), y: LONG(py))
  g_hwndTextEdit = CreateWindowExW(
    0,
    newWideCString("EDIT"),
    newWideCString(""),
    WS_CHILD or WS_VISIBLE or WS_BORDER or ES_MULTILINE or ES_AUTOVSCROLL or ES_WANTRETURN,
    cint(px), cint(py), cint(ew), cint(eh),
    g_hwndOverlay, TextEditCtrlId, GetModuleHandleW(nil), nil
  )
  if g_hwndTextEdit != 0:
    let fpx = cint(TextFontSizes[clamp(g_curSizeIdx, 0, 2)])
    g_textEditFont = CreateFontW(-fpx, 0, 0, 0, FW_MEDIUM, 0, 0, 0, DEFAULT_CHARSET,
                                 0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
    discard SendMessageW(g_hwndTextEdit, WM_SETFONT, WPARAM(g_textEditFont), LPARAM(1))
    g_textEditBrush = CreateSolidBrush(RGB(24, 28, 36))
    discard SnipSetWindowSubclass(g_hwndTextEdit, textEditSubProc, 0x5022, 0)
    g_state = osTextEditing
    SetFocus(g_hwndTextEdit)

proc undoAnnotation() =
  commitTextEdit()
  if g_annots.len == 0: return
  let item = g_annots[^1]
  g_annots.delete(g_annots.high)
  g_redoStack.add(item)
  if item.tool == atNumber and g_nextNumber > 1:
    dec g_nextNumber
  if g_hwndOverlay != 0:
    InvalidateRect(g_hwndOverlay, nil, FALSE)

proc redoAnnotation() =
  commitTextEdit()
  if g_redoStack.len == 0: return
  let item = g_redoStack[^1]
  g_redoStack.delete(g_redoStack.high)
  g_annots.add(item)
  if item.tool == atNumber:
    g_nextNumber = max(g_nextNumber, item.numVal + 1)
  if g_hwndOverlay != 0:
    InvalidateRect(g_hwndOverlay, nil, FALSE)

proc copySelectionCrop(rc: RECT): tuple[sx, sy, sw, sh: cint, bits: pointer] =
  let sw = cint(rc.right - rc.left)
  let sh = cint(rc.bottom - rc.top)
  if sw < 4 or sh < 4 or g_bitsOrig == nil:
    return (0'i32, 0'i32, 0'i32, 0'i32, nil)
  let imgBytes = int(sw) * int(sh) * 4
  let cropBits = c_malloc(csize_t(imgBytes))
  if cropBits == nil:
    return (0'i32, 0'i32, 0'i32, 0'i32, nil)

  # 用 32-bit DIB 内存 DC 合成原图与全部标注
  let hdcScr = GetDC(0)
  let hdcCrop = CreateCompatibleDC(hdcScr)
  var dibBits: pointer = nil
  let hbmCrop = makeDib32(hdcScr, sw, sh, addr dibBits)
  ReleaseDC(0, hdcScr)
  if hbmCrop == 0 or dibBits == nil:
    if hbmCrop != 0: DeleteObject(hbmCrop)
    DeleteDC(hdcCrop)
    c_free(cropBits)
    return (0'i32, 0'i32, 0'i32, 0'i32, nil)

  let oldBm = SelectObject(hdcCrop, hbmCrop)
  let srcBase = cast[uint](g_bitsOrig)
  let dstBase = cast[uint](dibBits)
  let strideSrc = int(g_vw) * 4
  let strideDst = int(sw) * 4
  for row in 0 ..< int(sh):
    let sPtr = cast[pointer](srcBase + uint((int(rc.top) + row) * strideSrc + int(rc.left) * 4))
    let dPtr = cast[pointer](dstBase + uint(row * strideDst))
    copyMem(dPtr, sPtr, strideDst)

  if g_annots.len > 0:
    renderAllAnnotations(hdcCrop, dibBits, sw, sh, rc, int(rc.left), int(rc.top), includeCurrent = false)
    GdiFlush()

  copyMem(cropBits, dibBits, imgBytes)
  SelectObject(hdcCrop, oldBm)
  DeleteObject(hbmCrop)
  DeleteDC(hdcCrop)

  # 确保所有像素 Alpha=255，防止个别软件粘贴 CF_DIB 时出现透明空洞
  let pOut = cast[ptr UncheckedArray[uint8]](cropBits)
  var i = 3
  while i < imgBytes:
    pOut[i] = 255'u8
    i += 4

  let sx = g_vx + cint(rc.left)
  let sy = g_vy + cint(rc.top)
  saveLastSnipCache(cropBits, sw, sh)
  result = (sx, sy, sw, sh, cropBits)

proc execOverlayAction(actionId: int) =
  commitTextEdit()
  let rc = activeSelRect()
  if rc.right - rc.left < 4 or rc.bottom - rc.top < 4:
    closeOverlay()
    return
  let crop = copySelectionCrop(rc)
  if crop.bits == nil:
    closeOverlay()
    return

  if g_hwndOverlay != 0:
    DestroyWindow(g_hwndOverlay)
    g_hwndOverlay = 0
  g_state = osHover
  g_hasHoverWin = false
  g_winRects.setLen(0)
  g_winCands.setLen(0)
  g_annots.setLen(0)
  g_redoStack.setLen(0)
  g_curTool = atNone
  g_curAnnot.tool = atNone
  g_nextNumber = 1
  freeOverlayBitmaps()
  syncEscHotkey()

  case actionId
  of OvBtnTrans:
    openResultWindow(crop.sx, crop.sy, crop.sw, crop.sh, crop.bits)
  of OvBtnPin:
    discard createPinWindow(crop.sx, crop.sy, crop.sw, crop.sh, crop.bits)
  of OvBtnSave:
    if saveBitsAsPngDialog(g_hwndMain, crop.bits, crop.sw, crop.sh) and g_notifyCb != nil:
      g_notifyCb("（截图已保存）")
  of OvBtnQuickSave:
    var savedPath = ""
    if saveBitsAsPngDefault(crop.bits, crop.sw, crop.sh, savedPath):
      if g_notifyCb != nil:
        g_notifyCb("（已保存：" & extractFilename(savedPath) & "）")
    else:
      if g_notifyCb != nil:
        g_notifyCb("（截图保存失败）")
  of OvBtnCopy:
    if copyDibToClipboard(g_hwndMain, crop.bits, crop.sw, crop.sh) and g_notifyCb != nil:
      g_notifyCb("（已复制截图）")
  else: discard
  c_free(crop.bits)

proc drawToolIcon(g: pointer, hdc: HDC, id: int, r: RECT, colRgb: COLORREF, disabled: bool) =
  let a = if disabled: 95'u8 else: 245'u8
  let rCol = uint8(colRgb and 0xFF)
  let gCol = uint8((colRgb shr 8) and 0xFF)
  let bCol = uint8((colRgb shr 16) and 0xFF)
  let c = argb(rCol, gCol, bCol, a)
  let cx = cfloat(r.left + r.right) * 0.5'f32
  let cy = cfloat(r.top + r.bottom) * 0.5'f32

  var pen: pointer = nil
  var br: pointer = nil
  discard GdipCreatePen1(c, 1.6'f32, 2, addr pen)
  discard GdipCreateSolidFill(c, addr br)
  if pen != nil:
    discard GdipSetPenStartCap(pen, LineCapRound)
    discard GdipSetPenEndCap(pen, LineCapRound)
    discard GdipSetPenLineJoin(pen, LineJoinRound)

  case id
  of OvBtnRect:
    if pen != nil: discard GdipDrawRectangle(g, pen, cx - 6.5'f32, cy - 5.0'f32, 13.0'f32, 10.0'f32)
  of OvBtnEllipse:
    if pen != nil: discard GdipDrawEllipse(g, pen, cx - 6.5'f32, cy - 5.0'f32, 13.0'f32, 10.0'f32)
  of OvBtnLine:
    if pen != nil: discard GdipDrawLine(g, pen, cx - 6.0'f32, cy + 5.0'f32, cx + 6.0'f32, cy - 5.0'f32)
  of OvBtnArrow:
    if pen != nil:
      discard GdipDrawLine(g, pen, cx - 5.5'f32, cy + 5.5'f32, cx + 5.5'f32, cy - 5.5'f32)
      discard GdipDrawLine(g, pen, cx + 5.5'f32, cy - 5.5'f32, cx + 0.5'f32, cy - 5.5'f32)
      discard GdipDrawLine(g, pen, cx + 5.5'f32, cy - 5.5'f32, cx + 5.5'f32, cy - 0.5'f32)
  of OvBtnPen:
    var pts: array[4, GpPointF] = [
      GpPointF(x: cx - 6.0'f32, y: cy + 3.0'f32),
      GpPointF(x: cx - 2.0'f32, y: cy - 4.0'f32),
      GpPointF(x: cx + 2.0'f32, y: cy + 4.0'f32),
      GpPointF(x: cx + 6.0'f32, y: cy - 3.0'f32)
    ]
    if pen != nil: discard GdipDrawLines(g, pen, addr pts[0], 4)
  of OvBtnMarker:
    var penW: pointer = nil
    if GdipCreatePen1(argb(rCol, gCol, bCol, if disabled: 70'u8 else: 140'u8), 5.0'f32, 2, addr penW) == 0 and penW != nil:
      discard GdipSetPenStartCap(penW, LineCapRound)
      discard GdipSetPenEndCap(penW, LineCapRound)
      discard GdipDrawLine(g, penW, cx - 5.0'f32, cy + 4.0'f32, cx + 5.0'f32, cy - 4.0'f32)
      discard GdipDeletePen(penW)
    if pen != nil: discard GdipDrawLine(g, pen, cx - 5.5'f32, cy + 6.0'f32, cx + 5.5'f32, cy + 6.0'f32)
  of OvBtnNumber:
    if pen != nil: discard GdipDrawEllipse(g, pen, cx - 6.5'f32, cy - 6.5'f32, 13.0'f32, 13.0'f32)
    if pen != nil: discard GdipDrawLine(g, pen, cx, cy - 3.0'f32, cx, cy + 3.5'f32)
  of OvBtnText:
    var ptsA: array[3, GpPointF] = [
      GpPointF(x: cx - 4.5'f32, y: cy + 5.5'f32),
      GpPointF(x: cx, y: cy - 5.5'f32),
      GpPointF(x: cx + 4.5'f32, y: cy + 5.5'f32)
    ]
    if pen != nil:
      discard GdipDrawLines(g, pen, addr ptsA[0], 3)
      discard GdipDrawLine(g, pen, cx - 2.8'f32, cy + 1.5'f32, cx + 2.8'f32, cy + 1.5'f32)
  of OvBtnMosaic:
    if br != nil:
      for row in 0 .. 2:
        for col in 0 .. 2:
          if ((row + col) and 1) == 0:
            discard GdipFillRectangle(g, br, cx - 6.0'f32 + cfloat(col) * 4.2'f32,
                                      cy - 6.0'f32 + cfloat(row) * 4.2'f32, 3.6'f32, 3.6'f32)
  of OvBtnBlur:
    if pen != nil:
      discard GdipDrawEllipse(g, pen, cx - 6.0'f32, cy - 6.0'f32, 12.0'f32, 12.0'f32)
      discard GdipDrawEllipse(g, pen, cx - 2.8'f32, cy - 2.8'f32, 5.6'f32, 5.6'f32)
  of OvBtnUndo:
    var ptsU: array[4, GpPointF] = [
      GpPointF(x: cx - 5.5'f32, y: cy - 1.5'f32),
      GpPointF(x: cx + 2.0'f32, y: cy - 4.5'f32),
      GpPointF(x: cx + 5.5'f32, y: cy + 0.5'f32),
      GpPointF(x: cx + 1.5'f32, y: cy + 4.5'f32)
    ]
    if pen != nil:
      discard GdipDrawLines(g, pen, addr ptsU[0], 4)
      discard GdipDrawLine(g, pen, cx - 5.5'f32, cy - 1.5'f32, cx - 1.5'f32, cy - 4.5'f32)
      discard GdipDrawLine(g, pen, cx - 5.5'f32, cy - 1.5'f32, cx - 2.5'f32, cy + 2.2'f32)
  of OvBtnRedo:
    var ptsR: array[4, GpPointF] = [
      GpPointF(x: cx + 5.5'f32, y: cy - 1.5'f32),
      GpPointF(x: cx - 2.0'f32, y: cy - 4.5'f32),
      GpPointF(x: cx - 5.5'f32, y: cy + 0.5'f32),
      GpPointF(x: cx - 1.5'f32, y: cy + 4.5'f32)
    ]
    if pen != nil:
      discard GdipDrawLines(g, pen, addr ptsR[0], 4)
      discard GdipDrawLine(g, pen, cx + 5.5'f32, cy - 1.5'f32, cx + 1.5'f32, cy - 4.5'f32)
      discard GdipDrawLine(g, pen, cx + 5.5'f32, cy - 1.5'f32, cx + 2.5'f32, cy + 2.2'f32)
  else: discard

  if pen != nil: discard GdipDeletePen(pen)
  if br != nil: discard GdipDeleteBrush(br)

proc drawToolbars(hdcBack: HDC) =
  # 主工具栏
  let barBr = CreateSolidBrush(RGB(30, 34, 42))
  FillRect(hdcBack, addr g_ovBarRc, barBr)
  DeleteObject(barBr)
  let barPen = CreatePen(PS_SOLID, 1, RGB(68, 76, 92))
  let op = SelectObject(hdcBack, barPen)
  let ob = SelectObject(hdcBack, GetStockObject(NULL_BRUSH))
  Rectangle(hdcBack, g_ovBarRc.left, g_ovBarRc.top, g_ovBarRc.right, g_ovBarRc.bottom)

  # 两组分隔竖线
  let sepPen = CreatePen(PS_SOLID, 1, RGB(60, 68, 82))
  discard SelectObject(hdcBack, sepPen)
  for idx in [10, 12]:
    let sx = g_ovBtns[idx].rect.left - 4
    MoveToEx(hdcBack, sx, g_ovBarRc.top + 7, nil)
    LineTo(hdcBack, sx, g_ovBarRc.bottom - 7)
  SelectObject(hdcBack, ob)
  SelectObject(hdcBack, op)
  DeleteObject(sepPen)
  DeleteObject(barPen)

  let hfBtn = CreateFontW(-13, 0, 0, 0, FW_MEDIUM, 0, 0, 0, DEFAULT_CHARSET,
                          0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldF = SelectObject(hdcBack, hfBtn)
  SetBkMode(hdcBack, TRANSPARENT)
  let activeToolBtn = toolToBtn(g_curTool)
  var hoverTip = ""
  var hoverRc: RECT

  ensureGdiplus()
  var gIcons: pointer = nil
  if g_gpReady:
    if GdipCreateFromHDC(hdcBack, addr gIcons) == 0 and gIcons != nil:
      discard GdipSetSmoothingMode(gIcons, SmoothingAntiAlias)

  for b in g_ovBtns:
    let hot = (g_ovHoverBtn == b.id)
    let active = (activeToolBtn != OvBtnNone and b.id == activeToolBtn)
    let disabled = (b.id == OvBtnUndo and g_annots.len == 0) or
                   (b.id == OvBtnRedo and g_redoStack.len == 0)
    let bCol = if b.id == OvBtnCopy: (if hot: RGB(79, 145, 250) else: RGB(59, 130, 246))
               elif b.id == OvBtnCancel and hot: RGB(200, 62, 62)
               elif active: RGB(59, 130, 246)
               elif hot and not disabled: RGB(64, 72, 88)
               else: RGB(42, 48, 58)
    let br = CreateSolidBrush(bCol)
    var r = b.rect
    FillRect(hdcBack, addr r, br)
    DeleteObject(br)
    let tCol = if disabled: RGB(105, 114, 130) else: RGB(242, 246, 252)
    if b.id in OvBtnRect .. OvBtnRedo and gIcons != nil:
      drawToolIcon(gIcons, hdcBack, b.id, r, tCol, disabled)
    else:
      SetTextColor(hdcBack, tCol)
      DrawTextW(hdcBack, newWideCString(b.labelText), -1, addr r,
                DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
    if hot and b.tipText.len > 0:
      hoverTip = b.tipText
      hoverRc = r

  if gIcons != nil:
    discard GdipDeleteGraphics(gIcons)

  # 二级属性子栏（粗细 + 空心/实心 + 8色调色盘）
  if g_curTool != atNone and g_ovSubBarRc.right > g_ovSubBarRc.left:
    let sBr = CreateSolidBrush(RGB(26, 30, 38))
    FillRect(hdcBack, addr g_ovSubBarRc, sBr)
    DeleteObject(sBr)
    let sPen = CreatePen(PS_SOLID, 1, RGB(68, 76, 92))
    let op2 = SelectObject(hdcBack, sPen)
    let ob2 = SelectObject(hdcBack, GetStockObject(NULL_BRUSH))
    Rectangle(hdcBack, g_ovSubBarRc.left, g_ovSubBarRc.top, g_ovSubBarRc.right, g_ovSubBarRc.bottom)
    SelectObject(hdcBack, ob2)
    SelectObject(hdcBack, op2)
    DeleteObject(sPen)

    # 3 档粗细圆点
    ensureGdiplus()
    if g_gpReady:
      var g: pointer = nil
      if GdipCreateFromHDC(hdcBack, addr g) == 0 and g != nil:
        discard GdipSetSmoothingMode(g, SmoothingAntiAlias)
        let diams = [5.0'f32, 9.0'f32, 13.0'f32]
        for i in 0 .. 2:
          let r = g_sizeRcs[i]
          let active = (g_curSizeIdx == i)
          let hot = (g_subHoverHit == SubHitSize0 + i)
          if active or hot:
            let bgC = if active: argb(59, 130, 246, 85) else: argb(255, 255, 255, 30)
            var brBg: pointer = nil
            if GdipCreateSolidFill(bgC, addr brBg) == 0 and brBg != nil:
              discard GdipFillEllipse(g, brBg, cfloat(r.left + 2), cfloat(r.top + 2), 20.0'f32, 20.0'f32)
              discard GdipDeleteBrush(brBg)
          let d = diams[i]
          let cx = cfloat(r.left + r.right) * 0.5'f32 - d * 0.5'f32
          let cy = cfloat(r.top + r.bottom) * 0.5'f32 - d * 0.5'f32
          let dotC = if active: argb(96, 165, 250) else: argb(218, 226, 238)
          var brDot: pointer = nil
          if GdipCreateSolidFill(dotC, addr brDot) == 0 and brDot != nil:
            discard GdipFillEllipse(g, brDot, cx, cy, d, d)
            discard GdipDeleteBrush(brDot)
        discard GdipDeleteGraphics(g)

    # 矩形 / 椭圆空心实心开关
    if g_curTool in {atRect, atEllipse}:
      let hot = (g_subHoverHit == SubHitFill)
      let fCol = if g_curFilled: RGB(59, 130, 246)
                 elif hot: RGB(64, 72, 88)
                 else: RGB(42, 48, 58)
      let brF = CreateSolidBrush(fCol)
      var fr = g_fillToggleRc
      FillRect(hdcBack, addr fr, brF)
      DeleteObject(brF)
      SetTextColor(hdcBack, RGB(240, 244, 250))
      DrawTextW(hdcBack, newWideCString(if g_curFilled: "实心" else: "空心"), -1, addr fr,
                DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)

    # 8 色块 或 马赛克/模糊提示
    if g_curTool notin {atMosaic, atBlur}:
      for i in 0 .. 7:
        let rc = g_colorRcs[i]
        let c = AnnotColors[i]
        let swBr = CreateSolidBrush(RGB(c[0], c[1], c[2]))
        var cr = rc
        FillRect(hdcBack, addr cr, swBr)
        DeleteObject(swBr)
        if g_curColorIdx == i or g_subHoverHit == SubHitColorBase + i:
          let ringCol = if g_curColorIdx == i: RGB(255, 255, 255) else: RGB(147, 197, 253)
          let rp = CreatePen(PS_SOLID, if g_curColorIdx == i: 2 else: 1, ringCol)
          let op3 = SelectObject(hdcBack, rp)
          let ob3 = SelectObject(hdcBack, GetStockObject(NULL_BRUSH))
          Rectangle(hdcBack, rc.left - 2, rc.top - 2, rc.right + 2, rc.bottom + 2)
          SelectObject(hdcBack, ob3)
          SelectObject(hdcBack, op3)
          DeleteObject(rp)
    else:
      var hintRc = RECT(left: g_sizeRcs[2].right + 10, top: g_ovSubBarRc.top,
                        right: g_ovSubBarRc.right - 6, bottom: g_ovSubBarRc.bottom)
      SetTextColor(hdcBack, RGB(158, 170, 190))
      DrawTextW(hdcBack, newWideCString("强度/粗细(滚轮可调)"), -1, addr hintRc,
                DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)

  # 按钮悬停微型提示标签
  if hoverTip.len > 0:
    let wTip = newWideCString(hoverTip)
    var calcRc = RECT(left: 0, top: 0, right: 0, bottom: 0)
    discard DrawTextW(hdcBack, cast[LPCWSTR](wTip), -1, addr calcRc, DT_CALCRECT or DT_SINGLELINE or DT_NOPREFIX)
    var tipW = clamp(int(calcRc.right - calcRc.left) + 18, 56, 340)
    var tx = clamp((int(hoverRc.left) + int(hoverRc.right) - tipW) div 2, 4, max(4, int(g_vw) - tipW - 4))
    var ty = int(g_ovBarRc.top) - 24
    if ty < 4: ty = int(g_ovBarRc.bottom) + 6
    var tRc = RECT(left: LONG(tx), top: LONG(ty), right: LONG(tx + tipW), bottom: LONG(ty + 20))
    let tBr = CreateSolidBrush(RGB(20, 24, 32))
    FillRect(hdcBack, addr tRc, tBr)
    DeleteObject(tBr)
    SetTextColor(hdcBack, RGB(230, 238, 250))
    DrawTextW(hdcBack, cast[LPCWSTR](wTip), -1, addr tRc,
              DT_CENTER or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)

  SelectObject(hdcBack, oldF)
  DeleteObject(hfBtn)

proc drawMagnifier(hdcBack: HDC, hdcSrcOrig: HDC, mx, my: int, selRc: RECT) =
  if g_bitsOrig == nil or mx < 0 or my < 0 or mx >= int(g_vw) or my >= int(g_vh): return
  const BoxW = 132
  const MagH = 96
  const InfoH = 60
  const BoxH = MagH + InfoH

  var bx = mx + 18
  var by = my + 22
  if bx + BoxW > int(g_vw) - 6: bx = mx - BoxW - 18
  if by + BoxH > int(g_vh) - 6: by = my - BoxH - 18
  bx = clamp(bx, 4, max(4, int(g_vw) - BoxW - 4))
  by = clamp(by, 4, max(4, int(g_vh) - BoxH - 4))

  var fullRc = RECT(left: LONG(bx), top: LONG(by), right: LONG(bx + BoxW), bottom: LONG(by + BoxH))
  let bgBr = CreateSolidBrush(RGB(24, 27, 34))
  FillRect(hdcBack, addr fullRc, bgBr)
  DeleteObject(bgBr)

  let srcX = mx - 5
  let srcY = my - 4
  let clipL = max(0, srcX)
  let clipT = max(0, srcY)
  let clipR = min(int(g_vw), srcX + 11)
  let clipB = min(int(g_vh), srcY + 8)
  if clipR > clipL and clipB > clipT:
    SetStretchBltMode(hdcBack, COLORONCOLOR)
    let dstL = bx + (clipL - srcX) * 12
    let dstT = by + (clipT - srcY) * 12
    let dstW = (clipR - clipL) * 12
    let dstH = (clipB - clipT) * 12
    StretchBlt(hdcBack, cint(dstL), cint(dstT), cint(dstW), cint(dstH),
               hdcSrcOrig, cint(clipL), cint(clipT), cint(clipR - clipL), cint(clipB - clipT), SRCCOPY)

  let penCross = CreatePen(PS_SOLID, 1, RGB(59, 130, 246))
  let oldP = SelectObject(hdcBack, penCross)
  let oldB = SelectObject(hdcBack, GetStockObject(NULL_BRUSH))
  Rectangle(hdcBack, cint(bx + 60), cint(by + 48), cint(bx + 72), cint(by + 60))
  SelectObject(hdcBack, oldB)
  SelectObject(hdcBack, oldP)
  DeleteObject(penCross)

  let p = cast[ptr UncheckedArray[uint8]](g_bitsOrig)
  let off = (my * int(g_vw) + mx) * 4
  let bVal = p[off]
  let gVal = p[off + 1]
  let rVal = p[off + 2]
  let hexStr = "#" & toHex(int(rVal), 2) & toHex(int(gVal), 2) & toHex(int(bVal), 2)

  var swRc = RECT(left: LONG(bx + 8), top: LONG(by + MagH + 8),
                  right: LONG(bx + 22), bottom: LONG(by + MagH + 22))
  let swBr = CreateSolidBrush(RGB(rVal, gVal, bVal))
  FillRect(hdcBack, addr swRc, swBr)
  DeleteObject(swBr)

  let hf = CreateFontW(-11, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                       0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
  let oldF = SelectObject(hdcBack, hf)
  SetBkMode(hdcBack, TRANSPARENT)
  SetTextColor(hdcBack, RGB(236, 242, 252))
  let sw = max(0, int(selRc.right - selRc.left))
  let sh = max(0, int(selRc.bottom - selRc.top))
  var line1Rc = RECT(left: LONG(bx + 28), top: LONG(by + MagH + 5),
                     right: LONG(bx + BoxW - 4), bottom: LONG(by + MagH + 24))
  DrawTextW(hdcBack, newWideCString(hexStr & "  " & $sw & "×" & $sh), -1, addr line1Rc,
            DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_NOPREFIX)
  SetTextColor(hdcBack, RGB(148, 163, 184))
  var line2Rc = RECT(left: LONG(bx + 8), top: LONG(by + MagH + 25),
                     right: LONG(bx + BoxW - 4), bottom: LONG(by + BoxH - 4))
  DrawTextW(hdcBack, newWideCString("RGB(" & $rVal & "," & $gVal & "," & $bVal & ")\n按 C 复制色值"),
            -1, addr line2Rc, DT_LEFT or DT_TOP or DT_NOPREFIX)
  SelectObject(hdcBack, oldF)
  DeleteObject(hf)

  let penBd = CreatePen(PS_SOLID, 1, RGB(68, 76, 92))
  let oldP2 = SelectObject(hdcBack, penBd)
  let oldB2 = SelectObject(hdcBack, GetStockObject(NULL_BRUSH))
  Rectangle(hdcBack, cint(bx), cint(by), cint(bx + BoxW), cint(by + BoxH))
  SelectObject(hdcBack, oldB2)
  SelectObject(hdcBack, oldP2)
  DeleteObject(penBd)

proc pastePinFromClipboard*(hwndMain: HWND, cfg: SnipCfg, copyCb: proc (s: string),
                            notifyCb: proc (msg: string) = nil) =
  g_hwndMain = hwndMain
  g_lastCfg = cfg
  g_copyTextCb = copyCb
  if notifyCb != nil:
    g_notifyCb = notifyCb

  if g_hwndOverlay != 0:
    let rc = activeSelRect()
    if rc.right - rc.left >= 4 and rc.bottom - rc.top >= 4:
      execOverlayAction(OvBtnPin)
    else:
      closeOverlay()
    return

  # 1. 尝试读取剪贴板图片 (CF_DIB)
  if IsClipboardFormatAvailable(CF_DIB) != 0 and OpenClipboard(hwndMain) != 0:
    let hMem = GetClipboardData(CF_DIB)
    if hMem != 0:
      let pMem = GlobalLock(hMem)
      if pMem != nil:
        let bih = cast[ptr BITMAPINFOHEADER](pMem)
        let w = cint(bih.biWidth)
        let h = cint(abs(int(bih.biHeight)))
        if w > 0 and h > 0 and w <= 8192 and h <= 8192:
          var off = int(bih.biSize)
          if bih.biBitCount <= 8:
            let used = if bih.biClrUsed != 0: int(bih.biClrUsed) else: (1 shl int(bih.biBitCount))
            off += used * 4
          elif bih.biCompression == BI_BITFIELDS and bih.biSize == DWORD(sizeof(BITMAPINFOHEADER)):
            off += 12
          let pPixels = cast[pointer](cast[uint](pMem) + uint(off))
          let hdcScr = GetDC(0)
          let hdcMem = CreateCompatibleDC(hdcScr)
          var outBits: pointer = nil
          let hbm = makeDib32(hdcScr, w, h, addr outBits)
          ReleaseDC(0, hdcScr)
          if hbm != 0 and outBits != nil:
            let oldBm = SelectObject(hdcMem, hbm)
            discard StretchDIBits(hdcMem, 0, 0, w, h, 0, 0, w, h, pPixels,
                                  cast[ptr BITMAPINFO](pMem), DIB_RGB_COLORS, SRCCOPY)
            GdiFlush()
            SelectObject(hdcMem, oldBm)
            GlobalUnlock(hMem)
            CloseClipboard()
            DeleteDC(hdcMem)
            let imgBytes = int(w) * int(h) * 4
            let pOut = cast[ptr UncheckedArray[uint8]](outBits)
            var k = 3
            while k < imgBytes:
              pOut[k] = 255'u8
              k += 4
            var pt: POINT
            discard GetCursorPos(addr pt)
            discard createPinWindow(cint(pt.x - int(w) div 2), cint(pt.y - int(h) div 2), w, h, outBits)
            DeleteObject(hbm)
            return
          if hbm != 0: DeleteObject(hbm)
          DeleteDC(hdcMem)
        GlobalUnlock(hMem)
    CloseClipboard()

  # 2. 尝试读取剪贴板文本 (CF_UNICODETEXT)
  if IsClipboardFormatAvailable(CF_UNICODETEXT) != 0 and OpenClipboard(hwndMain) != 0:
    var clipTxt = ""
    let hMem = GetClipboardData(CF_UNICODETEXT)
    if hMem != 0:
      let pMem = GlobalLock(hMem)
      if pMem != nil:
        clipTxt = $cast[WideCString](pMem)
        GlobalUnlock(hMem)
    CloseClipboard()
    if clipTxt.strip().len > 0:
      createTextPinAtCursor(clipTxt)
      return

  # 3. 回退到最近一次截图缓存
  if g_lastSnipBits != nil and g_lastSnipW > 0 and g_lastSnipH > 0:
    var pt: POINT
    discard GetCursorPos(addr pt)
    discard createPinWindow(cint(pt.x - int(g_lastSnipW) div 2), cint(pt.y - int(g_lastSnipH) div 2),
                            g_lastSnipW, g_lastSnipH, g_lastSnipBits)

proc snapLine45(p1, p2: POINT): POINT =
  let dx = float(p2.x - p1.x)
  let dy = float(p2.y - p1.y)
  let adx = abs(dx)
  let ady = abs(dy)
  if adx > ady * 2.414:
    return POINT(x: p2.x, y: p1.y)
  elif ady > adx * 2.414:
    return POINT(x: p1.x, y: p2.y)
  else:
    let d = int(round(max(adx, ady)))
    return POINT(
      x: p1.x + LONG(if dx >= 0: d else: -d),
      y: p1.y + LONG(if dy >= 0: d else: -d)
    )

proc startSnipCapture*(hwndMain: HWND, cfg: SnipCfg, copyCb: proc (s: string),
                       directTranslate = false, notifyCb: proc (msg: string) = nil) =
  g_hwndMain = hwndMain
  g_lastCfg = cfg
  g_copyTextCb = copyCb
  if notifyCb != nil:
    g_notifyCb = notifyCb
  g_directTranslate = directTranslate
  closeOverlay()
  if directTranslate:
    closeResultWindow()

  g_vx = GetSystemMetrics(SM_XVIRTUALSCREEN)
  g_vy = GetSystemMetrics(SM_YVIRTUALSCREEN)
  g_vw = GetSystemMetrics(SM_CXVIRTUALSCREEN)
  g_vh = GetSystemMetrics(SM_CYVIRTUALSCREEN)
  if g_vw <= 0 or g_vh <= 0: return

  enumScreenWindows()

  let hdcScr = GetDC(0)
  var bitsDark: pointer = nil
  g_hbmOrig = makeDib32(hdcScr, g_vw, g_vh, addr g_bitsOrig)
  g_hbmDark = makeDib32(hdcScr, g_vw, g_vh, addr bitsDark)
  g_hbmBack = makeDib32(hdcScr, g_vw, g_vh, addr g_bitsBack)

  let hdcMem = CreateCompatibleDC(hdcScr)
  let oldBm = SelectObject(hdcMem, g_hbmOrig)
  BitBlt(hdcMem, 0, 0, g_vw, g_vh, hdcScr, g_vx, g_vy, SRCCOPY or CAPTUREBLT)
  GdiFlush()
  SelectObject(hdcMem, oldBm)
  DeleteDC(hdcMem)
  ReleaseDC(0, hdcScr)

  if g_bitsOrig != nil and bitsDark != nil:
    let total = int(g_vw) * int(g_vh) * 4
    let s = cast[ptr UncheckedArray[uint8]](g_bitsOrig)
    let d = cast[ptr UncheckedArray[uint8]](bitsDark)
    var i = 0
    while i < total:
      d[i] = uint8((uint32(s[i]) * 140'u32) shr 8)
      d[i + 1] = uint8((uint32(s[i + 1]) * 140'u32) shr 8)
      d[i + 2] = uint8((uint32(s[i + 2]) * 140'u32) shr 8)
      d[i + 3] = 255'u8
      i += 4

  var curPt: POINT
  discard GetCursorPos(addr curPt)
  g_ptMouse = POINT(x: curPt.x - LONG(g_vx), y: curPt.y - LONG(g_vy))
  g_state = osHover
  g_ovHoverBtn = 0
  g_subHoverHit = 0
  g_curTool = atNone
  g_annots.setLen(0)
  g_redoStack.setLen(0)
  g_nextNumber = 1
  updateHoverWindow(int(g_ptMouse.x), int(g_ptMouse.y))

  if not g_overlayClassReg:
    var wc: WNDCLASSEXW
    wc.cbSize = UINT(sizeof(WNDCLASSEXW))
    wc.style = CS_HREDRAW or CS_VREDRAW or CS_DBLCLKS
    wc.lpfnWndProc = proc (hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM): LRESULT {.stdcall.} =
      case msg
      of WM_PAINT:
        var ps: PAINTSTRUCT
        let hdc = BeginPaint(hwnd, addr ps)
        let hdcBack = CreateCompatibleDC(hdc)
        let oldBack = SelectObject(hdcBack, g_hbmBack)
        let hdcSrc = CreateCompatibleDC(hdc)

        let oldDark = SelectObject(hdcSrc, g_hbmDark)
        BitBlt(hdcBack, 0, 0, g_vw, g_vh, hdcSrc, 0, 0, SRCCOPY)

        let rc = activeSelRect()
        let sw = rc.right - rc.left
        let sh = rc.bottom - rc.top
        if sw > 0 and sh > 0:
          discard SelectObject(hdcSrc, g_hbmOrig)
          BitBlt(hdcBack, rc.left, rc.top, sw, sh, hdcSrc, rc.left, rc.top, SRCCOPY)

          # 在选区内绘制已提交和正在绘制的标注
          if g_state in {osAdjust, osMoving, osResizing, osDrawing, osTextEditing}:
            renderAllAnnotations(hdcBack, g_bitsBack, g_vw, g_vh, rc, 0, 0,
                                 includeCurrent = (g_state == osDrawing))

          let pen = CreatePen(PS_SOLID, 2, RGB(59, 130, 246))
          let oldP = SelectObject(hdcBack, pen)
          let oldB = SelectObject(hdcBack, GetStockObject(NULL_BRUSH))
          Rectangle(hdcBack, rc.left, rc.top, rc.right, rc.bottom)
          SelectObject(hdcBack, oldB)
          SelectObject(hdcBack, oldP)
          DeleteObject(pen)

          # 8 向控制手柄（未激活标注工具时显示）
          if g_state in {osAdjust, osMoving, osResizing} and g_curTool == atNone:
            let pts = getHandleCenters(rc)
            let hPen = CreatePen(PS_SOLID, 1, RGB(59, 130, 246))
            let hBr = CreateSolidBrush(RGB(255, 255, 255))
            let op = SelectObject(hdcBack, hPen)
            let ob = SelectObject(hdcBack, hBr)
            for p in pts:
              Rectangle(hdcBack, p.x - 4, p.y - 4, p.x + 4, p.y + 4)
            SelectObject(hdcBack, ob)
            SelectObject(hdcBack, op)
            DeleteObject(hBr)
            DeleteObject(hPen)

          let tipSuffix = if g_directTranslate: " · 松开翻译"
                          elif g_state == osHover: " · 单击选窗口 / 拖拽框选"
                          else: ""
          let info = $sw & " × " & $sh & tipSuffix
          var ty = rc.top - 24
          if ty < 4: ty = rc.top + 4
          let badgeW = if tipSuffix.len > 0: 210 else: 96
          var badge = RECT(left: rc.left, top: ty, right: rc.left + LONG(badgeW), bottom: ty + 20)
          let br = CreateSolidBrush(RGB(24, 28, 36))
          FillRect(hdcBack, addr badge, br)
          DeleteObject(br)
          let hf = CreateFontW(-12, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                               0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
          let oldF = SelectObject(hdcBack, hf)
          SetBkMode(hdcBack, TRANSPARENT)
          SetTextColor(hdcBack, RGB(235, 242, 255))
          DrawTextW(hdcBack, newWideCString(info), -1, addr badge, DT_CENTER or DT_VCENTER or DT_SINGLELINE)
          SelectObject(hdcBack, oldF)
          DeleteObject(hf)

        if g_state in {osAdjust, osDrawing, osTextEditing}:
          drawToolbars(hdcBack)

        if g_state in {osHover, osDragging, osResizing}:
          discard SelectObject(hdcSrc, g_hbmOrig)
          drawMagnifier(hdcBack, hdcSrc, int(g_ptMouse.x), int(g_ptMouse.y), rc)

        SelectObject(hdcSrc, oldDark)
        DeleteDC(hdcSrc)
        BitBlt(hdc, 0, 0, g_vw, g_vh, hdcBack, 0, 0, SRCCOPY)
        SelectObject(hdcBack, oldBack)
        DeleteDC(hdcBack)
        EndPaint(hwnd, addr ps)
        return 0

      of WM_CTLCOLOREDIT:
        if HWND(lp) == g_hwndTextEdit:
          let hdc = HDC(wp)
          let c = AnnotColors[clamp(g_curColorIdx, 0, AnnotColors.high)]
          SetTextColor(hdc, RGB(c[0], c[1], c[2]))
          SetBkColor(hdc, RGB(24, 28, 36))
          return cast[LRESULT](g_textEditBrush)
        return DefWindowProcW(hwnd, msg, wp, lp)

      of WM_COMMAND:
        let id = int(wp and 0xFFFF)
        let code = int((wp shr 16) and 0xFFFF)
        if id == int(TextEditCtrlId) and code == EN_KILLFOCUS:
          commitTextEdit()
          return 0
        return 0

      of WM_SETCURSOR:
        let mx = int(g_ptMouse.x)
        let my = int(g_ptMouse.y)
        if g_state in {osAdjust, osTextEditing}:
          if hitTestOvToolbar(mx, my) != 0:
            SetCursor(LoadCursorW(0, IDC_ARROW))
            return 1
          if g_curTool != atNone and ptInRect(g_selRc, mx, my):
            SetCursor(LoadCursorW(0, if g_curTool == atText: IDC_IBEAM else: IDC_CROSS))
            return 1
          let hit = hitTestHandles(g_selRc, mx, my)
          case hit
          of 1, 5: SetCursor(LoadCursorW(0, IDC_SIZENWSE))
          of 3, 7: SetCursor(LoadCursorW(0, IDC_SIZENESW))
          of 2, 6: SetCursor(LoadCursorW(0, IDC_SIZENS))
          of 4, 8: SetCursor(LoadCursorW(0, IDC_SIZEWE))
          of 9: SetCursor(LoadCursorW(0, IDC_SIZEALL))
          else: SetCursor(LoadCursorW(0, IDC_CROSS))
          return 1
        elif g_state == osMoving:
          SetCursor(LoadCursorW(0, IDC_SIZEALL))
          return 1
        SetCursor(LoadCursorW(0, IDC_CROSS))
        return 1

      of WM_LBUTTONDOWN:
        let mx = int(cast[int16](lp and 0xFFFF))
        let my = int(cast[int16]((lp shr 16) and 0xFFFF))
        g_ptMouse = POINT(x: LONG(mx), y: LONG(my))

        if g_state in {osAdjust, osTextEditing}:
          let bid = hitTestOvToolbar(mx, my)
          if bid != 0:
            if bid in OvBtnRect .. OvBtnBlur:
              toggleAnnotTool(btnToTool(bid))
            elif bid == OvBtnUndo:
              undoAnnotation()
            elif bid == OvBtnRedo:
              redoAnnotation()
            elif bid in OvBtnTrans .. OvBtnCopy:
              execOverlayAction(bid)
            elif bid in SubHitSize0 .. SubHitSize2:
              g_curSizeIdx = bid - SubHitSize0
              InvalidateRect(hwnd, nil, FALSE)
            elif bid == SubHitFill:
              g_curFilled = not g_curFilled
              InvalidateRect(hwnd, nil, FALSE)
            elif bid in SubHitColorBase .. (SubHitColorBase + 7):
              g_curColorIdx = bid - SubHitColorBase
              InvalidateRect(hwnd, nil, FALSE)
            return 0

          if g_state == osTextEditing:
            commitTextEdit()
            if g_curTool == atText and ptInRect(g_selRc, mx, my):
              startTextEditAt(mx, my)
            return 0

          # 选中标注工具后在选区内点击/拖拽即开始绘制
          if g_curTool != atNone and ptInRect(g_selRc, mx, my):
            if g_curTool == atText:
              startTextEditAt(mx, my)
              return 0
            g_state = osDrawing
            g_curAnnot = AnnotItem(
              tool: g_curTool,
              colorIdx: g_curColorIdx,
              sizeIdx: g_curSizeIdx,
              filled: g_curFilled,
              p1: g_ptMouse,
              p2: g_ptMouse,
              pts: @[g_ptMouse],
              numVal: g_nextNumber
            )
            SetCapture(hwnd)
            InvalidateRect(hwnd, nil, FALSE)
            return 0

          # 未选标注工具时允许 8 向缩放与移动选区
          if g_curTool == atNone:
            let hit = hitTestHandles(g_selRc, mx, my)
            if hit in 1..8:
              g_state = osResizing
              g_resizeHandle = hit
              g_dragOrigRc = g_selRc
              g_dragStartPt = g_ptMouse
              SetCapture(hwnd)
              InvalidateRect(hwnd, nil, FALSE)
              return 0
            elif hit == 9:
              g_state = osMoving
              g_dragOrigRc = g_selRc
              g_dragStartPt = g_ptMouse
              SetCapture(hwnd)
              InvalidateRect(hwnd, nil, FALSE)
              return 0

        # 点选区外（未选工具或重新框选）
        if g_curTool == atNone:
          g_annots.setLen(0)
          g_redoStack.setLen(0)
          g_nextNumber = 1
          g_state = osDragging
          g_ptStart = g_ptMouse
          g_ptCurr = g_ptMouse
          SetCapture(hwnd)
          InvalidateRect(hwnd, nil, FALSE)
        return 0

      of WM_LBUTTONDBLCLK:
        let mx = int(cast[int16](lp and 0xFFFF))
        let my = int(cast[int16]((lp shr 16) and 0xFFFF))
        if g_state == osAdjust and g_curTool == atNone and ptInRect(g_selRc, mx, my):
          execOverlayAction(OvBtnCopy)
        return 0

      of WM_MOUSEMOVE:
        let mx = int(cast[int16](lp and 0xFFFF))
        let my = int(cast[int16]((lp shr 16) and 0xFFFF))
        g_ptMouse = POINT(x: LONG(mx), y: LONG(my))

        case g_state
        of osHover:
          updateHoverWindow(mx, my)
          InvalidateRect(hwnd, nil, FALSE)
        of osDragging:
          g_ptCurr = g_ptMouse
          InvalidateRect(hwnd, nil, FALSE)
        of osMoving:
          let dx = clamp(mx - int(g_dragStartPt.x), -int(g_dragOrigRc.left), int(g_vw) - int(g_dragOrigRc.right))
          let dy = clamp(my - int(g_dragStartPt.y), -int(g_dragOrigRc.top), int(g_vh) - int(g_dragOrigRc.bottom))
          g_selRc = RECT(
            left: g_dragOrigRc.left + LONG(dx),
            top: g_dragOrigRc.top + LONG(dy),
            right: g_dragOrigRc.right + LONG(dx),
            bottom: g_dragOrigRc.bottom + LONG(dy)
          )
          layoutOverlayToolbar()
          InvalidateRect(hwnd, nil, FALSE)
        of osResizing:
          var r = g_dragOrigRc
          let cx = LONG(clamp(mx, 0, int(g_vw)))
          let cy = LONG(clamp(my, 0, int(g_vh)))
          if g_resizeHandle in {1, 7, 8}: r.left = min(cx, r.right - 4)
          if g_resizeHandle in {3, 4, 5}: r.right = max(cx, r.left + 4)
          if g_resizeHandle in {1, 2, 3}: r.top = min(cy, r.bottom - 4)
          if g_resizeHandle in {5, 6, 7}: r.bottom = max(cy, r.top + 4)
          g_selRc = r
          layoutOverlayToolbar()
          InvalidateRect(hwnd, nil, FALSE)
        of osDrawing:
          let clPt = POINT(
            x: LONG(clamp(mx, int(g_selRc.left), int(g_selRc.right))),
            y: LONG(clamp(my, int(g_selRc.top), int(g_selRc.bottom)))
          )
          if g_curAnnot.tool in {atPen, atMarker, atMosaic, atBlur}:
            let lastPt = g_curAnnot.pts[^1]
            if abs(int(clPt.x - lastPt.x)) >= 2 or abs(int(clPt.y - lastPt.y)) >= 2:
              g_curAnnot.pts.add(clPt)
          elif g_curAnnot.tool == atLine and (GetKeyState(VK_SHIFT) and 0x8000'i16) != 0:
            g_curAnnot.p2 = snapLine45(g_curAnnot.p1, clPt)
          else:
            g_curAnnot.p2 = clPt
          InvalidateRect(hwnd, nil, FALSE)
        of osAdjust, osTextEditing:
          let hit = hitTestOvToolbar(mx, my)
          let nhBtn = if hit in OvBtnRect .. OvBtnCopy: hit else: 0
          let nhSub = if hit >= SubHitSize0: hit else: 0
          if nhBtn != g_ovHoverBtn or nhSub != g_subHoverHit:
            g_ovHoverBtn = nhBtn
            g_subHoverHit = nhSub
            InvalidateRect(hwnd, nil, FALSE)
        return 0

      of WM_LBUTTONUP:
        let mx = int(cast[int16](lp and 0xFFFF))
        let my = int(cast[int16]((lp shr 16) and 0xFFFF))
        g_ptMouse = POINT(x: LONG(mx), y: LONG(my))

        case g_state
        of osDragging:
          ReleaseCapture()
          g_ptCurr = g_ptMouse
          let rc = normRect(g_ptStart, g_ptCurr)
          let sw = int(rc.right - rc.left)
          let sh = int(rc.bottom - rc.top)
          if sw < 6 and sh < 6 and g_hasHoverWin:
            g_selRc = g_hoverWinRc
          elif sw >= 8 and sh >= 8:
            g_selRc = rc
          else:
            if g_directTranslate:
              closeOverlay()
            else:
              g_state = osHover
              updateHoverWindow(mx, my)
              InvalidateRect(hwnd, nil, FALSE)
            return 0

          if g_directTranslate:
            let crop = copySelectionCrop(g_selRc)
            if crop.bits != nil:
              if g_hwndOverlay != 0:
                DestroyWindow(g_hwndOverlay)
                g_hwndOverlay = 0
              g_state = osHover
              freeOverlayBitmaps()
              syncEscHotkey()
              openResultWindow(crop.sx, crop.sy, crop.sw, crop.sh, crop.bits)
              c_free(crop.bits)
            else:
              closeOverlay()
          else:
            g_state = osAdjust
            layoutOverlayToolbar()
            InvalidateRect(hwnd, nil, FALSE)

        of osMoving, osResizing:
          ReleaseCapture()
          g_state = osAdjust
          layoutOverlayToolbar()
          InvalidateRect(hwnd, nil, FALSE)

        of osDrawing:
          ReleaseCapture()
          var keep = true
          if g_curAnnot.tool in {atRect, atEllipse, atLine, atArrow}:
            if abs(int(g_curAnnot.p2.x - g_curAnnot.p1.x)) < 3 and
               abs(int(g_curAnnot.p2.y - g_curAnnot.p1.y)) < 3:
              keep = false
          elif g_curAnnot.tool in {atPen, atMarker, atMosaic, atBlur}:
            if g_curAnnot.pts.len < 2:
              g_curAnnot.pts.add(g_curAnnot.p1)
          elif g_curAnnot.tool == atNumber:
            inc g_nextNumber

          if keep:
            g_annots.add(g_curAnnot)
            g_redoStack.setLen(0)
          g_curAnnot.tool = atNone
          g_state = osAdjust
          InvalidateRect(hwnd, nil, FALSE)
        else: discard
        return 0

      of WM_MOUSEWHEEL:
        if g_state == osAdjust and g_curTool != atNone:
          let delta = int(cast[int16]((wp shr 16) and 0xFFFF))
          if delta > 0 and g_curSizeIdx < 2:
            inc g_curSizeIdx
            InvalidateRect(hwnd, nil, FALSE)
          elif delta < 0 and g_curSizeIdx > 0:
            dec g_curSizeIdx
            InvalidateRect(hwnd, nil, FALSE)
        return 0

      of WM_RBUTTONUP:
        if g_state in {osDragging, osMoving, osResizing, osDrawing}:
          ReleaseCapture()
        if g_state == osTextEditing:
          commitTextEdit()
        elif g_state == osAdjust and g_curTool != atNone:
          g_curTool = atNone
          layoutOverlayToolbar()
          InvalidateRect(hwnd, nil, FALSE)
        elif g_state == osAdjust:
          g_annots.setLen(0)
          g_redoStack.setLen(0)
          g_nextNumber = 1
          g_state = osHover
          g_ovHoverBtn = 0
          updateHoverWindow(int(g_ptMouse.x), int(g_ptMouse.y))
          InvalidateRect(hwnd, nil, FALSE)
        else:
          closeOverlay()
        return 0

      of WM_KEYDOWN:
        let ctrlDown = (GetKeyState(VK_CONTROL) and 0x8000'i16) != 0
        let shiftDown = (GetKeyState(VK_SHIFT) and 0x8000'i16) != 0
        let altDown = (GetKeyState(VK_MENU) and 0x8000'i16) != 0
        var curMods = 0
        if ctrlDown: curMods = curMods or 1
        if shiftDown: curMods = curMods or 2
        if altDown: curMods = curMods or 4
        if wp == VK_ESCAPE:
          if g_state == osTextEditing:
            destroyTextEdit()
            g_state = osAdjust
            SetFocus(hwnd)
            InvalidateRect(hwnd, nil, FALSE)
          else:
            closeOverlay()
          return 0
        elif wp == WPARAM('C') and not ctrlDown and not altDown and g_bitsOrig != nil:
          let mx = clamp(int(g_ptMouse.x), 0, int(g_vw) - 1)
          let my = clamp(int(g_ptMouse.y), 0, int(g_vh) - 1)
          let p = cast[ptr UncheckedArray[uint8]](g_bitsOrig)
          let off = (my * int(g_vw) + mx) * 4
          let hexStr = "#" & toHex(int(p[off + 2]), 2) & toHex(int(p[off + 1]), 2) & toHex(int(p[off]), 2)
          if g_copyTextCb != nil: g_copyTextCb(hexStr)
          closeOverlay()
          return 0
        elif g_state in {osAdjust, osTextEditing} or (g_state == osHover and g_hasHoverWin):
          if g_lastCfg.pinVk != 0 and int(wp) == g_lastCfg.pinVk and curMods == g_lastCfg.pinMods:
            execOverlayAction(OvBtnPin)
            return 0
          if ctrlDown:
            if g_state in {osAdjust, osTextEditing}:
              let kt = hotkeyToTool(wp)
              if kt != atNone:
                toggleAnnotTool(kt)
                return 0
              if wp == WPARAM('Z'):
                if shiftDown:
                  redoAnnotation()
                else:
                  undoAnnotation()
                return 0
              elif wp == WPARAM('Y'):
                redoAnnotation()
                return 0
            if wp == WPARAM('C'):
              execOverlayAction(OvBtnCopy)
              return 0
            elif wp == WPARAM('S'):
              execOverlayAction(if shiftDown: OvBtnQuickSave else: OvBtnSave)
              return 0
          case wp
          of VK_RETURN, VK_SPACE:
            execOverlayAction(OvBtnCopy)
          of WPARAM('T'):
            execOverlayAction(OvBtnTrans)
          of VK_F3:
            execOverlayAction(OvBtnPin)
          of VK_LEFT:
            if g_state in {osAdjust, osTextEditing} and g_selRc.left > 0 and g_curTool == atNone:
              dec g_selRc.left; dec g_selRc.right
              layoutOverlayToolbar(); InvalidateRect(hwnd, nil, FALSE)
          of VK_RIGHT:
            if g_state in {osAdjust, osTextEditing} and g_selRc.right < g_vw and g_curTool == atNone:
              inc g_selRc.left; inc g_selRc.right
              layoutOverlayToolbar(); InvalidateRect(hwnd, nil, FALSE)
          of VK_UP:
            if g_state in {osAdjust, osTextEditing} and g_selRc.top > 0 and g_curTool == atNone:
              dec g_selRc.top; dec g_selRc.bottom
              layoutOverlayToolbar(); InvalidateRect(hwnd, nil, FALSE)
          of VK_DOWN:
            if g_state in {osAdjust, osTextEditing} and g_selRc.bottom < g_vh and g_curTool == atNone:
              inc g_selRc.top; inc g_selRc.bottom
              layoutOverlayToolbar(); InvalidateRect(hwnd, nil, FALSE)
          else: discard
          return 0
        return 0

      of WM_ERASEBKGND:
        return 1
      else:
        return DefWindowProcW(hwnd, msg, wp, lp)
    wc.hInstance = GetModuleHandleW(nil)
    wc.hCursor = LoadCursorW(0, IDC_CROSS)
    wc.lpszClassName = newWideCString("KuaiYiSnipOverlay")
    discard RegisterClassExW(addr wc)
    g_overlayClassReg = true

  g_hwndOverlay = CreateWindowExW(
    WS_EX_TOPMOST or WS_EX_TOOLWINDOW,
    newWideCString("KuaiYiSnipOverlay"),
    newWideCString(""),
    WS_POPUP or WS_VISIBLE,
    g_vx, g_vy, g_vw, g_vh,
    0, 0, GetModuleHandleW(nil), nil
  )
  SetForegroundWindow(g_hwndOverlay)
  SetFocus(g_hwndOverlay)
  syncEscHotkey()
