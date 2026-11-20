//! Snipaste 风格全屏截屏、10 种标注、放大镜取色、OCR 对照窗与多贴图浮窗

use crate::core::log::log_write;
use crate::core::ocr::{snip_translate, SnipResult};
use crate::core::str_util::utf8_to_wide;
use crate::ui::win32_ui::{
    argb, ensure_gdiplus, get_clipboard_bgra, get_clipboard_text, get_window_text,
    gp_fill_round_rect, gp_round_card, lparam_xy, rgb, save_bgra_as_png, save_file_dialog_png,
    set_clipboard_bgra, set_clipboard_text, wheel_delta, AlphaBlend, AppendMenuW, BeginPaint,
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen,
    CreatePopupMenu, CreateSolidBrush, CreateWindowExW, DefSubclassProc, DefWindowProcW, DeleteDC,
    DeleteObject, DestroyMenu, DestroyWindow, DrawTextW, DwmGetWindowAttribute, EndPaint,
    EnumWindows, FillRect, GdiFlush, GdipCreateFromHDC, GdipCreatePen1, GdipCreateSolidFill,
    GdipDeleteBrush, GdipDeleteGraphics, GdipDeletePen, GdipDrawEllipse, GdipDrawLine,
    GdipDrawLines, GdipDrawRectangle, GdipFillEllipse, GdipFillRectangle, GdipSetPenEndCap,
    GdipSetPenLineJoin, GdipSetPenStartCap, GdipSetSmoothingMode, GetAsyncKeyState, GetClientRect,
    GetCurrentProcessId, GetCursorPos, GetDC, GetModuleHandleW, GetStockObject, GetSystemMetrics,
    GetWindowRect, GetWindowThreadProcessId, GpPointF, InvalidateRect, IsIconic, IsWindow,
    IsWindowVisible, LoadCursorW, PostMessageW, RegisterClassExW, ReleaseCapture, ReleaseDC,
    ScreenToClient, SelectObject, SetBkMode, SetCapture, SetCursor, SetFocus,
    SetLayeredWindowAttributes, SetStretchBltMode, SetTextColor, SetWindowPos, SetWindowSubclass,
    ShowWindow, StretchBlt, TrackPopupMenu, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    BLENDFUNCTION, CLEARTYPE_QUALITY, COLORONCOLOR, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW,
    DEFAULT_CHARSET, DIB_RGB_COLORS, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX,
    DT_SINGLELINE, DT_TOP, DT_VCENTER, DT_WORDBREAK, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    DWORD_PTR, ES_AUTOHSCROLL, ES_MULTILINE, ES_WANTRETURN, FW_BOLD, FW_MEDIUM, FW_NORMAL,
    GP_LINE_CAP_ROUND, GP_LINE_JOIN_ROUND, GP_OK, GP_SMOOTHING_ANTIALIAS, GP_UNIT_PIXEL, HALFTONE,
    HBITMAP, HDC, HWND, HWND_TOPMOST, IDC_ARROW, IDC_CROSS, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
    IDC_SIZENWSE, IDC_SIZEWE, LPARAM, LRESULT, LWA_ALPHA, MA_NOACTIVATE, MF_CHECKED, MF_SEPARATOR,
    MF_STRING, NULL_BRUSH, PAINTSTRUCT, POINT, PS_DASH, PS_SOLID, RECT, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SRCCOPY, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SW_SHOW, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TRANSPARENT, UINT, UINT_PTR,
    VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_SHIFT, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DESTROY,
    WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SETCURSOR, WNDCLASSEXW, WPARAM, WS_CHILD, WS_CLIPCHILDREN, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
};
use crate::ui::{WM_APP_PIN_DONE, WM_APP_SNIP_DONE};
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{copy_nonoverlapping, null, null_mut};
use std::thread;

pub const OVERLAY_CLASS_NAME: &str = "KuaiYiSnipOverlay";
pub const RESULT_CLASS_NAME: &str = "KuaiYiSnipResult";
pub const PIN_CLASS_NAME: &str = "KuaiYiPinWindow";

const SNAP_DIST: i32 = 6;
const HANDLE_SIZE: i32 = 7;

// 8 种常用标注色 (R, G, B)
pub const ANNOT_COLORS: [(u8, u8, u8); 8] = [
    (239, 68, 68),   // 红
    (59, 130, 246),  // 蓝
    (34, 197, 94),   // 绿
    (250, 204, 21),  // 黄
    (249, 115, 22),  // 橙
    (168, 85, 247),  // 紫
    (248, 250, 252), // 白
    (24, 24, 27),    // 黑
];

const SIZE_WIDTHS: [i32; 3] = [2, 4, 7];

/// 截屏外部配置参数
#[derive(Clone, Default)]
pub struct SnipCfg {
    pub hwnd_main: isize,
    pub ai_url: String,
    pub ai_key: String,
    pub ai_model: String,
    pub baidu_appid: String,
    pub baidu_secret: String,
    pub baidu_url: String,
    pub cache_dir: String,
    pub save_dir: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnnotTool {
    #[default]
    None,
    Rect,
    Ellipse,
    Line,
    Arrow,
    Pen,
    Marker,
    Number,
    Text,
    Mosaic,
    Blur,
}

#[derive(Debug, Clone)]
pub struct AnnotItem {
    pub tool: AnnotTool,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub pts: Vec<(i32, i32)>,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub width: i32,
    pub filled: bool,
    pub num: i32,
    pub text: String,
}

#[derive(Clone, Copy)]
struct OvBtn {
    id: i32,
    rect: RECT,
    label: &'static str,
    tip: &'static str,
}

// 工具栏按钮 ID
const BTN_RECT: i32 = 1;
const BTN_ELLIPSE: i32 = 2;
const BTN_LINE: i32 = 3;
const BTN_ARROW: i32 = 4;
const BTN_PEN: i32 = 5;
const BTN_MARKER: i32 = 6;
const BTN_NUMBER: i32 = 7;
const BTN_TEXT: i32 = 8;
const BTN_MOSAIC: i32 = 9;
const BTN_BLUR: i32 = 10;
const BTN_UNDO: i32 = 11;
const BTN_REDO: i32 = 12;
const BTN_SAVE: i32 = 13;
const BTN_PIN: i32 = 14;
const BTN_TRANS: i32 = 15;
const BTN_CANCEL: i32 = 16;
const BTN_COPY: i32 = 17;

const SUB_SIZE_0: i32 = 100;
const SUB_FILL: i32 = 103;
const SUB_COLOR_BASE: i32 = 110;

/// 规范化矩形坐标，保证 left <= right 且 top <= bottom
#[inline]
pub fn norm_rect(x1: i32, y1: i32, x2: i32, y2: i32) -> RECT {
    RECT {
        left: x1.min(x2),
        top: y1.min(y2),
        right: x1.max(x2),
        bottom: y1.max(y2),
    }
}

/// 裁剪 32-bit top-down BGRA 子区域
pub fn crop_bgra(src: &[u8], src_w: i32, src_h: i32, rc: RECT) -> (i32, i32, Vec<u8>) {
    let x0 = rc.left.clamp(0, src_w);
    let y0 = rc.top.clamp(0, src_h);
    let x1 = rc.right.clamp(0, src_w);
    let y1 = rc.bottom.clamp(0, src_h);
    let cw = (x1 - x0).max(0);
    let ch = (y1 - y0).max(0);
    if cw <= 0 || ch <= 0 || src.len() < (src_w as usize) * (src_h as usize) * 4 {
        return (0, 0, Vec::new());
    }
    let mut out = vec![0u8; (cw as usize) * (ch as usize) * 4];
    let src_stride = (src_w as usize) * 4;
    let dst_stride = (cw as usize) * 4;
    for row in 0..ch as usize {
        let s_off = ((y0 as usize) + row) * src_stride + (x0 as usize) * 4;
        let d_off = row * dst_stride;
        out[d_off..d_off + dst_stride].copy_from_slice(&src[s_off..s_off + dst_stride]);
        for px in out[d_off..d_off + dst_stride].as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
    }
    (cw, ch, out)
}

/// 对 top-down BGRA 指定矩形执行块均值马赛克
pub fn apply_mosaic_to_bgra(bgra: &mut [u8], w: i32, h: i32, rc: RECT, block_px: i32) {
    if w <= 0 || h <= 0 || bgra.len() < (w as usize) * (h as usize) * 4 {
        return;
    }
    let blk = block_px.clamp(4, 32);
    let x0 = rc.left.clamp(0, w);
    let y0 = rc.top.clamp(0, h);
    let x1 = rc.right.clamp(0, w);
    let y1 = rc.bottom.clamp(0, h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let mut by = y0;
    while by < y1 {
        let bh_end = (by + blk).min(y1);
        let mut bx = x0;
        while bx < x1 {
            let bw_end = (bx + blk).min(x1);
            let mut sb = 0u64;
            let mut sg = 0u64;
            let mut sr = 0u64;
            let mut cnt = 0u64;
            for py in by..bh_end {
                for px in bx..bw_end {
                    let off = ((py as usize) * (w as usize) + (px as usize)) * 4;
                    sb += bgra[off] as u64;
                    sg += bgra[off + 1] as u64;
                    sr += bgra[off + 2] as u64;
                    cnt += 1;
                }
            }
            if let (Some(ab), Some(ag), Some(ar)) = (
                sb.checked_div(cnt),
                sg.checked_div(cnt),
                sr.checked_div(cnt),
            ) {
                let (ab, ag, ar) = (ab as u8, ag as u8, ar as u8);
                for py in by..bh_end {
                    for px in bx..bw_end {
                        let off = ((py as usize) * (w as usize) + (px as usize)) * 4;
                        bgra[off] = ab;
                        bgra[off + 1] = ag;
                        bgra[off + 2] = ar;
                        bgra[off + 3] = 255;
                    }
                }
            }
            bx += blk;
        }
        by += blk;
    }
}

/// 对 top-down BGRA 指定矩形执行可调半径均值模糊
pub fn apply_blur_to_bgra(bgra: &mut [u8], w: i32, h: i32, rc: RECT, radius: i32) {
    if w <= 0 || h <= 0 || bgra.len() < (w as usize) * (h as usize) * 4 {
        return;
    }
    let rad = radius.clamp(2, 16);
    let x0 = rc.left.clamp(0, w);
    let y0 = rc.top.clamp(0, h);
    let x1 = rc.right.clamp(0, w);
    let y1 = rc.bottom.clamp(0, h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let orig = bgra.to_vec();
    let step = (rad / 2).max(1);
    for py in y0..y1 {
        for px in x0..x1 {
            let mut sb = 0u32;
            let mut sg = 0u32;
            let mut sr = 0u32;
            let mut cnt = 0u32;
            let sy0 = (py - rad).max(0);
            let sy1 = (py + rad).min(h - 1);
            let sx0 = (px - rad).max(0);
            let sx1 = (px + rad).min(w - 1);
            let mut sy = sy0;
            while sy <= sy1 {
                let mut sx = sx0;
                while sx <= sx1 {
                    let off = ((sy as usize) * (w as usize) + (sx as usize)) * 4;
                    sb += orig[off] as u32;
                    sg += orig[off + 1] as u32;
                    sr += orig[off + 2] as u32;
                    cnt += 1;
                    sx += step;
                }
                sy += step;
            }
            if cnt == 0 {
                continue;
            }
            let off = ((py as usize) * (w as usize) + (px as usize)) * 4;
            bgra[off] = (sb / cnt) as u8;
            bgra[off + 1] = (sg / cnt) as u8;
            bgra[off + 2] = (sr / cnt) as u8;
            bgra[off + 3] = 255;
        }
    }
}

/// 在全屏桌面上截取 32-bit top-down BGRA，返回 `(vx, vy, vw, vh, bgra)`
pub fn capture_virtual_screen_bgra() -> Option<(i32, i32, i32, i32, Vec<u8>)> {
    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if vw <= 0 || vh <= 0 {
            return None;
        }
        let hdc_scr = GetDC(null_mut());
        let hdc_mem = CreateCompatibleDC(hdc_scr);
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: vw,
                biHeight: -vh, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [zeroed()],
        };
        let mut bits: *mut c_void = null_mut();
        let hbm = CreateDIBSection(hdc_scr, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if hbm.is_null() || bits.is_null() {
            DeleteDC(hdc_mem);
            ReleaseDC(null_mut(), hdc_scr);
            return None;
        }
        let old = SelectObject(hdc_mem, hbm);
        BitBlt(hdc_mem, 0, 0, vw, vh, hdc_scr, vx, vy, SRCCOPY);
        GdiFlush();

        let len = (vw as usize) * (vh as usize) * 4;
        let mut out = vec![0u8; len];
        copy_nonoverlapping(bits.cast::<u8>(), out.as_mut_ptr(), len);
        for px in out.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }

        SelectObject(hdc_mem, old);
        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(null_mut(), hdc_scr);
        Some((vx, vy, vw, vh, out))
    }
}

/// 将 top-down BGRA 创建为 32-bit DIB 位图（返回 `HBITMAP` 与可直接写像素的地址）
unsafe fn create_topdown_dib_from_bgra(
    hdc: HDC,
    bgra: &[u8],
    w: i32,
    h: i32,
) -> Option<(HBITMAP, *mut u8)> {
    if w <= 0 || h <= 0 || bgra.len() < (w as usize) * (h as usize) * 4 {
        return None;
    }
    let bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [zeroed()],
    };
    let mut bits: *mut c_void = null_mut();
    let hbm = CreateDIBSection(hdc, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    if hbm.is_null() || bits.is_null() {
        return None;
    }
    copy_nonoverlapping(
        bgra.as_ptr(),
        bits.cast::<u8>(),
        (w as usize) * (h as usize) * 4,
    );
    GdiFlush();
    Some((hbm, bits.cast::<u8>()))
}

// ---- 绘制全部矢量与文本标注 ----

unsafe fn draw_vector_annots(hdc: HDC, annots: &[AnnotItem], off_x: i32, off_y: i32) {
    ensure_gdiplus();
    let mut g: *mut c_void = null_mut();
    let gp_ok = GdipCreateFromHDC(hdc, &mut g) == GP_OK && !g.is_null();
    if gp_ok {
        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);
    }

    for it in annots {
        if matches!(it.tool, AnnotTool::Mosaic | AnnotTool::Blur) {
            continue;
        }
        let x1 = it.x1 - off_x;
        let y1 = it.y1 - off_y;
        let x2 = it.x2 - off_x;
        let y2 = it.y2 - off_y;
        let rc = norm_rect(x1, y1, x2, y2);
        let rw = (rc.right - rc.left) as f32;
        let rh = (rc.bottom - rc.top) as f32;

        if it.tool == AnnotTool::Text {
            if !it.text.is_empty() {
                let font_px = (14 + it.width * 3).max(14);
                let wf = utf8_to_wide("Microsoft YaHei UI");
                let hf = CreateFontW(
                    -font_px,
                    0,
                    0,
                    0,
                    FW_BOLD,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    0,
                    0,
                    CLEARTYPE_QUALITY,
                    0,
                    wf.as_ptr(),
                );
                let old_f = SelectObject(hdc, hf);
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, rgb(it.r, it.g, it.b));
                let mut tr = RECT {
                    left: x1,
                    top: y1,
                    right: (x1 + 1200),
                    bottom: (y1 + 600),
                };
                let wt = utf8_to_wide(&it.text);
                DrawTextW(
                    hdc,
                    wt.as_ptr(),
                    -1,
                    &mut tr,
                    DT_LEFT | DT_TOP | DT_NOPREFIX,
                );
                SelectObject(hdc, old_f);
                DeleteObject(hf);
            }
            continue;
        }

