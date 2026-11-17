## 快译NG 截屏翻译界面（参考有道词典截屏翻译）
## 1. 全屏虚拟桌面暗化遮罩 + 鼠标框选区域（ESC/右键取消）
## 2. 后台裸线程调用 fy_snip_translate（WinRT OCR + 英文→中文 + 原图覆盖渲染）
## 3. 结果悬浮窗：支持原图/译图切换、复制译文、复制图片、重新框选、拖拽移动

import winim/lean
import fastcore

const
  WmAppSnipDone* = WM_APP + 0x0202
  HkSnipEsc* = 99
  ToolbarH = 38
  MinWinW = 380

type
  SnipCfg* = object
    aiUrl*: string
    aiKey*: string
    aiModel*: string
    baiduAppId*: string
    baiduKey*: string
    baiduUrl*: string

  SnipJob = object
    seq: int
    hwndMain: HWND
    bgra: pointer
    w, h: cint
    aiUrl, aiKey, aiModel: cstring
    baiduAppId, baiduKey, baiduUrl: cstring

  ToolBtn = object
    rect: RECT
    labelText: string
    id: int

const
  BtnToggle = 1
  BtnCopyText = 2
  BtnCopyImg = 3
  BtnResnip = 4
  BtnClose = 5

# ---- 遮罩框选状态（仅 UI 线程读写） ----
var
  g_hwndMain: HWND
  g_lastCfg: SnipCfg
  g_copyTextCb: proc (s: string)
  g_overlayClassReg = false
  g_resultClassReg = false

  g_hwndOverlay: HWND
  g_vx, g_vy, g_vw, g_vh: cint
  g_hbmOrig, g_hbmDark, g_hbmBack: HBITMAP
  g_bitsOrig: pointer
  g_dragging = false
  g_ptStart, g_ptCurr: POINT

# ---- 结果悬浮窗状态（仅 UI 线程读写） ----
  g_hwndResult: HWND
  g_snipSeq = 0
  g_resW, g_resH: cint
  g_dispW, g_dispH: cint
  g_winW, g_winH: cint
  g_hbmResOrig, g_hbmResTrans: HBITMAP
  g_bitsResOrig, g_bitsResTrans: pointer
  g_showOrig = false
  g_translating = false
  g_zhText = ""
  g_statusMsg = ""
  g_btns: array[5, ToolBtn]
  g_hoverBtn = 0

proc c_malloc(sz: csize_t): pointer {.importc: "malloc", header: "<stdlib.h>".}
proc c_free(p: pointer) {.importc: "free", header: "<stdlib.h>".}

proc startSnipCapture*(hwndMain: HWND, cfg: SnipCfg, copyCb: proc (s: string))

proc makeDib32(hdc: HDC, w, h: cint, bitsOut: ptr pointer): HBITMAP =
  var bmi: BITMAPINFO
  bmi.bmiHeader.biSize = DWORD(sizeof(BITMAPINFOHEADER))
  bmi.bmiHeader.biWidth = LONG(w)
  bmi.bmiHeader.biHeight = LONG(-h)   # top-down
  bmi.bmiHeader.biPlanes = 1
  bmi.bmiHeader.biBitCount = 32
  bmi.bmiHeader.biCompression = BI_RGB
  result = CreateDIBSection(hdc, addr bmi, DIB_RGB_COLORS, bitsOut, 0, 0)

proc freeOverlayBitmaps() =
  if g_hbmOrig != 0: DeleteObject(g_hbmOrig); g_hbmOrig = 0
  if g_hbmDark != 0: DeleteObject(g_hbmDark); g_hbmDark = 0
  if g_hbmBack != 0: DeleteObject(g_hbmBack); g_hbmBack = 0
  g_bitsOrig = nil

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

# ---------------- 后台 OCR + 翻译裸线程 ----------------
proc snipWorker(param: LPVOID): DWORD {.stdcall.} =
  # 裸线程不触碰 Nim GC，全用 C 堆与纯值句柄
  let job = cast[ptr SnipJob](param)
  let seq = job.seq
  let hwndMain = job.hwndMain
  let res = fy_snip_translate(job.bgra, job.w, job.h,
                              job.aiUrl, job.aiKey, job.aiModel,
                              job.baiduAppId, job.baiduKey, job.baiduUrl)
  c_free(job.bgra)
  fy_free(job.aiUrl)
  fy_free(job.aiKey)
  fy_free(job.aiModel)
  fy_free(job.baiduAppId)
  fy_free(job.baiduKey)
  fy_free(job.baiduUrl)
  dealloc(job)
  PostMessageW(hwndMain, UINT(WmAppSnipDone), WPARAM(seq), cast[LPARAM](res))
  return 0