        if !gp_ok {
            continue;
        }

        let alpha = if it.tool == AnnotTool::Marker {
            115
        } else {
            255
        };
        let col = argb(it.r, it.g, it.b, alpha);
        let pen_w = if it.tool == AnnotTool::Marker {
            (it.width as f32) * 3.4
        } else {
            it.width as f32
        };

        let mut pen: *mut c_void = null_mut();
        if GdipCreatePen1(col, pen_w, GP_UNIT_PIXEL, &mut pen) == GP_OK && !pen.is_null() {
            GdipSetPenStartCap(pen, GP_LINE_CAP_ROUND);
            GdipSetPenEndCap(pen, GP_LINE_CAP_ROUND);
            GdipSetPenLineJoin(pen, GP_LINE_JOIN_ROUND);

            match it.tool {
                AnnotTool::Rect => {
                    if it.filled {
                        let mut br: *mut c_void = null_mut();
                        if GdipCreateSolidFill(argb(it.r, it.g, it.b, 200), &mut br) == GP_OK
                            && !br.is_null()
                        {
                            GdipFillRectangle(g, br, rc.left as f32, rc.top as f32, rw, rh);
                            GdipDeleteBrush(br);
                        }
                    } else {
                        GdipDrawRectangle(g, pen, rc.left as f32, rc.top as f32, rw, rh);
                    }
                }
                AnnotTool::Ellipse => {
                    if it.filled {
                        let mut br: *mut c_void = null_mut();
                        if GdipCreateSolidFill(argb(it.r, it.g, it.b, 200), &mut br) == GP_OK
                            && !br.is_null()
                        {
                            GdipFillEllipse(g, br, rc.left as f32, rc.top as f32, rw, rh);
                            GdipDeleteBrush(br);
                        }
                    } else {
                        GdipDrawEllipse(g, pen, rc.left as f32, rc.top as f32, rw, rh);
                    }
                }
                AnnotTool::Line => {
                    GdipDrawLine(g, pen, x1 as f32, y1 as f32, x2 as f32, y2 as f32);
                }
                AnnotTool::Arrow => {
                    let fx1 = x1 as f32;
                    let fy1 = y1 as f32;
                    let fx2 = x2 as f32;
                    let fy2 = y2 as f32;
                    GdipDrawLine(g, pen, fx1, fy1, fx2, fy2);
                    let ang = (fy2 - fy1).atan2(fx2 - fx1);
                    let head = (8.0 + pen_w * 2.2).max(10.0);
                    let p1x = fx2 - head * (ang - 0.42).cos();
                    let p1y = fy2 - head * (ang - 0.42).sin();
                    let p2x = fx2 - head * (ang + 0.42).cos();
                    let p2y = fy2 - head * (ang + 0.42).sin();
                    GdipDrawLine(g, pen, fx2, fy2, p1x, p1y);
                    GdipDrawLine(g, pen, fx2, fy2, p2x, p2y);
                }
                AnnotTool::Pen | AnnotTool::Marker => {
                    if it.pts.len() >= 2 {
                        let pts: Vec<GpPointF> = it
                            .pts
                            .iter()
                            .map(|&(px, py)| GpPointF {
                                x: (px - off_x) as f32,
                                y: (py - off_y) as f32,
                            })
                            .collect();
                        GdipDrawLines(g, pen, pts.as_ptr(), pts.len() as i32);
                    }
                }
                AnnotTool::Number => {
                    let rad = (10 + it.width * 2) as f32;
                    let mut br: *mut c_void = null_mut();
                    if GdipCreateSolidFill(argb(it.r, it.g, it.b, 255), &mut br) == GP_OK
                        && !br.is_null()
                    {
                        GdipFillEllipse(
                            g,
                            br,
                            (x1 as f32) - rad,
                            (y1 as f32) - rad,
                            rad * 2.0,
                            rad * 2.0,
                        );
                        GdipDeleteBrush(br);
                    }
                    let wf = utf8_to_wide("Segoe UI");
                    let hf = CreateFontW(
                        -(rad * 1.25) as i32,
                        0,
                        0,
                        0,
                        FW_BOLD,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET,
                        0,
                        0,
                        CLEARTYPE_QUALITY,
                        0,
                        wf.as_ptr(),
                    );
                    let old_f = SelectObject(hdc, hf);
                    SetBkMode(hdc, TRANSPARENT);
                    SetTextColor(hdc, rgb(255, 255, 255));
                    let mut tr = RECT {
                        left: x1 - rad as i32 - 2,
                        top: y1 - rad as i32 - 2,
                        right: x1 + rad as i32 + 2,
                        bottom: y1 + rad as i32 + 2,
                    };
                    let wn = utf8_to_wide(&it.num.to_string());
                    DrawTextW(
                        hdc,
                        wn.as_ptr(),
                        -1,
                        &mut tr,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                    );
                    SelectObject(hdc, old_f);
                    DeleteObject(hf);
                }
                _ => {}
            }
            GdipDeletePen(pen);
        }
    }

    if gp_ok {
        GdipDeleteGraphics(g);
    }
}

/// 将选区裁剪出来并按顺序叠加像素级滤镜与矢量标注，输出完整 32-bit BGRA
fn flatten_selection(
    orig_bgra: &[u8],
    vw: i32,
    vh: i32,
    sel: RECT,
    annots: &[AnnotItem],
) -> (i32, i32, Vec<u8>) {
    let (cw, ch, mut crop) = crop_bgra(orig_bgra, vw, vh, sel);
    if cw <= 0 || ch <= 0 {
        return (0, 0, Vec::new());
    }
    for it in annots {
        if !matches!(it.tool, AnnotTool::Mosaic | AnnotTool::Blur) {
            continue;
        }
        let local_rc = norm_rect(
            it.x1 - sel.left,
            it.y1 - sel.top,
            it.x2 - sel.left,
            it.y2 - sel.top,
        );
        if it.tool == AnnotTool::Mosaic {
            apply_mosaic_to_bgra(&mut crop, cw, ch, local_rc, it.width * 2 + 4);
        } else {
            apply_blur_to_bgra(&mut crop, cw, ch, local_rc, it.width + 4);
        }
    }

    let has_vectors = annots
        .iter()
        .any(|it| !matches!(it.tool, AnnotTool::Mosaic | AnnotTool::Blur));
    if !has_vectors {
        return (cw, ch, crop);
    }

    unsafe {
        let hdc_scr = GetDC(null_mut());
        let hdc_mem = CreateCompatibleDC(hdc_scr);
        if let Some((hbm, bits)) = create_topdown_dib_from_bgra(hdc_scr, &crop, cw, ch) {
            let old = SelectObject(hdc_mem, hbm);
            draw_vector_annots(hdc_mem, annots, sel.left, sel.top);
            GdiFlush();
            copy_nonoverlapping(bits, crop.as_mut_ptr(), (cw as usize) * (ch as usize) * 4);
            for px in crop.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
            SelectObject(hdc_mem, old);
            DeleteObject(hbm);
        }
        DeleteDC(hdc_mem);
        ReleaseDC(null_mut(), hdc_scr);
    }
    (cw, ch, crop)
}

/// 生成纯文本贴图的 32-bit top-down BGRA 圆角卡片
fn render_text_card_bgra(text: &str) -> (i32, i32, Vec<u8>) {
    unsafe {
        let hdc_scr = GetDC(null_mut());
        let wf = utf8_to_wide("Microsoft YaHei UI");
        let hf = CreateFontW(
            -16,
            0,
            0,
            0,
            FW_NORMAL,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            0,
            0,
            CLEARTYPE_QUALITY,
            0,
            wf.as_ptr(),
        );
        let old_f = SelectObject(hdc_scr, hf);
        let mut calc = RECT {
            left: 0,
            top: 0,
            right: 520,
            bottom: 0,
        };
        let wt = utf8_to_wide(text);
        DrawTextW(
            hdc_scr,
            wt.as_ptr(),
            -1,
            &mut calc,
            DT_LEFT | DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX,
        );
        let cw = (calc.right - calc.left + 28).clamp(120, 580);
        let ch = (calc.bottom - calc.top + 22).clamp(44, 620);

        let hdc_mem = CreateCompatibleDC(hdc_scr);
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: cw,
                biHeight: -ch,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [zeroed()],
        };
        let mut bits: *mut c_void = null_mut();
        let hbm = CreateDIBSection(hdc_scr, &bi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        let mut out = vec![0u8; (cw as usize) * (ch as usize) * 4];
        if !hbm.is_null() && !bits.is_null() {
            let old_bm = SelectObject(hdc_mem, hbm);
            let full = RECT {
                left: 0,
                top: 0,
                right: cw,
                bottom: ch,
            };
            let bg = CreateSolidBrush(rgb(28, 32, 40));
            FillRect(hdc_mem, &full, bg);
            DeleteObject(bg);
            let pen = CreatePen(PS_SOLID, 1, rgb(70, 82, 102));
            let op = SelectObject(hdc_mem, pen);
            let ob = SelectObject(hdc_mem, GetStockObject(NULL_BRUSH));
            crate::ui::win32_ui::Rectangle(hdc_mem, 0, 0, cw, ch);
            SelectObject(hdc_mem, ob);
            SelectObject(hdc_mem, op);
            DeleteObject(pen);

            let old_f2 = SelectObject(hdc_mem, hf);
            SetBkMode(hdc_mem, TRANSPARENT);
            SetTextColor(hdc_mem, rgb(236, 242, 250));
            let mut tr = RECT {
                left: 14,
                top: 11,
                right: cw - 14,
                bottom: ch - 11,
            };
            DrawTextW(
                hdc_mem,
                wt.as_ptr(),
                -1,
                &mut tr,
                DT_LEFT | DT_WORDBREAK | DT_NOPREFIX,
            );
            SelectObject(hdc_mem, old_f2);
            GdiFlush();
            copy_nonoverlapping(bits.cast::<u8>(), out.as_mut_ptr(), out.len());
            for px in out.as_chunks_mut::<4>().0 {
                px[3] = 255;
            }
            SelectObject(hdc_mem, old_bm);
            DeleteObject(hbm);
        }
        SelectObject(hdc_scr, old_f);
        DeleteObject(hf);
        DeleteDC(hdc_mem);
        ReleaseDC(null_mut(), hdc_scr);
        (cw, ch, out)
    }
}

// ---- 全局状态容器 ----

#[derive(Clone, Default)]
struct OverlayState {
    hwnd: HWND,
    active: bool,
    trans_mode: bool, // true = 截屏直接翻译模式，false = 全能工具栏截屏模式
    vw: i32,
    vh: i32,
    orig_bgra: Vec<u8>,
    work_bgra: Vec<u8>, // 含马赛克/模糊后的预览底图（按需局部重算）
    win_rects: Vec<RECT>,

    dragging_sel: bool,
    drag_mode: i32, // 0=无, 1=新建, 2=移动, 10..17=8手柄
    drag_start_pt: POINT,
    drag_orig_rc: RECT,
    has_sel: bool,
    sel_rc: RECT,
    hover_win_rc: RECT,

    cur_tool: AnnotTool,
    cur_size_idx: usize,
    cur_color_idx: usize,
    cur_filled: bool,
    next_num: i32,
    drawing_annot: bool,
    draft_annot: Option<AnnotItem>,
    annots: Vec<AnnotItem>,
    redo_stack: Vec<AnnotItem>,

    hwnd_text_edit: HWND,
    text_edit_pos: POINT,

    btns: Vec<OvBtn>,
    bar_rc: RECT,
    sub_bar_rc: RECT,
    size_rcs: [RECT; 3],
    fill_rc: RECT,
    color_rcs: [RECT; 8],
    hover_btn: i32,
    sub_hover: i32,
    mouse_pt: POINT,
}

#[derive(Clone, Default)]
struct ResultWinState {
    hwnd: HWND,
    img_w: i32,
    img_h: i32,
    bgra_zh: Vec<u8>,
    bgra_orig: Vec<u8>,
    show_orig: bool,
    zh_text: String,
    en_text: String,
    zoom: f64,
    hover_btn: i32,
    btn_rcs: [RECT; 7],
    dragging: bool,
    drag_off: POINT,
}

#[derive(Clone)]
struct PinEntry {
    hwnd: HWND,
    img_w: i32,
    img_h: i32,
    bgra_orig: Vec<u8>,
    bgra_zh: Vec<u8>,
    show_zh: bool,
    translating: bool,
    zh_text: String,
    en_text: String,
    zoom: f64,
    alpha: u8,
    hover_trans: bool,
    dragging: bool,
    drag_off: POINT,
}

#[derive(Default)]
struct SnipManager {
    cfg: SnipCfg,
    classes_registered: bool,
    overlay: OverlayState,
    result: ResultWinState,
    pins: Vec<PinEntry>,
    last_cap_w: i32,
    last_cap_h: i32,
    last_cap_bgra: Vec<u8>,
}

thread_local! {
    static SNIP_MGR: RefCell<SnipManager> = RefCell::new(SnipManager::default());
}

pub fn update_snip_cfg(cfg: SnipCfg) {
    SNIP_MGR.with(|m| m.borrow_mut().cfg = cfg);
}

pub fn has_active_pins() -> bool {
    SNIP_MGR.with(|m| !m.borrow().pins.is_empty())
}

pub fn close_all_pins() {
    let hwnds: Vec<HWND> = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let hs = mgr.pins.iter().map(|p| p.hwnd).collect();
        mgr.pins.clear();
        hs
    });
    for h in hwnds {
        if !h.is_null() {
            unsafe { DestroyWindow(h) };
        }
    }
}

unsafe fn ensure_snip_classes() {
    let already = SNIP_MGR.with(|m| m.borrow().classes_registered);
    if already {
        return;
    }
    let h_inst = GetModuleHandleW(null());
    let reg =
        |name: &str, wnd_proc: crate::ui::win32_ui::WNDPROC, cursor: *const u16, dbl: bool| {
            let cls = utf8_to_wide(name);
            let wc = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW | if dbl { CS_DBLCLKS } else { 0 },
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: h_inst,
                hIcon: null_mut(),
                hCursor: LoadCursorW(null_mut(), cursor),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: cls.as_ptr(),
                hIconSm: null_mut(),
            };
            RegisterClassExW(&wc);
        };
    reg(OVERLAY_CLASS_NAME, overlay_wnd_proc, IDC_CROSS, true);
    reg(RESULT_CLASS_NAME, result_wnd_proc, IDC_ARROW, false);
    reg(PIN_CLASS_NAME, pin_wnd_proc, IDC_ARROW, true);
    SNIP_MGR.with(|m| m.borrow_mut().classes_registered = true);
}

// ---- 顶层窗口自动识别 ----

unsafe fn enum_visible_windows(vx: i32, vy: i32) -> Vec<RECT> {
    struct Ctx {
        own_pid: u32,
        vx: i32,
        vy: i32,
        list: Vec<RECT>,
    }
    unsafe extern "system" fn enum_cb(hwnd: HWND, lp: LPARAM) -> i32 {
        let ctx = &mut *(lp as *mut Ctx);
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == ctx.own_pid {
            return 1;
        }
        let mut cloaked: u32 = 0;
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast(),
            size_of::<u32>() as u32,
        ) == 0
            && cloaked != 0
        {
            return 1;
        }
        let mut rc = RECT::default();
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            (&mut rc as *mut RECT).cast(),
            size_of::<RECT>() as u32,
        ) != 0
            && GetWindowRect(hwnd, &mut rc) == 0
        {
            return 1;
        }
        let w = rc.right - rc.left;
        let h = rc.bottom - rc.top;
        if w > 12 && h > 12 {
            ctx.list.push(RECT {
                left: rc.left - ctx.vx,
                top: rc.top - ctx.vy,
                right: rc.right - ctx.vx,
                bottom: rc.bottom - ctx.vy,
            });
        }
        1
    }
    let mut ctx = Ctx {
        own_pid: GetCurrentProcessId(),
        vx,
        vy,
        list: Vec::new(),
    };
    EnumWindows(enum_cb, (&mut ctx as *mut Ctx) as LPARAM);
    ctx.list
}

// ---- 工具栏布局与命中 ----

fn tool_to_btn(t: AnnotTool) -> i32 {
    match t {
        AnnotTool::None => 0,
        AnnotTool::Rect => BTN_RECT,
        AnnotTool::Ellipse => BTN_ELLIPSE,
        AnnotTool::Line => BTN_LINE,
        AnnotTool::Arrow => BTN_ARROW,
        AnnotTool::Pen => BTN_PEN,
        AnnotTool::Marker => BTN_MARKER,
        AnnotTool::Number => BTN_NUMBER,
        AnnotTool::Text => BTN_TEXT,
        AnnotTool::Mosaic => BTN_MOSAIC,
        AnnotTool::Blur => BTN_BLUR,
    }
}

fn layout_overlay_toolbar(ov: &mut OverlayState) {
    ov.btns.clear();
    let sel = norm_rect(
        ov.sel_rc.left,
        ov.sel_rc.top,
        ov.sel_rc.right,
        ov.sel_rc.bottom,
    );
    let specs: [(i32, i32, &str, &str); 17] = [
        (BTN_RECT, 28, "", "矩形"),
        (BTN_ELLIPSE, 28, "", "椭圆"),
        (BTN_LINE, 28, "", "直线"),
        (BTN_ARROW, 28, "", "箭头"),
        (BTN_PEN, 28, "", "画笔"),
        (BTN_MARKER, 28, "", "荧光笔"),
        (BTN_NUMBER, 28, "", "序号"),
        (BTN_TEXT, 28, "", "文字"),
        (BTN_MOSAIC, 28, "", "马赛克"),
        (BTN_BLUR, 28, "", "模糊"),
        (BTN_UNDO, 28, "", "撤销 (Ctrl+Z)"),
        (BTN_REDO, 28, "", "重做 (Ctrl+Y)"),
        (
            BTN_SAVE,
            44,
            "保存",
            "Ctrl+S 另存为 / Ctrl+Shift+S 快速保存",
        ),
        (BTN_PIN, 44, "贴图", "贴图到屏幕"),
        (BTN_TRANS, 44, "翻译", "识别选区英文并翻译"),
        (BTN_CANCEL, 44, "取消", "取消截屏 (Esc)"),
        (BTN_COPY, 52, "完成", "完成并复制到剪贴板 (Enter)"),
    ];
    let gap = 3i32;
    let total_w: i32 =
        specs.iter().map(|s| s.1).sum::<i32>() + gap * ((specs.len() as i32) - 1) + 20;
    let bar_h = 34;
    let has_sub = ov.cur_tool != AnnotTool::None;
    let extra_h = if has_sub { 38 } else { 0 };
    let bx = (sel.right - total_w).clamp(8, (ov.vw - total_w - 8).max(8));
    let mut by = sel.bottom + 8;
    if by + bar_h + extra_h > ov.vh - 6 {
        by = (sel.top - bar_h - 8 - extra_h).max(6);
    }
    ov.bar_rc = RECT {
        left: bx,
        top: by,
        right: bx + total_w,
        bottom: by + bar_h,
    };

    let mut x = bx + 6;
    for (idx, (id, w, lbl, tip)) in specs.into_iter().enumerate() {
        if idx == 10 || idx == 12 {
            x += 5;
        }
        ov.btns.push(OvBtn {
            id,
            rect: RECT {
                left: x,
                top: by + 3,
                right: x + w,
                bottom: by + 31,
            },
            label: lbl,
            tip,
        });
        x += w + gap;
    }

    if has_sub {
        let sub_w = 344;
        let sx = bx.clamp(8, (ov.vw - sub_w - 8).max(8));
        let sy = by + bar_h + 4;
        ov.sub_bar_rc = RECT {
            left: sx,
            top: sy,
            right: sx + sub_w,
            bottom: sy + 32,
        };
        let mut rx = sx + 8;
        for i in 0..3 {
            ov.size_rcs[i] = RECT {
                left: rx,
                top: sy + 4,
                right: rx + 24,
                bottom: sy + 28,
            };
            rx += 28;
        }
        ov.fill_rc = RECT {
            left: rx + 4,
            top: sy + 4,
            right: rx + 48,
            bottom: sy + 28,
        };
        let mut cx = if matches!(ov.cur_tool, AnnotTool::Rect | AnnotTool::Ellipse) {
            rx + 58
        } else {
            rx + 8
        };
        for i in 0..8 {
            ov.color_rcs[i] = RECT {
                left: cx,
                top: sy + 6,
                right: cx + 20,
                bottom: sy + 26,
            };
            cx += 24;
        }
    } else {
        ov.sub_bar_rc = RECT::default();
    }
}

fn rebuild_work_bgra(ov: &mut OverlayState) {
    ov.work_bgra = ov.orig_bgra.clone();
    for it in &ov.annots {
        let rc = norm_rect(it.x1, it.y1, it.x2, it.y2);
        match it.tool {
            AnnotTool::Mosaic => {
                apply_mosaic_to_bgra(&mut ov.work_bgra, ov.vw, ov.vh, rc, it.width * 2 + 4)
            }
            AnnotTool::Blur => {
                apply_blur_to_bgra(&mut ov.work_bgra, ov.vw, ov.vh, rc, it.width + 4)
            }
            _ => {}
        }
    }
    if let Some(d) = &ov.draft_annot {
        let rc = norm_rect(d.x1, d.y1, d.x2, d.y2);
        match d.tool {
            AnnotTool::Mosaic => {
                apply_mosaic_to_bgra(&mut ov.work_bgra, ov.vw, ov.vh, rc, d.width * 2 + 4)
            }
            AnnotTool::Blur => apply_blur_to_bgra(&mut ov.work_bgra, ov.vw, ov.vh, rc, d.width + 4),
            _ => {}
        }
    }
}

// ---- 启动区域截屏 / 截屏翻译 ----

pub fn start_snip_translate() {
    start_overlay(true);
}

pub fn start_snip_capture() {
    start_overlay(false);
}

fn start_overlay(trans_mode: bool) {
    let active = SNIP_MGR.with(|m| m.borrow().overlay.active);
    if active {
        return;
    }
    let Some((vx, vy, vw, vh, bgra)) = capture_virtual_screen_bgra() else {
        return;
    };
    unsafe { ensure_snip_classes() };
    let win_rects = unsafe { enum_visible_windows(vx, vy) };

    SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        mgr.overlay = OverlayState {
            active: true,
            trans_mode,
            vw,
            vh,
            orig_bgra: bgra.clone(),
            work_bgra: bgra,
            win_rects,
            cur_tool: AnnotTool::None,
            cur_size_idx: 1,
            cur_color_idx: 0,
            next_num: 1,
            hover_btn: 0,
            sub_hover: 0,
            ..Default::default()
        };
    });

    unsafe {
        let h_inst = GetModuleHandleW(null());
        let cls = utf8_to_wide(OVERLAY_CLASS_NAME);
        let title = utf8_to_wide("");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            cls.as_ptr(),
            title.as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN,
            vx,
            vy,
            vw,
            vh,
            null_mut(),
            null_mut(),
            h_inst,
            null_mut(),
        );
        if hwnd.is_null() {
            SNIP_MGR.with(|m| m.borrow_mut().overlay.active = false);
            return;
        }
        SNIP_MGR.with(|m| m.borrow_mut().overlay.hwnd = hwnd);
        ShowWindow(hwnd, SW_SHOW);
        crate::ui::win32_ui::SetForegroundWindow(hwnd);
        SetFocus(hwnd);
    }
}

fn close_overlay() {
    let (hwnd, edt) = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let h = mgr.overlay.hwnd;
        let e = mgr.overlay.hwnd_text_edit;
        mgr.overlay = OverlayState::default();
        (h, e)
    });
    unsafe {
        if !edt.is_null() {
            DestroyWindow(edt);
        }
        if !hwnd.is_null() {
            DestroyWindow(hwnd);
        }
    }
}

unsafe fn take_and_commit_inline_text_edit() {
    let (edt, txt, pos, rgb_c, width) = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let ov = &mut mgr.overlay;
        let e = ov.hwnd_text_edit;
        if e.is_null() {
            return (null_mut(), String::new(), POINT::default(), (0, 0, 0), 0);
        }
        let txt = get_window_text(e).trim().to_string();
        ov.hwnd_text_edit = null_mut();
        (
            e,
            txt,
            ov.text_edit_pos,
            ANNOT_COLORS[ov.cur_color_idx],
            SIZE_WIDTHS[ov.cur_size_idx],
        )
    });
    if edt.is_null() {
        return;
    }
    // 借用释放后再销毁 Win32 控件，彻底防止 DestroyWindow 同步消息重入 SNIP_MGR
    DestroyWindow(edt);
    if !txt.is_empty() {
        SNIP_MGR.with(|m| {
            let ov = &mut m.borrow_mut().overlay;
            ov.annots.push(AnnotItem {
                tool: AnnotTool::Text,
                x1: pos.x,
                y1: pos.y,
                x2: pos.x,
                y2: pos.y,
                pts: Vec::new(),
                r: rgb_c.0,
                g: rgb_c.1,
                b: rgb_c.2,
                width,
                filled: false,
                num: 0,
                text: txt,
            });
            ov.redo_stack.clear();
        });
    }
}