# ---------------- 结果悬浮窗 ----------------
proc layoutResultButtons() =
  let y0 = g_dispH + 5
  let bh = ToolbarH - 10
  let pad = 8
  let labels = [
    (BtnToggle, if g_showOrig: "显示译图" else: "对照原图", 74),
    (BtnCopyText, "复制译文", 70),
    (BtnCopyImg, "复制图片", 70),
    (BtnResnip, "重新框选", 70),
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
    x -= 6

proc openResultWindow(screenX, screenY, w, h: cint, cropBits: pointer) =
  closeResultWindow()
  inc g_snipSeq
  g_resW = w
  g_resH = h
  g_showOrig = false
  g_translating = true
  g_zhText = ""
  g_statusMsg = "正在识别并翻译…"
  g_hoverBtn = 0

  # 若截图过大，等比缩放到屏幕工作区 82% 以内
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

        # 图片区底色
        var topRc = RECT(left: 0, top: 0, right: LONG(g_winW), bottom: LONG(g_dispH))
        let bgImg = CreateSolidBrush(RGB(24, 26, 32))
        FillRect(hdcBuf, addr topRc, bgImg)
        DeleteObject(bgImg)

        # 居中绘制图片
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

        # 底部工具栏
        var barRc = RECT(left: 0, top: LONG(g_dispH), right: LONG(g_winW), bottom: LONG(g_winH))
        let bgBar = CreateSolidBrush(RGB(34, 38, 46))
        FillRect(hdcBuf, addr barRc, bgBar)
        DeleteObject(bgBar)

        let hf = CreateFontW(-13, 0, 0, 0, FW_NORMAL, 0, 0, 0, DEFAULT_CHARSET,
                             0, 0, CLEARTYPE_QUALITY, 0, newWideCString("Microsoft YaHei UI"))
        let oldF = SelectObject(hdcBuf, hf)
        SetBkMode(hdcBuf, TRANSPARENT)

        # 左侧状态文字
        if g_statusMsg.len > 0:
          SetTextColor(hdcBuf, RGB(175, 195, 225))
          var stRc = RECT(left: 10, top: LONG(g_dispH), right: g_btns[0].rect.left - 6, bottom: LONG(g_winH))
          DrawTextW(hdcBuf, newWideCString(g_statusMsg), -1, addr stRc,
                    DT_LEFT or DT_VCENTER or DT_SINGLELINE or DT_END_ELLIPSIS or DT_NOPREFIX)

        # 按钮
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

        # 1px 外框描边
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
            of BtnResnip:
              closeResultWindow()
              startSnipCapture(g_hwndMain, g_lastCfg, g_copyTextCb)
            of BtnClose:
              closeResultWindow()
            else: discard
            return 0
        # 点在图片或空白处：直接拖拽整个悬浮窗
        if my < g_dispH or g_hoverBtn == 0:
          ReleaseCapture()
          SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0)
        return 0

      of WM_KEYDOWN:
        if wp == VK_ESCAPE:
          closeResultWindow()
          return 0
        elif wp == VK_SPACE:
          g_showOrig = not g_showOrig
          layoutResultButtons()
          InvalidateRect(hwnd, nil, FALSE)
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

  # 启动后台 OCR + 翻译线程
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
    g_statusMsg = "翻译完成（空格切换原图）"
    g_showOrig = false
  else:
    g_statusMsg = if res.err != nil: $res.err else: "未识别到英文文本"
  layoutResultButtons()
  InvalidateRect(g_hwndResult, nil, FALSE)

# ---------------- 全屏框选遮罩 ----------------
proc closeOverlay() =
  g_dragging = false
  if g_hwndOverlay != 0:
    DestroyWindow(g_hwndOverlay)
    g_hwndOverlay = 0
  freeOverlayBitmaps()
  syncEscHotkey()

proc cancelSnip*() =
  closeOverlay()
  closeResultWindow()

proc finishSelection() =
  let rc = normRect(g_ptStart, g_ptCurr)
  let sw = cint(rc.right - rc.left)
  let sh = cint(rc.bottom - rc.top)
  if sw < 12 or sh < 12 or g_bitsOrig == nil:
    closeOverlay()
    return
  let imgBytes = int(sw) * int(sh) * 4
  let cropBits = c_malloc(csize_t(imgBytes))
  if cropBits == nil:
    closeOverlay()
    return
  let srcBase = cast[uint](g_bitsOrig)
  let dstBase = cast[uint](cropBits)
  let strideSrc = int(g_vw) * 4
  let strideDst = int(sw) * 4
  for row in 0 ..< int(sh):
    let sPtr = cast[pointer](srcBase + uint((int(rc.top) + row) * strideSrc + int(rc.left) * 4))
    let dPtr = cast[pointer](dstBase + uint(row * strideDst))
    copyMem(dPtr, sPtr, strideDst)

  let sx = g_vx + cint(rc.left)
  let sy = g_vy + cint(rc.top)
  if g_hwndOverlay != 0:
    DestroyWindow(g_hwndOverlay)
    g_hwndOverlay = 0
  g_dragging = false
  freeOverlayBitmaps()
  openResultWindow(sx, sy, sw, sh, cropBits)
  c_free(cropBits)