unsafe extern "system" fn text_edit_subclass(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
    _id: UINT_PTR,
    _ref: DWORD_PTR,
) -> LRESULT {
    if msg == WM_KEYDOWN && wp as i32 == VK_ESCAPE {
        let p = crate::ui::win32_ui::GetParent(hwnd);
        let e = SNIP_MGR.with(|m| {
            let mut mgr = m.borrow_mut();
            let old = mgr.overlay.hwnd_text_edit;
            mgr.overlay.hwnd_text_edit = null_mut();
            old
        });
        if !e.is_null() {
            DestroyWindow(e);
        }
        if !p.is_null() {
            InvalidateRect(p, null(), 0);
        }
        return 0;
    }
    if msg == WM_KEYDOWN
        && wp as i32 == VK_RETURN
        && (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) == 0
    {
        let p = crate::ui::win32_ui::GetParent(hwnd);
        take_and_commit_inline_text_edit();
        if !p.is_null() {
            InvalidateRect(p, null(), 0);
        }
        return 0;
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

fn finish_selection_action(action: i32) {
    unsafe { take_and_commit_inline_text_edit() };
    let (cw, ch, flat_bgra, cfg) = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let sel = norm_rect(
            mgr.overlay.sel_rc.left,
            mgr.overlay.sel_rc.top,
            mgr.overlay.sel_rc.right,
            mgr.overlay.sel_rc.bottom,
        );
        let (cw, ch, flat) = flatten_selection(
            &mgr.overlay.orig_bgra,
            mgr.overlay.vw,
            mgr.overlay.vh,
            sel,
            &mgr.overlay.annots,
        );
        if cw > 0 && ch > 0 {
            mgr.last_cap_w = cw;
            mgr.last_cap_h = ch;
            mgr.last_cap_bgra = flat.clone();
        }
        (cw, ch, flat, mgr.cfg.clone())
    });

    close_overlay();
    if cw <= 0 || ch <= 0 || flat_bgra.is_empty() {
        return;
    }

    match action {
        BTN_COPY => {
            set_clipboard_bgra(&flat_bgra, cw, ch);
        }
        BTN_SAVE => {
            let now = unsafe {
                let mut st = zeroed();
                crate::core::win32::GetLocalTime(&mut st);
                format!(
                    "KuaiYiNG_{:04}{:02}{:02}_{:02}{:02}{:02}.png",
                    st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
                )
            };
            let quick_save = (unsafe { GetAsyncKeyState(VK_SHIFT) } as u16 & 0x8000) != 0;
            let save_path = if quick_save && !cfg.save_dir.is_empty() {
                let _ = std::fs::create_dir_all(&cfg.save_dir);
                Some(
                    PathBuf::from(&cfg.save_dir)
                        .join(&now)
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                save_file_dialog_png(cfg.hwnd_main as HWND, &cfg.save_dir, &now)
            };
            if let Some(p) = save_path {
                save_bgra_as_png(&p, &flat_bgra, cw, ch);
            }
        }
        BTN_PIN => {
            create_pin_window_bgra(cw, ch, flat_bgra, String::new(), String::new());
        }
        BTN_TRANS => {
            trigger_ocr_result_thread(cw, ch, flat_bgra, cfg);
        }
        _ => {}
    }
}

struct SnipDonePayload {
    res: SnipResult,
    orig_bgra: Vec<u8>,
}

fn trigger_ocr_result_thread(w: i32, h: i32, bgra: Vec<u8>, cfg: SnipCfg) {
    let hwnd_main = cfg.hwnd_main;
    thread::spawn(move || {
        let orig = bgra.clone();
        let res = snip_translate(
            &bgra,
            w,
            h,
            &cfg.ai_url,
            &cfg.ai_key,
            &cfg.ai_model,
            &cfg.baidu_appid,
            &cfg.baidu_secret,
            &cfg.baidu_url,
            &cfg.cache_dir,
        );
        if hwnd_main != 0 {
            let payload = Box::into_raw(Box::new(SnipDonePayload {
                res,
                orig_bgra: orig,
            }));
            unsafe { PostMessageW(hwnd_main as HWND, WM_APP_SNIP_DONE, 0, payload as LPARAM) };
        }
    });
}

/// 主线程收到截屏 OCR 结果消息时调用
pub fn handle_snip_done_message(lp: LPARAM) {
    if lp == 0 {
        return;
    }
    let payload = unsafe { Box::from_raw(lp as *mut SnipDonePayload) };
    if !payload.res.ok {
        log_write("截屏OCR", &payload.res.err);
        let msg = utf8_to_wide(&format!("截屏翻译失败：{}", payload.res.err));
        let cap = utf8_to_wide("快译NG");
        let hwnd_main = SNIP_MGR.with(|m| m.borrow().cfg.hwnd_main as HWND);
        unsafe { crate::ui::win32_ui::MessageBoxW(hwnd_main, msg.as_ptr(), cap.as_ptr(), 0x30) };
        return;
    }
    open_result_window(
        payload.res.width,
        payload.res.height,
        payload.res.bgra_out,
        payload.orig_bgra,
        payload.res.zh_text,
        payload.res.en_text,
    );
}

// ---- Overlay 绘制与事件处理 ----

unsafe fn draw_overlay_tool_icon(g: *mut c_void, id: i32, rc: RECT, col: u32) {
    let cx = (rc.left + rc.right) as f32 * 0.5;
    let cy = (rc.top + rc.bottom) as f32 * 0.5;
    let mut pen: *mut c_void = null_mut();
    let mut br: *mut c_void = null_mut();
    GdipCreatePen1(col, 1.6, GP_UNIT_PIXEL, &mut pen);
    GdipCreateSolidFill(col, &mut br);
    if !pen.is_null() {
        GdipSetPenStartCap(pen, GP_LINE_CAP_ROUND);
        GdipSetPenEndCap(pen, GP_LINE_CAP_ROUND);
        GdipSetPenLineJoin(pen, GP_LINE_JOIN_ROUND);
    }
    match id {
        BTN_RECT if !pen.is_null() => {
            GdipDrawRectangle(g, pen, cx - 6.5, cy - 5.0, 13.0, 10.0);
        }
        BTN_ELLIPSE if !pen.is_null() => {
            GdipDrawEllipse(g, pen, cx - 6.5, cy - 5.0, 13.0, 10.0);
        }
        BTN_LINE if !pen.is_null() => {
            GdipDrawLine(g, pen, cx - 6.0, cy + 5.0, cx + 6.0, cy - 5.0);
        }
        BTN_ARROW if !pen.is_null() => {
            GdipDrawLine(g, pen, cx - 5.5, cy + 5.5, cx + 5.5, cy - 5.5);
            GdipDrawLine(g, pen, cx + 5.5, cy - 5.5, cx + 0.5, cy - 5.5);
            GdipDrawLine(g, pen, cx + 5.5, cy - 5.5, cx + 5.5, cy - 0.5);
        }
        BTN_PEN if !pen.is_null() => {
            let pts = [
                GpPointF {
                    x: cx - 6.0,
                    y: cy + 3.0,
                },
                GpPointF {
                    x: cx - 2.0,
                    y: cy - 4.0,
                },
                GpPointF {
                    x: cx + 2.0,
                    y: cy + 4.0,
                },
                GpPointF {
                    x: cx + 6.0,
                    y: cy - 3.0,
                },
            ];
            GdipDrawLines(g, pen, pts.as_ptr(), 4);
        }
        BTN_MARKER => {
            let mut pw: *mut c_void = null_mut();
            if GdipCreatePen1(
                (col & 0x00FF_FFFF) | (135 << 24),
                5.0,
                GP_UNIT_PIXEL,
                &mut pw,
            ) == GP_OK
                && !pw.is_null()
            {
                GdipSetPenStartCap(pw, GP_LINE_CAP_ROUND);
                GdipSetPenEndCap(pw, GP_LINE_CAP_ROUND);
                GdipDrawLine(g, pw, cx - 5.0, cy + 4.0, cx + 5.0, cy - 4.0);
                GdipDeletePen(pw);
            }
            if !pen.is_null() {
                GdipDrawLine(g, pen, cx - 5.5, cy + 6.0, cx + 5.5, cy + 6.0);
            }
        }
        BTN_NUMBER if !pen.is_null() => {
            GdipDrawEllipse(g, pen, cx - 6.5, cy - 6.5, 13.0, 13.0);
            GdipDrawLine(g, pen, cx, cy - 3.0, cx, cy + 3.5);
        }
        BTN_TEXT if !pen.is_null() => {
            let pts = [
                GpPointF {
                    x: cx - 4.5,
                    y: cy + 5.5,
                },
                GpPointF { x: cx, y: cy - 5.5 },
                GpPointF {
                    x: cx + 4.5,
                    y: cy + 5.5,
                },
            ];
            GdipDrawLines(g, pen, pts.as_ptr(), 3);
            GdipDrawLine(g, pen, cx - 2.8, cy + 1.5, cx + 2.8, cy + 1.5);
        }
        BTN_MOSAIC if !br.is_null() => {
            for r in 0..3 {
                for c in 0..3 {
                    if ((r + c) & 1) == 0 {
                        GdipFillRectangle(
                            g,
                            br,
                            cx - 6.0 + (c as f32) * 4.2,
                            cy - 6.0 + (r as f32) * 4.2,
                            3.6,
                            3.6,
                        );
                    }
                }
            }
        }
        BTN_BLUR if !pen.is_null() => {
            GdipDrawEllipse(g, pen, cx - 6.0, cy - 6.0, 12.0, 12.0);
            GdipDrawEllipse(g, pen, cx - 2.8, cy - 2.8, 5.6, 5.6);
        }
        BTN_UNDO if !pen.is_null() => {
            let pts = [
                GpPointF {
                    x: cx - 5.5,
                    y: cy - 1.5,
                },
                GpPointF {
                    x: cx + 2.0,
                    y: cy - 4.5,
                },
                GpPointF {
                    x: cx + 5.5,
                    y: cy + 0.5,
                },
                GpPointF {
                    x: cx + 1.5,
                    y: cy + 4.5,
                },
            ];
            GdipDrawLines(g, pen, pts.as_ptr(), 4);
            GdipDrawLine(g, pen, cx - 5.5, cy - 1.5, cx - 1.5, cy - 4.5);
            GdipDrawLine(g, pen, cx - 5.5, cy - 1.5, cx - 2.5, cy + 2.2);
        }
        BTN_REDO if !pen.is_null() => {
            let pts = [
                GpPointF {
                    x: cx + 5.5,
                    y: cy - 1.5,
                },
                GpPointF {
                    x: cx - 2.0,
                    y: cy - 4.5,
                },
                GpPointF {
                    x: cx - 5.5,
                    y: cy + 0.5,
                },
                GpPointF {
                    x: cx - 1.5,
                    y: cy + 4.5,
                },
            ];
            GdipDrawLines(g, pen, pts.as_ptr(), 4);
            GdipDrawLine(g, pen, cx + 5.5, cy - 1.5, cx + 1.5, cy - 4.5);
            GdipDrawLine(g, pen, cx + 5.5, cy - 1.5, cx + 2.5, cy + 2.2);
        }
        _ => {}
    }
    if !pen.is_null() {
        GdipDeletePen(pen);
    }
    if !br.is_null() {
        GdipDeleteBrush(br);
    }
}

unsafe fn draw_overlay_magnifier(hdc_back: HDC, hdc_orig: HDC, ov: &OverlayState, sel: RECT) {
    let mx = ov.mouse_pt.x;
    let my = ov.mouse_pt.y;
    if mx < 0 || my < 0 || mx >= ov.vw || my >= ov.vh {
        return;
    }
    const BOX_W: i32 = 132;
    const MAG_H: i32 = 96;
    const INFO_H: i32 = 60;
    const BOX_H: i32 = MAG_H + INFO_H;

    let mut bx = mx + 18;
    let mut by = my + 22;
    if bx + BOX_W > ov.vw - 6 {
        bx = mx - BOX_W - 18;
    }
    if by + BOX_H > ov.vh - 6 {
        by = my - BOX_H - 18;
    }
    bx = bx.clamp(4, (ov.vw - BOX_W - 4).max(4));
    by = by.clamp(4, (ov.vh - BOX_H - 4).max(4));

    let full_rc = RECT {
        left: bx,
        top: by,
        right: bx + BOX_W,
        bottom: by + BOX_H,
    };
    let bg = CreateSolidBrush(rgb(24, 27, 34));
    FillRect(hdc_back, &full_rc, bg);
    DeleteObject(bg);

    let sx = mx - 5;
    let sy = my - 4;
    let cl = sx.max(0);
    let ct = sy.max(0);
    let cr = (sx + 11).min(ov.vw);
    let cb = (sy + 8).min(ov.vh);
    if cr > cl && cb > ct {
        SetStretchBltMode(hdc_back, COLORONCOLOR);
        StretchBlt(
            hdc_back,
            bx + (cl - sx) * 12,
            by + (ct - sy) * 12,
            (cr - cl) * 12,
            (cb - ct) * 12,
            hdc_orig,
            cl,
            ct,
            cr - cl,
            cb - ct,
            SRCCOPY,
        );
    }

    let pen_cross = CreatePen(PS_SOLID, 1, rgb(59, 130, 246));
    let op = SelectObject(hdc_back, pen_cross);
    let ob = SelectObject(hdc_back, GetStockObject(NULL_BRUSH));
    crate::ui::win32_ui::Rectangle(hdc_back, bx + 60, by + 48, bx + 72, by + 60);
    SelectObject(hdc_back, ob);
    SelectObject(hdc_back, op);
    DeleteObject(pen_cross);

    let off = ((my as usize) * (ov.vw as usize) + (mx as usize)) * 4;
    let (b_v, g_v, r_v) = (
        ov.orig_bgra[off],
        ov.orig_bgra[off + 1],
        ov.orig_bgra[off + 2],
    );
    let hex_s = format!("#{r_v:02X}{g_v:02X}{b_v:02X}");

    let sw_rc = RECT {
        left: bx + 8,
        top: by + MAG_H + 8,
        right: bx + 22,
        bottom: by + MAG_H + 22,
    };
    let sw_br = CreateSolidBrush(rgb(r_v, g_v, b_v));
    FillRect(hdc_back, &sw_rc, sw_br);
    DeleteObject(sw_br);

    let wf = utf8_to_wide("Microsoft YaHei UI");
    let hf = CreateFontW(
        -11,
        0,
        0,
        0,
        FW_NORMAL,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        0,
        0,
        CLEARTYPE_QUALITY,
        0,
        wf.as_ptr(),
    );
    let old_f = SelectObject(hdc_back, hf);
    SetBkMode(hdc_back, TRANSPARENT);
    SetTextColor(hdc_back, rgb(236, 242, 252));
    let sw = (sel.right - sel.left).max(0);
    let sh = (sel.bottom - sel.top).max(0);
    let mut r1 = RECT {
        left: bx + 28,
        top: by + MAG_H + 5,
        right: bx + BOX_W - 4,
        bottom: by + MAG_H + 24,
    };
    let wt1 = utf8_to_wide(&format!("{hex_s}  {sw}×{sh}"));
    DrawTextW(
        hdc_back,
        wt1.as_ptr(),
        -1,
        &mut r1,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    SetTextColor(hdc_back, rgb(148, 163, 184));
    let mut r2 = RECT {
        left: bx + 8,
        top: by + MAG_H + 25,
        right: bx + BOX_W - 4,
        bottom: by + BOX_H - 4,
    };
    let wt2 = utf8_to_wide(&format!("RGB({r_v},{g_v},{b_v})\n按 C 复制色值"));
    DrawTextW(
        hdc_back,
        wt2.as_ptr(),
        -1,
        &mut r2,
        DT_LEFT | DT_TOP | DT_NOPREFIX,
    );
    SelectObject(hdc_back, old_f);
    DeleteObject(hf);

    let pen_bd = CreatePen(PS_SOLID, 1, rgb(68, 76, 92));
    let op2 = SelectObject(hdc_back, pen_bd);
    let ob2 = SelectObject(hdc_back, GetStockObject(NULL_BRUSH));
    crate::ui::win32_ui::Rectangle(hdc_back, bx, by, bx + BOX_W, by + BOX_H);
    SelectObject(hdc_back, ob2);
    SelectObject(hdc_back, op2);
    DeleteObject(pen_bd);
}

unsafe fn paint_overlay(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let ov = SNIP_MGR.with(|m| m.borrow().overlay.clone());
    if ov.vw <= 0 || ov.vh <= 0 {
        EndPaint(hwnd, &ps);
        return;
    }

    let hdc_back = CreateCompatibleDC(hdc);
    let hbm_back = CreateCompatibleBitmap(hdc, ov.vw, ov.vh);
    let old_back = SelectObject(hdc_back, hbm_back);

    let hdc_src = CreateCompatibleDC(hdc);
    let Some((hbm_src, _)) = create_topdown_dib_from_bgra(hdc, &ov.work_bgra, ov.vw, ov.vh) else {
        SelectObject(hdc_back, old_back);
        DeleteObject(hbm_back);
        DeleteDC(hdc_back);
        DeleteDC(hdc_src);
        EndPaint(hwnd, &ps);
        return;
    };
    let old_src = SelectObject(hdc_src, hbm_src);
    BitBlt(hdc_back, 0, 0, ov.vw, ov.vh, hdc_src, 0, 0, SRCCOPY);

    // 绘制矢量标注（原图坐标偏移 0,0）
    let mut all_vec = ov.annots.clone();
    if let Some(d) = &ov.draft_annot {
        all_vec.push(d.clone());
    }
    draw_vector_annots(hdc_back, &all_vec, 0, 0);

    // 暗化 45% 蒙版（用 1x1 黑色位图绕过选区或高亮悬停窗口）
    let sel = if ov.has_sel {
        norm_rect(
            ov.sel_rc.left,
            ov.sel_rc.top,
            ov.sel_rc.right,
            ov.sel_rc.bottom,
        )
    } else if ov.hover_win_rc.right > ov.hover_win_rc.left {
        ov.hover_win_rc
    } else {
        RECT::default()
    };

    let dc_blk = CreateCompatibleDC(hdc);
    let mut px_blk = [0u8, 0, 0, 255];
    if let Some((bm_blk, _)) = create_topdown_dib_from_bgra(hdc, &px_blk, 1, 1) {
        let ob_blk = SelectObject(dc_blk, bm_blk);
        let bf = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER,
            BlendFlags: 0,
            SourceConstantAlpha: 120,
            AlphaFormat: 0,
        };
        if sel.right > sel.left && sel.bottom > sel.top {
            if sel.top > 0 {
                AlphaBlend(hdc_back, 0, 0, ov.vw, sel.top, dc_blk, 0, 0, 1, 1, bf);
            }
            if sel.bottom < ov.vh {
                AlphaBlend(
                    hdc_back,
                    0,
                    sel.bottom,
                    ov.vw,
                    ov.vh - sel.bottom,
                    dc_blk,
                    0,
                    0,
                    1,
                    1,
                    bf,
                );
            }
            if sel.left > 0 {
                AlphaBlend(
                    hdc_back,
                    0,
                    sel.top,
                    sel.left,
                    sel.bottom - sel.top,
                    dc_blk,
                    0,
                    0,
                    1,
                    1,
                    bf,
                );
            }
            if sel.right < ov.vw {
                AlphaBlend(
                    hdc_back,
                    sel.right,
                    sel.top,
                    ov.vw - sel.right,
                    sel.bottom - sel.top,
                    dc_blk,
                    0,
                    0,
                    1,
                    1,
                    bf,
                );
            }
        } else {
            AlphaBlend(hdc_back, 0, 0, ov.vw, ov.vh, dc_blk, 0, 0, 1, 1, bf);
        }
        let _ = &mut px_blk;
        SelectObject(dc_blk, ob_blk);
        DeleteObject(bm_blk);
    }
    DeleteDC(dc_blk);

    // 选区亮蓝边界与 8 个调节手柄 + 尺寸角标
    if sel.right > sel.left && sel.bottom > sel.top {
        let pen = CreatePen(PS_SOLID, 2, rgb(59, 130, 246));
        let op = SelectObject(hdc_back, pen);
        let ob = SelectObject(hdc_back, GetStockObject(NULL_BRUSH));
        crate::ui::win32_ui::Rectangle(hdc_back, sel.left, sel.top, sel.right, sel.bottom);
        SelectObject(hdc_back, ob);
        SelectObject(hdc_back, op);
        DeleteObject(pen);

        if ov.has_sel {
            let hs = HANDLE_SIZE / 2;
            let pts = [
                (sel.left, sel.top),
                ((sel.left + sel.right) / 2, sel.top),
                (sel.right, sel.top),
                (sel.right, (sel.top + sel.bottom) / 2),
                (sel.right, sel.bottom),
                ((sel.left + sel.right) / 2, sel.bottom),
                (sel.left, sel.bottom),
                (sel.left, (sel.top + sel.bottom) / 2),
            ];
            let br_w = CreateSolidBrush(rgb(255, 255, 255));
            let pen_h = CreatePen(PS_SOLID, 1, rgb(59, 130, 246));
            let op_h = SelectObject(hdc_back, pen_h);
            for (hx, hy) in pts {
                let r = RECT {
                    left: hx - hs,
                    top: hy - hs,
                    right: hx + hs + 1,
                    bottom: hy + hs + 1,
                };
                FillRect(hdc_back, &r, br_w);
                let ob_h = SelectObject(hdc_back, GetStockObject(NULL_BRUSH));
                crate::ui::win32_ui::Rectangle(hdc_back, r.left, r.top, r.right, r.bottom);
                SelectObject(hdc_back, ob_h);
            }
            SelectObject(hdc_back, op_h);
            DeleteObject(pen_h);
            DeleteObject(br_w);
        }

        // 左上角尺寸提示徽标
        let sz_txt = format!("{} × {}", sel.right - sel.left, sel.bottom - sel.top);
        let tag_y = if sel.top >= 26 {
            sel.top - 24
        } else {
            sel.top + 4
        };
        let mut tr = RECT {
            left: sel.left.clamp(4, (ov.vw - 110).max(4)),
            top: tag_y,
            right: (sel.left + 104).min(ov.vw - 4),
            bottom: tag_y + 20,
        };
        let br_t = CreateSolidBrush(rgb(20, 24, 32));
        FillRect(hdc_back, &tr, br_t);
        DeleteObject(br_t);
        let wf = utf8_to_wide("Segoe UI");
        let hf = CreateFontW(
            -12,
            0,
            0,
            0,
            FW_MEDIUM,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            0,
            0,
            CLEARTYPE_QUALITY,
            0,
            wf.as_ptr(),
        );
        let old_f = SelectObject(hdc_back, hf);
        SetBkMode(hdc_back, TRANSPARENT);
        SetTextColor(hdc_back, rgb(240, 244, 250));
        let wt = utf8_to_wide(&sz_txt);
        DrawTextW(
            hdc_back,
            wt.as_ptr(),
            -1,
            &mut tr,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(hdc_back, old_f);
        DeleteObject(hf);
    }

    // 选区完成且非翻译极速模式下绘制悬浮工具栏与子栏
    if ov.has_sel && !ov.dragging_sel && !ov.trans_mode && !ov.btns.is_empty() {
        let br_bar = CreateSolidBrush(rgb(30, 34, 42));
        FillRect(hdc_back, &ov.bar_rc, br_bar);
        DeleteObject(br_bar);
        let p_bd = CreatePen(PS_SOLID, 1, rgb(68, 76, 92));
        let op_b = SelectObject(hdc_back, p_bd);
        let ob_b = SelectObject(hdc_back, GetStockObject(NULL_BRUSH));
        crate::ui::win32_ui::Rectangle(
            hdc_back,
            ov.bar_rc.left,
            ov.bar_rc.top,
            ov.bar_rc.right,
            ov.bar_rc.bottom,
        );
        SelectObject(hdc_back, ob_b);
        SelectObject(hdc_back, op_b);
        DeleteObject(p_bd);

        ensure_gdiplus();
        let mut g_ic: *mut c_void = null_mut();
        if GdipCreateFromHDC(hdc_back, &mut g_ic) == GP_OK && !g_ic.is_null() {
            GdipSetSmoothingMode(g_ic, GP_SMOOTHING_ANTIALIAS);
        }

        let wf = utf8_to_wide("Microsoft YaHei UI");
        let hf_btn = CreateFontW(
            -12,
            0,
            0,
            0,
            FW_MEDIUM,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            0,
            0,
            CLEARTYPE_QUALITY,
            0,
            wf.as_ptr(),
        );
        let old_f = SelectObject(hdc_back, hf_btn);
        SetBkMode(hdc_back, TRANSPARENT);
        let active_id = tool_to_btn(ov.cur_tool);
        let mut hover_tip = "";
        let mut hover_rc = RECT::default();

        for b in &ov.btns {
            let hot = ov.hover_btn == b.id;
            let active = active_id != 0 && b.id == active_id;
            let dis = (b.id == BTN_UNDO && ov.annots.is_empty())
                || (b.id == BTN_REDO && ov.redo_stack.is_empty());
            let b_col = if b.id == BTN_COPY {
                if hot {
                    rgb(79, 145, 250)
                } else {
                    rgb(59, 130, 246)
                }
            } else if b.id == BTN_CANCEL && hot {
                rgb(200, 62, 62)
            } else if active {
                rgb(59, 130, 246)
            } else if hot && !dis {
                rgb(64, 72, 88)
            } else {
                rgb(42, 48, 58)
            };
            let br_b = CreateSolidBrush(b_col);
            let mut r = b.rect;
            FillRect(hdc_back, &r, br_b);
            DeleteObject(br_b);
            let t_col = if dis {
                rgb(105, 114, 130)
            } else {
                rgb(242, 246, 252)
            };
            if b.id <= BTN_REDO && !g_ic.is_null() {
                let c_gp = argb(
                    crate::ui::win32_ui::get_r_value(t_col) as u8,
                    crate::ui::win32_ui::get_g_value(t_col) as u8,
                    crate::ui::win32_ui::get_b_value(t_col) as u8,
                    if dis { 100 } else { 245 },
                );
                draw_overlay_tool_icon(g_ic, b.id, r, c_gp);
            } else {
                SetTextColor(hdc_back, t_col);
                let wl = utf8_to_wide(b.label);
                DrawTextW(
                    hdc_back,
                    wl.as_ptr(),
                    -1,
                    &mut r,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }
            if hot && !b.tip.is_empty() {
                hover_tip = b.tip;
                hover_rc = r;
            }
        }

        // 二级子属性栏
        if ov.cur_tool != AnnotTool::None && ov.sub_bar_rc.right > ov.sub_bar_rc.left {
            let br_s = CreateSolidBrush(rgb(26, 30, 38));
            FillRect(hdc_back, &ov.sub_bar_rc, br_s);
            DeleteObject(br_s);
            if !g_ic.is_null() {
                let diams = [5.0f32, 9.0, 13.0];
                for (i, &d) in diams.iter().enumerate() {
                    let r = ov.size_rcs[i];
                    let act = ov.cur_size_idx == i;
                    let hot = ov.sub_hover == SUB_SIZE_0 + (i as i32);
                    if act || hot {
                        let mut bg_c: *mut c_void = null_mut();
                        let cc = if act {
                            argb(59, 130, 246, 90)
                        } else {
                            argb(255, 255, 255, 32)
                        };
                        if GdipCreateSolidFill(cc, &mut bg_c) == GP_OK && !bg_c.is_null() {
                            GdipFillEllipse(
                                g_ic,
                                bg_c,
                                (r.left + 2) as f32,
                                (r.top + 2) as f32,
                                20.0,
                                20.0,
                            );
                            GdipDeleteBrush(bg_c);
                        }
                    }
                    let cx = (r.left + r.right) as f32 * 0.5 - d * 0.5;
                    let cy = (r.top + r.bottom) as f32 * 0.5 - d * 0.5;
                    let mut dc: *mut c_void = null_mut();
                    let col = if act {
                        argb(96, 165, 250, 255)
                    } else {
                        argb(218, 226, 238, 255)
                    };
                    if GdipCreateSolidFill(col, &mut dc) == GP_OK && !dc.is_null() {
                        GdipFillEllipse(g_ic, dc, cx, cy, d, d);
                        GdipDeleteBrush(dc);
                    }
                }
            }
            if matches!(ov.cur_tool, AnnotTool::Rect | AnnotTool::Ellipse) {
                let mut fr = ov.fill_rc;
                let fc = if ov.cur_filled {
                    rgb(59, 130, 246)
                } else if ov.sub_hover == SUB_FILL {
                    rgb(64, 72, 88)
                } else {
                    rgb(42, 48, 58)
                };
                let br_f = CreateSolidBrush(fc);
                FillRect(hdc_back, &fr, br_f);
                DeleteObject(br_f);
                SetTextColor(hdc_back, rgb(240, 244, 250));
                let wf_t = utf8_to_wide(if ov.cur_filled { "实心" } else { "空心" });
                DrawTextW(
                    hdc_back,
                    wf_t.as_ptr(),
                    -1,
                    &mut fr,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            }
            if !matches!(ov.cur_tool, AnnotTool::Mosaic | AnnotTool::Blur) {
                for (i, &(r, g, b)) in ANNOT_COLORS.iter().enumerate() {
                    let rc = ov.color_rcs[i];
                    let br_sw = CreateSolidBrush(rgb(r, g, b));
                    FillRect(hdc_back, &rc, br_sw);
                    DeleteObject(br_sw);
                    if ov.cur_color_idx == i || ov.sub_hover == SUB_COLOR_BASE + (i as i32) {
                        let ring = if ov.cur_color_idx == i {
                            rgb(255, 255, 255)
                        } else {
                            rgb(147, 197, 253)
                        };
                        let rp =
                            CreatePen(PS_SOLID, if ov.cur_color_idx == i { 2 } else { 1 }, ring);
                        let op_r = SelectObject(hdc_back, rp);
                        let ob_r = SelectObject(hdc_back, GetStockObject(NULL_BRUSH));
                        crate::ui::win32_ui::Rectangle(
                            hdc_back,
                            rc.left - 2,
                            rc.top - 2,
                            rc.right + 2,
                            rc.bottom + 2,
                        );
                        SelectObject(hdc_back, ob_r);
                        SelectObject(hdc_back, op_r);
                        DeleteObject(rp);
                    }
                }
            } else {
                let mut hrc = RECT {
                    left: ov.size_rcs[2].right + 10,
                    top: ov.sub_bar_rc.top,
                    right: ov.sub_bar_rc.right - 6,
                    bottom: ov.sub_bar_rc.bottom,
                };
                SetTextColor(hdc_back, rgb(158, 170, 190));
                let wh = utf8_to_wide("强度/粗细 (左选圆点)");
                DrawTextW(
                    hdc_back,
                    wh.as_ptr(),
                    -1,
                    &mut hrc,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );
            }
        }

        if !hover_tip.is_empty() {
            let wt = utf8_to_wide(hover_tip);
            let mut cr = RECT::default();
            DrawTextW(
                hdc_back,
                wt.as_ptr(),
                -1,
                &mut cr,
                DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
            );
            let tw = (cr.right - cr.left + 16).clamp(54, 340);
            let tx = ((hover_rc.left + hover_rc.right - tw) / 2).clamp(4, (ov.vw - tw - 4).max(4));
            let ty = if ov.bar_rc.top >= 26 {
                ov.bar_rc.top - 24
            } else {
                ov.bar_rc.bottom + 6
            };
            let mut tr = RECT {
                left: tx,
                top: ty,
                right: tx + tw,
                bottom: ty + 20,
            };
            let tb = CreateSolidBrush(rgb(20, 24, 32));
            FillRect(hdc_back, &tr, tb);
            DeleteObject(tb);
            SetTextColor(hdc_back, rgb(230, 238, 250));
            DrawTextW(
                hdc_back,
                wt.as_ptr(),
                -1,
                &mut tr,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
        }

        if !g_ic.is_null() {
            GdipDeleteGraphics(g_ic);
        }
        SelectObject(hdc_back, old_f);
        DeleteObject(hf_btn);
    }

    // 未确定选区或正在调整选区大小时显示 12x 放大镜取色器
    if !ov.has_sel || ov.dragging_sel {
        draw_overlay_magnifier(hdc_back, hdc_src, &ov, sel);
    }

    BitBlt(hdc, 0, 0, ov.vw, ov.vh, hdc_back, 0, 0, SRCCOPY);
    SelectObject(hdc_src, old_src);
    DeleteObject(hbm_src);
    DeleteDC(hdc_src);
    SelectObject(hdc_back, old_back);
    DeleteObject(hbm_back);
    DeleteDC(hdc_back);
    EndPaint(hwnd, &ps);
}

fn hit_test_handles(sel: RECT, x: i32, y: i32) -> i32 {
    let pts = [
        (sel.left, sel.top, 10),
        ((sel.left + sel.right) / 2, sel.top, 11),
        (sel.right, sel.top, 12),
        (sel.right, (sel.top + sel.bottom) / 2, 13),
        (sel.right, sel.bottom, 14),
        ((sel.left + sel.right) / 2, sel.bottom, 15),
        (sel.left, sel.bottom, 16),
        (sel.left, (sel.top + sel.bottom) / 2, 17),
    ];
    for (hx, hy, mode) in pts {
        if (x - hx).abs() <= SNAP_DIST && (y - hy).abs() <= SNAP_DIST {
            return mode;
        }
    }
    0
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint_overlay(hwnd);
            0
        }
        WM_SETCURSOR => {
            let cur = SNIP_MGR.with(|m| {
                let ov = &m.borrow().overlay;
                let (x, y) = (ov.mouse_pt.x, ov.mouse_pt.y);
                if ov.has_sel && !ov.dragging_sel {
                    if ov.hover_btn != 0 || ov.sub_hover != 0 {
                        return IDC_ARROW;
                    }
                    let hm = hit_test_handles(ov.sel_rc, x, y);
                    match hm {
                        10 | 14 => IDC_SIZENWSE,
                        12 | 16 => IDC_SIZENESW,
                        11 | 15 => IDC_SIZENS,
                        13 | 17 => IDC_SIZEWE,
                        _ if ov.cur_tool == AnnotTool::None
                            && x >= ov.sel_rc.left
                            && x < ov.sel_rc.right
                            && y >= ov.sel_rc.top
                            && y < ov.sel_rc.bottom =>
                        {
                            IDC_SIZEALL
                        }
                        _ => IDC_CROSS,
                    }
                } else {
                    IDC_CROSS
                }
            });
            SetCursor(LoadCursorW(null_mut(), cur));
            1
        }
        WM_MOUSEMOVE => {
            let (x, y) = lparam_xy(lp);
            let need_rebuild = SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                let ov = &mut mgr.overlay;
                ov.mouse_pt = POINT { x, y };

                if ov.dragging_sel {
                    let dx = x - ov.drag_start_pt.x;
                    let dy = y - ov.drag_start_pt.y;
                    match ov.drag_mode {
                        1 => {
                            ov.sel_rc = norm_rect(ov.drag_start_pt.x, ov.drag_start_pt.y, x, y);
                            ov.has_sel = true;
                        }
                        2 => {
                            let w = ov.drag_orig_rc.right - ov.drag_orig_rc.left;
                            let h = ov.drag_orig_rc.bottom - ov.drag_orig_rc.top;
                            let nx = (ov.drag_orig_rc.left + dx).clamp(0, (ov.vw - w).max(0));
                            let ny = (ov.drag_orig_rc.top + dy).clamp(0, (ov.vh - h).max(0));
                            ov.sel_rc = RECT {
                                left: nx,
                                top: ny,
                                right: nx + w,
                                bottom: ny + h,
                            };
                        }
                        10..=17 => {
                            let mut r = ov.drag_orig_rc;
                            if matches!(ov.drag_mode, 10 | 16 | 17) {
                                r.left += dx;
                            }
                            if matches!(ov.drag_mode, 12..=14) {
                                r.right += dx;
                            }
                            if matches!(ov.drag_mode, 10..=12) {
                                r.top += dy;
                            }
                            if matches!(ov.drag_mode, 14..=16) {
                                r.bottom += dy;
                            }
                            ov.sel_rc = norm_rect(r.left, r.top, r.right, r.bottom);
                        }
                        _ => {}
                    }
                    return false;
                }

                if ov.drawing_annot {
                    if let Some(d) = ov.draft_annot.as_mut() {
                        d.x2 = x;
                        d.y2 = y;
                        if matches!(d.tool, AnnotTool::Pen | AnnotTool::Marker) {
                            d.pts.push((x, y));
                        }
                        if matches!(d.tool, AnnotTool::Mosaic | AnnotTool::Blur) {
                            return true;
                        }
                    }
                    return false;
                }

                if !ov.has_sel {
                    let mut best = RECT::default();
                    let mut best_area = i64::MAX;
                    for &wrc in &ov.win_rects {
                        if x >= wrc.left && x < wrc.right && y >= wrc.top && y < wrc.bottom {
                            let area =
                                ((wrc.right - wrc.left) as i64) * ((wrc.bottom - wrc.top) as i64);
                            if area < best_area {
                                best_area = area;
                                best = wrc;
                            }
                        }
                    }
                    ov.hover_win_rc = best;
                } else {
                    ov.hover_btn = 0;
                    for b in &ov.btns {
                        if x >= b.rect.left
                            && x < b.rect.right
                            && y >= b.rect.top
                            && y < b.rect.bottom
                        {
                            ov.hover_btn = b.id;
                            break;
                        }
                    }
                    ov.sub_hover = 0;
                    if ov.cur_tool != AnnotTool::None {
                        for (i, r) in ov.size_rcs.iter().enumerate() {
                            if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                                ov.sub_hover = SUB_SIZE_0 + (i as i32);
                            }
                        }
                        if matches!(ov.cur_tool, AnnotTool::Rect | AnnotTool::Ellipse)
                            && x >= ov.fill_rc.left
                            && x < ov.fill_rc.right
                            && y >= ov.fill_rc.top
                            && y < ov.fill_rc.bottom
                        {
                            ov.sub_hover = SUB_FILL;
                        }
                        if !matches!(ov.cur_tool, AnnotTool::Mosaic | AnnotTool::Blur) {
                            for (i, r) in ov.color_rcs.iter().enumerate() {
                                if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                                    ov.sub_hover = SUB_COLOR_BASE + (i as i32);
                                }
                            }
                        }
                    }
                }
                false
            });
            if need_rebuild {
                SNIP_MGR.with(|m| rebuild_work_bgra(&mut m.borrow_mut().overlay));
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            take_and_commit_inline_text_edit();
            let mut need_text_edit = false;
            let clicked_id = SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                let ov = &mut mgr.overlay;

                if ov.has_sel && !ov.trans_mode {
                    for b in &ov.btns {
                        if x >= b.rect.left
                            && x < b.rect.right
                            && y >= b.rect.top
                            && y < b.rect.bottom
                        {
                            return Some(b.id);
                        }
                    }
                    if ov.cur_tool != AnnotTool::None
                        && x >= ov.sub_bar_rc.left
                        && x < ov.sub_bar_rc.right
                        && y >= ov.sub_bar_rc.top
                        && y < ov.sub_bar_rc.bottom
                    {
                        for (i, r) in ov.size_rcs.iter().enumerate() {
                            if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                                ov.cur_size_idx = i;
                                return Some(-1);
                            }
                        }
                        if matches!(ov.cur_tool, AnnotTool::Rect | AnnotTool::Ellipse)
                            && x >= ov.fill_rc.left
                            && x < ov.fill_rc.right
                            && y >= ov.fill_rc.top
                            && y < ov.fill_rc.bottom
                        {
                            ov.cur_filled = !ov.cur_filled;
                            return Some(-1);
                        }
                        if !matches!(ov.cur_tool, AnnotTool::Mosaic | AnnotTool::Blur) {
                            for (i, r) in ov.color_rcs.iter().enumerate() {
                                if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                                    ov.cur_color_idx = i;
                                    return Some(-1);
                                }
                            }
                        }
                        return Some(-1);
                    }
                }

                if !ov.has_sel {
                    ov.dragging_sel = true;
                    ov.drag_mode = 1;
                    ov.drag_start_pt = POINT { x, y };
                    ov.sel_rc = norm_rect(x, y, x, y);
                } else {
                    let hm = hit_test_handles(ov.sel_rc, x, y);
                    if hm != 0 {
                        ov.dragging_sel = true;
                        ov.drag_mode = hm;
                        ov.drag_start_pt = POINT { x, y };
                        ov.drag_orig_rc = ov.sel_rc;
                    } else if x >= ov.sel_rc.left
                        && x < ov.sel_rc.right
                        && y >= ov.sel_rc.top
                        && y < ov.sel_rc.bottom
                    {
                        if ov.cur_tool == AnnotTool::None {
                            ov.dragging_sel = true;
                            ov.drag_mode = 2;
                            ov.drag_start_pt = POINT { x, y };
                            ov.drag_orig_rc = ov.sel_rc;
                        } else if ov.cur_tool == AnnotTool::Number {
                            let (r, g, b) = ANNOT_COLORS[ov.cur_color_idx];
                            let n = ov.next_num;
                            ov.next_num += 1;
                            let w = SIZE_WIDTHS[ov.cur_size_idx];
                            ov.annots.push(AnnotItem {
                                tool: AnnotTool::Number,
                                x1: x,
                                y1: y,
                                x2: x,
                                y2: y,
                                pts: Vec::new(),
                                r,
                                g,
                                b,
                                width: w,
                                filled: false,
                                num: n,
                                text: String::new(),
                            });
                            ov.redo_stack.clear();
                        } else if ov.cur_tool == AnnotTool::Text {
                            need_text_edit = true;
                        } else {
                            let (r, g, b) = ANNOT_COLORS[ov.cur_color_idx];
                            let w = SIZE_WIDTHS[ov.cur_size_idx];
                            ov.drawing_annot = true;
                            ov.draft_annot = Some(AnnotItem {
                                tool: ov.cur_tool,
                                x1: x,
                                y1: y,
                                x2: x,
                                y2: y,
                                pts: vec![(x, y)],
                                r,
                                g,
                                b,
                                width: w,
                                filled: ov.cur_filled,
                                num: 0,
                                text: String::new(),
                            });
                        }
                    } else {
                        ov.dragging_sel = true;
                        ov.drag_mode = 1;
                        ov.drag_start_pt = POINT { x, y };
                        ov.sel_rc = norm_rect(x, y, x, y);
                        ov.cur_tool = AnnotTool::None;
                        ov.annots.clear();
                        ov.redo_stack.clear();
                        rebuild_work_bgra(ov);
                    }
                }
                None
            });

            if need_text_edit {
                let cls = utf8_to_wide("EDIT");
                let emp = utf8_to_wide("");
                let h_ed = CreateWindowExW(
                    0,
                    cls.as_ptr(),
                    emp.as_ptr(),
                    WS_CHILD | WS_VISIBLE | ES_MULTILINE | ES_WANTRETURN | ES_AUTOHSCROLL,
                    x,
                    y,
                    180,
                    28,
                    hwnd,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                );
                SetWindowSubclass(h_ed, text_edit_subclass, 0x5445, 0);
                SetFocus(h_ed);
                SNIP_MGR.with(|m| {
                    let ov = &mut m.borrow_mut().overlay;
                    ov.hwnd_text_edit = h_ed;
                    ov.text_edit_pos = POINT { x, y };
                });
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }

            if let Some(bid) = clicked_id {
                match bid {
                    -1 => {}
                    BTN_RECT | BTN_ELLIPSE | BTN_LINE | BTN_ARROW | BTN_PEN | BTN_MARKER
                    | BTN_NUMBER | BTN_TEXT | BTN_MOSAIC | BTN_BLUR => {
                        let tool = match bid {
                            BTN_RECT => AnnotTool::Rect,
                            BTN_ELLIPSE => AnnotTool::Ellipse,
                            BTN_LINE => AnnotTool::Line,
                            BTN_ARROW => AnnotTool::Arrow,
                            BTN_PEN => AnnotTool::Pen,
                            BTN_MARKER => AnnotTool::Marker,
                            BTN_NUMBER => AnnotTool::Number,
                            BTN_TEXT => AnnotTool::Text,
                            BTN_MOSAIC => AnnotTool::Mosaic,
                            _ => AnnotTool::Blur,
                        };
                        SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            ov.cur_tool = if ov.cur_tool == tool {
                                AnnotTool::None
                            } else {
                                tool
                            };
                            layout_overlay_toolbar(ov);
                        });
                    }
                    BTN_UNDO => {
                        SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            if let Some(a) = ov.annots.pop() {
                                ov.redo_stack.push(a);
                                rebuild_work_bgra(ov);
                            }
                        });
                    }
                    BTN_REDO => {
                        SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            if let Some(a) = ov.redo_stack.pop() {
                                ov.annots.push(a);
                                rebuild_work_bgra(ov);
                            }
                        });
                    }
                    BTN_CANCEL => {
                        close_overlay();
                        return 0;
                    }
                    BTN_SAVE | BTN_PIN | BTN_TRANS | BTN_COPY => {
                        finish_selection_action(bid);
                        return 0;
                    }
                    _ => {}
                }
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }

            SetCapture(hwnd);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let auto_trans = SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                let ov = &mut mgr.overlay;
                if ov.dragging_sel {
                    ov.dragging_sel = false;
                    let w = ov.sel_rc.right - ov.sel_rc.left;
                    let h = ov.sel_rc.bottom - ov.sel_rc.top;
                    if (w < 6 || h < 6)
                        && ov.drag_mode == 1
                        && ov.hover_win_rc.right > ov.hover_win_rc.left
                    {
                        ov.sel_rc = ov.hover_win_rc;
                        ov.has_sel = true;
                    } else {
                        ov.has_sel = w >= 6 && h >= 6;
                    }
                    if ov.has_sel {
                        if ov.trans_mode {
                            return true;
                        }
                        layout_overlay_toolbar(ov);
                    }
                } else if ov.drawing_annot {
                    ov.drawing_annot = false;
                    if let Some(d) = ov.draft_annot.take() {
                        let valid =
                            (d.x2 - d.x1).abs() > 2 || (d.y2 - d.y1).abs() > 2 || d.pts.len() > 2;
                        if valid {
                            ov.annots.push(d);
                            ov.redo_stack.clear();
                        }
                        rebuild_work_bgra(ov);
                    }
                }
                false
            });
            if auto_trans {
                finish_selection_action(BTN_TRANS);
                return 0;
            }
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDBLCLK => {
            let ok = SNIP_MGR.with(|m| {
                let ov = &m.borrow().overlay;
                ov.has_sel && ov.cur_tool == AnnotTool::None
            });
            if ok {
                finish_selection_action(BTN_COPY);
            }
            0
        }
        WM_RBUTTONDOWN => 0,
        WM_RBUTTONUP => {
            let had_sel = SNIP_MGR.with(|m| m.borrow().overlay.has_sel);
            if had_sel {
                SNIP_MGR.with(|m| {
                    let ov = &mut m.borrow_mut().overlay;
                    ov.has_sel = false;
                    ov.cur_tool = AnnotTool::None;
                    ov.annots.clear();
                    ov.redo_stack.clear();
                    rebuild_work_bgra(ov);
                });
                InvalidateRect(hwnd, null(), 0);
            } else {
                close_overlay();
            }
            0
        }
        WM_KEYDOWN => {
            let vk = (wp & 0xFF) as i32;
            let ctrl = (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0;
            if vk == VK_ESCAPE {
                close_overlay();
                return 0;
            }
            if vk == VK_RETURN {
                if SNIP_MGR.with(|m| m.borrow().overlay.has_sel) {
                    finish_selection_action(BTN_COPY);
                }
                return 0;
            }
            if vk == 'C' as i32 && !ctrl {
                // 复制放大镜下当前像素十六进制颜色
                let hex_opt = SNIP_MGR.with(|m| {
                    let ov = &m.borrow().overlay;
                    let (mx, my) = (ov.mouse_pt.x, ov.mouse_pt.y);
                    if mx >= 0 && my >= 0 && mx < ov.vw && my < ov.vh {
                        let off = ((my as usize) * (ov.vw as usize) + (mx as usize)) * 4;
                        let (b, g, r) = (
                            ov.orig_bgra[off],
                            ov.orig_bgra[off + 1],
                            ov.orig_bgra[off + 2],
                        );
                        return Some(format!("#{r:02X}{g:02X}{b:02X}"));
                    }
                    None
                });
                if let Some(hex) = hex_opt {
                    set_clipboard_text(&hex);
                }
                close_overlay();
                return 0;
            }
            if ctrl && vk == 'C' as i32 {
                if SNIP_MGR.with(|m| m.borrow().overlay.has_sel) {
                    finish_selection_action(BTN_COPY);
                }
                return 0;
            }
            if ctrl && vk == 'S' as i32 {
                if SNIP_MGR.with(|m| m.borrow().overlay.has_sel) {
                    finish_selection_action(BTN_SAVE);
                }
                return 0;
            }
            if ctrl && vk == 'Z' as i32 {
                SNIP_MGR.with(|m| {
                    let ov = &mut m.borrow_mut().overlay;
                    if let Some(a) = ov.annots.pop() {
                        ov.redo_stack.push(a);
                        rebuild_work_bgra(ov);
                    }
                });
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }
            if ctrl && vk == 'Y' as i32 {
                SNIP_MGR.with(|m| {
                    let ov = &mut m.borrow_mut().overlay;
                    if let Some(a) = ov.redo_stack.pop() {
                        ov.annots.push(a);
                        rebuild_work_bgra(ov);
                    }
                });
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }
            0
        }
        WM_CAPTURECHANGED => 0,
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ---- OCR 结果浮窗 ----

const RES_BTN_NAMES: [&str; 7] = [
    "原图",
    "复制译文",
    "复制原文",
    "复制图片",
    "保存PNG",
    "贴图",
    "重截",
];

pub fn open_result_window(
    w: i32,
    h: i32,
    bgra_zh: Vec<u8>,
    bgra_orig: Vec<u8>,
    zh_text: String,
    en_text: String,
) {
    unsafe { ensure_snip_classes() };
    let old_hwnd = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let oh = mgr.result.hwnd;
        mgr.result = ResultWinState {
            hwnd: null_mut(),
            img_w: w,
            img_h: h,
            bgra_zh,
            bgra_orig,
            show_orig: false,
            zh_text,
            en_text,
            zoom: 1.0,
            hover_btn: -1,
            btn_rcs: [RECT::default(); 7],
            dragging: false,
            drag_off: POINT::default(),
        };
        oh
    });
    if !old_hwnd.is_null() {
        unsafe { DestroyWindow(old_hwnd) };
    }

    let win_w = (w + 16).clamp(480, 1200);
    let win_h = (h + 52).clamp(180, 820);
    let wa = crate::ui::win32_ui::primary_work_area();
    let x = wa.left + ((wa.right - wa.left) - win_w).max(0) / 2;
    let y = wa.top + ((wa.bottom - wa.top) - win_h).max(0) / 2;

    unsafe {
        let h_inst = GetModuleHandleW(null());
        let cls = utf8_to_wide(RESULT_CLASS_NAME);
        let wt = utf8_to_wide("截屏翻译结果 - 快译NG");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            cls.as_ptr(),
            wt.as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN,
            x,
            y,
            win_w,
            win_h,
            null_mut(),
            null_mut(),
            h_inst,
            null_mut(),
        );
        if hwnd.is_null() {
            return;
        }
        crate::ui::win32_ui::apply_dwm_window_style(hwnd, true, true);
        SNIP_MGR.with(|m| {
            let mut mgr = m.borrow_mut();
            mgr.result.hwnd = hwnd;
            layout_result_btns(&mut mgr.result, win_w, win_h);
        });
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(hwnd);
    }
}

fn layout_result_btns(res: &mut ResultWinState, win_w: i32, win_h: i32) {
    let by = win_h - 36;
    let mut x = 10;
    let gap = 6;
    let _ = win_w;
    for (i, rc) in res.btn_rcs.iter_mut().enumerate() {
        let bw = if RES_BTN_NAMES[i].chars().count() <= 2 {
            48
        } else {
            70
        };
        *rc = RECT {
            left: x,
            top: by,
            right: x + bw,
            bottom: by + 26,
        };
        x += bw + gap;
    }
}

unsafe fn paint_result_window(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc = RECT::default();
    GetClientRect(hwnd, &mut rc);
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    if w <= 0 || h <= 0 {
        EndPaint(hwnd, &ps);
        return;
    }
    let res = SNIP_MGR.with(|m| m.borrow().result.clone());

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm_buf = CreateCompatibleBitmap(hdc, w, h);
    let old = SelectObject(hdc_buf, hbm_buf);

    let bg = CreateSolidBrush(rgb(24, 27, 34));
    FillRect(hdc_buf, &rc, bg);
    DeleteObject(bg);

    let active_bgra = if res.show_orig {
        &res.bgra_orig
    } else {
        &res.bgra_zh
    };
    if res.img_w > 0 && res.img_h > 0 && !active_bgra.is_empty() {
        let dw = ((res.img_w as f64) * res.zoom).round() as i32;
        let dh = ((res.img_h as f64) * res.zoom).round() as i32;
        let dx = ((w - dw) / 2).max(8);
        let dy = ((h - 46 - dh) / 2).max(8);
        if let Some((hbm_img, _)) =
            create_topdown_dib_from_bgra(hdc, active_bgra, res.img_w, res.img_h)
        {
            let dc_i = CreateCompatibleDC(hdc);
            let oi = SelectObject(dc_i, hbm_img);
            SetStretchBltMode(hdc_buf, HALFTONE);
            StretchBlt(
                hdc_buf, dx, dy, dw, dh, dc_i, 0, 0, res.img_w, res.img_h, SRCCOPY,
            );
            SelectObject(dc_i, oi);
            DeleteObject(hbm_img);
            DeleteDC(dc_i);
        }
    }

    // 底部按钮栏
    let wf = utf8_to_wide("Microsoft YaHei UI");
    let hf = CreateFontW(
        -12,
        0,
        0,
        0,
        FW_MEDIUM,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        0,
        0,
        CLEARTYPE_QUALITY,
        0,
        wf.as_ptr(),
    );
    let old_f = SelectObject(hdc_buf, hf);
    SetBkMode(hdc_buf, TRANSPARENT);
    SetTextColor(hdc_buf, rgb(240, 244, 250));
    for (i, mut brc) in res.btn_rcs.into_iter().enumerate() {
        let hot = res.hover_btn == i as i32;
        let col = if i == 0 && res.show_orig {
            rgb(59, 130, 246)
        } else if hot {
            rgb(64, 72, 88)
        } else {
            rgb(42, 48, 58)
        };
        let br = CreateSolidBrush(col);
        FillRect(hdc_buf, &brc, br);
        DeleteObject(br);
        let lbl = if i == 0 && res.show_orig {
            "译图"
        } else {
            RES_BTN_NAMES[i]
        };
        let wl = utf8_to_wide(lbl);
        DrawTextW(
            hdc_buf,
            wl.as_ptr(),
            -1,
            &mut brc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }

    let z_txt = format!(
        "{}% (空格对比 | Esc关闭)",
        (res.zoom * 100.0).round() as i32
    );
    let mut zrc = RECT {
        left: res.btn_rcs[6].right + 10,
        top: h - 36,
        right: w - 10,
        bottom: h - 10,
    };
    SetTextColor(hdc_buf, rgb(148, 163, 184));
    let wz = utf8_to_wide(&z_txt);
    DrawTextW(
        hdc_buf,
        wz.as_ptr(),
        -1,
        &mut zrc,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS,
    );

    SelectObject(hdc_buf, old_f);
    DeleteObject(hf);

    let pen_bd = CreatePen(PS_SOLID, 1, rgb(64, 72, 88));
    let op = SelectObject(hdc_buf, pen_bd);
    let ob = SelectObject(hdc_buf, GetStockObject(NULL_BRUSH));
    crate::ui::win32_ui::Rectangle(hdc_buf, 0, 0, w, h);
    SelectObject(hdc_buf, ob);
    SelectObject(hdc_buf, op);
    DeleteObject(pen_bd);

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old);
    DeleteObject(hbm_buf);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

unsafe extern "system" fn result_wnd_proc(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint_result_window(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = lparam_xy(lp);
            let dragging = SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                if mgr.result.dragging {
                    return Some(mgr.result.drag_off);
                }
                let mut nh = -1i32;
                for (i, r) in mgr.result.btn_rcs.iter().enumerate() {
                    if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                        nh = i as i32;
                        break;
                    }
                }
                if nh != mgr.result.hover_btn {
                    mgr.result.hover_btn = nh;
                    InvalidateRect(hwnd, null(), 0);
                }
                None
            });
            if let Some(off) = dragging {
                let mut cur = POINT::default();
                GetCursorPos(&mut cur);
                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    cur.x - off.x,
                    cur.y - off.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            let clicked = SNIP_MGR.with(|m| {
                let mgr = m.borrow();
                for (i, r) in mgr.result.btn_rcs.iter().enumerate() {
                    if x >= r.left && x < r.right && y >= r.top && y < r.bottom {
                        return Some(i);
                    }
                }
                None
            });
            if let Some(idx) = clicked {
                match idx {
                    0 => {
                        SNIP_MGR.with(|m| {
                            m.borrow_mut().result.show_orig = !m.borrow().result.show_orig
                        });
                        InvalidateRect(hwnd, null(), 0);
                    }
                    1 => {
                        let t = SNIP_MGR.with(|m| m.borrow().result.zh_text.clone());
                        set_clipboard_text(&t);
                    }
                    2 => {
                        let t = SNIP_MGR.with(|m| m.borrow().result.en_text.clone());
                        set_clipboard_text(&t);
                    }
                    3 => {
                        let (w, h, bg) = SNIP_MGR.with(|m| {
                            let r = &m.borrow().result;
                            let b = if r.show_orig {
                                r.bgra_orig.clone()
                            } else {
                                r.bgra_zh.clone()
                            };
                            (r.img_w, r.img_h, b)
                        });
                        set_clipboard_bgra(&bg, w, h);
                    }
                    4 => {
                        let (w, h, bg, sd) = SNIP_MGR.with(|m| {
                            let mgr = m.borrow();
                            let b = if mgr.result.show_orig {
                                mgr.result.bgra_orig.clone()
                            } else {
                                mgr.result.bgra_zh.clone()
                            };
                            (
                                mgr.result.img_w,
                                mgr.result.img_h,
                                b,
                                mgr.cfg.save_dir.clone(),
                            )
                        });
                        if let Some(p) = save_file_dialog_png(hwnd, &sd, "KuaiYiNG_Trans.png") {
                            save_bgra_as_png(&p, &bg, w, h);
                        }
                    }
                    5 => {
                        let (w, h, bo, zh, en) = SNIP_MGR.with(|m| {
                            let r = &m.borrow().result;
                            (
                                r.img_w,
                                r.img_h,
                                r.bgra_zh.clone(),
                                r.zh_text.clone(),
                                r.en_text.clone(),
                            )
                        });
                        create_pin_window_bgra(w, h, bo, zh, en);
                    }
                    6 => {
                        DestroyWindow(hwnd);
                        start_snip_translate();
                    }
                    _ => {}
                }
                return 0;
            }
            let mut wr = RECT::default();
            GetWindowRect(hwnd, &mut wr);
            let mut cur = POINT::default();
            GetCursorPos(&mut cur);
            SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                mgr.result.dragging = true;
                mgr.result.drag_off = POINT {
                    x: cur.x - wr.left,
                    y: cur.y - wr.top,
                };
            });
            SetCapture(hwnd);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            SNIP_MGR.with(|m| m.borrow_mut().result.dragging = false);
            0
        }
        WM_MOUSEWHEEL => {
            let notches = wheel_delta(wp) / 120;
            if notches != 0 {
                let (nw, nh) = SNIP_MGR.with(|m| {
                    let mut mgr = m.borrow_mut();
                    let factor = if notches > 0 { 1.1 } else { 0.9 };
                    mgr.result.zoom = (mgr.result.zoom * factor).clamp(0.25, 4.0);
                    let w = (((mgr.result.img_w as f64) * mgr.result.zoom).round() as i32 + 16)
                        .clamp(480, 1400);
                    let h = (((mgr.result.img_h as f64) * mgr.result.zoom).round() as i32 + 52)
                        .clamp(180, 960);
                    layout_result_btns(&mut mgr.result, w, h);
                    (w, h)
                });
                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0,
                    0,
                    nw,
                    nh,
                    SWP_NOMOVE | SWP_NOACTIVATE,
                );
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_KEYDOWN => {
            let vk = (wp & 0xFF) as i32;
            if vk == VK_ESCAPE {
                DestroyWindow(hwnd);
            } else if vk == 0x20 {
                SNIP_MGR.with(|m| {
                    let mut mgr = m.borrow_mut();
                    mgr.result.show_orig = !mgr.result.show_orig;
                });
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_CLOSE | WM_DESTROY => {
            SNIP_MGR.with(|m| m.borrow_mut().result.hwnd = null_mut());
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ---- 屏幕多贴图浮窗 ----

pub fn create_pin_from_clipboard() {
    if let Some((w, h, bgra)) = get_clipboard_bgra() {
        create_pin_window_bgra(w, h, bgra, String::new(), String::new());
        return;
    }
    if let Some(txt) = get_clipboard_text() {
        let trimmed = txt.trim();
        if !trimmed.is_empty() {
            let (w, h, bgra) = render_text_card_bgra(trimmed);
            create_pin_window_bgra(w, h, bgra, String::new(), trimmed.to_string());
            return;
        }
    }
    let last = SNIP_MGR.with(|m| {
        let mgr = m.borrow();
        if mgr.last_cap_w > 0 && mgr.last_cap_h > 0 && !mgr.last_cap_bgra.is_empty() {
            Some((mgr.last_cap_w, mgr.last_cap_h, mgr.last_cap_bgra.clone()))
        } else {
            None
        }
    });
    if let Some((w, h, bgra)) = last {
        create_pin_window_bgra(w, h, bgra, String::new(), String::new());
    }
}

pub fn create_pin_window_bgra(w: i32, h: i32, bgra: Vec<u8>, zh_text: String, en_text: String) {
    if w <= 0 || h <= 0 || bgra.is_empty() {
        return;
    }
    unsafe { ensure_snip_classes() };
    let mut cur = POINT::default();
    unsafe { GetCursorPos(&mut cur) };
    let wa = crate::ui::win32_ui::work_area_from_point(cur);
    let init_w = w.min((wa.right - wa.left) - 40).max(40);
    let init_h = h.min((wa.bottom - wa.top) - 40).max(40);
    let zoom = ((init_w as f64) / (w as f64))
        .min((init_h as f64) / (h as f64))
        .clamp(0.15, 1.0);
    let draw_w = ((w as f64) * zoom).round() as i32;
    let draw_h = ((h as f64) * zoom).round() as i32;
    let x = (cur.x - draw_w / 2).clamp(wa.left, (wa.right - draw_w).max(wa.left));
    let y = (cur.y - draw_h / 2).clamp(wa.top, (wa.bottom - draw_h).max(wa.top));

    unsafe {
        let h_inst = GetModuleHandleW(null());
        let cls = utf8_to_wide(PIN_CLASS_NAME);
        let wt = utf8_to_wide("快译NG 贴图");
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED,
            cls.as_ptr(),
            wt.as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN,
            x,
            y,
            draw_w,
            draw_h,
            null_mut(),
            null_mut(),
            h_inst,
            null_mut(),
        );
        if hwnd.is_null() {
            return;
        }
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        SNIP_MGR.with(|m| {
            m.borrow_mut().pins.push(PinEntry {
                hwnd,
                img_w: w,
                img_h: h,
                bgra_orig: bgra,
                bgra_zh: Vec::new(),
                show_zh: false,
                translating: false,
                zh_text,
                en_text,
                zoom,
                alpha: 255,
                hover_trans: false,
                dragging: false,
                drag_off: POINT::default(),
            });
        });
        ShowWindow(hwnd, SW_SHOW);
        SetFocus(hwnd);
    }
}

struct PinDonePayload {
    res: SnipResult,
}

fn start_pin_translate(pin_hwnd: HWND) {
    let params = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let cfg = mgr.cfg.clone();
        if let Some(p) = mgr.pins.iter_mut().find(|x| x.hwnd == pin_hwnd) {
            if !p.bgra_zh.is_empty() {
                p.show_zh = !p.show_zh;
                return Ok(None);
            }
            if p.translating {
                return Err(());
            }
            p.translating = true;
            return Ok(Some((p.img_w, p.img_h, p.bgra_orig.clone(), cfg)));
        }
        Err(())
    });

    unsafe { InvalidateRect(pin_hwnd, null(), 0) };
    let Ok(Some((w, h, bgra, cfg))) = params else {
        return;
    };
    let hwnd_main = cfg.hwnd_main;
    let pin_raw = pin_hwnd as usize;
    thread::spawn(move || {
        let res = snip_translate(
            &bgra,
            w,
            h,
            &cfg.ai_url,
            &cfg.ai_key,
            &cfg.ai_model,
            &cfg.baidu_appid,
            &cfg.baidu_secret,
            &cfg.baidu_url,
            &cfg.cache_dir,
        );
        if hwnd_main != 0 {
            let payload = Box::into_raw(Box::new(PinDonePayload { res }));
            unsafe {
                PostMessageW(
                    hwnd_main as HWND,
                    WM_APP_PIN_DONE,
                    pin_raw,
                    payload as LPARAM,
                )
            };
        }
    });
}

/// 主线程收到贴图 OCR 完成通知后更新对应贴图窗口
pub fn handle_pin_done_message(wp: WPARAM, lp: LPARAM) {
    if lp == 0 {
        return;
    }
    let payload = unsafe { Box::from_raw(lp as *mut PinDonePayload) };
    let pin_hwnd = wp as HWND;
    if unsafe { IsWindow(pin_hwnd) == 0 } {
        return;
    }
    SNIP_MGR.with(|m| {
        if let Some(p) = m.borrow_mut().pins.iter_mut().find(|x| x.hwnd == pin_hwnd) {
            p.translating = false;
            if payload.res.ok {
                p.bgra_zh = payload.res.bgra_out;
                p.zh_text = payload.res.zh_text;
                p.en_text = payload.res.en_text;
                p.show_zh = true;
            }
        }
    });
    unsafe { InvalidateRect(pin_hwnd, null(), 0) };
}

#[inline]
fn pin_trans_btn_rc(win_w: i32) -> RECT {
    RECT {
        left: (win_w - 28).max(4),
        top: 6,
        right: (win_w - 6).max(26),
        bottom: 26,
    }
}

unsafe fn paint_pin_window(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc = RECT::default();
    GetClientRect(hwnd, &mut rc);
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    if w <= 0 || h <= 0 {
        EndPaint(hwnd, &ps);
        return;
    }

    let pin_opt = SNIP_MGR.with(|m| m.borrow().pins.iter().find(|p| p.hwnd == hwnd).cloned());
    let Some(pin) = pin_opt else {
        EndPaint(hwnd, &ps);
        return;
    };

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm_buf = CreateCompatibleBitmap(hdc, w, h);
    let old = SelectObject(hdc_buf, hbm_buf);

    let bgra = if pin.show_zh && !pin.bgra_zh.is_empty() {
        &pin.bgra_zh
    } else {
        &pin.bgra_orig
    };
    if let Some((hbm_src, _)) = create_topdown_dib_from_bgra(hdc, bgra, pin.img_w, pin.img_h) {
        let dc_s = CreateCompatibleDC(hdc);
        let os = SelectObject(dc_s, hbm_src);
        SetStretchBltMode(hdc_buf, HALFTONE);
        StretchBlt(
            hdc_buf, 0, 0, w, h, dc_s, 0, 0, pin.img_w, pin.img_h, SRCCOPY,
        );
        SelectObject(dc_s, os);
        DeleteObject(hbm_src);
        DeleteDC(dc_s);
    }

    // 淡灰细边框
    let pen = CreatePen(PS_SOLID, 1, rgb(86, 98, 120));
    let op = SelectObject(hdc_buf, pen);
    let ob = SelectObject(hdc_buf, GetStockObject(NULL_BRUSH));
    crate::ui::win32_ui::Rectangle(hdc_buf, 0, 0, w, h);
    SelectObject(hdc_buf, ob);
    SelectObject(hdc_buf, op);
    DeleteObject(pen);

    // 悬停右上角翻译角标 ("译" / "原" / "...")
    let mut pt = POINT::default();
    GetCursorPos(&mut pt);
    ScreenToClient(hwnd, &mut pt);
    if pt.x >= 0 && pt.x < w && pt.y >= 0 && pt.y < h {
        let mut brc = pin_trans_btn_rc(w);
        let col = if pin.translating {
            rgb(100, 116, 139)
        } else if pin.show_zh || pin.hover_trans {
            rgb(59, 130, 246)
        } else {
            rgb(30, 34, 44)
        };
        let br = CreateSolidBrush(col);
        FillRect(hdc_buf, &brc, br);
        DeleteObject(br);
        let wf = utf8_to_wide("Microsoft YaHei UI");
        let hf = CreateFontW(
            -11,
            0,
            0,
            0,
            FW_BOLD,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            0,
            0,
            CLEARTYPE_QUALITY,
            0,
            wf.as_ptr(),
        );
        let old_f = SelectObject(hdc_buf, hf);
        SetBkMode(hdc_buf, TRANSPARENT);
        SetTextColor(hdc_buf, rgb(255, 255, 255));
        let lbl = if pin.translating {
            "…"
        } else if pin.show_zh {
            "原"
        } else {
            "译"
        };
        let wl = utf8_to_wide(lbl);
        DrawTextW(
            hdc_buf,
            wl.as_ptr(),
            -1,
            &mut brc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(hdc_buf, old_f);
        DeleteObject(hf);
    }

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old);
    DeleteObject(hbm_buf);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

unsafe extern "system" fn pin_wnd_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint_pin_window(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = lparam_xy(lp);
            let mut rc = RECT::default();
            GetClientRect(hwnd, &mut rc);
            let brc = pin_trans_btn_rc(rc.right - rc.left);
            let drag_off = SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                if let Some(p) = mgr.pins.iter_mut().find(|i| i.hwnd == hwnd) {
                    if p.dragging {
                        return Some(p.drag_off);
                    }
                    let ht = x >= brc.left && x < brc.right && y >= brc.top && y < brc.bottom;
                    if ht != p.hover_trans {
                        p.hover_trans = ht;
                    }
                }
                None
            });
            if let Some(off) = drag_off {
                let mut cur = POINT::default();
                GetCursorPos(&mut cur);
                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    cur.x - off.x,
                    cur.y - off.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE,
                );
            } else {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            let mut rc = RECT::default();
            GetClientRect(hwnd, &mut rc);
            let brc = pin_trans_btn_rc(rc.right - rc.left);
            if x >= brc.left && x < brc.right && y >= brc.top && y < brc.bottom {
                start_pin_translate(hwnd);
                return 0;
            }
            let mut wr = RECT::default();
            GetWindowRect(hwnd, &mut wr);
            let mut cur = POINT::default();
            GetCursorPos(&mut cur);
            SNIP_MGR.with(|m| {
                if let Some(p) = m.borrow_mut().pins.iter_mut().find(|i| i.hwnd == hwnd) {
                    p.dragging = true;
                    p.drag_off = POINT {
                        x: cur.x - wr.left,
                        y: cur.y - wr.top,
                    };
                }
            });
            SetCapture(hwnd);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            SNIP_MGR.with(|m| {
                if let Some(p) = m.borrow_mut().pins.iter_mut().find(|i| i.hwnd == hwnd) {
                    p.dragging = false;
                }
            });
            0
        }
        WM_LBUTTONDBLCLK => {
            DestroyWindow(hwnd);
            0
        }
        WM_RBUTTONUP => {
            let menu = CreatePopupMenu();
            if !menu.is_null() {
                let (has_zh, show_zh) = SNIP_MGR.with(|m| {
                    m.borrow()
                        .pins
                        .iter()
                        .find(|p| p.hwnd == hwnd)
                        .map(|p| (!p.bgra_zh.is_empty(), p.show_zh))
                        .unwrap_or((false, false))
                });
                let add = |id: usize, txt: &str, flags: UINT| {
                    let wt = utf8_to_wide(txt);
                    AppendMenuW(menu, flags, id, wt.as_ptr());
                };
                add(
                    1,
                    if show_zh {
                        "切换原图"
                    } else {
                        "原位翻译图片 (T)"
                    },
                    MF_STRING | if show_zh { MF_CHECKED } else { 0 },
                );
                if has_zh {
                    add(2, "复制 OCR 译文", MF_STRING);
                }
                add(3, "复制图片", MF_STRING);
                add(4, "另存为 PNG", MF_STRING);
                add(5, "还原 100% 大小", MF_STRING);
                AppendMenuW(menu, MF_SEPARATOR, 0, null());
                add(6, "关闭此贴图 (Esc)", MF_STRING);
                add(7, "关闭所有贴图", MF_STRING);

                let mut cur = POINT::default();
                GetCursorPos(&mut cur);
                let cmd = TrackPopupMenu(
                    menu,
                    TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                    cur.x,
                    cur.y,
                    0,
                    hwnd,
                    null(),
                );
                DestroyMenu(menu);
                match cmd {
                    1 => start_pin_translate(hwnd),
                    2 => {
                        let zh = SNIP_MGR.with(|m| {
                            m.borrow()
                                .pins
                                .iter()
                                .find(|p| p.hwnd == hwnd)
                                .map(|p| p.zh_text.clone())
                                .unwrap_or_default()
                        });
                        set_clipboard_text(&zh);
                    }
                    3 => {
                        let data = SNIP_MGR.with(|m| {
                            m.borrow().pins.iter().find(|p| p.hwnd == hwnd).map(|p| {
                                let b = if p.show_zh && !p.bgra_zh.is_empty() {
                                    p.bgra_zh.clone()
                                } else {
                                    p.bgra_orig.clone()
                                };
                                (p.img_w, p.img_h, b)
                            })
                        });
                        if let Some((w, h, b)) = data {
                            set_clipboard_bgra(&b, w, h);
                        }
                    }
                    4 => {
                        let (data, sd) = SNIP_MGR.with(|m| {
                            let mgr = m.borrow();
                            let d = mgr.pins.iter().find(|p| p.hwnd == hwnd).map(|p| {
                                let b = if p.show_zh && !p.bgra_zh.is_empty() {
                                    p.bgra_zh.clone()
                                } else {
                                    p.bgra_orig.clone()
                                };
                                (p.img_w, p.img_h, b)
                            });
                            (d, mgr.cfg.save_dir.clone())
                        });
                        if let Some((w, h, b)) = data {
                            if let Some(p) = save_file_dialog_png(hwnd, &sd, "KuaiYiNG_Pin.png") {
                                save_bgra_as_png(&p, &b, w, h);
                            }
                        }
                    }
                    5 => {
                        let sz = SNIP_MGR.with(|m| {
                            if let Some(p) = m.borrow_mut().pins.iter_mut().find(|i| i.hwnd == hwnd)
                            {
                                p.zoom = 1.0;
                                Some((p.img_w, p.img_h))
                            } else {
                                None
                            }
                        });
                        if let Some((w, h)) = sz {
                            SetWindowPos(
                                hwnd,
                                HWND_TOPMOST,
                                0,
                                0,
                                w,
                                h,
                                SWP_NOMOVE | SWP_NOACTIVATE,
                            );
                            InvalidateRect(hwnd, null(), 0);
                        }
                    }
                    6 => {
                        DestroyWindow(hwnd);
                    }
                    7 => close_all_pins(),
                    _ => {}
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            let notches = wheel_delta(wp) / 120;
            if notches == 0 {
                return 0;
            }
            let ctrl = (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0;
            if ctrl {
                let na = SNIP_MGR.with(|m| {
                    m.borrow_mut()
                        .pins
                        .iter_mut()
                        .find(|i| i.hwnd == hwnd)
                        .map(|p| {
                            let v = (p.alpha as i32 + notches * 20).clamp(70, 255) as u8;
                            p.alpha = v;
                            v
                        })
                });
                if let Some(a) = na {
                    SetLayeredWindowAttributes(hwnd, 0, a, LWA_ALPHA);
                }
            } else {
                let mut cur = POINT::default();
                GetCursorPos(&mut cur);
                let mut wr = RECT::default();
                GetWindowRect(hwnd, &mut wr);
                let next_rect = SNIP_MGR.with(|m| {
                    m.borrow_mut()
                        .pins
                        .iter_mut()
                        .find(|i| i.hwnd == hwnd)
                        .map(|p| {
                            let old_w = (wr.right - wr.left).max(1) as f64;
                            let old_h = (wr.bottom - wr.top).max(1) as f64;
                            let rx = ((cur.x - wr.left) as f64) / old_w;
                            let ry = ((cur.y - wr.top) as f64) / old_h;
                            let factor = if notches > 0 { 1.1 } else { 0.9 };
                            p.zoom = (p.zoom * factor).clamp(0.2, 4.0);
                            let nw = ((p.img_w as f64) * p.zoom).round().max(32.0) as i32;
                            let nh = ((p.img_h as f64) * p.zoom).round().max(32.0) as i32;
                            let nx = cur.x - ((nw as f64) * rx).round() as i32;
                            let ny = cur.y - ((nh as f64) * ry).round() as i32;
                            (nx, ny, nw, nh)
                        })
                });
                if let Some((nx, ny, nw, nh)) = next_rect {
                    SetWindowPos(hwnd, HWND_TOPMOST, nx, ny, nw, nh, SWP_NOACTIVATE);
                    InvalidateRect(hwnd, null(), 0);
                }
            }
            0
        }
        WM_KEYDOWN => {
            let vk = (wp & 0xFF) as i32;
            if vk == VK_ESCAPE {
                DestroyWindow(hwnd);
            } else if vk == 'T' as i32 || vk == 0x20 {
                start_pin_translate(hwnd);
            }
            0
        }
        WM_CLOSE | WM_DESTROY => {
            SNIP_MGR.with(|m| m.borrow_mut().pins.retain(|p| p.hwnd != hwnd));
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

#[allow(dead_code)]
fn unused_keep() {
    let _ = (
        gp_fill_round_rect,
        gp_round_card,
        PS_DASH,
        WM_KILLFOCUS,
        WM_MOUSEACTIVATE,
        MA_NOACTIVATE,
        WM_CHAR,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_norm_rect_crop_and_mosaic_blur() {
        let rc = norm_rect(50, 40, 10, 20);
        assert_eq!((rc.left, rc.top, rc.right, rc.bottom), (10, 20, 50, 40));

        // 构造 8x8 BGRA 测试图（左上四分之一红，其余蓝）
        let mut img = vec![0u8; 8 * 8 * 4];
        for y in 0..8 {
            for x in 0..8 {
                let off = (y * 8 + x) * 4;
                if x < 4 && y < 4 {
                    img[off] = 0;
                    img[off + 1] = 0;
                    img[off + 2] = 200;
                    img[off + 3] = 255;
                } else {
                    img[off] = 200;
                    img[off + 1] = 0;
                    img[off + 2] = 0;
                    img[off + 3] = 255;
                }
            }
        }
        let (cw, ch, mut sub) = crop_bgra(
            &img,
            8,
            8,
            RECT {
                left: 0,
                top: 0,
                right: 6,
                bottom: 6,
            },
        );
        assert_eq!((cw, ch), (6, 6));
        apply_mosaic_to_bgra(
            &mut sub,
            cw,
            ch,
            RECT {
                left: 0,
                top: 0,
                right: 4,
                bottom: 4,
            },
            4,
        );
        assert_eq!(sub[2], 200);
        apply_blur_to_bgra(
            &mut sub,
            cw,
            ch,
            RECT {
                left: 2,
                top: 2,
                right: 6,
                bottom: 6,
            },
            2,
        );
        // 混合交界处应既有 R 又有 B
        let mid_off = (3 * 6 + 3) * 4;
        assert!(sub[mid_off] > 0 && sub[mid_off + 2] > 0);
    }
}