proc startSnipCapture*(hwndMain: HWND, cfg: SnipCfg, copyCb: proc (s: string)) =
  g_hwndMain = hwndMain
  g_lastCfg = cfg
  g_copyTextCb = copyCb
  closeOverlay()
  closeResultWindow()

  g_vx = GetSystemMetrics(SM_XVIRTUALSCREEN)
  g_vy = GetSystemMetrics(SM_YVIRTUALSCREEN)
  g_vw = GetSystemMetrics(SM_CXVIRTUALSCREEN)
  g_vh = GetSystemMetrics(SM_CYVIRTUALSCREEN)
  if g_vw <= 0 or g_vh <= 0: return

  let hdcScr = GetDC(0)
  var bitsDark: pointer = nil
  var bitsBack: pointer = nil
  g_hbmOrig = makeDib32(hdcScr, g_vw, g_vh, addr g_bitsOrig)
  g_hbmDark = makeDib32(hdcScr, g_vw, g_vh, addr bitsDark)
  g_hbmBack = makeDib32(hdcScr, g_vw, g_vh, addr bitsBack)

  let hdcMem = CreateCompatibleDC(hdcScr)
  let oldBm = SelectObject(hdcMem, g_hbmOrig)
  BitBlt(hdcMem, 0, 0, g_vw, g_vh, hdcScr, g_vx, g_vy, SRCCOPY or CAPTUREBLT)
  GdiFlush()
  SelectObject(hdcMem, oldBm)
  DeleteDC(hdcMem)
  ReleaseDC(0, hdcScr)

  # 生成暗化底图（亮度约 55%）
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

  if not g_overlayClassReg:
    var wc: WNDCLASSEXW
    wc.cbSize = UINT(sizeof(WNDCLASSEXW))
    wc.style = CS_HREDRAW or CS_VREDRAW
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

        if g_dragging:
          let rc = normRect(g_ptStart, g_ptCurr)
          let sw = rc.right - rc.left
          let sh = rc.bottom - rc.top
          if sw > 0 and sh > 0:
            discard SelectObject(hdcSrc, g_hbmOrig)
            BitBlt(hdcBack, rc.left, rc.top, sw, sh, hdcSrc, rc.left, rc.top, SRCCOPY)
            let pen = CreatePen(PS_SOLID, 2, RGB(0, 150, 255))
            let oldP = SelectObject(hdcBack, pen)
            let oldB = SelectObject(hdcBack, GetStockObject(NULL_BRUSH))
            Rectangle(hdcBack, rc.left, rc.top, rc.right, rc.bottom)
            SelectObject(hdcBack, oldB)
            SelectObject(hdcBack, oldP)
            DeleteObject(pen)

            # 尺寸提示条
            let info = $sw & " × " & $sh & " · 松开翻译 / 右键取消"
            var ty = rc.top - 24
            if ty < 4: ty = rc.top + 4
            var badge = RECT(left: rc.left, top: ty, right: rc.left + 210, bottom: ty + 20)
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

        SelectObject(hdcSrc, oldDark)
        DeleteDC(hdcSrc)
        BitBlt(hdc, 0, 0, g_vw, g_vh, hdcBack, 0, 0, SRCCOPY)
        SelectObject(hdcBack, oldBack)
        DeleteDC(hdcBack)
        EndPaint(hwnd, addr ps)
        return 0

      of WM_LBUTTONDOWN:
        g_dragging = true
        g_ptStart.x = LONG(cast[int16](lp and 0xFFFF))
        g_ptStart.y = LONG(cast[int16]((lp shr 16) and 0xFFFF))
        g_ptCurr = g_ptStart
        SetCapture(hwnd)
        InvalidateRect(hwnd, nil, FALSE)
        return 0

      of WM_MOUSEMOVE:
        if g_dragging:
          g_ptCurr.x = LONG(cast[int16](lp and 0xFFFF))
          g_ptCurr.y = LONG(cast[int16]((lp shr 16) and 0xFFFF))
          InvalidateRect(hwnd, nil, FALSE)
        return 0

      of WM_LBUTTONUP:
        if g_dragging:
          ReleaseCapture()
          g_ptCurr.x = LONG(cast[int16](lp and 0xFFFF))
          g_ptCurr.y = LONG(cast[int16]((lp shr 16) and 0xFFFF))
          finishSelection()
        return 0

      of WM_RBUTTONUP:
        if g_dragging: ReleaseCapture()
        closeOverlay()
        return 0

      of WM_KEYDOWN:
        if wp == VK_ESCAPE:
          if g_dragging: ReleaseCapture()
          closeOverlay()
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
