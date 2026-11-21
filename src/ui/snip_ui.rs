//! Snipaste 风格全屏截屏、10 种标注、放大镜取色、OCR 对照窗与多贴图浮窗

use crate::core::log::log_write;
use crate::core::ocr::{snip_translate, OcrLine, SnipBlock, SnipResult};
use crate::ui::snip_toast::{show_bubble, show_toast, Tone};
use crate::core::str_util::{utf8_to_wide, wide_to_utf8};
use crate::ui::win32_ui::{
    argb, ensure_gdiplus, enum_display_monitors_info, get_clipboard_bgra, get_clipboard_text,
    get_window_text, gp_fill_round_rect, gp_round_card, lparam_xy, rgb, save_bgra_as_png,
    save_file_dialog_png, set_clipboard_bgra, set_clipboard_text, wheel_delta,
    work_area_from_point, AppendMenuW, BeginPaint, BitBlt, ClientToScreen, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreatePen, CreatePopupMenu, CreateRectRgn,
    CreateSolidBrush, CreateWindowExW, DefSubclassProc, DefWindowProcW, DeleteDC, DeleteObject,
    DestroyMenu, DestroyWindow, DrawTextW, DwmGetWindowAttribute, EndPaint, EnumChildWindows,
    EnumWindows, FillRect,
    GdiFlush, GdipCreateFromHDC, GdipCreatePen1, GdipCreateSolidFill, GdipDeleteBrush,
    GdipDeleteGraphics, GdipDeletePen, GdipDrawEllipse, GdipDrawLine, GdipDrawLines,
    GdipDrawRectangle, GdipFillEllipse, GdipFillRectangle, GdipSetPenEndCap, GdipSetPenLineJoin,
    GdipSetPenStartCap, GdipSetSmoothingMode, GetAncestor, GetAsyncKeyState, GetClassNameW,
    GetClientRect, GetCurrentProcessId, GetCursorPos, GetDC, GetDesktopWindow, GetForegroundWindow,
    GetModuleHandleW, GetStockObject, GetSystemMetrics, GetTopWindow, GetWindow, GetWindowLongPtrW,
    GetWindowRect, GetWindowThreadProcessId, GpPointF, InvalidateRect, IsIconic, IsWindow,
    IsWindowVisible, LoadCursorW, PostMessageW, RegisterClassExW, ReleaseCapture, ReleaseDC,
    ScreenToClient, SelectClipRgn, SelectObject, SetBkMode, SetCapture, SetCursor, SetFocus,
    SetLayeredWindowAttributes, SetStretchBltMode, SetTextColor, SetWindowPos, SetWindowSubclass,
    ShowWindow, StretchBlt, TrackPopupMenu, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
    CLEARTYPE_QUALITY, COLORONCOLOR, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, DEFAULT_CHARSET,
    DIB_RGB_COLORS, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE,
    DT_TOP, DT_VCENTER, DT_WORDBREAK, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWORD_PTR,
    ES_AUTOHSCROLL, ES_MULTILINE, ES_WANTRETURN, FW_BOLD, FW_MEDIUM, FW_NORMAL, GA_ROOT,
    GP_LINE_CAP_ROUND, GP_LINE_JOIN_ROUND, GP_OK, GP_SMOOTHING_ANTIALIAS, GP_UNIT_PIXEL,
    GWL_EXSTYLE, GW_HWNDNEXT, HALFTONE, HBITMAP, HDC, HFONT, HGDIOBJ, HWND, HWND_TOPMOST, IDC_ARROW, IDC_CROSS,
    IDC_IBEAM, IDC_SIZENESW, IDC_SIZENS, IDC_SIZENWSE, IDC_SIZEWE, LPARAM, LRESULT, LWA_ALPHA,
    MA_NOACTIVATE, MF_CHECKED, MF_SEPARATOR, MF_STRING, MINMAXINFO, NULL_BRUSH, PAINTSTRUCT, POINT,
    PS_DASH, PS_SOLID, RECT, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SRCCOPY, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_SHOW,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TRANSPARENT, UINT, UINT_PTR, VK_CONTROL,
    VK_ESCAPE, VK_RETURN, VK_SHIFT, WM_CAPTURECHANGED, WM_CHAR, WM_CLOSE, WM_DESTROY,
    WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDBLCLK,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WNDCLASSEXW, WPARAM, WS_CHILD, WS_CLIPCHILDREN,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
};
use crate::ui::{
    WM_APP_FLASH_HINT, WM_APP_PIN_DONE, WM_APP_PIN_LINES, WM_APP_SEL_TRANS, WM_APP_SNIP_DONE,
};
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
#[derive(Clone)]
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
    pub pin_mods: i32,
    pub pin_vk: i32,
    pub sub_window: bool,
}

impl Default for SnipCfg {
    fn default() -> Self {
        Self {
            hwnd_main: 0,
            ai_url: String::new(),
            ai_key: String::new(),
            ai_model: String::new(),
            baidu_appid: String::new(),
            baidu_secret: String::new(),
            baidu_url: String::new(),
            cache_dir: String::new(),
            save_dir: String::new(),
            pin_mods: 0,
            pin_vk: 0,
            sub_window: true,
        }
    }
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

/// 判断点是否落在矩形内
#[inline]
fn pt_in_rect(rc: RECT, x: i32, y: i32) -> bool {
    x >= rc.left && x < rc.right && y >= rc.top && y < rc.bottom
}

/// 两个矩形求交
#[inline]
fn intersect_rects(a: RECT, b: RECT) -> RECT {
    RECT {
        left: a.left.max(b.left),
        top: a.top.max(b.top),
        right: a.right.min(b.right),
        bottom: a.bottom.min(b.bottom),
    }
}

/// 比较两矩形是否完全一致
#[inline]
fn rect_eq(a: RECT, b: RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}

/// 按 3 档线宽返回画笔半径（像素）
#[inline]
pub fn brush_radius_for_width(width: i32) -> i32 {
    if width <= 2 {
        10
    } else if width <= 4 {
        18
    } else {
        28
    }
}

/// 按 3 档线宽返回马赛克块大小（像素）
#[inline]
pub fn mosaic_block_for_width(width: i32) -> i32 {
    if width <= 2 {
        8
    } else if width <= 4 {
        12
    } else {
        16
    }
}

/// 按 3 档线宽返回模糊半径（像素）
#[inline]
pub fn blur_radius_for_width(width: i32) -> i32 {
    if width <= 2 {
        6
    } else if width <= 4 {
        10
    } else {
        14
    }
}

/// 按全图全局网格对 ROI 区域预计算块均值马赛克图层，返回规范化 ROI 与紧凑 BGRA 缓冲
pub fn compute_mosaic_layer(
    orig: &[u8],
    w: i32,
    h: i32,
    roi: RECT,
    block_px: i32,
) -> (RECT, Vec<u8>) {
    if w <= 0 || h <= 0 || orig.len() < (w as usize) * (h as usize) * 4 {
        return (RECT::default(), Vec::new());
    }
    let blk = block_px.clamp(4, 32);
    let x0 = roi.left.clamp(0, w);
    let y0 = roi.top.clamp(0, h);
    let x1 = roi.right.clamp(0, w);
    let y1 = roi.bottom.clamp(0, h);
    if x1 <= x0 || y1 <= y0 {
        return (RECT::default(), Vec::new());
    }
    let lw = (x1 - x0) as usize;
    let lh = (y1 - y0) as usize;
    let mut out = vec![0u8; lw * lh * 4];
    let bx_start = (x0 / blk) * blk;
    let by_start = (y0 / blk) * blk;

    let mut by = by_start;
    while by < y1 {
        let bh_end = (by + blk).min(h);
        let wy0 = by.max(y0);
        let wy1 = bh_end.min(y1);
        let mut bx = bx_start;
        while bx < x1 {
            let bw_end = (bx + blk).min(w);
            let wx0 = bx.max(x0);
            let wx1 = bw_end.min(x1);
            let mut sb = 0u64;
            let mut sg = 0u64;
            let mut sr = 0u64;
            let mut cnt = 0u64;
            for py in by..bh_end {
                let row_off = (py as usize) * (w as usize);
                for px in bx..bw_end {
                    let off = (row_off + (px as usize)) * 4;
                    sb += orig[off] as u64;
                    sg += orig[off + 1] as u64;
                    sr += orig[off + 2] as u64;
                    cnt += 1;
                }
            }
            if let (Some(ab), Some(ag), Some(ar)) = (
                sb.checked_div(cnt),
                sg.checked_div(cnt),
                sr.checked_div(cnt),
            ) {
                let (ab, ag, ar) = (ab as u8, ag as u8, ar as u8);
                for py in wy0..wy1 {
                    let dst_row = ((py - y0) as usize) * lw;
                    for px in wx0..wx1 {
                        let d_off = (dst_row + ((px - x0) as usize)) * 4;
                        out[d_off] = ab;
                        out[d_off + 1] = ag;
                        out[d_off + 2] = ar;
                        out[d_off + 3] = 255;
                    }
                }
            }
            bx += blk;
        }
        by += blk;
    }

    (
        RECT {
            left: x0,
            top: y0,
            right: x1,
            bottom: y1,
        },
        out,
    )
}

/// 将指定图层矩形拷贝回目标 BGRA 画布
fn blit_layer_rect_to_bgra(dst: &mut [u8], dst_w: i32, dst_h: i32, layer_rc: RECT, layer: &[u8]) {
    let lw = (layer_rc.right - layer_rc.left).max(0);
    let lh = (layer_rc.bottom - layer_rc.top).max(0);
    if dst_w <= 0
        || dst_h <= 0
        || lw <= 0
        || lh <= 0
        || dst.len() < (dst_w as usize) * (dst_h as usize) * 4
        || layer.len() < (lw as usize) * (lh as usize) * 4
    {
        return;
    }
    let row_bytes = (lw as usize) * 4;
    for row in 0..lh {
        let s_off = (row as usize) * row_bytes;
        let d_off =
            (((layer_rc.top + row) as usize) * (dst_w as usize) + (layer_rc.left as usize)) * 4;
        dst[d_off..d_off + row_bytes].copy_from_slice(&layer[s_off..s_off + row_bytes]);
    }
}

/// 对 top-down BGRA 指定矩形执行块均值马赛克
pub fn apply_mosaic_to_bgra(bgra: &mut [u8], w: i32, h: i32, rc: RECT, block_px: i32) {
    let (layer_rc, layer) = compute_mosaic_layer(bgra, w, h, rc, block_px);
    if !layer.is_empty() {
        blit_layer_rect_to_bgra(bgra, w, h, layer_rc, &layer);
    }
}

/// 滑动窗口水平 1D 盒式模糊（边缘 clamp 复制，消除边缘黑边）
fn box_blur_h(src: &[u8], dst: &mut [u8], pw: i32, ph: i32, rad: i32) {
    let ksize = (2 * rad + 1) as u32;
    let half = ksize / 2;
    let pw_u = pw as usize;
    for y in 0..ph as usize {
        let row = y * pw_u;
        let p0 = row * 4;
        let b0 = src[p0] as u32;
        let g0 = src[p0 + 1] as u32;
        let r0 = src[p0 + 2] as u32;
        let mut sb = b0 * ((rad as u32) + 1);
        let mut sg = g0 * ((rad as u32) + 1);
        let mut sr = r0 * ((rad as u32) + 1);
        for k in 1..=rad {
            let sx = k.min(pw - 1) as usize;
            let off = (row + sx) * 4;
            sb += src[off] as u32;
            sg += src[off + 1] as u32;
            sr += src[off + 2] as u32;
        }
        for x in 0..pw {
            let d_off = (row + (x as usize)) * 4;
            dst[d_off] = ((sb + half) / ksize) as u8;
            dst[d_off + 1] = ((sg + half) / ksize) as u8;
            dst[d_off + 2] = ((sr + half) / ksize) as u8;
            dst[d_off + 3] = 255;

            let add_x = (x + 1 + rad).min(pw - 1) as usize;
            let sub_x = (x - rad).max(0) as usize;
            let a_off = (row + add_x) * 4;
            let s_off = (row + sub_x) * 4;
            sb = sb + (src[a_off] as u32) - (src[s_off] as u32);
            sg = sg + (src[a_off + 1] as u32) - (src[s_off + 1] as u32);
            sr = sr + (src[a_off + 2] as u32) - (src[s_off + 2] as u32);
        }
    }
}

/// 行优先滑动窗口垂直 1D 盒式模糊（高缓存命中，无列跳跃）
fn box_blur_v(src: &[u8], dst: &mut [u8], pw: i32, ph: i32, rad: i32, col_sum: &mut [u32]) {
    let ksize = (2 * rad + 1) as u32;
    let half = ksize / 2;
    let pw_u = pw as usize;
    let mul0 = (rad as u32) + 1;
    for x in 0..pw_u {
        let off = x * 4;
        let c = x * 3;
        col_sum[c] = (src[off] as u32) * mul0;
        col_sum[c + 1] = (src[off + 1] as u32) * mul0;
        col_sum[c + 2] = (src[off + 2] as u32) * mul0;
    }
    for k in 1..=rad {
        let sy = k.min(ph - 1) as usize;
        let row_off = sy * pw_u;
        for x in 0..pw_u {
            let off = (row_off + x) * 4;
            let c = x * 3;
            col_sum[c] += src[off] as u32;
            col_sum[c + 1] += src[off + 1] as u32;
            col_sum[c + 2] += src[off + 2] as u32;
        }
    }
    for y in 0..ph {
        let d_row = (y as usize) * pw_u;
        for x in 0..pw_u {
            let d_off = (d_row + x) * 4;
            let c = x * 3;
            dst[d_off] = ((col_sum[c] + half) / ksize) as u8;
            dst[d_off + 1] = ((col_sum[c + 1] + half) / ksize) as u8;
            dst[d_off + 2] = ((col_sum[c + 2] + half) / ksize) as u8;
            dst[d_off + 3] = 255;
        }
        if y + 1 < ph {
            let add_y = (y + 1 + rad).min(ph - 1) as usize;
            let sub_y = (y - rad).max(0) as usize;
            let a_row = add_y * pw_u;
            let s_row = sub_y * pw_u;
            for x in 0..pw_u {
                let a_off = (a_row + x) * 4;
                let s_off = (s_row + x) * 4;
                let c = x * 3;
                col_sum[c] = col_sum[c] + (src[a_off] as u32) - (src[s_off] as u32);
                col_sum[c + 1] = col_sum[c + 1] + (src[a_off + 1] as u32) - (src[s_off + 1] as u32);
                col_sum[c + 2] = col_sum[c + 2] + (src[a_off + 2] as u32) - (src[s_off + 2] as u32);
            }
        }
    }
}

/// 对 ROI 区域基于外扩上下文执行 2 轮可分离平滑盒式模糊，返回规范化 ROI 与紧凑 BGRA 缓冲
pub fn compute_blur_layer(orig: &[u8], w: i32, h: i32, roi: RECT, radius: i32) -> (RECT, Vec<u8>) {
    if w <= 0 || h <= 0 || orig.len() < (w as usize) * (h as usize) * 4 {
        return (RECT::default(), Vec::new());
    }
    let rad = radius.clamp(2, 24);
    let rx0 = roi.left.clamp(0, w);
    let ry0 = roi.top.clamp(0, h);
    let rx1 = roi.right.clamp(0, w);
    let ry1 = roi.bottom.clamp(0, h);
    if rx1 <= rx0 || ry1 <= ry0 {
        return (RECT::default(), Vec::new());
    }
    let pad = (rad * 2).min(48);
    let px0 = (rx0 - pad).max(0);
    let py0 = (ry0 - pad).max(0);
    let px1 = (rx1 + pad).min(w);
    let py1 = (ry1 + pad).min(h);
    let (pw, ph, mut buf) = crop_bgra(
        orig,
        w,
        h,
        RECT {
            left: px0,
            top: py0,
            right: px1,
            bottom: py1,
        },
    );
    if pw <= 0 || ph <= 0 {
        return (RECT::default(), Vec::new());
    }
    let mut tmp = vec![0u8; buf.len()];
    let mut col_sum = vec![0u32; (pw as usize) * 3];
    for _ in 0..2 {
        box_blur_h(&buf, &mut tmp, pw, ph, rad);
        box_blur_v(&tmp, &mut buf, pw, ph, rad, &mut col_sum);
    }

    let (_, _, out) = crop_bgra(
        &buf,
        pw,
        ph,
        RECT {
            left: rx0 - px0,
            top: ry0 - py0,
            right: rx1 - px0,
            bottom: ry1 - py0,
        },
    );
    (
        RECT {
            left: rx0,
            top: ry0,
            right: rx1,
            bottom: ry1,
        },
        out,
    )
}

/// 对 top-down BGRA 指定矩形执行可分离平滑均值模糊
pub fn apply_blur_to_bgra(bgra: &mut [u8], w: i32, h: i32, rc: RECT, radius: i32) {
    let (layer_rc, layer) = compute_blur_layer(bgra, w, h, rc, radius);
    if !layer.is_empty() {
        blit_layer_rect_to_bgra(bgra, w, h, layer_rc, &layer);
    }
}

/// 将预计算滤镜图层沿线段 `(p0 -> p1)` 的圆角胶囊画笔范围印到目标 BGRA 画布
#[allow(clippy::too_many_arguments)]
pub fn stamp_brush_capsule(
    dst: &mut [u8],
    dst_w: i32,
    dst_h: i32,
    layer_rc: RECT,
    layer: &[u8],
    p0: (i32, i32),
    p1: (i32, i32),
    brush_r: i32,
    clip_rc: RECT,
) {
    let lw = layer_rc.right - layer_rc.left;
    let lh = layer_rc.bottom - layer_rc.top;
    if dst_w <= 0
        || dst_h <= 0
        || lw <= 0
        || lh <= 0
        || dst.len() < (dst_w as usize) * (dst_h as usize) * 4
        || layer.len() < (lw as usize) * (lh as usize) * 4
    {
        return;
    }
    let r = brush_r.max(1);
    let cap_rc = RECT {
        left: p0.0.min(p1.0) - r,
        top: p0.1.min(p1.1) - r,
        right: p0.0.max(p1.0) + r + 1,
        bottom: p0.1.max(p1.1) + r + 1,
    };
    let dst_rc = RECT {
        left: 0,
        top: 0,
        right: dst_w,
        bottom: dst_h,
    };
    let valid_rc = intersect_rects(
        intersect_rects(cap_rc, dst_rc),
        intersect_rects(layer_rc, clip_rc),
    );
    if valid_rc.right <= valid_rc.left || valid_rc.bottom <= valid_rc.top {
        return;
    }

    let vx = (p1.0 - p0.0) as f32;
    let vy = (p1.1 - p0.1) as f32;
    let len2 = vx * vx + vy * vy;
    let r2_i = (r as i64) * (r as i64);
    let r2_f = r2_i as f32;

    for py in valid_rc.top..valid_rc.bottom {
        let dst_row = (py as usize) * (dst_w as usize);
        let lyr_row = ((py - layer_rc.top) as usize) * (lw as usize);
        let dy0 = py - p0.1;
        for px in valid_rc.left..valid_rc.right {
            let hit = if len2 < 0.0001 {
                let dx0 = (px - p0.0) as i64;
                let dy0_i = dy0 as i64;
                dx0 * dx0 + dy0_i * dy0_i <= r2_i
            } else {
                let t = (((px - p0.0) as f32 * vx + (dy0 as f32) * vy) / len2).clamp(0.0, 1.0);
                let cx = (p0.0 as f32) + t * vx - (px as f32);
                let cy = (p0.1 as f32) + t * vy - (py as f32);
                cx * cx + cy * cy <= r2_f
            };
            if hit {
                let s_off = (lyr_row + ((px - layer_rc.left) as usize)) * 4;
                let d_off = (dst_row + (px as usize)) * 4;
                dst[d_off] = layer[s_off];
                dst[d_off + 1] = layer[s_off + 1];
                dst[d_off + 2] = layer[s_off + 2];
                dst[d_off + 3] = 255;
            }
        }
    }
}

/// 将整条自由涂抹路径从预计算滤镜图层印到目标 BGRA 画布
#[allow(clippy::too_many_arguments)]
pub fn apply_brush_stroke_from_layer(
    dst: &mut [u8],
    dst_w: i32,
    dst_h: i32,
    layer_rc: RECT,
    layer: &[u8],
    pts: &[(i32, i32)],
    brush_r: i32,
    clip_rc: RECT,
) {
    if pts.is_empty() {
        return;
    }
    if pts.len() == 1 {
        stamp_brush_capsule(
            dst, dst_w, dst_h, layer_rc, layer, pts[0], pts[0], brush_r, clip_rc,
        );
        return;
    }
    for seg in pts.windows(2) {
        stamp_brush_capsule(
            dst, dst_w, dst_h, layer_rc, layer, seg[0], seg[1], brush_r, clip_rc,
        );
    }
}

/// 根据在线物理显示器矩形计算最小虚拟桌面外接边界 `(vx, vy, vw, vh)`
pub fn compute_virtual_bounds(
    monitors: &[RECT],
    fallback: (i32, i32, i32, i32),
) -> (i32, i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    let mut any = false;
    for &rc in monitors {
        if rc.right > rc.left && rc.bottom > rc.top {
            min_x = min_x.min(rc.left);
            min_y = min_y.min(rc.top);
            max_x = max_x.max(rc.right);
            max_y = max_y.max(rc.bottom);
            any = true;
        }
    }
    if any {
        (min_x, min_y, max_x - min_x, max_y - min_y)
    } else {
        fallback
    }
}

/// 返回包含指定局部坐标 `(x, y)` 或距离最近的显示器局部边界（避免悬浮控件落入异形多屏死角）
pub fn monitor_bounds_for_point(mon_rects: &[RECT], vw: i32, vh: i32, x: i32, y: i32) -> RECT {
    for &rc in mon_rects {
        if pt_in_rect(rc, x, y) {
            return rc;
        }
    }
    let mut best: Option<RECT> = None;
    let mut best_d2 = i64::MAX;
    for &rc in mon_rects {
        if rc.right <= rc.left || rc.bottom <= rc.top {
            continue;
        }
        let dx = if x < rc.left {
            (rc.left - x) as i64
        } else if x >= rc.right {
            (x - (rc.right - 1)) as i64
        } else {
            0
        };
        let dy = if y < rc.top {
            (rc.top - y) as i64
        } else if y >= rc.bottom {
            (y - (rc.bottom - 1)) as i64
        } else {
            0
        };
        let d2 = dx * dx + dy * dy;
        if d2 < best_d2 {
            best_d2 = d2;
            best = Some(rc);
        }
    }
    best.unwrap_or(RECT {
        left: 0,
        top: 0,
        right: vw.max(1),
        bottom: vh.max(1),
    })
}

struct VirtualScreenCapture {
    vx: i32,
    vy: i32,
    vw: i32,
    vh: i32,
    bgra: Vec<u8>,
    mon_rects: Vec<RECT>,
    work_rects: Vec<RECT>,
}

/// 逐屏抓取虚拟桌面 32-bit top-down BGRA，同时返回以 `(vx, vy)` 为原点的局部显示器与工作区边界列表
fn capture_virtual_screen_with_monitors() -> Option<VirtualScreenCapture> {
    unsafe {
        let fallback = (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        );
        let mon_infos = enum_display_monitors_info();
        let mons: Vec<RECT> = mon_infos.iter().map(|m| m.monitor).collect();
        let (vx, vy, vw, vh) = compute_virtual_bounds(&mons, fallback);
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
        let len = (vw as usize) * (vh as usize) * 4;
        std::ptr::write_bytes(bits, 0, len);
        let old = SelectObject(hdc_mem, hbm);
        if !mons.is_empty() {
            for &rc in &mons {
                let mw = rc.right - rc.left;
                let mh = rc.bottom - rc.top;
                if mw > 0 && mh > 0 {
                    BitBlt(
                        hdc_mem,
                        rc.left - vx,
                        rc.top - vy,
                        mw,
                        mh,
                        hdc_scr,
                        rc.left,
                        rc.top,
                        SRCCOPY | CAPTUREBLT,
                    );
                }
            }
        } else {
            BitBlt(hdc_mem, 0, 0, vw, vh, hdc_scr, vx, vy, SRCCOPY | CAPTUREBLT);
        }
        GdiFlush();

        let mut out = vec![0u8; len];
        copy_nonoverlapping(bits.cast::<u8>(), out.as_mut_ptr(), len);
        for px in out.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }

        SelectObject(hdc_mem, old);
        DeleteObject(hbm);
        DeleteDC(hdc_mem);
        ReleaseDC(null_mut(), hdc_scr);

        let local_mons = mon_infos
            .iter()
            .map(|m| RECT {
                left: m.monitor.left - vx,
                top: m.monitor.top - vy,
                right: m.monitor.right - vx,
                bottom: m.monitor.bottom - vy,
            })
            .collect();
        let local_works = mon_infos
            .iter()
            .map(|m| RECT {
                left: m.work.left - vx,
                top: m.work.top - vy,
                right: m.work.right - vx,
                bottom: m.work.bottom - vy,
            })
            .collect();
        Some(VirtualScreenCapture {
            vx,
            vy,
            vw,
            vh,
            bgra: out,
            mon_rects: local_mons,
            work_rects: local_works,
        })
    }
}

/// 在全屏桌面上截取 32-bit top-down BGRA，返回 `(vx, vy, vw, vh, bgra)`
pub fn capture_virtual_screen_bgra() -> Option<(i32, i32, i32, i32, Vec<u8>)> {
    capture_virtual_screen_with_monitors().map(|c| (c.vx, c.vy, c.vw, c.vh, c.bgra))
}

/// 创建未填充像素的 32-bit top-down DIB 位图
unsafe fn create_empty_topdown_dib(hdc: HDC, w: i32, h: i32) -> Option<(HBITMAP, *mut u8)> {
    if w <= 0 || h <= 0 {
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
        if !hbm.is_null() {
            DeleteObject(hbm);
        }
        return None;
    }
    Some((hbm, bits.cast::<u8>()))
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
    let (hbm, bits) = create_empty_topdown_dib(hdc, w, h)?;
    copy_nonoverlapping(bgra.as_ptr(), bits, (w as usize) * (h as usize) * 4);
    GdiFlush();
    Some((hbm, bits))
}

// ---- 绘制全部矢量与文本标注 ----

unsafe fn draw_vector_annots(hdc: HDC, annots: &[AnnotItem], off_x: i32, off_y: i32) {
    draw_vector_annots_with_draft(hdc, annots, None, off_x, off_y);
}

unsafe fn draw_vector_annots_with_draft(
    hdc: HDC,
    annots: &[AnnotItem],
    draft: Option<&AnnotItem>,
    off_x: i32,
    off_y: i32,
) {
    ensure_gdiplus();
    let mut g: *mut c_void = null_mut();
    let gp_ok = GdipCreateFromHDC(hdc, &mut g) == GP_OK && !g.is_null();
    if gp_ok {
        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);
    }

    for it in annots.iter().chain(draft) {
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
    let crop_rc = RECT {
        left: 0,
        top: 0,
        right: cw,
        bottom: ch,
    };
    let mut cached_layers: Vec<(AnnotTool, i32, Vec<u8>)> = Vec::new();
    for it in annots {
        if !matches!(it.tool, AnnotTool::Mosaic | AnnotTool::Blur) {
            continue;
        }
        let layer_idx = if let Some(idx) = cached_layers
            .iter()
            .position(|(t, w, _)| *t == it.tool && *w == it.width)
        {
            idx
        } else {
            let (_, layer) = match it.tool {
                AnnotTool::Mosaic => {
                    compute_mosaic_layer(orig_bgra, vw, vh, sel, mosaic_block_for_width(it.width))
                }
                _ => compute_blur_layer(orig_bgra, vw, vh, sel, blur_radius_for_width(it.width)),
            };
            cached_layers.push((it.tool, it.width, layer));
            cached_layers.len() - 1
        };
        let layer = &cached_layers[layer_idx].2;
        if !it.pts.is_empty() {
            let local_pts: Vec<(i32, i32)> = it
                .pts
                .iter()
                .map(|&(px, py)| (px - sel.left, py - sel.top))
                .collect();
            apply_brush_stroke_from_layer(
                &mut crop,
                cw,
                ch,
                crop_rc,
                layer,
                &local_pts,
                brush_radius_for_width(it.width),
                crop_rc,
            );
        } else {
            // 兼容旧的矩形数据格式回退
            let local_rc = norm_rect(
                it.x1 - sel.left,
                it.y1 - sel.top,
                it.x2 - sel.left,
                it.y2 - sel.top,
            );
            if it.tool == AnnotTool::Mosaic {
                apply_mosaic_to_bgra(
                    &mut crop,
                    cw,
                    ch,
                    local_rc,
                    mosaic_block_for_width(it.width),
                );
            } else {
                apply_blur_to_bgra(&mut crop, cw, ch, local_rc, blur_radius_for_width(it.width));
            }
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

// ---- 全局状态容器与持久 GDI 缓存 ----

struct OverlayGdiCache {
    vw: i32,
    vh: i32,
    hdc_back: HDC,
    hbm_back: HBITMAP,
    old_back: HGDIOBJ,
    hdc_dim: HDC,
    hbm_dim: HBITMAP,
    old_dim: HGDIOBJ,
    hdc_work: HDC,
    hbm_work: HBITMAP,
    old_work: HGDIOBJ,
    work_bits: *mut u8,
    hf_badge: HFONT,
    hf_btn: HFONT,
    hf_mag: HFONT,
}

impl Default for OverlayGdiCache {
    fn default() -> Self {
        Self {
            vw: 0,
            vh: 0,
            hdc_back: null_mut(),
            hbm_back: null_mut(),
            old_back: null_mut(),
            hdc_dim: null_mut(),
            hbm_dim: null_mut(),
            old_dim: null_mut(),
            hdc_work: null_mut(),
            hbm_work: null_mut(),
            old_work: null_mut(),
            work_bits: null_mut(),
            hf_badge: null_mut(),
            hf_btn: null_mut(),
            hf_mag: null_mut(),
        }
    }
}

impl Drop for OverlayGdiCache {
    fn drop(&mut self) {
        self.free();
    }
}

impl OverlayGdiCache {
    fn free(&mut self) {
        unsafe {
            if !self.hdc_back.is_null() {
                if !self.old_back.is_null() {
                    SelectObject(self.hdc_back, self.old_back);
                }
                DeleteDC(self.hdc_back);
            }
            if !self.hbm_back.is_null() {
                DeleteObject(self.hbm_back);
            }
            if !self.hdc_dim.is_null() {
                if !self.old_dim.is_null() {
                    SelectObject(self.hdc_dim, self.old_dim);
                }
                DeleteDC(self.hdc_dim);
            }
            if !self.hbm_dim.is_null() {
                DeleteObject(self.hbm_dim);
            }
            if !self.hdc_work.is_null() {
                if !self.old_work.is_null() {
                    SelectObject(self.hdc_work, self.old_work);
                }
                DeleteDC(self.hdc_work);
            }
            if !self.hbm_work.is_null() {
                DeleteObject(self.hbm_work);
            }
            if !self.hf_badge.is_null() {
                DeleteObject(self.hf_badge);
            }
            if !self.hf_btn.is_null() {
                DeleteObject(self.hf_btn);
            }
            if !self.hf_mag.is_null() {
                DeleteObject(self.hf_mag);
            }
        }
        // 逐字段清零而非 `*self = Self::default()`，防止在 Drop::drop 内触发二次析构无限递归
        self.vw = 0;
        self.vh = 0;
        self.hdc_back = null_mut();
        self.hbm_back = null_mut();
        self.old_back = null_mut();
        self.hdc_dim = null_mut();
        self.hbm_dim = null_mut();
        self.old_dim = null_mut();
        self.hdc_work = null_mut();
        self.hbm_work = null_mut();
        self.old_work = null_mut();
        self.work_bits = null_mut();
        self.hf_badge = null_mut();
        self.hf_btn = null_mut();
        self.hf_mag = null_mut();
    }

    unsafe fn init(&mut self, vw: i32, vh: i32, orig_bgra: &[u8]) -> bool {
        self.free();
        let total = (vw as usize).saturating_mul(vh as usize).saturating_mul(4);
        if vw <= 0 || vh <= 0 || orig_bgra.len() < total {
            return false;
        }
        let hdc_scr = GetDC(null_mut());
        if hdc_scr.is_null() {
            return false;
        }

        let mut ok = false;
        'build: {
            self.hdc_back = CreateCompatibleDC(hdc_scr);
            self.hbm_back = CreateCompatibleBitmap(hdc_scr, vw, vh);
            if self.hdc_back.is_null() || self.hbm_back.is_null() {
                break 'build;
            }
            self.old_back = SelectObject(self.hdc_back, self.hbm_back);

            self.hdc_dim = CreateCompatibleDC(hdc_scr);
            let Some((hbm_d, dim_bits)) = create_empty_topdown_dib(hdc_scr, vw, vh) else {
                break 'build;
            };
            self.hbm_dim = hbm_d;
            self.old_dim = SelectObject(self.hdc_dim, self.hbm_dim);
            let dim_dst = std::slice::from_raw_parts_mut(dim_bits, total);
            for (d, s) in dim_dst
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(orig_bgra[..total].as_chunks::<4>().0)
            {
                d[0] = (((s[0] as u32) * 136) >> 8) as u8;
                d[1] = (((s[1] as u32) * 136) >> 8) as u8;
                d[2] = (((s[2] as u32) * 136) >> 8) as u8;
                d[3] = 255;
            }

            self.hdc_work = CreateCompatibleDC(hdc_scr);
            let Some((hbm_w, work_bits)) = create_topdown_dib_from_bgra(hdc_scr, orig_bgra, vw, vh)
            else {
                break 'build;
            };
            self.hbm_work = hbm_w;
            self.old_work = SelectObject(self.hdc_work, self.hbm_work);
            self.work_bits = work_bits;

            let wf_badge = utf8_to_wide("Segoe UI");
            self.hf_badge = CreateFontW(
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
                wf_badge.as_ptr(),
            );
            let wf_ui = utf8_to_wide("Microsoft YaHei UI");
            self.hf_btn = CreateFontW(
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
                wf_ui.as_ptr(),
            );
            self.hf_mag = CreateFontW(
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
                wf_ui.as_ptr(),
            );
            GdiFlush();
            self.vw = vw;
            self.vh = vh;
            ok = true;
        }
        ReleaseDC(null_mut(), hdc_scr);
        if !ok {
            self.free();
        }
        ok
    }

    unsafe fn work_slice_mut(&mut self) -> Option<&mut [u8]> {
        if self.work_bits.is_null() || self.vw <= 0 || self.vh <= 0 {
            return None;
        }
        let total = (self.vw as usize) * (self.vh as usize) * 4;
        Some(std::slice::from_raw_parts_mut(self.work_bits, total))
    }

    unsafe fn copy_orig(&mut self, orig: &[u8]) {
        if self.work_bits.is_null() || self.vw <= 0 || self.vh <= 0 {
            return;
        }
        let total = (self.vw as usize) * (self.vh as usize) * 4;
        if orig.len() >= total {
            copy_nonoverlapping(orig.as_ptr(), self.work_bits, total);
        }
    }
}

struct FilterLayerCache {
    tool: AnnotTool,
    width: i32,
    roi: RECT,
    bgra: Vec<u8>,
}

#[derive(Default)]
struct OverlayState {
    hwnd: HWND,
    active: bool,
    trans_mode: bool, // true = 截屏直接翻译模式，false = 全能工具栏截屏模式
    #[allow(dead_code)]
    vx: i32,
    #[allow(dead_code)]
    vy: i32,
    vw: i32,
    vh: i32,
    mon_rects: Vec<RECT>,
    work_rects: Vec<RECT>,
    orig_bgra: Vec<u8>,
    gdi: OverlayGdiCache,
    filter_caches: Vec<FilterLayerCache>,
    win_rects: Vec<WindowCandidate>,

    dragging_sel: bool,
    drag_mode: i32, // 0=无, 1=新建, 2=移动, 10..17=8手柄
    drag_start_pt: POINT,
    drag_orig_rc: RECT,
    has_sel: bool,
    sel_rc: RECT,
    hover_win_rc: RECT,
    hover_win_hwnd: isize,
    hover_depth: usize,
    hover_whole_idx: usize,
    hover_levels_len: usize,
    hover_lock_whole: bool,

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
    /// OCR 行框 / 翻译块：结果窗也支持划选取字
    lines: Vec<OcrLine>,
    blocks: Vec<SnipBlock>,
    /// 正在识别（状态栏显示「识别中…」，译文相关按钮禁用）
    translating: bool,
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
    /// OCR 行框/翻译块：Ctrl+左键划选取字用
    lines: Vec<OcrLine>,
    blocks: Vec<SnipBlock>,
    /// 选中态（Ctrl+左键进入）：划字复制、双击复制整图，不关闭贴图
    sel: PinSel,
    zoom: f64,
    alpha: u8,
    hover_trans: bool,
    dragging: bool,
    drag_off: POINT,
}

/// 贴图「选中态」状态机
#[derive(Clone, Default)]
struct PinSel {
    /// 已进入选中态（描边变蓝、光标 IBeam）
    mode: bool,
    /// 正在拖框
    dragging: bool,
    /// 拖框起止点（窗口客户区坐标）
    from: POINT,
    to: POINT,
    /// 已有确认选区（窗口客户区坐标）
    rect: RECT,
    has_rect: bool,
    /// 选中的文本与语言（true=译图态取中文）
    text: String,
    is_zh: bool,
    /// 行框懒加载中
    loading: bool,
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

pub fn update_snip_cfg_sub_win(enabled: bool) {
    SNIP_MGR.with(|m| m.borrow_mut().cfg.sub_window = enabled);
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

/// 判断窗口类是否属于任务栏、桌面外壳或系统隐形覆盖层，应从自动窗口框选中排除
pub fn should_skip_auto_window_class(cls: &str) -> bool {
    matches!(
        cls,
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "Progman"
            | "WorkerW"
            | "Windows.UI.Core.CoreWindow"
            | "EdgeUiInputTopWndClass"
            | "EdgeUiInputWndClass"
            | "ForegroundStaging"
            | "ApplicationManager_DesktopShellWindow"
            | "DummyDwmListenerWindow"
            | "MultitaskingViewFrame"
            | "NotifyIconOverflowWindow"
    )
}

/// 将外部可见窗口局部坐标按所属屏幕工作区裁切，排除底部任务栏与最大化边框外溢
pub fn clip_auto_window_rect(
    local_rc: RECT,
    mon_rects: &[RECT],
    work_rects: &[RECT],
) -> Option<RECT> {
    let rw = local_rc.right - local_rc.left;
    let rh = local_rc.bottom - local_rc.top;
    if rw <= 12 || rh <= 12 {
        return None;
    }
    if mon_rects.is_empty() || mon_rects.len() != work_rects.len() {
        return Some(local_rc);
    }

    let win_area = (rw as i64) * (rh as i64);
    let mut best_inter = 0i64;
    let mut best_work = RECT::default();
    let mut union_work = RECT::default();
    let mut has_union = false;

    for (&mon, &work) in mon_rects.iter().zip(work_rects.iter()) {
        let inter = intersect_rects(local_rc, mon);
        if inter.right <= inter.left || inter.bottom <= inter.top {
            continue;
        }
        let area = ((inter.right - inter.left) as i64) * ((inter.bottom - inter.top) as i64);
        if area > best_inter {
            best_inter = area;
            best_work = work;
        }
        if !has_union {
            union_work = work;
            has_union = true;
        } else {
            union_work.left = union_work.left.min(work.left);
            union_work.top = union_work.top.min(work.top);
            union_work.right = union_work.right.max(work.right);
            union_work.bottom = union_work.bottom.max(work.bottom);
        }
    }

    if best_inter <= 0 {
        return None;
    }

    let clipped = if (best_inter as i128) * 100 >= (win_area as i128) * 80 {
        intersect_rects(local_rc, best_work)
    } else {
        intersect_rects(local_rc, union_work)
    };
    if clipped.right - clipped.left > 12 && clipped.bottom - clipped.top > 12 {
        Some(clipped)
    } else {
        None
    }
}

/// 自动识别窗口候选（含顶层窗口句柄、顶层窗口矩形以及该窗口内部的子窗口、顶部工具栏、标题栏、客户区等）
#[derive(Clone, Debug, Default)]
pub struct WindowCandidate {
    pub hwnd: isize,
    pub top_rect: RECT,
    pub sub_rects: Vec<RECT>,
}

impl From<RECT> for WindowCandidate {
    fn from(top_rect: RECT) -> Self {
        Self {
            hwnd: 0,
            top_rect,
            sub_rects: Vec::new(),
        }
    }
}

/// 当前鼠标坐标下按面积从小到大排列的嵌套候选层级链
#[derive(Clone, Debug, Default)]
pub struct HoverHierarchy {
    pub win_hwnd: isize,
    pub levels: Vec<RECT>,
    pub whole_win_idx: usize,
}

/// 判断坐标是否位于窗口左、右、下外框边缘热区（保留顶部工具栏，靠左/右/下边时自然选中整窗）
pub fn is_on_window_frame_edge(whole_rc: RECT, x: i32, y: i32) -> bool {
    const EDGE_BAND: i32 = 10;
    let w = whole_rc.right - whole_rc.left;
    let h = whole_rc.bottom - whole_rc.top;
    if w <= EDGE_BAND * 4 || h <= EDGE_BAND * 4 {
        return false;
    }
    x <= whole_rc.left + EDGE_BAND
        || x >= whole_rc.right - EDGE_BAND
        || y >= whole_rc.bottom - EDGE_BAND
}

/// 收集指定坐标 (x, y) 处的嵌套识别层级：
/// 严格按 Z-order 仅锁定最前方命中的顶级窗口；在该窗口内收集由小到大的子分区与整窗矩形，
/// 末尾附加显示器工作区以便滚轮切换扩大选区。
pub fn collect_hover_hierarchy(
    candidates: &[WindowCandidate],
    work_rects: &[RECT],
    x: i32,
    y: i32,
) -> HoverHierarchy {
    let cur_work = work_rects.iter().copied().find(|&r| pt_in_rect(r, x, y));
    if !work_rects.is_empty() && cur_work.is_none() {
        return HoverHierarchy::default();
    }

    let clamp_work = |mut r: RECT| -> RECT {
        if let Some(wk) = cur_work {
            r.top = r.top.max(wk.top);
            r.bottom = r.bottom.min(wk.bottom);
        }
        r
    };

    for cand in candidates {
        if pt_in_rect(cand.top_rect, x, y) {
            let whole_rc = clamp_work(cand.top_rect);
            if whole_rc.right <= whole_rc.left || whole_rc.bottom <= whole_rc.top {
                continue;
            }
            let whole_area = ((whole_rc.right - whole_rc.left) as i64)
                * ((whole_rc.bottom - whole_rc.top) as i64);

            let mut sub_items: Vec<(i64, RECT)> = Vec::new();
            for &sub in &cand.sub_rects {
                if pt_in_rect(sub, x, y) {
                    let s = clamp_work(sub);
                    let w = s.right - s.left;
                    let h = s.bottom - s.top;
                    if w >= 24 && h >= 14 {
                        let area = (w as i64) * (h as i64);
                        if area < whole_area {
                            sub_items.push((area, s));
                        }
                    }
                }
            }
            sub_items.sort_by_key(|item| item.0);

            let mut levels: Vec<RECT> = Vec::new();
            for (_, s) in sub_items {
                let dup = levels.iter().any(|&ex| {
                    (ex.left - s.left).abs() <= 2
                        && (ex.top - s.top).abs() <= 2
                        && (ex.right - s.right).abs() <= 2
                        && (ex.bottom - s.bottom).abs() <= 2
                });
                if !dup {
                    levels.push(s);
                }
            }

            let whole_win_idx = levels.len();
            levels.push(whole_rc);

            if let Some(wk) = cur_work {
                let wk_area = ((wk.right - wk.left) as i64) * ((wk.bottom - wk.top) as i64);
                if wk_area > whole_area {
                    levels.push(wk);
                }
            }

            return HoverHierarchy {
                win_hwnd: cand.hwnd,
                levels,
                whole_win_idx,
            };
        }
    }

    if let Some(wk) = cur_work {
        HoverHierarchy {
            win_hwnd: 0,
            levels: vec![wk],
            whole_win_idx: 0,
        }
    } else {
        HoverHierarchy::default()
    }
}

/// 根据层级状态与修饰键刷新 Overlay 悬停窗口高亮框
fn update_overlay_hover_state(ov: &mut OverlayState, x: i32, y: i32, shift_down: bool) {
    let hier = collect_hover_hierarchy(&ov.win_rects, &ov.work_rects, x, y);
    if hier.levels.is_empty() {
        ov.hover_win_rc = RECT::default();
        ov.hover_win_hwnd = 0;
        ov.hover_depth = 0;
        ov.hover_whole_idx = 0;
        ov.hover_levels_len = 0;
        ov.hover_lock_whole = false;
        return;
    }

    if hier.win_hwnd != ov.hover_win_hwnd {
        ov.hover_win_hwnd = hier.win_hwnd;
        ov.hover_depth = 0;
        ov.hover_lock_whole = false;
    }

    let whole_rc = hier.levels[hier.whole_win_idx];
    let idx = if ov.hover_lock_whole {
        if ov.hover_depth > ov.hover_whole_idx {
            hier.levels.len() - 1
        } else {
            hier.whole_win_idx
        }
    } else if shift_down || is_on_window_frame_edge(whole_rc, x, y) {
        hier.whole_win_idx
    } else {
        ov.hover_depth.min(hier.levels.len() - 1)
    };

    ov.hover_depth = idx;
    ov.hover_whole_idx = hier.whole_win_idx;
    ov.hover_levels_len = hier.levels.len();
    ov.hover_win_rc = hier.levels[idx];
}

/// 通过滚轮步进 (delta != 0) 或 Tab 键 (toggle_whole) 切换子分区与整窗层级
fn step_overlay_hover_level(ov: &mut OverlayState, delta: isize, toggle_whole: bool) -> bool {
    if ov.has_sel {
        return false;
    }
    let hier = collect_hover_hierarchy(&ov.win_rects, &ov.work_rects, ov.mouse_pt.x, ov.mouse_pt.y);
    if hier.levels.len() <= 1 {
        return false;
    }
    let max_idx = hier.levels.len() - 1;
    let cur_idx = ov.hover_depth.min(max_idx);
    let next_idx = if toggle_whole {
        if cur_idx < hier.whole_win_idx {
            hier.whole_win_idx
        } else {
            0
        }
    } else if delta > 0 {
        (cur_idx + (delta as usize)).min(max_idx)
    } else if delta < 0 {
        cur_idx.saturating_sub((-delta) as usize)
    } else {
        cur_idx
    };

    ov.hover_win_hwnd = hier.win_hwnd;
    ov.hover_depth = next_idx;
    ov.hover_whole_idx = hier.whole_win_idx;
    ov.hover_levels_len = hier.levels.len();
    ov.hover_lock_whole = next_idx >= hier.whole_win_idx;
    ov.hover_win_rc = hier.levels[next_idx];
    next_idx != cur_idx
}

/// 按 Z-order 与所在显示器工作区选取鼠标悬停的自动识别框。
/// 优先命中当前最顶层应用；在该应用内进一步匹配更精准的子区域（如顶部工具栏、功能区面板、客户区），
/// 鼠标在空白边缘时自动回退为整窗大界面；鼠标在任务栏上不高亮，空白桌面回退所在显示器工作区。
pub fn pick_hover_window_rect(
    candidates: &[WindowCandidate],
    work_rects: &[RECT],
    x: i32,
    y: i32,
) -> RECT {
    let hier = collect_hover_hierarchy(candidates, work_rects, x, y);
    hier.levels.first().copied().unwrap_or_default()
}

/// 收集顶级窗口内部的子窗口以及推导出的顶部栏（标题栏/工具栏/标签栏）与客户区
unsafe fn collect_child_and_toolbar_rects(
    hwnd: HWND,
    top_rc: RECT,
    vx: i32,
    vy: i32,
) -> Vec<RECT> {
    let mut out = Vec::new();

    // 1. 通过 GetClientRect 和 ClientToScreen 计算顶部非客户区（标题栏、菜单栏、顶部工具条）与客户区
    let mut cl_rc = RECT::default();
    if GetClientRect(hwnd, &mut cl_rc) != 0 && cl_rc.right > 0 && cl_rc.bottom > 0 {
        let mut pt = POINT { x: 0, y: 0 };
        ClientToScreen(hwnd, &mut pt);
        let cl_screen_left = pt.x - vx;
        let cl_screen_top = pt.y - vy;
        let cl_screen_right = cl_screen_left + (cl_rc.right - cl_rc.left);
        let cl_screen_bottom = cl_screen_top + (cl_rc.bottom - cl_rc.top);
        let cl_local = RECT {
            left: cl_screen_left,
            top: cl_screen_top,
            right: cl_screen_right,
            bottom: cl_screen_bottom,
        };
        let client_inter = intersect_rects(top_rc, cl_local);

        // 如果客户区顶部高于顶级窗口顶部至少 16px，说明上方有标题栏/菜单栏/顶部工具条
        if client_inter.top - top_rc.top >= 16 && client_inter.top - top_rc.top <= 200 {
            let top_bar = RECT {
                left: top_rc.left,
                top: top_rc.top,
                right: top_rc.right,
                bottom: client_inter.top,
            };
            out.push(top_bar);
        }
        // 客户区自身也是有效候选区域（若尺寸合理）
        if client_inter.right - client_inter.left >= 32
            && client_inter.bottom - client_inter.top >= 24
        {
            out.push(client_inter);
        }
    }

    // 2. 枚举所有可见子窗口（包括工具栏 ToolbarWindow32、ReBarWindow32、各类子控件和子面板）
    struct ChildCtx {
        top_rc: RECT,
        vx: i32,
        vy: i32,
        sub_list: Vec<RECT>,
    }
    unsafe extern "system" fn child_cb(chwnd: HWND, lp: LPARAM) -> i32 {
        let ctx = &mut *(lp as *mut ChildCtx);
        if IsWindowVisible(chwnd) == 0 {
            return 1;
        }
        let ex_style = GetWindowLongPtrW(chwnd, GWL_EXSTYLE) as u32;
        if (ex_style & WS_EX_TRANSPARENT) != 0 {
            return 1;
        }
        let mut rc = RECT::default();
        if GetWindowRect(chwnd, &mut rc) == 0 {
            return 1;
        }
        let cw = rc.right - rc.left;
        let ch = rc.bottom - rc.top;
        if cw < 24 || ch < 14 {
            return 1;
        }
        let local = RECT {
            left: rc.left - ctx.vx,
            top: rc.top - ctx.vy,
            right: rc.right - ctx.vx,
            bottom: rc.bottom - ctx.vy,
        };
        let inter = intersect_rects(local, ctx.top_rc);
        let iw = inter.right - inter.left;
        let ih = inter.bottom - inter.top;
        if iw >= 24 && ih >= 14 {
            ctx.sub_list.push(inter);

            // 如果该子窗口顶部在顶级窗口下方 16~200px 处，且宽度占顶级窗口大部（>=70%）：
            // 典型为浏览器内容渲染窗口、IDE/代码编辑器视图。
            // 那么它上方的横条区域即为该软件的顶部标签栏/地址栏/工具栏区！
            let top_gap = inter.top - ctx.top_rc.top;
            let top_w = ctx.top_rc.right - ctx.top_rc.left;
            if top_gap >= 16 && top_gap <= 200 && iw * 10 >= top_w * 7 {
                let derived_top_bar = RECT {
                    left: ctx.top_rc.left,
                    top: ctx.top_rc.top,
                    right: ctx.top_rc.right,
                    bottom: inter.top,
                };
                ctx.sub_list.push(derived_top_bar);
            }
        }
        if ctx.sub_list.len() >= 64 {
            0
        } else {
            1
        }
    }

    let mut child_ctx = ChildCtx {
        top_rc,
        vx,
        vy,
        sub_list: Vec::new(),
    };
    EnumChildWindows(hwnd, child_cb, (&mut child_ctx as *mut ChildCtx) as LPARAM);
    out.extend(child_ctx.sub_list);

    // 3. 去重与规范化
    let mut cleaned = Vec::new();
    for r in out {
        let rw = r.right - r.left;
        let rh = r.bottom - r.top;
        let tw = top_rc.right - top_rc.left;
        let th = top_rc.bottom - top_rc.top;
        if (rw - tw).abs() <= 2 && (rh - th).abs() <= 2 {
            continue;
        }
        if rw < 24 || rh < 14 {
            continue;
        }
        let dup = cleaned.iter().any(|&c: &RECT| {
            (c.left - r.left).abs() <= 2
                && (c.top - r.top).abs() <= 2
                && (c.right - r.right).abs() <= 2
                && (c.bottom - r.bottom).abs() <= 2
        });
        if !dup {
            cleaned.push(r);
        }
    }
    if cleaned.len() > 40 {
        cleaned.truncate(40);
    }
    cleaned
}

unsafe fn enum_visible_windows(
    fg_root: HWND,
    vx: i32,
    vy: i32,
    mon_rects: &[RECT],
    work_rects: &[RECT],
    sub_window_enabled: bool,
) -> Vec<WindowCandidate> {
    // 1. 优先按 Windows 原生 Z-order 链获取顶层窗口列表 (从最顶到底)
    let mut z_hwnds = Vec::new();
    let mut cur = GetTopWindow(GetDesktopWindow());
    if cur.is_null() {
        cur = GetTopWindow(null_mut());
    }
    while !cur.is_null() {
        z_hwnds.push(cur);
        cur = GetWindow(cur, GW_HWNDNEXT);
    }

    // 2. 结合 EnumWindows 兜底补充可能未挂在直接桌面子链中的顶层窗口
    struct EnumCtx {
        seen: Vec<HWND>,
    }
    unsafe extern "system" fn append_enum_cb(hwnd: HWND, lp: LPARAM) -> i32 {
        let ctx = &mut *(lp as *mut EnumCtx);
        if !ctx.seen.contains(&hwnd) {
            ctx.seen.push(hwnd);
        }
        1
    }
    let mut enum_ctx = EnumCtx { seen: z_hwnds };
    EnumWindows(append_enum_cb, (&mut enum_ctx as *mut EnumCtx) as LPARAM);

    let own_pid = GetCurrentProcessId();
    let mut topmost_cands = Vec::new();
    let mut normal_cands = Vec::new();

    for hwnd in enum_ctx.seen {
        if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
            continue;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == own_pid {
            continue;
        }
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if (ex_style & WS_EX_TRANSPARENT) != 0 {
            continue;
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
            continue;
        }
        let mut cls_buf = [0u16; 128];
        let cls_len = GetClassNameW(hwnd, cls_buf.as_mut_ptr(), cls_buf.len() as i32);
        if cls_len > 0 {
            let cls = wide_to_utf8(&cls_buf[..cls_len as usize]);
            if should_skip_auto_window_class(&cls) {
                continue;
            }
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
            continue;
        }
        let local_rc = RECT {
            left: rc.left - vx,
            top: rc.top - vy,
            right: rc.right - vx,
            bottom: rc.bottom - vy,
        };
        if let Some(clipped) = clip_auto_window_rect(local_rc, mon_rects, work_rects) {
            let sub_rects = if sub_window_enabled {
                collect_child_and_toolbar_rects(hwnd, clipped, vx, vy)
            } else {
                Vec::new()
            };
            let cand = WindowCandidate {
                hwnd: hwnd as isize,
                top_rect: clipped,
                sub_rects,
            };
            if (ex_style & WS_EX_TOPMOST) != 0 {
                topmost_cands.push(cand);
            } else {
                normal_cands.push(cand);
            }
        }
    }

    // 3. 前台窗口优先提权：若截图瞬间的前台窗口处于常规窗口列表中，将其置于常规窗口最前列
    if !fg_root.is_null() {
        let fg_root_isize = fg_root as isize;
        if let Some(pos) = normal_cands.iter().position(|c| c.hwnd == fg_root_isize) {
            let fg_cand = normal_cands.remove(pos);
            normal_cands.insert(0, fg_cand);
        }
    }

    // 4. 组合最终物理 Z-order 候选列表：系统置顶窗口在前，常规窗口（前台窗口首位）在后
    let mut list = topmost_cands;
    list.extend(normal_cands);

    if !list.is_empty() {
        let top3: Vec<String> = list
            .iter()
            .take(4)
            .map(|c| {
                format!(
                    "0x{:x}({},{},{},{})",
                    c.hwnd, c.top_rect.left, c.top_rect.top, c.top_rect.right, c.top_rect.bottom
                )
            })
            .collect();
        log_write(
            "截屏",
            &format!("物理Z-order顶层窗口 [{}]: {}", list.len(), top3.join(" -> ")),
        );
    }

    list
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
        (BTN_PIN, 44, "贴图", "贴图到屏幕 (F3)"),
        (BTN_TRANS, 44, "翻译", "识别选区英文并翻译 (F2)"),
        (BTN_CANCEL, 44, "取消", "取消截屏 (Esc)"),
        (BTN_COPY, 52, "完成", "完成并复制到剪贴板 (Enter)"),
    ];
    let gap = 3i32;
    let total_w: i32 =
        specs.iter().map(|s| s.1).sum::<i32>() + gap * ((specs.len() as i32) - 1) + 20;
    let bar_h = 34;
    let has_sub = ov.cur_tool != AnnotTool::None;
    let extra_h = if has_sub { 38 } else { 0 };
    let mon_x = ((sel.left + sel.right) / 2).clamp(0, (ov.vw - 1).max(0));
    let mon_y = (sel.bottom - 1).clamp(sel.top.clamp(0, (ov.vh - 1).max(0)), (ov.vh - 1).max(0));
    let mon = monitor_bounds_for_point(&ov.mon_rects, ov.vw, ov.vh, mon_x, mon_y);
    let min_x = mon.left + 8;
    let max_x = (mon.right - total_w - 8).max(min_x);
    let bx = (sel.right - total_w).clamp(min_x, max_x);
    let mut by = sel.bottom + 8;
    if by + bar_h + extra_h > mon.bottom - 6 {
        by = (sel.top - bar_h - 8 - extra_h).max(mon.top + 6);
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
        let sub_max_x = (mon.right - sub_w - 8).max(min_x);
        let sx = bx.clamp(min_x, sub_max_x);
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

fn ensure_filter_layer(ov: &mut OverlayState, tool: AnnotTool, width: i32) -> Option<usize> {
    if !matches!(tool, AnnotTool::Mosaic | AnnotTool::Blur) {
        return None;
    }
    let roi = if ov.has_sel {
        norm_rect(
            ov.sel_rc.left,
            ov.sel_rc.top,
            ov.sel_rc.right,
            ov.sel_rc.bottom,
        )
    } else {
        RECT {
            left: 0,
            top: 0,
            right: ov.vw,
            bottom: ov.vh,
        }
    };
    if roi.right <= roi.left || roi.bottom <= roi.top {
        return None;
    }
    if let Some(idx) = ov
        .filter_caches
        .iter()
        .position(|c| c.tool == tool && c.width == width && rect_eq(c.roi, roi))
    {
        return Some(idx);
    }
    let (layer_rc, bgra) = match tool {
        AnnotTool::Mosaic => compute_mosaic_layer(
            &ov.orig_bgra,
            ov.vw,
            ov.vh,
            roi,
            mosaic_block_for_width(width),
        ),
        _ => compute_blur_layer(
            &ov.orig_bgra,
            ov.vw,
            ov.vh,
            roi,
            blur_radius_for_width(width),
        ),
    };
    if bgra.is_empty() {
        return None;
    }
    ov.filter_caches.push(FilterLayerCache {
        tool,
        width,
        roi: layer_rc,
        bgra,
    });
    Some(ov.filter_caches.len() - 1)
}

fn apply_brush_segment_to_work(
    ov: &mut OverlayState,
    tool: AnnotTool,
    width: i32,
    p0: (i32, i32),
    p1: (i32, i32),
) {
    let Some(idx) = ensure_filter_layer(ov, tool, width) else {
        return;
    };
    let sel = norm_rect(
        ov.sel_rc.left,
        ov.sel_rc.top,
        ov.sel_rc.right,
        ov.sel_rc.bottom,
    );
    let layer_rc = ov.filter_caches[idx].roi;
    let layer = &ov.filter_caches[idx].bgra;
    let vw = ov.vw;
    let vh = ov.vh;
    let r = brush_radius_for_width(width);
    if let Some(dst) = unsafe { ov.gdi.work_slice_mut() } {
        stamp_brush_capsule(dst, vw, vh, layer_rc, layer, p0, p1, r, sel);
    }
}

fn rebuild_work_bits(ov: &mut OverlayState) {
    struct PixelStroke {
        tool: AnnotTool,
        width: i32,
        rect: RECT,
        pts: Vec<(i32, i32)>,
    }

    unsafe { ov.gdi.copy_orig(&ov.orig_bgra) };
    let mut pixel_strokes: Vec<PixelStroke> = Vec::new();
    for it in &ov.annots {
        if matches!(it.tool, AnnotTool::Mosaic | AnnotTool::Blur) {
            pixel_strokes.push(PixelStroke {
                tool: it.tool,
                width: it.width,
                rect: norm_rect(it.x1, it.y1, it.x2, it.y2),
                pts: it.pts.clone(),
            });
        }
    }
    if let Some(d) = &ov.draft_annot {
        if matches!(d.tool, AnnotTool::Mosaic | AnnotTool::Blur) {
            pixel_strokes.push(PixelStroke {
                tool: d.tool,
                width: d.width,
                rect: norm_rect(d.x1, d.y1, d.x2, d.y2),
                pts: d.pts.clone(),
            });
        }
    }
    if pixel_strokes.is_empty() {
        return;
    }
    let sel = norm_rect(
        ov.sel_rc.left,
        ov.sel_rc.top,
        ov.sel_rc.right,
        ov.sel_rc.bottom,
    );
    let vw = ov.vw;
    let vh = ov.vh;
    for st in pixel_strokes {
        let Some(idx) = ensure_filter_layer(ov, st.tool, st.width) else {
            continue;
        };
        let layer_rc = ov.filter_caches[idx].roi;
        let layer = &ov.filter_caches[idx].bgra;
        let r = brush_radius_for_width(st.width);
        if let Some(dst) = unsafe { ov.gdi.work_slice_mut() } {
            if !st.pts.is_empty() {
                apply_brush_stroke_from_layer(dst, vw, vh, layer_rc, layer, &st.pts, r, sel);
            } else if st.tool == AnnotTool::Mosaic {
                apply_mosaic_to_bgra(dst, vw, vh, st.rect, mosaic_block_for_width(st.width));
            } else {
                apply_blur_to_bgra(dst, vw, vh, st.rect, blur_radius_for_width(st.width));
            }
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
    let Some(VirtualScreenCapture {
        vx,
        vy,
        vw,
        vh,
        bgra,
        mon_rects,
        work_rects,
    }) = capture_virtual_screen_with_monitors()
    else {
        return;
    };
    log_write(
        "截屏",
        &format!("虚拟桌面 ({vx},{vy}) {vw}x{vh} 屏幕数={}", mon_rects.len()),
    );
    unsafe { ensure_snip_classes() };
    let fg_hwnd = unsafe { GetForegroundWindow() };
    let fg_root = if !fg_hwnd.is_null() {
        unsafe { GetAncestor(fg_hwnd, GA_ROOT) }
    } else {
        null_mut()
    };
    let sub_win_enabled = SNIP_MGR.with(|m| m.borrow().cfg.sub_window);
    let win_rects = unsafe {
        enum_visible_windows(
            fg_root,
            vx,
            vy,
            &mon_rects,
            &work_rects,
            sub_win_enabled,
        )
    };

    let mut cur = POINT::default();
    unsafe { GetCursorPos(&mut cur) };
    let init_mouse = POINT {
        x: (cur.x - vx).clamp(0, (vw - 1).max(0)),
        y: (cur.y - vy).clamp(0, (vh - 1).max(0)),
    };
    let init_hover_win =
        pick_hover_window_rect(&win_rects, &work_rects, init_mouse.x, init_mouse.y);

    let mut gdi = OverlayGdiCache::default();
    if !unsafe { gdi.init(vw, vh, &bgra) } {
        return;
    }

    SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        mgr.overlay = OverlayState {
            active: true,
            trans_mode,
            vx,
            vy,
            vw,
            vh,
            mon_rects,
            work_rects,
            orig_bgra: bgra,
            gdi,
            filter_caches: Vec::new(),
            win_rects,
            hover_win_rc: init_hover_win,
            cur_tool: AnnotTool::None,
            cur_size_idx: 1,
            cur_color_idx: 0,
            next_num: 1,
            hover_btn: 0,
            sub_hover: 0,
            mouse_pt: init_mouse,
            ..Default::default()
        };
        update_overlay_hover_state(&mut mgr.overlay, init_mouse.x, init_mouse.y, false);
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
            SNIP_MGR.with(|m| m.borrow_mut().overlay = OverlayState::default());
            return;
        }
        SNIP_MGR.with(|m| m.borrow_mut().overlay.hwnd = hwnd);
        SetWindowPos(hwnd, HWND_TOPMOST, vx, vy, vw, vh, SWP_SHOWWINDOW);
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

/// 获取当前有效选区（优先已框选区域，其次悬停窗口矩形）
fn active_overlay_sel_rect(ov: &OverlayState) -> Option<RECT> {
    let rc = if ov.has_sel {
        norm_rect(ov.sel_rc.left, ov.sel_rc.top, ov.sel_rc.right, ov.sel_rc.bottom)
    } else if ov.hover_win_rc.right - ov.hover_win_rc.left >= 4
        && ov.hover_win_rc.bottom - ov.hover_win_rc.top >= 4
    {
        norm_rect(
            ov.hover_win_rc.left,
            ov.hover_win_rc.top,
            ov.hover_win_rc.right,
            ov.hover_win_rc.bottom,
        )
    } else {
        return None;
    };
    if rc.right - rc.left >= 4 && rc.bottom - rc.top >= 4 {
        Some(rc)
    } else {
        None
    }
}

/// 切换遮罩层标注工具并重排工具栏
fn toggle_overlay_tool(ov: &mut OverlayState, tool: AnnotTool) {
    ov.cur_tool = if ov.cur_tool == tool {
        AnnotTool::None
    } else {
        tool
    };
    layout_overlay_toolbar(ov);
}

/// 数字键（含小键盘）转标注工具
fn hotkey_to_annot_tool(vk: i32) -> AnnotTool {
    match vk {
        0x31 | 0x61 => AnnotTool::Rect,
        0x32 | 0x62 => AnnotTool::Ellipse,
        0x33 | 0x63 => AnnotTool::Line,
        0x34 | 0x64 => AnnotTool::Arrow,
        0x35 | 0x65 => AnnotTool::Pen,
        0x36 | 0x66 => AnnotTool::Marker,
        0x37 | 0x67 => AnnotTool::Number,
        0x38 | 0x68 => AnnotTool::Text,
        0x39 | 0x69 => AnnotTool::Mosaic,
        0x30 | 0x60 => AnnotTool::Blur,
        _ => AnnotTool::None,
    }
}

/// 通知主窗口在提示栏临时闪烁显示操作反馈
fn notify_main_hint(hwnd_main: isize, msg: &str) {
    if hwnd_main != 0 {
        let ptr = Box::into_raw(Box::new(msg.to_string()));
        unsafe { PostMessageW(hwnd_main as HWND, WM_APP_FLASH_HINT, 0, ptr as LPARAM) };
    }
}

fn notify_mgr_hint(msg: &str) {
    let hwnd_main = SNIP_MGR.with(|m| m.borrow().cfg.hwnd_main);
    notify_main_hint(hwnd_main, msg);
}

fn finish_selection_action(action: i32) {
    unsafe { take_and_commit_inline_text_edit() };
    let (cw, ch, sx, sy, flat_bgra, cfg) = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let Some(sel) = active_overlay_sel_rect(&mgr.overlay) else {
            return (0, 0, 0, 0, Vec::new(), mgr.cfg.clone());
        };
        let sx = mgr.overlay.vx + sel.left;
        let sy = mgr.overlay.vy + sel.top;
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
        (cw, ch, sx, sy, flat, mgr.cfg.clone())
    });

    close_overlay();
    if cw <= 0 || ch <= 0 || flat_bgra.is_empty() {
        return;
    }

    match action {
        BTN_COPY => {
            if set_clipboard_bgra(&flat_bgra, cw, ch) {
                notify_main_hint(cfg.hwnd_main, "（已复制截图）");
            }
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
                if save_bgra_as_png(&p, &flat_bgra, cw, ch) {
                    if quick_save {
                        let fname = PathBuf::from(&p)
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or(now);
                        notify_main_hint(cfg.hwnd_main, &format!("（已保存：{}）", fname));
                    } else {
                        notify_main_hint(cfg.hwnd_main, "（截图已保存）");
                    }
                } else {
                    notify_main_hint(cfg.hwnd_main, "（截图保存失败）");
                }
            }
        }
        BTN_PIN => {
            create_pin_window_bgra_at(Some((sx, sy)), cw, ch, flat_bgra, String::new(), String::new());
        }
        BTN_TRANS => {
            // 先把原图摆出来，OCR 结果回来再原地替换：这就是「在截出来的图片里翻译」
            open_result_window(cw, ch, Vec::new(), flat_bgra.clone(), String::new(), String::new());
            let target = SNIP_MGR.with(|m| m.borrow().result.hwnd);
            if !target.is_null() {
                start_result_translate(target, cw, ch, flat_bgra, cfg);
            }
        }
        _ => {}
    }
}

struct SnipDonePayload {
    res: SnipResult,
    orig_bgra: Vec<u8>,
}

/// 对一张图发起 OCR 翻译；`target` 非空表示结果回填该结果窗（否则新建结果窗）
fn spawn_snip_translate(w: i32, h: i32, bgra: Vec<u8>, cfg: SnipCfg, target: HWND) {
    let hwnd_main = cfg.hwnd_main;
    // HWND 是裸指针，跨线程只传数值，线程内再还原
    let target_raw = target as usize;
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
            unsafe {
                PostMessageW(
                    hwnd_main as HWND,
                    WM_APP_SNIP_DONE,
                    target_raw,
                    payload as LPARAM,
                )
            };
        }
    });
}

/// 由结果窗发起的翻译（F2 / 首次打开时自动触发）
fn start_result_translate(target: HWND, w: i32, h: i32, orig: Vec<u8>, cfg: SnipCfg) {
    SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        mgr.result.translating = true;
    });
    unsafe { InvalidateRect(target, null(), 0) };
    spawn_snip_translate(w, h, orig, cfg, target);
}

/// OCR/翻译失败文案归类：一律走浮层提示，绝不弹模态框
fn classify_snip_error(err: &str) -> (Tone, String) {
    let e = err.trim();
    if e.contains("未识别到文本") || e.contains("未识别到文字") {
        (Tone::Warn, "未识别到文字".to_string())
    } else if e.contains("未检测到需要翻译的英文") || e.contains("英文") {
        (Tone::Warn, "未识别到英文，无需翻译".to_string())
    } else {
        let brief: String = e.chars().take(40).collect();
        (Tone::Error, format!("翻译失败：{brief}"))
    }
}

/// 主线程收到截屏 OCR 结果消息时调用
///
/// `wp` = 目标结果窗句柄（0 表示新建结果窗）
pub fn handle_snip_done_message(wp: WPARAM, lp: LPARAM) {
    if lp == 0 {
        return;
    }
    let payload = unsafe { Box::from_raw(lp as *mut SnipDonePayload) };
    let target = wp as HWND;
    let target_alive = !target.is_null() && unsafe { IsWindow(target) } != 0;

    if !payload.res.ok {
        log_write("截屏OCR", &payload.res.err);
        let (tone, msg) = classify_snip_error(&payload.res.err);
        let anchor = if target_alive {
            target
        } else {
            SNIP_MGR.with(|m| m.borrow().cfg.hwnd_main as HWND)
        };
        if target_alive {
            SNIP_MGR.with(|m| {
                m.borrow_mut().result.translating = false;
            });
            unsafe { InvalidateRect(target, null(), 0) };
        }
        if !anchor.is_null() {
            crate::ui::snip_toast::show_toast(anchor, &msg, tone);
        }
        return;
    }

    let SnipResult {
        width,
        height,
        bgra_out,
        zh_text,
        en_text,
        lines,
        blocks,
        ..
    } = payload.res;

    if target_alive && SNIP_MGR.with(|m| m.borrow().result.hwnd) == target {
        SNIP_MGR.with(|m| {
            let mut mgr = m.borrow_mut();
            let r = &mut mgr.result;
            r.bgra_zh = bgra_out;
            r.zh_text = zh_text;
            r.en_text = en_text;
            r.lines = lines;
            r.blocks = blocks;
            r.show_orig = false;
            r.translating = false;
        });
        unsafe { InvalidateRect(target, null(), 0) };
        return;
    }

    open_result_window(
        width,
        height,
        bgra_out,
        payload.orig_bgra,
        zh_text,
        en_text,
    );
    SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        mgr.result.lines = lines;
        mgr.result.blocks = blocks;
        mgr.result.translating = false;
    });
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

unsafe fn draw_overlay_magnifier(
    hdc_back: HDC,
    hdc_orig: HDC,
    ov: &OverlayState,
    sel: RECT,
    hf_mag: HFONT,
) {
    let mx = ov.mouse_pt.x;
    let my = ov.mouse_pt.y;
    if mx < 0 || my < 0 || mx >= ov.vw || my >= ov.vh {
        return;
    }
    const BOX_W: i32 = 132;
    const MAG_H: i32 = 96;
    const INFO_H: i32 = 76;
    const BOX_H: i32 = MAG_H + INFO_H;

    let mon = monitor_bounds_for_point(&ov.mon_rects, ov.vw, ov.vh, mx, my);
    let mut bx = mx + 18;
    let mut by = my + 22;
    if bx + BOX_W > mon.right - 6 {
        bx = mx - BOX_W - 18;
    }
    if by + BOX_H > mon.bottom - 6 {
        by = my - BOX_H - 18;
    }
    bx = bx.clamp(mon.left + 4, (mon.right - BOX_W - 4).max(mon.left + 4));
    by = by.clamp(mon.top + 4, (mon.bottom - BOX_H - 4).max(mon.top + 4));

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

    let own_hf = if hf_mag.is_null() {
        let wf = utf8_to_wide("Microsoft YaHei UI");
        CreateFontW(
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
        )
    } else {
        null_mut()
    };
    let use_hf = if hf_mag.is_null() { own_hf } else { hf_mag };
    let old_f = SelectObject(hdc_back, use_hf);
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
    let hint_line = if !ov.has_sel && ov.hover_levels_len > 1 {
        if ov.hover_depth < ov.hover_whole_idx {
            "\n滚轮/Tab 选整窗"
        } else {
            "\n滚轮/Tab 选分区"
        }
    } else {
        ""
    };
    let wt2 = utf8_to_wide(&format!("RGB({r_v},{g_v},{b_v})\n按 C 复制色值{hint_line}"));
    DrawTextW(
        hdc_back,
        wt2.as_ptr(),
        -1,
        &mut r2,
        DT_LEFT | DT_TOP | DT_NOPREFIX,
    );
    SelectObject(hdc_back, old_f);
    if !own_hf.is_null() {
        DeleteObject(own_hf);
    }

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
    SNIP_MGR.with(|m| {
        let Ok(mgr) = m.try_borrow() else {
            return;
        };
        let ov = &mgr.overlay;
        let gdi = &ov.gdi;
        if ov.vw <= 0 || ov.vh <= 0 || gdi.hdc_back.is_null() {
            return;
        }
        let hdc_back = gdi.hdc_back;

        // 1. 极速复制预暗化全屏底图（零 AlphaBlend / 零每帧位图分配）
        BitBlt(hdc_back, 0, 0, ov.vw, ov.vh, gdi.hdc_dim, 0, 0, SRCCOPY);

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

        let show_brush_ring = ov.has_sel
            && !ov.dragging_sel
            && matches!(ov.cur_tool, AnnotTool::Mosaic | AnnotTool::Blur)
            && pt_in_rect(sel, ov.mouse_pt.x, ov.mouse_pt.y)
            && !pt_in_rect(ov.bar_rc, ov.mouse_pt.x, ov.mouse_pt.y)
            && (ov.cur_tool == AnnotTool::None
                || !pt_in_rect(ov.sub_bar_rc, ov.mouse_pt.x, ov.mouse_pt.y));

        // 2. 将选区（含已涂抹工作底图）BitBlt 到高亮区，并裁剪绘制矢量标注与画笔预览圈
        if sel.right > sel.left && sel.bottom > sel.top {
            BitBlt(
                hdc_back,
                sel.left,
                sel.top,
                sel.right - sel.left,
                sel.bottom - sel.top,
                gdi.hdc_work,
                sel.left,
                sel.top,
                SRCCOPY,
            );

            let has_vectors = ov
                .annots
                .iter()
                .any(|it| !matches!(it.tool, AnnotTool::Mosaic | AnnotTool::Blur))
                || ov
                    .draft_annot
                    .as_ref()
                    .is_some_and(|d| !matches!(d.tool, AnnotTool::Mosaic | AnnotTool::Blur));

            if has_vectors || show_brush_ring {
                let hrgn = CreateRectRgn(sel.left, sel.top, sel.right, sel.bottom);
                SelectClipRgn(hdc_back, hrgn);
                if has_vectors {
                    draw_vector_annots_with_draft(
                        hdc_back,
                        &ov.annots,
                        ov.draft_annot.as_ref(),
                        0,
                        0,
                    );
                }
                if show_brush_ring {
                    ensure_gdiplus();
                    let mut g_br: *mut c_void = null_mut();
                    if GdipCreateFromHDC(hdc_back, &mut g_br) == GP_OK && !g_br.is_null() {
                        GdipSetSmoothingMode(g_br, GP_SMOOTHING_ANTIALIAS);
                        let r = brush_radius_for_width(SIZE_WIDTHS[ov.cur_size_idx]) as f32;
                        let bx = (ov.mouse_pt.x as f32) - r;
                        let by = (ov.mouse_pt.y as f32) - r;
                        let d = r * 2.0;
                        let mut p_outer: *mut c_void = null_mut();
                        if GdipCreatePen1(argb(20, 24, 32, 200), 2.2, GP_UNIT_PIXEL, &mut p_outer)
                            == GP_OK
                            && !p_outer.is_null()
                        {
                            GdipDrawEllipse(g_br, p_outer, bx, by, d, d);
                            GdipDeletePen(p_outer);
                        }
                        let mut p_inner: *mut c_void = null_mut();
                        if GdipCreatePen1(
                            argb(255, 255, 255, 235),
                            1.2,
                            GP_UNIT_PIXEL,
                            &mut p_inner,
                        ) == GP_OK
                            && !p_inner.is_null()
                        {
                            GdipDrawEllipse(g_br, p_inner, bx, by, d, d);
                            GdipDeletePen(p_inner);
                        }
                        GdipDeleteGraphics(g_br);
                    }
                }
                SelectClipRgn(hdc_back, null_mut());
                if !hrgn.is_null() {
                    DeleteObject(hrgn);
                }
            }

            // 3. 选区亮蓝边界与 8 个调节手柄 + 尺寸角标
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

            // 左上角尺寸提示徽标（按当前所在显示器边界夹紧）
            let sz_txt = format!("{} × {}", sel.right - sel.left, sel.bottom - sel.top);
            let badge_mon = monitor_bounds_for_point(
                &ov.mon_rects,
                ov.vw,
                ov.vh,
                sel.left.clamp(0, (ov.vw - 1).max(0)),
                sel.top.clamp(0, (ov.vh - 1).max(0)),
            );
            let tag_y = if sel.top - badge_mon.top >= 26 {
                sel.top - 24
            } else {
                sel.top + 4
            }
            .clamp(
                badge_mon.top + 4,
                (badge_mon.bottom - 24).max(badge_mon.top + 4),
            );
            let tag_x = sel.left.clamp(
                badge_mon.left + 4,
                (badge_mon.right - 110).max(badge_mon.left + 4),
            );
            let mut tr = RECT {
                left: tag_x,
                top: tag_y,
                right: (tag_x + 104).min(badge_mon.right - 4),
                bottom: tag_y + 20,
            };
            let br_t = CreateSolidBrush(rgb(20, 24, 32));
            FillRect(hdc_back, &tr, br_t);
            DeleteObject(br_t);
            let old_f = if !gdi.hf_badge.is_null() {
                SelectObject(hdc_back, gdi.hf_badge)
            } else {
                null_mut()
            };
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
            if !old_f.is_null() {
                SelectObject(hdc_back, old_f);
            }
        }

        // 4. 选区完成且非翻译极速模式下绘制悬浮工具栏与子栏
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

            let old_f = if !gdi.hf_btn.is_null() {
                SelectObject(hdc_back, gdi.hf_btn)
            } else {
                null_mut()
            };
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
                            let rp = CreatePen(
                                PS_SOLID,
                                if ov.cur_color_idx == i { 2 } else { 1 },
                                ring,
                            );
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
                    let wh = utf8_to_wide("笔刷粗细 (左选圆点)");
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
                let tip_mon = monitor_bounds_for_point(
                    &ov.mon_rects,
                    ov.vw,
                    ov.vh,
                    ((hover_rc.left + hover_rc.right) / 2).clamp(0, (ov.vw - 1).max(0)),
                    hover_rc.top.clamp(0, (ov.vh - 1).max(0)),
                );
                let tx = ((hover_rc.left + hover_rc.right - tw) / 2).clamp(
                    tip_mon.left + 4,
                    (tip_mon.right - tw - 4).max(tip_mon.left + 4),
                );
                let ty = if ov.bar_rc.top - tip_mon.top >= 26 {
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
            if !old_f.is_null() {
                SelectObject(hdc_back, old_f);
            }
        }

        // 5. 未确定选区或正在调整选区大小时显示 12x 放大镜取色器
        if !ov.has_sel || ov.dragging_sel {
            draw_overlay_magnifier(hdc_back, gdi.hdc_work, ov, sel, gdi.hf_mag);
        }

        BitBlt(hdc, 0, 0, ov.vw, ov.vh, hdc_back, 0, 0, SRCCOPY);
    });
    EndPaint(hwnd, &ps);
}

fn hit_test_handles(sel: RECT, x: i32, y: i32) -> i32 {
    let l = sel.left;
    let r = sel.right;
    let t = sel.top;
    let b = sel.bottom;

    const CORNER_TOL: i32 = 8;
    let border_tol = SNAP_DIST;

    // 1. 优先判定四个角点（10=TL, 12=TR, 14=BR, 16=BL）
    if (x - l).abs() <= CORNER_TOL && (y - t).abs() <= CORNER_TOL {
        return 10;
    }
    if (x - r).abs() <= CORNER_TOL && (y - t).abs() <= CORNER_TOL {
        return 12;
    }
    if (x - r).abs() <= CORNER_TOL && (y - b).abs() <= CORNER_TOL {
        return 14;
    }
    if (x - l).abs() <= CORNER_TOL && (y - b).abs() <= CORNER_TOL {
        return 16;
    }

    // 2. 判定四条边（11=T, 13=R, 15=B, 17=L，支持沿整个边界拖动调整尺寸）
    if (y - t).abs() <= border_tol && x >= l - border_tol && x <= r + border_tol {
        return 11;
    }
    if (y - b).abs() <= border_tol && x >= l - border_tol && x <= r + border_tol {
        return 15;
    }
    if (x - l).abs() <= border_tol && y >= t - border_tol && y <= b + border_tol {
        return 17;
    }
    if (x - r).abs() <= border_tol && y >= t - border_tol && y <= b + border_tol {
        return 13;
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
                        _ if pt_in_rect(ov.sel_rc, x, y) => {
                            if ov.cur_tool == AnnotTool::None {
                                IDC_ARROW
                            } else if ov.cur_tool == AnnotTool::Text {
                                IDC_IBEAM
                            } else {
                                IDC_CROSS
                            }
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
            let need_repaint = SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                let ov = &mut mgr.overlay;
                let prev_pt = ov.mouse_pt;
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
                    return true;
                }

                if ov.drawing_annot {
                    let sel = norm_rect(
                        ov.sel_rc.left,
                        ov.sel_rc.top,
                        ov.sel_rc.right,
                        ov.sel_rc.bottom,
                    );
                    struct StampSeg {
                        tool: AnnotTool,
                        width: i32,
                        p0: (i32, i32),
                        p1: (i32, i32),
                    }
                    let mut stamp_info: Option<StampSeg> = None;
                    let mut repaint = false;
                    if let Some(d) = ov.draft_annot.as_mut() {
                        match d.tool {
                            AnnotTool::Mosaic | AnnotTool::Blur => {
                                let cx = x.clamp(sel.left, (sel.right - 1).max(sel.left));
                                let cy = y.clamp(sel.top, (sel.bottom - 1).max(sel.top));
                                d.x2 = cx;
                                d.y2 = cy;
                                if let Some(&last) = d.pts.last() {
                                    let dx = cx - last.0;
                                    let dy = cy - last.1;
                                    if dx * dx + dy * dy >= 4 {
                                        d.pts.push((cx, cy));
                                        stamp_info = Some(StampSeg {
                                            tool: d.tool,
                                            width: d.width,
                                            p0: last,
                                            p1: (cx, cy),
                                        });
                                    }
                                } else {
                                    d.pts.push((cx, cy));
                                    stamp_info = Some(StampSeg {
                                        tool: d.tool,
                                        width: d.width,
                                        p0: (cx, cy),
                                        p1: (cx, cy),
                                    });
                                }
                                repaint = true;
                            }
                            AnnotTool::Pen | AnnotTool::Marker => {
                                d.x2 = x;
                                d.y2 = y;
                                let push_pt = d.pts.last().is_none_or(|&(lx, ly)| {
                                    let dx = x - lx;
                                    let dy = y - ly;
                                    dx * dx + dy * dy >= 4
                                });
                                if push_pt {
                                    d.pts.push((x, y));
                                    repaint = true;
                                }
                            }
                            _ => {
                                if d.x2 != x || d.y2 != y {
                                    d.x2 = x;
                                    d.y2 = y;
                                    repaint = true;
                                }
                            }
                        }
                    }
                    if let Some(seg) = stamp_info {
                        apply_brush_segment_to_work(ov, seg.tool, seg.width, seg.p0, seg.p1);
                    }
                    return repaint;
                }

                if !ov.has_sel {
                    let shift_down = (GetAsyncKeyState(VK_SHIFT) as u16 & 0x8000) != 0;
                    update_overlay_hover_state(ov, x, y, shift_down);
                    prev_pt.x != x || prev_pt.y != y
                } else {
                    let prev_hover_btn = ov.hover_btn;
                    let prev_sub_hover = ov.sub_hover;
                    ov.hover_btn = 0;
                    for b in &ov.btns {
                        if pt_in_rect(b.rect, x, y) {
                            ov.hover_btn = b.id;
                            break;
                        }
                    }
                    ov.sub_hover = 0;
                    if ov.cur_tool != AnnotTool::None {
                        for (i, &r) in ov.size_rcs.iter().enumerate() {
                            if pt_in_rect(r, x, y) {
                                ov.sub_hover = SUB_SIZE_0 + (i as i32);
                            }
                        }
                        if matches!(ov.cur_tool, AnnotTool::Rect | AnnotTool::Ellipse)
                            && pt_in_rect(ov.fill_rc, x, y)
                        {
                            ov.sub_hover = SUB_FILL;
                        }
                        if !matches!(ov.cur_tool, AnnotTool::Mosaic | AnnotTool::Blur) {
                            for (i, &r) in ov.color_rcs.iter().enumerate() {
                                if pt_in_rect(r, x, y) {
                                    ov.sub_hover = SUB_COLOR_BASE + (i as i32);
                                }
                            }
                        }
                    }
                    let hover_changed =
                        ov.hover_btn != prev_hover_btn || ov.sub_hover != prev_sub_hover;
                    let brush_ring_move =
                        matches!(ov.cur_tool, AnnotTool::Mosaic | AnnotTool::Blur)
                            && (pt_in_rect(ov.sel_rc, prev_pt.x, prev_pt.y)
                                || pt_in_rect(ov.sel_rc, x, y));
                    hover_changed || brush_ring_move
                }
            });
            if need_repaint {
                InvalidateRect(hwnd, null(), 0);
            }
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
                        if pt_in_rect(b.rect, x, y) {
                            return Some(b.id);
                        }
                    }
                    if ov.cur_tool != AnnotTool::None && pt_in_rect(ov.sub_bar_rc, x, y) {
                        for (i, &r) in ov.size_rcs.iter().enumerate() {
                            if pt_in_rect(r, x, y) {
                                ov.cur_size_idx = i;
                                return Some(-1);
                            }
                        }
                        if matches!(ov.cur_tool, AnnotTool::Rect | AnnotTool::Ellipse)
                            && pt_in_rect(ov.fill_rc, x, y)
                        {
                            ov.cur_filled = !ov.cur_filled;
                            return Some(-1);
                        }
                        if !matches!(ov.cur_tool, AnnotTool::Mosaic | AnnotTool::Blur) {
                            for (i, &r) in ov.color_rcs.iter().enumerate() {
                                if pt_in_rect(r, x, y) {
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
                    } else if pt_in_rect(ov.sel_rc, x, y) {
                        if ov.cur_tool == AnnotTool::None {
                            // 选区内部：不可以拖动整个选框，拦截事件，不启动移动拖拽，也不触发重新新建选区
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
                            let tool = ov.cur_tool;
                            ov.drawing_annot = true;
                            ov.draft_annot = Some(AnnotItem {
                                tool,
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
                            if matches!(tool, AnnotTool::Mosaic | AnnotTool::Blur) {
                                apply_brush_segment_to_work(ov, tool, w, (x, y), (x, y));
                            }
                        }
                    } else {
                        ov.dragging_sel = true;
                        ov.drag_mode = 1;
                        ov.drag_start_pt = POINT { x, y };
                        ov.sel_rc = norm_rect(x, y, x, y);
                        ov.cur_tool = AnnotTool::None;
                        ov.annots.clear();
                        ov.redo_stack.clear();
                        ov.filter_caches.clear();
                        rebuild_work_bits(ov);
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
                                let is_pixel =
                                    matches!(a.tool, AnnotTool::Mosaic | AnnotTool::Blur);
                                ov.redo_stack.push(a);
                                if is_pixel {
                                    rebuild_work_bits(ov);
                                }
                            }
                        });
                    }
                    BTN_REDO => {
                        SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            if let Some(a) = ov.redo_stack.pop() {
                                let is_pixel =
                                    matches!(a.tool, AnnotTool::Mosaic | AnnotTool::Blur);
                                ov.annots.push(a);
                                if is_pixel {
                                    rebuild_work_bits(ov);
                                }
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
                    let old_mode = ov.drag_mode;
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
                    if old_mode != 1 {
                        ov.filter_caches.clear();
                        if ov
                            .annots
                            .iter()
                            .any(|a| matches!(a.tool, AnnotTool::Mosaic | AnnotTool::Blur))
                        {
                            rebuild_work_bits(ov);
                        }
                    }
                } else if ov.drawing_annot {
                    ov.drawing_annot = false;
                    if let Some(d) = ov.draft_annot.take() {
                        let is_pixel = matches!(d.tool, AnnotTool::Mosaic | AnnotTool::Blur);
                        let valid = if is_pixel {
                            !d.pts.is_empty()
                        } else {
                            (d.x2 - d.x1).abs() > 2 || (d.y2 - d.y1).abs() > 2 || d.pts.len() > 2
                        };
                        if valid {
                            ov.annots.push(d);
                            ov.redo_stack.clear();
                        } else if is_pixel {
                            rebuild_work_bits(ov);
                        }
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
            take_and_commit_inline_text_edit();
            let act = SNIP_MGR.with(|m| {
                let ov = &mut m.borrow_mut().overlay;
                if ov.has_sel && ov.cur_tool != AnnotTool::None {
                    ov.cur_tool = AnnotTool::None;
                    layout_overlay_toolbar(ov);
                    1
                } else if ov.has_sel {
                    ov.has_sel = false;
                    ov.cur_tool = AnnotTool::None;
                    ov.annots.clear();
                    ov.redo_stack.clear();
                    ov.filter_caches.clear();
                    rebuild_work_bits(ov);
                    2
                } else {
                    0
                }
            });
            if act != 0 {
                InvalidateRect(hwnd, null(), 0);
            } else {
                close_overlay();
            }
            0
        }
        WM_KEYDOWN => {
            const VK_MENU_KEY: i32 = 0x12;
            const VK_SPACE_KEY: i32 = 0x20;
            const VK_LEFT_KEY: i32 = 0x25;
            const VK_UP_KEY: i32 = 0x26;
            const VK_RIGHT_KEY: i32 = 0x27;
            const VK_DOWN_KEY: i32 = 0x28;
            const VK_F3_KEY: i32 = 0x72;

            let vk = (wp & 0xFF) as i32;
            let ctrl = (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0;
            let shift = (GetAsyncKeyState(VK_SHIFT) as u16 & 0x8000) != 0;
            let alt = (GetAsyncKeyState(VK_MENU_KEY) as u16 & 0x8000) != 0;
            let cur_mods = (if ctrl { 1 } else { 0 })
                | (if shift { 2 } else { 0 })
                | (if alt { 4 } else { 0 });

            if vk == VK_ESCAPE {
                // 逐层退出：结束文字输入 → 退出当前标注工具 → 关闭遮罩
                let ed = SNIP_MGR.with(|m| {
                    let ov = &mut m.borrow_mut().overlay;
                    let old = ov.hwnd_text_edit;
                    ov.hwnd_text_edit = null_mut();
                    old
                });
                if !ed.is_null() {
                    DestroyWindow(ed);
                    SetFocus(hwnd);
                    InvalidateRect(hwnd, null(), 0);
                    return 0;
                }
                let had_tool = SNIP_MGR.with(|m| {
                    let ov = &mut m.borrow_mut().overlay;
                    let had = ov.cur_tool != AnnotTool::None;
                    if had {
                        ov.cur_tool = AnnotTool::None;
                        layout_overlay_toolbar(ov);
                    }
                    had
                });
                if had_tool {
                    InvalidateRect(hwnd, null(), 0);
                } else {
                    close_overlay();
                }
                return 0;
            }
            if vk == 'C' as i32 && !ctrl && !alt {
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
                    notify_mgr_hint(&format!("（已复制颜色：{hex}）"));
                }
                close_overlay();
                return 0;
            }

            let (has_active_sel, has_drag_sel, pin_vk, pin_mods) = SNIP_MGR.with(|m| {
                let mgr = m.borrow();
                (
                    active_overlay_sel_rect(&mgr.overlay).is_some(),
                    mgr.overlay.has_sel,
                    mgr.cfg.pin_vk,
                    mgr.cfg.pin_mods,
                )
            });
            if !has_active_sel {
                return 0;
            }

            if (pin_vk != 0 && vk == pin_vk && cur_mods == pin_mods)
                || (vk == VK_F3_KEY && cur_mods == 0)
            {
                finish_selection_action(BTN_PIN);
                return 0;
            }
            if ctrl {
                if vk == 'A' as i32 {
                    // Snipaste 习惯：Ctrl+A = 选中整个屏幕
                    SNIP_MGR.with(|m| {
                        let ov = &mut m.borrow_mut().overlay;
                        ov.sel_rc = RECT {
                            left: 0,
                            top: 0,
                            right: ov.vw,
                            bottom: ov.vh,
                        };
                        ov.has_sel = true;
                        layout_overlay_toolbar(ov);
                    });
                    InvalidateRect(hwnd, null(), 0);
                    return 0;
                }
                if has_drag_sel {
                    let kt = hotkey_to_annot_tool(vk);
                    if kt != AnnotTool::None {
                        SNIP_MGR.with(|m| toggle_overlay_tool(&mut m.borrow_mut().overlay, kt));
                        InvalidateRect(hwnd, null(), 0);
                        return 0;
                    }
                    if vk == 'Z' as i32 {
                        SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            let item = if shift {
                                ov.redo_stack.pop()
                            } else {
                                ov.annots.pop()
                            };
                            if let Some(a) = item {
                                let is_pixel =
                                    matches!(a.tool, AnnotTool::Mosaic | AnnotTool::Blur);
                                if shift {
                                    ov.annots.push(a);
                                } else {
                                    ov.redo_stack.push(a);
                                }
                                if is_pixel {
                                    rebuild_work_bits(ov);
                                }
                            }
                        });
                        InvalidateRect(hwnd, null(), 0);
                        return 0;
                    }
                    if vk == 'Y' as i32 {
                        SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            if let Some(a) = ov.redo_stack.pop() {
                                let is_pixel =
                                    matches!(a.tool, AnnotTool::Mosaic | AnnotTool::Blur);
                                ov.annots.push(a);
                                if is_pixel {
                                    rebuild_work_bits(ov);
                                }
                            }
                        });
                        InvalidateRect(hwnd, null(), 0);
                        return 0;
                    }
                }
                if vk == 'C' as i32 {
                    finish_selection_action(BTN_COPY);
                    return 0;
                }
                if vk == 'S' as i32 {
                    finish_selection_action(BTN_SAVE);
                    return 0;
                }
            }

            if !ctrl && !alt {
                // 数字键直切标注工具（Snipaste 式：不用按修饰键）
                if has_drag_sel {
                    let kt = hotkey_to_annot_tool(vk);
                    if kt != AnnotTool::None {
                        SNIP_MGR.with(|m| toggle_overlay_tool(&mut m.borrow_mut().overlay, kt));
                        InvalidateRect(hwnd, null(), 0);
                        return 0;
                    }
                }
                match vk {
                    VK_RETURN | VK_SPACE_KEY => {
                        finish_selection_action(BTN_COPY);
                        return 0;
                    }
                    v if v == 'T' as i32 || v == 0x71 => {
                        // F2 为主键；T 保留一版兼容
                        finish_selection_action(BTN_TRANS);
                        return 0;
                    }
                    VK_LEFT_KEY | VK_RIGHT_KEY | VK_UP_KEY | VK_DOWN_KEY => {
                        // 方向键移动 1px，Shift 加速到 10px，Alt 改成调整选区边界
                        let step = if shift { 10 } else { 1 };
                        let resize = alt;
                        let moved = SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            if !ov.has_sel || ov.cur_tool != AnnotTool::None {
                                return false;
                            }
                            if resize {
                                match vk {
                                    VK_LEFT_KEY if ov.sel_rc.right - step > ov.sel_rc.left + 4 => {
                                        ov.sel_rc.left += step;
                                    }
                                    VK_RIGHT_KEY if ov.sel_rc.left + step < ov.sel_rc.right - 4 => {
                                        ov.sel_rc.right -= step;
                                    }
                                    VK_UP_KEY if ov.sel_rc.bottom - step > ov.sel_rc.top + 4 => {
                                        ov.sel_rc.top += step;
                                    }
                                    VK_DOWN_KEY if ov.sel_rc.top + step < ov.sel_rc.bottom - 4 => {
                                        ov.sel_rc.bottom -= step;
                                    }
                                    _ => return false,
                                }
                            } else {
                                match vk {
                                    VK_LEFT_KEY if ov.sel_rc.left - step >= 0 => {
                                        ov.sel_rc.left -= step;
                                        ov.sel_rc.right -= step;
                                    }
                                    VK_RIGHT_KEY if ov.sel_rc.right + step <= ov.vw => {
                                        ov.sel_rc.left += step;
                                        ov.sel_rc.right += step;
                                    }
                                    VK_UP_KEY if ov.sel_rc.top - step >= 0 => {
                                        ov.sel_rc.top -= step;
                                        ov.sel_rc.bottom -= step;
                                    }
                                    VK_DOWN_KEY if ov.sel_rc.bottom + step <= ov.vh => {
                                        ov.sel_rc.top += step;
                                        ov.sel_rc.bottom += step;
                                    }
                                    _ => return false,
                                }
                            }
                            layout_overlay_toolbar(ov);
                            true
                        });
                        if moved {
                            InvalidateRect(hwnd, null(), 0);
                        }
                        return 0;
                    }
                    0x09 => {
                        let changed = SNIP_MGR.with(|m| {
                            let ov = &mut m.borrow_mut().overlay;
                            step_overlay_hover_level(ov, 0, true)
                        });
                        if changed {
                            InvalidateRect(hwnd, null(), 0);
                        }
                        return 0;
                    }
                    _ => {}
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            let notches = wheel_delta(wp) / 120;
            if notches != 0 {
                let changed = SNIP_MGR.with(|m| {
                    let ov = &mut m.borrow_mut().overlay;
                    step_overlay_hover_level(ov, notches as isize, false)
                });
                if changed {
                    InvalidateRect(hwnd, null(), 0);
                }
            }
            0
        }
        WM_DPICHANGED => 0,
        WM_GETMINMAXINFO => {
            if lp != 0 {
                let (vw, vh) = SNIP_MGR.with(|m| {
                    m.try_borrow()
                        .map(|mgr| (mgr.overlay.vw, mgr.overlay.vh))
                        .unwrap_or((0, 0))
                });
                if vw > 0 && vh > 0 {
                    let mmi = &mut *(lp as *mut MINMAXINFO);
                    if mmi.ptMaxSize.x < vw {
                        mmi.ptMaxSize.x = vw;
                    }
                    if mmi.ptMaxSize.y < vh {
                        mmi.ptMaxSize.y = vh;
                    }
                    if mmi.ptMaxTrackSize.x < vw {
                        mmi.ptMaxTrackSize.x = vw;
                    }
                    if mmi.ptMaxTrackSize.y < vh {
                        mmi.ptMaxTrackSize.y = vh;
                    }
                }
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
    // 译图为空时表示「刚截出来、还没翻译」：先把原图摆出来
    let has_zh = !bgra_zh.is_empty();
    let old_hwnd = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let oh = mgr.result.hwnd;
        mgr.result = ResultWinState {
            hwnd: null_mut(),
            img_w: w,
            img_h: h,
            bgra_zh,
            bgra_orig,
            show_orig: !has_zh,
            zh_text,
            en_text,
            lines: Vec::new(),
            blocks: Vec::new(),
            translating: !has_zh,
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
    crate::ui::snip_toast::hide_all();

    let mut cur = POINT::default();
    unsafe { GetCursorPos(&mut cur) };
    let wa = work_area_from_point(cur);
    // 优先 1:1 显示（和贴图窗观感一致），放不下才等比缩小；最小宽受底部按钮栏约束
    let avail_w = ((wa.right - wa.left) - 80).max(360);
    let avail_h = ((wa.bottom - wa.top) - 120).max(200);
    let zoom = if w > 0 && h > 0 {
        (1.0f64)
            .min(avail_w as f64 / (w as f64 + 16.0))
            .min(avail_h as f64 / (h as f64 + 52.0))
            .clamp(0.2, 1.0)
    } else {
        1.0
    };
    SNIP_MGR.with(|m| m.borrow_mut().result.zoom = zoom);
    let win_w = (((w as f64) * zoom) as i32 + 16).clamp(480, 1400);
    let win_h = (((h as f64) * zoom) as i32 + 52).clamp(180, 960);
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

    // 译图还没回来（或用户切到原图）时一律画原图，避免空白窗
    let active_bgra = if res.show_orig || res.bgra_zh.is_empty() {
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
    let has_zh = !res.bgra_zh.is_empty();
    for (i, mut brc) in res.btn_rcs.into_iter().enumerate() {
        let hot = res.hover_btn == i as i32;
        // 1=复制译文 2=复制原文：没有译文时置灰且不可点
        let disabled = !has_zh && (i == 1 || i == 2);
        let col = if i == 0 && (res.show_orig || !has_zh) {
            rgb(59, 130, 246)
        } else if disabled {
            rgb(32, 37, 45)
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
        if disabled {
            SetTextColor(hdc_buf, rgb(110, 120, 136));
        } else {
            SetTextColor(hdc_buf, rgb(240, 244, 250));
        }
        let wl = utf8_to_wide(lbl);
        DrawTextW(
            hdc_buf,
            wl.as_ptr(),
            -1,
            &mut brc,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
    SetTextColor(hdc_buf, rgb(240, 244, 250));

    let z_txt = if res.translating {
        "识别中…".to_string()
    } else if !has_zh {
        format!(
            "{}% (F2 翻译 | 空格对比 | Esc关闭)",
            (res.zoom * 100.0).round() as i32
        )
    } else {
        format!(
            "{}% (F2 译文/原图 | 空格对比 | Esc关闭)",
            (res.zoom * 100.0).round() as i32
        )
    };
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

/// 结果窗「当前显示」的图片数据（原图/译图）
fn result_active_bgra(res: &ResultWinState) -> Vec<u8> {
    if res.show_orig || res.bgra_zh.is_empty() {
        res.bgra_orig.clone()
    } else {
        res.bgra_zh.clone()
    }
}

/// F2：没有译文就发起翻译，有译文就原图↔译图切换
fn result_f2(hwnd: HWND) {
    let (has_zh, translating, w, h, orig, cfg) = SNIP_MGR.with(|m| {
        let mgr = m.borrow();
        (
            !mgr.result.bgra_zh.is_empty(),
            mgr.result.translating,
            mgr.result.img_w,
            mgr.result.img_h,
            mgr.result.bgra_orig.clone(),
            mgr.cfg.clone(),
        )
    });
    if !has_zh && !translating {
        start_result_translate(hwnd, w, h, orig, cfg);
    } else {
        SNIP_MGR.with(|m| {
            let mut mgr = m.borrow_mut();
            mgr.result.show_orig = !mgr.result.show_orig;
        });
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
}

/// Enter / Ctrl+C：有译文给译文，否则复制图片
fn result_copy(hwnd: HWND) {
    let (has_zh, show_orig, zh, w, h, bgra) = SNIP_MGR.with(|m| {
        let r = m.borrow().result.clone();
        (r.bgra_zh.is_empty(), r.show_orig, r.zh_text.clone(), r.img_w, r.img_h, result_active_bgra(&r))
    });
    if !has_zh && !show_orig && !zh.is_empty() {
        set_clipboard_text(&zh);
        show_toast(hwnd, "已复制译文", Tone::Info);
        return;
    }
    if set_clipboard_bgra(&bgra, w, h) {
        show_toast(hwnd, if show_orig { "已复制原图" } else { "已复制图片" }, Tone::Info);
    }
}

/// Ctrl+S：另存 PNG
fn result_save_png(hwnd: HWND) {
    let (w, h, bgra, sd) = SNIP_MGR.with(|m| {
        let mgr = m.borrow();
        (
            mgr.result.img_w,
            mgr.result.img_h,
            result_active_bgra(&mgr.result),
            mgr.cfg.save_dir.clone(),
        )
    });
    if let Some(p) = save_file_dialog_png(hwnd, &sd, "KuaiYiNG_Trans.png") {
        if save_bgra_as_png(&p, &bgra, w, h) {
            show_toast(hwnd, "截图已保存", Tone::Info);
        } else {
            show_toast(hwnd, "保存失败", Tone::Error);
        }
    }
}

/// F3：把当前显示贴到屏幕
fn result_pin(hwnd: HWND) {
    let _ = hwnd;
    let (w, h, bgra, zh, en) = SNIP_MGR.with(|m| {
        let mgr = m.borrow();
        (
            mgr.result.img_w,
            mgr.result.img_h,
            result_active_bgra(&mgr.result),
            mgr.result.zh_text.clone(),
            mgr.result.en_text.clone(),
        )
    });
    create_pin_window_bgra(w, h, bgra, zh, en);
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
                let no_zh = SNIP_MGR.with(|m| m.borrow().result.bgra_zh.is_empty());
                if no_zh && (idx == 1 || idx == 2) {
                    return 0;
                }
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
                        if !t.is_empty() {
                            notify_mgr_hint("（已复制译文）");
                        }
                    }
                    2 => {
                        let t = SNIP_MGR.with(|m| m.borrow().result.en_text.clone());
                        set_clipboard_text(&t);
                        if !t.is_empty() {
                            notify_mgr_hint("（已复制原文）");
                        }
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
                        if set_clipboard_bgra(&bg, w, h) {
                            notify_mgr_hint("（已复制图片）");
                        }
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
                            if save_bgra_as_png(&p, &bg, w, h) {
                                notify_mgr_hint("（截图已保存）");
                            } else {
                                notify_mgr_hint("（截图保存失败）");
                            }
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
            const VK_F2_KEY: i32 = 0x71;
            const VK_F3_KEY: i32 = 0x72;
            let vk = (wp & 0xFF) as i32;
            let ctrl = (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0;
            let no_zh = SNIP_MGR.with(|m| m.borrow().result.bgra_zh.is_empty());
            match vk {
                VK_ESCAPE => {
                    DestroyWindow(hwnd);
                }
                VK_F2_KEY => result_f2(hwnd),
                VK_F3_KEY => result_pin(hwnd),
                0x43 if ctrl => result_copy(hwnd),
                0x53 if ctrl => result_save_png(hwnd),
                VK_RETURN => result_copy(hwnd),
                0x20 => {
                    if !no_zh {
                        SNIP_MGR.with(|m| {
                            let mut mgr = m.borrow_mut();
                            mgr.result.show_orig = !mgr.result.show_orig;
                        });
                        InvalidateRect(hwnd, null(), 0);
                    }
                }
                _ => {}
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
    // 截屏遮罩层开启时，按 F3/贴图热键直接将当前选区（或悬停窗口）贴图，绝不回退剪贴板或上次贴图
    let (overlay_active, has_sel) = SNIP_MGR.with(|m| {
        let ov = &m.borrow().overlay;
        (ov.active, active_overlay_sel_rect(ov).is_some())
    });
    if overlay_active {
        if has_sel {
            finish_selection_action(BTN_PIN);
        } else {
            close_overlay();
        }
        return;
    }

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
    create_pin_window_bgra_at(None, w, h, bgra, zh_text, en_text);
}

pub fn create_pin_window_bgra_at(
    pos: Option<(i32, i32)>,
    w: i32,
    h: i32,
    bgra: Vec<u8>,
    zh_text: String,
    en_text: String,
) {
    if w <= 0 || h <= 0 || bgra.is_empty() {
        return;
    }
    unsafe { ensure_snip_classes() };
    let mut cur = POINT::default();
    unsafe { GetCursorPos(&mut cur) };
    let ref_pt = pos.map(|(px, py)| POINT { x: px, y: py }).unwrap_or(cur);
    let wa = crate::ui::win32_ui::work_area_from_point(ref_pt);
    let init_w = w.min((wa.right - wa.left) - 40).max(40);
    let init_h = h.min((wa.bottom - wa.top) - 40).max(40);
    let zoom = ((init_w as f64) / (w as f64))
        .min((init_h as f64) / (h as f64))
        .clamp(0.15, 1.0);
    let draw_w = ((w as f64) * zoom).round() as i32;
    let draw_h = ((h as f64) * zoom).round() as i32;
    let (x, y) = if let Some((px, py)) = pos {
        (
            px.clamp(wa.left, (wa.right - draw_w).max(wa.left)),
            py.clamp(wa.top, (wa.bottom - draw_h).max(wa.top)),
        )
    } else {
        (
            (cur.x - draw_w / 2).clamp(wa.left, (wa.right - draw_w).max(wa.left)),
            (cur.y - draw_h / 2).clamp(wa.top, (wa.bottom - draw_h).max(wa.top)),
        )
    };

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
                lines: Vec::new(),
                blocks: Vec::new(),
                sel: PinSel::default(),
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
    let ok = payload.res.ok;
    let err = payload.res.err.clone();
    SNIP_MGR.with(|m| {
        if let Some(p) = m.borrow_mut().pins.iter_mut().find(|x| x.hwnd == pin_hwnd) {
            p.translating = false;
            if ok {
                p.bgra_zh = payload.res.bgra_out.clone();
                p.zh_text = payload.res.zh_text.clone();
                p.en_text = payload.res.en_text.clone();
                p.lines = payload.res.lines.clone();
                p.blocks = payload.res.blocks.clone();
                p.show_zh = true;
            }
        }
    });
    if !ok {
        log_write("截屏OCR", &err);
        let (tone, msg) = classify_snip_error(&err);
        show_toast(pin_hwnd, &msg, tone);
    }
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

    // 选中态下把行框/选区高亮画在图上（图像坐标 → 窗口坐标）
    if pin.sel.mode && !pin.lines.is_empty() {
        let mut mpt = POINT::default();
        GetCursorPos(&mut mpt);
        ScreenToClient(hwnd, &mut mpt);
        let (hx, hy) = pin_img_pt(&pin, mpt.x, mpt.y);
        let hover = hit_ocr_line(&pin.lines, hx, hy);
        if hover >= 0 {
            if let Some(l) = pin.lines.get(hover as usize) {
                let lr = pin_win_rc(
                    &pin,
                    &RECT {
                        left: l.x.floor() as i32,
                        top: l.y.floor() as i32,
                        right: (l.x + l.w).ceil() as i32,
                        bottom: (l.y + l.h).ceil() as i32,
                    },
                );
                let hr = lr;
                // GDI 没有 alpha：用调过的深蓝近似「25% 蓝叠加」
                let light = CreateSolidBrush(rgb(46, 74, 122));
                FillRect(hdc_buf, &hr, light);
                DeleteObject(light);
                let pen_h = CreatePen(PS_SOLID, 1, rgb(59, 130, 246));
                let op_h = SelectObject(hdc_buf, pen_h);
                let ob_h = SelectObject(hdc_buf, GetStockObject(NULL_BRUSH));
                crate::ui::win32_ui::Rectangle(hdc_buf, hr.left, hr.top, hr.right, hr.bottom);
                SelectObject(hdc_buf, ob_h);
                SelectObject(hdc_buf, op_h);
                DeleteObject(pen_h);
            }
        }
    }
    if pin.sel.mode && (pin.sel.dragging || pin.sel.has_rect) {
        let sr = if pin.sel.dragging {
            RECT {
                left: pin.sel.from.x.min(pin.sel.to.x),
                top: pin.sel.from.y.min(pin.sel.to.y),
                right: pin.sel.from.x.max(pin.sel.to.x),
                bottom: pin.sel.from.y.max(pin.sel.to.y),
            }
        } else {
            pin.sel.rect
        };
        if sr.right > sr.left && sr.bottom > sr.top {
            let pen_s = CreatePen(PS_DASH, 1, rgb(59, 130, 246));
            let op_s = SelectObject(hdc_buf, pen_s);
            let ob_s = SelectObject(hdc_buf, GetStockObject(NULL_BRUSH));
            crate::ui::win32_ui::Rectangle(hdc_buf, sr.left, sr.top, sr.right, sr.bottom);
            SelectObject(hdc_buf, ob_s);
            SelectObject(hdc_buf, op_s);
            DeleteObject(pen_s);
        }
    }

    // 边框：选中态蓝色 2px，平时淡灰 1px
    let (bd_col, bd_w) = if pin.sel.mode {
        (rgb(59, 130, 246), 2)
    } else {
        (rgb(86, 98, 120), 1)
    };
    let pen = CreatePen(PS_SOLID, bd_w, bd_col);
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
    if !pin.sel.mode && pt.x >= 0 && pt.x < w && pt.y >= 0 && pt.y < h {
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

// ---- 贴图选中态与划选取字 ----

/// 窗口客户区坐标 → 图像像素坐标
fn pin_img_pt(pin: &PinEntry, x: i32, y: i32) -> (f32, f32) {
    let z = if pin.zoom > 0.05 { pin.zoom } else { 1.0 };
    (x as f32 / z as f32, y as f32 / z as f32)
}

/// 图像像素矩形 → 窗口客户区矩形
fn pin_win_rc(pin: &PinEntry, rc: &RECT) -> RECT {
    let z = if pin.zoom > 0.05 { pin.zoom } else { 1.0 };
    RECT {
        left: (rc.left as f64 * z).round() as i32,
        top: (rc.top as f64 * z).round() as i32,
        right: (rc.right as f64 * z).round() as i32,
        bottom: (rc.bottom as f64 * z).round() as i32,
    }
}

/// 客户区矩形 → 屏幕矩形（气泡定位用）
unsafe fn client_rect_to_screen(hwnd: HWND, rc: RECT) -> RECT {
    let mut tl = POINT {
        x: rc.left,
        y: rc.top,
    };
    let mut br = POINT {
        x: rc.right,
        y: rc.bottom,
    };
    ClientToScreen(hwnd, &mut tl);
    ClientToScreen(hwnd, &mut br);
    RECT {
        left: tl.x,
        top: tl.y,
        right: br.x,
        bottom: br.y,
    }
}

/// 命中哪一行（图像坐标），-1 表示没命中
fn hit_ocr_line(lines: &[OcrLine], ix: f32, iy: f32) -> i32 {
    let mut best: i32 = -1;
    let mut best_area = f32::MAX;
    for (i, l) in lines.iter().enumerate() {
        if ix >= l.x && ix <= l.x + l.w && iy >= l.y && iy <= l.y + l.h {
            let area = l.w * l.h;
            if area < best_area {
                best_area = area;
                best = i as i32;
            }
        }
    }
    best
}

/// 与矩形相交的行索引（按阅读顺序：先 y 后 x）
fn lines_in_img_rect(lines: &[OcrLine], rc: RECT) -> Vec<usize> {
    let mut hit: Vec<usize> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let ox = l.x.max(rc.left as f32) < (l.x + l.w).min(rc.right as f32);
        let oy = l.y.max(rc.top as f32) < (l.y + l.h).min(rc.bottom as f32);
        if ox && oy {
            hit.push(i);
        }
    }
    hit.sort_by(|&a, &b| {
        let la = &lines[a];
        let lb = &lines[b];
        (la.y as i32, la.x as i32).cmp(&(lb.y as i32, lb.x as i32))
    });
    hit
}

/// 按命中行取文本：原图态取英文行，译图态取所在块译文
fn pin_sel_text(pin: &PinEntry, idxs: &[usize]) -> (String, bool) {
    let is_zh = pin.show_zh && !pin.bgra_zh.is_empty();
    let mut parts: Vec<String> = Vec::new();
    if is_zh {
        let mut used: Vec<String> = Vec::new();
        for &i in idxs {
            let Some(l) = pin.lines.get(i) else { continue };
            let cx = l.x + l.w * 0.5;
            let cy = l.y + l.h * 0.5;
            let Some(b) = pin.blocks.iter().find(|b| b.contains(cx, cy)) else {
                continue;
            };
            if used.iter().any(|u| u == &b.en) {
                continue;
            }
            used.push(b.en.clone());
            if !b.zh.is_empty() {
                parts.push(b.zh.clone());
            } else if !b.en.is_empty() {
                parts.push(b.en.clone());
            }
        }
    } else {
        for &i in idxs {
            if let Some(l) = pin.lines.get(i) {
                if !l.text.is_empty() {
                    parts.push(l.text.clone());
                }
            }
        }
    }
    (parts.join("\n"), is_zh)
}

/// 行框懒加载：贴图还没翻译过时，进入选中态先跑一次纯 OCR
fn pin_ensure_lines(hwnd: HWND) {
    let need = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let Some(p) = mgr.pins.iter_mut().find(|x| x.hwnd == hwnd) else {
            return None;
        };
        if !p.lines.is_empty() || p.sel.loading {
            return None;
        }
        p.sel.loading = true;
        Some((p.img_w, p.img_h, p.bgra_orig.clone()))
    });
    let Some((w, h, bgra)) = need else {
        return;
    };
    let hwnd_main = SNIP_MGR.with(|m| m.borrow().cfg.hwnd_main);
    if hwnd_main == 0 {
        return;
    }
    show_toast(hwnd, "正在识别文字…", Tone::Info);
    let target = hwnd as usize;
    thread::spawn(move || {
        let res = crate::core::ocr::snip_ocr_lines(&bgra, w, h);
        let (lines, err) = match res {
            Ok(l) => (l, String::new()),
            Err(e) => (Vec::new(), e),
        };
        let payload = Box::into_raw(Box::new(PinLinesPayload { target, lines, err }));
        unsafe {
            PostMessageW(hwnd_main as HWND, WM_APP_PIN_LINES, 0, payload as LPARAM);
        }
    });
}

struct PinLinesPayload {
    target: usize,
    lines: Vec<OcrLine>,
    err: String,
}

/// 主线程：把懒加载的行框塞回贴图
pub fn handle_pin_lines_message(lp: LPARAM) {
    if lp == 0 {
        return;
    }
    let payload = unsafe { Box::from_raw(lp as *mut PinLinesPayload) };
    let hwnd = payload.target as HWND;
    if unsafe { IsWindow(hwnd) } == 0 {
        return;
    }
    let empty = payload.lines.is_empty();
    let err = payload.err.clone();
    SNIP_MGR.with(|m| {
        if let Some(p) = m.borrow_mut().pins.iter_mut().find(|x| x.hwnd == hwnd) {
            p.sel.loading = false;
            if !empty {
                p.lines = payload.lines.clone();
            }
        }
    });
    if empty {
        let src = if err.is_empty() { "未识别到文本" } else { &err };
        let (tone, msg) = classify_snip_error(src);
        SNIP_MGR.with(|m| {
            if let Some(p) = m.borrow_mut().pins.iter_mut().find(|x| x.hwnd == hwnd) {
                p.sel.mode = false;
            }
        });
        show_toast(hwnd, &msg, tone);
    }
    unsafe { InvalidateRect(hwnd, null(), 0) };
}

/// Ctrl+左键：进入/退出选中态（贴图不关）
fn pin_toggle_sel(hwnd: HWND) {
    let (now_on, has_lines, has_ocr_job) = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let Some(p) = mgr.pins.iter_mut().find(|x| x.hwnd == hwnd) else {
            return (false, false, false);
        };
        p.sel.mode = !p.sel.mode;
        p.sel.dragging = false;
        p.sel.has_rect = false;
        p.sel.text.clear();
        (p.sel.mode, !p.lines.is_empty(), mgr.cfg.hwnd_main != 0)
    });
    if now_on {
        if !has_lines && has_ocr_job {
            pin_ensure_lines(hwnd);
        }
        show_toast(hwnd, "选字模式：拖选文字复制 · 空白单击复制图片", Tone::Info);
    }
    unsafe { InvalidateRect(hwnd, null(), 0) };
}

/// 划选结束：有字→复制文本 + 气泡；没字→复制整图（贴图保留）
fn pin_finish_selection(hwnd: HWND) {
    let payload = SNIP_MGR.with(|m| {
        let mut mgr = m.borrow_mut();
        let Some(p) = mgr.pins.iter_mut().find(|x| x.hwnd == hwnd) else {
            return None;
        };
        if !p.sel.dragging {
            return None;
        }
        p.sel.dragging = false;
        let (x1, y1) = (p.sel.from.x, p.sel.from.y);
        let (x2, y2) = (p.sel.to.x, p.sel.to.y);
        let rc = RECT {
            left: x1.min(x2),
            top: y1.min(y2),
            right: x1.max(x2),
            bottom: y1.max(y2),
        };
        // 单击（拖框小于 4px）不算划选
        if (x2 - x1).abs() < 4 && (y2 - y1).abs() < 4 {
            p.sel.has_rect = false;
            return Some((true, String::new(), false, rc, p.img_w, p.img_h));
        }
        let (ix1, iy1) = pin_img_pt(p, rc.left, rc.top);
        let (ix2, iy2) = pin_img_pt(p, rc.right, rc.bottom);
        let img_rc = RECT {
            left: ix1.floor() as i32,
            top: iy1.floor() as i32,
            right: ix2.ceil() as i32,
            bottom: iy2.ceil() as i32,
        };
        let idxs = lines_in_img_rect(&p.lines, img_rc);
        let (text, is_zh) = pin_sel_text(p, &idxs);
        p.sel.rect = rc;
        p.sel.has_rect = true;
        p.sel.text = text.clone();
        p.sel.is_zh = is_zh;
        Some((false, text, is_zh, rc, p.img_w, p.img_h))
    });
    let Some((blank, text, is_zh, rc, img_w, img_h)) = payload else {
        return;
    };
    let win_rc = SNIP_MGR.with(|m| {
        m.borrow()
            .pins
            .iter()
            .find(|p| p.hwnd == hwnd)
            .map(|p| pin_win_rc(p, &rc))
            .unwrap_or(rc)
    });
    if blank || text.is_empty() {
        let img = SNIP_MGR.with(|m| {
            m.borrow().pins.iter().find(|p| p.hwnd == hwnd).map(|p| {
                if p.show_zh && !p.bgra_zh.is_empty() {
                    p.bgra_zh.clone()
                } else {
                    p.bgra_orig.clone()
                }
            })
        });
        if let Some(bgra) = img {
            if set_clipboard_bgra(&bgra, img_w, img_h) {
                show_toast(hwnd, "已复制图片", Tone::Info);
            }
        }
        unsafe { InvalidateRect(hwnd, null(), 0) };
        return;
    }
    set_clipboard_text(&text);
    let preview: String = text.chars().take(90).collect();
    let tag = if is_zh { "已复制译文" } else { "已复制原文" };
    let scr = unsafe { client_rect_to_screen(hwnd, win_rc) };
    show_bubble(scr, vec![preview, tag.to_string()], Tone::Info);
    unsafe { InvalidateRect(hwnd, null(), 0) };
}

/// F2（选中态且有选区）：翻译所选文字
fn pin_translate_selection(hwnd: HWND) {
    let job = SNIP_MGR.with(|m| {
        let mgr = m.borrow();
        let Some(p) = mgr.pins.iter().find(|x| x.hwnd == hwnd) else {
            return None;
        };
        if !p.sel.mode || !p.sel.has_rect || p.sel.is_zh {
            return None;
        }
        let text = p.sel.text.trim().to_string();
        if text.is_empty() {
            return None;
        }
        let win_rc = pin_win_rc(p, &p.sel.rect);
        Some((text, win_rc))
    });
    let Some((text, win_rc)) = job else {
        return;
    };
    let cfg = SNIP_MGR.with(|m| m.borrow().cfg.clone());
    let hwnd_main = cfg.hwnd_main;
    if hwnd_main == 0 {
        return;
    }
    show_toast(hwnd, "翻译中…", Tone::Info);
    let target = hwnd as usize;
    thread::spawn(move || {
        let zh = crate::core::bing::translate_chain_ex(
            &text,
            crate::core::FY_EN2ZH,
            &cfg.ai_url,
            &cfg.ai_key,
            &cfg.ai_model,
            &cfg.baidu_appid,
            &cfg.baidu_secret,
            &cfg.baidu_url,
            0,
        );
        let payload = Box::into_raw(Box::new(SelTransPayload {
            target,
            src: text,
            zh: zh.unwrap_or_default(),
            rc: win_rc,
        }));
        unsafe {
            PostMessageW(hwnd_main as HWND, WM_APP_SEL_TRANS, 0, payload as LPARAM);
        }
    });
}

struct SelTransPayload {
    target: usize,
    src: String,
    zh: String,
    rc: RECT,
}

/// 主线程：选区翻译结果 → 气泡（原文 + 译文），顺手复制译文
pub fn handle_sel_trans_message(lp: LPARAM) {
    if lp == 0 {
        return;
    }
    let payload = unsafe { Box::from_raw(lp as *mut SelTransPayload) };
    let hwnd = payload.target as HWND;
    if unsafe { IsWindow(hwnd) } == 0 {
        return;
    }
    let scr = unsafe { client_rect_to_screen(hwnd, payload.rc) };
    if payload.zh.trim().is_empty() {
        show_toast(hwnd, "翻译失败，详情见运行日志", Tone::Error);
        return;
    }
    let src: String = payload.src.chars().take(90).collect();
    let zh: String = payload.zh.chars().take(90).collect();
    set_clipboard_text(&payload.zh);
    show_bubble(
        scr,
        vec![src, zh, "已复制译文".to_string()],
        Tone::Info,
    );
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
            // 选中态：拖框划字，不移动窗口
            let sel_drag = SNIP_MGR.with(|m| {
                let mut mgr = m.borrow_mut();
                let Some(p) = mgr.pins.iter_mut().find(|i| i.hwnd == hwnd) else {
                    return false;
                };
                if p.sel.dragging {
                    p.sel.to = POINT { x, y };
                    return true;
                }
                false
            });
            if sel_drag {
                InvalidateRect(hwnd, null(), 0);
                return 0;
            }
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
                crate::ui::snip_toast::follow(hwnd);
            } else {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            // Ctrl+左键：进/出选中态（不关贴图）
            if (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0 {
                pin_toggle_sel(hwnd);
                return 0;
            }
            let in_sel = SNIP_MGR.with(|m| {
                m.borrow()
                    .pins
                    .iter()
                    .find(|i| i.hwnd == hwnd)
                    .map(|p| p.sel.mode)
                    .unwrap_or(false)
            });
            if in_sel {
                // 左键拖 = 划字；松手时若没拖动则视为单击 → 复制整图
                SNIP_MGR.with(|m| {
                    if let Some(p) = m.borrow_mut().pins.iter_mut().find(|i| i.hwnd == hwnd) {
                        p.sel.dragging = true;
                        p.sel.has_rect = false;
                        p.sel.text.clear();
                        p.sel.from = POINT { x, y };
                        p.sel.to = POINT { x, y };
                    }
                });
                SetCapture(hwnd);
                return 0;
            }
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
            let in_sel_drag = SNIP_MGR.with(|m| {
                m.borrow()
                    .pins
                    .iter()
                    .find(|i| i.hwnd == hwnd)
                    .map(|p| p.sel.dragging)
                    .unwrap_or(false)
            });
            if in_sel_drag {
                pin_finish_selection(hwnd);
                return 0;
            }
            SNIP_MGR.with(|m| {
                if let Some(p) = m.borrow_mut().pins.iter_mut().find(|i| i.hwnd == hwnd) {
                    p.dragging = false;
                }
            });
            0
        }
        WM_LBUTTONDBLCLK => {
            let (x, y) = lparam_xy(lp);
            let in_sel = SNIP_MGR.with(|m| {
                m.borrow()
                    .pins
                    .iter()
                    .find(|i| i.hwnd == hwnd)
                    .map(|p| p.sel.mode)
                    .unwrap_or(false)
            });
            if in_sel {
                // 选中态双击 = 复制整张图，贴图保留
                SNIP_MGR.with(|m| {
                    if let Some(p) = m.borrow_mut().pins.iter_mut().find(|i| i.hwnd == hwnd) {
                        p.sel.dragging = true;
                        p.sel.from = POINT { x, y };
                        p.sel.to = POINT { x, y };
                    }
                });
                pin_finish_selection(hwnd);
                return 0;
            }
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
                        "原位翻译图片 (F2)"
                    },
                    MF_STRING | if show_zh { MF_CHECKED } else { 0 },
                );
                if has_zh {
                    add(2, "复制 OCR 译文", MF_STRING);
                }
                add(3, "复制图片", MF_STRING);
                add(4, "另存为 PNG", MF_STRING);
                add(5, "还原 100% 大小", MF_STRING);
                let sel_on = SNIP_MGR.with(|m| {
                    m.borrow()
                        .pins
                        .iter()
                        .find(|p| p.hwnd == hwnd)
                        .map(|p| p.sel.mode)
                        .unwrap_or(false)
                });
                add(
                    8,
                    if sel_on { "退出选字模式" } else { "选字模式 (Ctrl+左键)" },
                    MF_STRING | if sel_on { MF_CHECKED } else { 0 },
                );
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
                        if !zh.is_empty() {
                            notify_mgr_hint("（已复制译文）");
                        }
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
                            if set_clipboard_bgra(&b, w, h) {
                                notify_mgr_hint("（已复制贴图）");
                            }
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
                                if save_bgra_as_png(&p, &b, w, h) {
                                    notify_mgr_hint("（贴图已保存）");
                                } else {
                                    notify_mgr_hint("（贴图保存失败）");
                                }
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
                    8 => pin_toggle_sel(hwnd),
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
            const VK_F2_KEY: i32 = 0x71;
            let vk = (wp & 0xFF) as i32;
            let (in_sel, has_rect) = SNIP_MGR.with(|m| {
                m.borrow()
                    .pins
                    .iter()
                    .find(|i| i.hwnd == hwnd)
                    .map(|p| (p.sel.mode, p.sel.has_rect))
                    .unwrap_or((false, false))
            });
            if vk == VK_ESCAPE {
                if in_sel {
                    // 先退选字模式，贴图仍在
                    SNIP_MGR.with(|m| {
                        if let Some(p) = m.borrow_mut().pins.iter_mut().find(|i| i.hwnd == hwnd) {
                            p.sel.mode = false;
                            p.sel.has_rect = false;
                            p.sel.text.clear();
                        }
                    });
                    InvalidateRect(hwnd, null(), 0);
                } else {
                    DestroyWindow(hwnd);
                }
            } else if vk == VK_F2_KEY {
                if in_sel && has_rect {
                    pin_translate_selection(hwnd);
                } else {
                    start_pin_translate(hwnd);
                }
            } else if vk == 'T' as i32 || vk == 0x20 {
                start_pin_translate(hwnd);
            }
            0
        }
        WM_SETCURSOR => {
            let in_sel = SNIP_MGR.with(|m| {
                m.borrow()
                    .pins
                    .iter()
                    .find(|i| i.hwnd == hwnd)
                    .map(|p| p.sel.mode)
                    .unwrap_or(false)
            });
            if in_sel {
                SetCursor(LoadCursorW(null_mut(), IDC_IBEAM));
                return 1;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_CLOSE | WM_DESTROY => {
            crate::ui::snip_toast::hide_all();
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
    fn test_hit_test_handles_and_disable_whole_rect_drag() {
        let sel = RECT {
            left: 100,
            top: 100,
            right: 300,
            bottom: 300,
        };

        // 1. 四个角点（10=TL, 12=TR, 14=BR, 16=BL）
        assert_eq!(hit_test_handles(sel, 100, 100), 10);
        assert_eq!(hit_test_handles(sel, 300, 100), 12);
        assert_eq!(hit_test_handles(sel, 300, 300), 14);
        assert_eq!(hit_test_handles(sel, 100, 300), 16);

        // 2. 边框上的任意位置（即使偏离中点，也可顺畅拖动边界）
        assert_eq!(hit_test_handles(sel, 150, 100), 11); // T
        assert_eq!(hit_test_handles(sel, 250, 102), 11); // T（带边框容差）
        assert_eq!(hit_test_handles(sel, 300, 180), 13); // R
        assert_eq!(hit_test_handles(sel, 200, 300), 15); // B
        assert_eq!(hit_test_handles(sel, 100, 220), 17); // L

        // 3. 选区内部：不属于边界手柄，不返回任何手柄编号
        assert_eq!(hit_test_handles(sel, 200, 200), 0);
        assert_eq!(hit_test_handles(sel, 150, 150), 0);
        assert!(pt_in_rect(sel, 200, 200));

        // 4. 选区外部远距离
        assert_eq!(hit_test_handles(sel, 50, 50), 0);
        assert!(!pt_in_rect(sel, 50, 50));
    }

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

    #[test]
    fn test_brush_mosaic_and_blur_capsule_stroke() {
        // 构造 32x32 黑白棋盘测试图（4x4 格子）
        let w = 32;
        let h = 32;
        let mut orig = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let off = ((y * w + x) * 4) as usize;
                let dark = ((x / 4) + (y / 4)) % 2 == 0;
                let v = if dark { 20u8 } else { 220u8 };
                orig[off] = v;
                orig[off + 1] = v;
                orig[off + 2] = v;
                orig[off + 3] = 255;
            }
        }
        let full_rc = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };

        // 1. 画笔式马赛克（斜线轨迹）
        let (m_rc, m_layer) = compute_mosaic_layer(&orig, w, h, full_rc, 8);
        let mut dst_mosaic = orig.clone();
        let pts = vec![(8, 8), (22, 22)];
        apply_brush_stroke_from_layer(&mut dst_mosaic, w, h, m_rc, &m_layer, &pts, 5, full_rc);
        // 轨迹内部某点（如 (15,15)）应变为灰阶均值 ~120，而不是原始极端值 20 或 220
        let in_off = ((15 * w + 15) * 4) as usize;
        assert!(
            dst_mosaic[in_off] > 50 && dst_mosaic[in_off] < 190,
            "马赛克轨迹内像素值异常: {}",
            dst_mosaic[in_off]
        );
        // 远离轨迹的角落 (0, 30) 应保持原图不变
        let out_off = ((30 * w) * 4) as usize;
        assert_eq!(
            &dst_mosaic[out_off..out_off + 3],
            &orig[out_off..out_off + 3]
        );

        // 2. 画笔式模糊（单点点击生效）
        let (b_rc, b_layer) = compute_blur_layer(&orig, w, h, full_rc, 4);
        let mut dst_blur = orig.clone();
        apply_brush_stroke_from_layer(&mut dst_blur, w, h, b_rc, &b_layer, &[(16, 16)], 4, full_rc);
        let center_off = ((16 * w + 16) * 4) as usize;
        assert_ne!(dst_blur[center_off], orig[center_off]);
        assert_eq!(&dst_blur[out_off..out_off + 3], &orig[out_off..out_off + 3]);

        // 3. flatten_selection 集成验证：画笔标注输出与选区外部隔离
        let sel = RECT {
            left: 4,
            top: 4,
            right: 28,
            bottom: 28,
        };
        let annots = vec![
            AnnotItem {
                tool: AnnotTool::Mosaic,
                x1: 10,
                y1: 10,
                x2: 20,
                y2: 20,
                pts: vec![(10, 10), (20, 20)],
                r: 0,
                g: 0,
                b: 0,
                width: 2,
                filled: false,
                num: 0,
                text: String::new(),
            },
            AnnotItem {
                tool: AnnotTool::Blur,
                x1: 22,
                y1: 10,
                x2: 22,
                y2: 10,
                pts: vec![(22, 10)],
                r: 0,
                g: 0,
                b: 0,
                width: 2,
                filled: false,
                num: 0,
                text: String::new(),
            },
        ];
        let (cw, ch, flat) = flatten_selection(&orig, w, h, sel, &annots);
        assert_eq!((cw, ch), (24, 24));
        // 原图全局 (15, 15) 对应局部 (11, 11) 在马赛克轨迹上
        let flat_in = ((11 * cw + 11) * 4) as usize;
        assert!(flat[flat_in] > 50 && flat[flat_in] < 190);
        // 选区左下角局部 (0, 23) 对应全局 (4, 27)，远离所有画笔半径范围（r=10），保持原样
        let flat_corner = (23 * cw * 4) as usize;
        let orig_corner = ((27 * w + 4) * 4) as usize;
        assert_eq!(
            &flat[flat_corner..flat_corner + 3],
            &orig[orig_corner..orig_corner + 3]
        );

        // 验证空缓存及析构路径不发生递归栈溢出
        let mut empty_cache = OverlayGdiCache::default();
        empty_cache.free();
        drop(empty_cache);
    }

    #[test]
    fn test_multimonitor_virtual_bounds_and_asymmetric_clamping() {
        // 1. 异形混合 DPI 双屏（左 2560x1440 + 右 2560x1600）
        let mons_asym = [
            RECT {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1440,
            },
            RECT {
                left: 2560,
                top: 0,
                right: 5120,
                bottom: 1600,
            },
        ];
        assert_eq!(
            compute_virtual_bounds(&mons_asym, (0, 0, 1, 1)),
            (0, 0, 5120, 1600)
        );

        // 2. 负坐标左副屏拓扑
        let mons_neg = [
            RECT {
                left: -1920,
                top: 0,
                right: 0,
                bottom: 1080,
            },
            RECT {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1440,
            },
        ];
        assert_eq!(
            compute_virtual_bounds(&mons_neg, (0, 0, 1, 1)),
            (-1920, 0, 4480, 1440)
        );

        // 3. monitor_bounds_for_point 区分左屏、右屏与左下不可见死角
        let m_left = monitor_bounds_for_point(&mons_asym, 5120, 1600, 100, 500);
        assert_eq!((m_left.left, m_left.right, m_left.bottom), (0, 2560, 1440));
        let m_right = monitor_bounds_for_point(&mons_asym, 5120, 1600, 3000, 1500);
        assert_eq!(
            (m_right.left, m_right.right, m_right.bottom),
            (2560, 5120, 1600)
        );
        // 点 (1000, 1550) 处于左屏正下方无屏幕覆盖死角，最近屏幕为左屏 (bottom=1440)
        let m_dead = monitor_bounds_for_point(&mons_asym, 5120, 1600, 1000, 1550);
        assert_eq!((m_dead.left, m_dead.right, m_dead.bottom), (0, 2560, 1440));

        // 4. 在左屏底边拉框选时，悬浮工具栏应自动翻转到选区上方且不超过 1440
        let mut ov = OverlayState {
            vw: 5120,
            vh: 1600,
            mon_rects: mons_asym.to_vec(),
            has_sel: true,
            sel_rc: RECT {
                left: 100,
                top: 1300,
                right: 900,
                bottom: 1438,
            },
            ..Default::default()
        };
        layout_overlay_toolbar(&mut ov);
        assert!(ov.bar_rc.bottom <= 1440);
        assert!(ov.bar_rc.left >= 0 && ov.bar_rc.right <= 2560);
    }

    #[test]
    fn test_auto_window_pick_excludes_taskbar_and_follows_zorder() {
        // 1. 屏蔽任务栏与桌面外壳类，保留常规应用窗口类
        assert!(should_skip_auto_window_class("Shell_TrayWnd"));
        assert!(should_skip_auto_window_class("Shell_SecondaryTrayWnd"));
        assert!(should_skip_auto_window_class("Progman"));
        assert!(should_skip_auto_window_class("WorkerW"));
        assert!(should_skip_auto_window_class("Windows.UI.Core.CoreWindow"));
        assert!(!should_skip_auto_window_class("Chrome_WidgetWin_1"));
        assert!(!should_skip_auto_window_class("CabinetWClass"));

        let mons = [
            RECT {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1440,
            },
            RECT {
                left: 2560,
                top: 0,
                right: 5120,
                bottom: 1600,
            },
        ];
        let works = [
            RECT {
                left: 0,
                top: 0,
                right: 2560,
                bottom: 1392,
            },
            RECT {
                left: 2560,
                top: 0,
                right: 5120,
                bottom: 1528,
            },
        ];

        // 2. 副屏最大化或外溢到任务栏区间的窗口应严格裁切到 work_rects[1]
        let maximized_sec = RECT {
            left: 2552,
            top: -8,
            right: 5128,
            bottom: 1600,
        };
        let clipped = clip_auto_window_rect(maximized_sec, &mons, &works).unwrap();
        assert_eq!(
            (clipped.left, clipped.top, clipped.right, clipped.bottom),
            (2560, 0, 5120, 1528)
        );

        // 3. Z-order 优先：前置大窗口覆盖在后置小窗口上方时，鼠标落在重叠区应命中前窗
        let win_front = RECT {
            left: 2700,
            top: 100,
            right: 3600,
            bottom: 900,
        };
        let win_back_small = RECT {
            left: 2800,
            top: 200,
            right: 3100,
            bottom: 500,
        };
        let z_list = [
            WindowCandidate::from(win_front),
            WindowCandidate::from(win_back_small),
        ];
        let hit_overlap = pick_hover_window_rect(&z_list, &works, 2900, 300);
        assert_eq!(
            (
                hit_overlap.left,
                hit_overlap.top,
                hit_overlap.right,
                hit_overlap.bottom
            ),
            (2700, 100, 3600, 900)
        );

        // 4. 副屏空白桌面命中 work_rects[1]（底部不含 72px 任务栏），副屏任务栏区不产生高亮框
        let hit_desktop = pick_hover_window_rect(&z_list, &works, 4500, 800);
        assert_eq!(
            (
                hit_desktop.left,
                hit_desktop.top,
                hit_desktop.right,
                hit_desktop.bottom
            ),
            (2560, 0, 5120, 1528)
        );
        let hit_taskbar = pick_hover_window_rect(&z_list, &works, 3000, 1560);
        assert_eq!(
            (
                hit_taskbar.left,
                hit_taskbar.top,
                hit_taskbar.right,
                hit_taskbar.bottom
            ),
            (0, 0, 0, 0)
        );

        // 5. 顶部工具栏与子窗口精细选取：大窗口内部包含顶部工具栏与内容区
        let win_ide = WindowCandidate {
            hwnd: 0,
            top_rect: RECT {
                left: 100,
                top: 100,
                right: 1300,
                bottom: 900,
            },
            sub_rects: vec![
                // 顶部工具栏 (高 36px)
                RECT {
                    left: 100,
                    top: 100,
                    right: 1300,
                    bottom: 136,
                },
                // 主编辑内容区 (高 764px)
                RECT {
                    left: 100,
                    top: 136,
                    right: 1300,
                    bottom: 900,
                },
            ],
        };
        let cands = [win_ide];
        // 鼠标停在顶部工具栏上时，精确命中顶部工具栏而不是整个大界面
        let hit_toolbar = pick_hover_window_rect(&cands, &works, 500, 118);
        assert_eq!(
            (
                hit_toolbar.left,
                hit_toolbar.top,
                hit_toolbar.right,
                hit_toolbar.bottom
            ),
            (100, 100, 1300, 136)
        );
        // 鼠标停在内容区时，命中内容区
        let hit_editor = pick_hover_window_rect(&cands, &works, 500, 400);
        assert_eq!(
            (
                hit_editor.left,
                hit_editor.top,
                hit_editor.right,
                hit_editor.bottom
            ),
            (100, 136, 1300, 900)
        );
        // 鼠标停在没有子区域覆盖的边缘时，回退到大窗口自身
        let win_simple = WindowCandidate::from(RECT {
            left: 100,
            top: 100,
            right: 1300,
            bottom: 900,
        });
        let hit_whole = pick_hover_window_rect(&[win_simple], &works, 500, 118);
        assert_eq!(
            (hit_whole.left, hit_whole.top, hit_whole.right, hit_whole.bottom),
            (100, 100, 1300, 900)
        );

        // 6. 前台应用（浏览器）与后台应用（文件夹小分区）的严格前后层级遮挡判定：
        // 浏览器在前 (200, 50, 1600, 950)，带顶部标签工具栏 (200, 50, 1600, 90)；
        // 文件夹在后 (100, 100, 1500, 1000)，内部有更小的子分区：左侧树 (100, 150, 300, 950)、文件列表 (300, 150, 1500, 950)
        let front_browser = WindowCandidate {
            hwnd: 0x1001,
            top_rect: RECT { left: 200, top: 50, right: 1600, bottom: 950 },
            sub_rects: vec![
                RECT { left: 200, top: 50, right: 1600, bottom: 90 }, // 顶部标签栏
            ],
        };
        let back_explorer = WindowCandidate {
            hwnd: 0x1002,
            top_rect: RECT { left: 100, top: 100, right: 1500, bottom: 1000 },
            sub_rects: vec![
                RECT { left: 100, top: 150, right: 300, bottom: 950 },  // 左侧导航树
                RECT { left: 300, top: 150, right: 1500, bottom: 950 }, // 文件列表视图 (面积更小更诱人)
            ],
        };
        let z_browser_first = [front_browser, back_explorer];

        // A. 鼠标停在浏览器网页内容区 (500, 400)：虽然此点落在后台文件夹的文件列表中，但绝不可穿透，必须命中前置浏览器整窗
        let hit_browser_body = pick_hover_window_rect(&z_browser_first, &works, 500, 400);
        assert_eq!(
            (hit_browser_body.left, hit_browser_body.top, hit_browser_body.right, hit_browser_body.bottom),
            (200, 50, 1600, 950)
        );

        // B. 鼠标停在浏览器顶部标签栏 (500, 70)：精确命中浏览器的顶部标签栏
        let hit_browser_tab = pick_hover_window_rect(&z_browser_first, &works, 500, 70);
        assert_eq!(
            (hit_browser_tab.left, hit_browser_tab.top, hit_browser_tab.right, hit_browser_tab.bottom),
            (200, 50, 1600, 90)
        );

        // C. 鼠标停在未被浏览器遮挡的文件夹左侧外露区域 (150, 200)：正常命中后台文件夹露出的左侧导航树
        let hit_explorer_tree = pick_hover_window_rect(&z_browser_first, &works, 150, 200);
        assert_eq!(
            (hit_explorer_tree.left, hit_explorer_tree.top, hit_explorer_tree.right, hit_explorer_tree.bottom),
            (100, 150, 300, 950)
        );
    }

    #[test]
    fn test_active_overlay_sel_rect_and_tool_hotkey() {
        let mut ov = OverlayState::default();
        assert!(active_overlay_sel_rect(&ov).is_none());

        ov.hover_win_rc = RECT {
            left: 100,
            top: 200,
            right: 400,
            bottom: 500,
        };
        let rc = active_overlay_sel_rect(&ov).expect("悬停窗口应作为回退选区");
        assert_eq!((rc.left, rc.top, rc.right, rc.bottom), (100, 200, 400, 500));

        ov.has_sel = true;
        ov.sel_rc = RECT {
            left: 50,
            top: 60,
            right: 250,
            bottom: 160,
        };
        let rc2 = active_overlay_sel_rect(&ov).expect("已有选区应优先返回");
        assert_eq!((rc2.left, rc2.top, rc2.right, rc2.bottom), (50, 60, 250, 160));

        assert_eq!(hotkey_to_annot_tool(0x31), AnnotTool::Rect);
        assert_eq!(hotkey_to_annot_tool(0x64), AnnotTool::Arrow);
        assert_eq!(hotkey_to_annot_tool(0x30), AnnotTool::Blur);
        assert_eq!(hotkey_to_annot_tool(0x41), AnnotTool::None);
    }

    #[test]
    fn test_hover_hierarchy_wheel_tab_and_frame_edge() {
        let works = [RECT {
            left: 0,
            top: 0,
            right: 2560,
            bottom: 1400,
        }];
        let folder = WindowCandidate {
            hwnd: 0x2001,
            top_rect: RECT { left: 100, top: 100, right: 1200, bottom: 900 },
            sub_rects: vec![
                RECT { left: 100, top: 100, right: 1200, bottom: 140 }, // 顶部地址/工具栏
                RECT { left: 300, top: 140, right: 1200, bottom: 900 }, // 右侧主面板
                RECT { left: 320, top: 200, right: 900, bottom: 800 },  // 更小的文件列表视图
            ],
        };
        let mut ov = OverlayState {
            work_rects: works.to_vec(),
            win_rects: vec![folder],
            mouse_pt: POINT { x: 500, y: 400 },
            ..Default::default()
        };

        // 1. 初始默认选中最小分区（文件列表视图）
        update_overlay_hover_state(&mut ov, 500, 400, false);
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (320, 200, 900, 800)
        );

        // 2. 向上滚轮一次：放大到右侧主面板
        assert!(step_overlay_hover_level(&mut ov, 1, false));
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (300, 140, 1200, 900)
        );

        // 3. 再向上滚轮一次：放大到整个文件夹窗口，并自动记忆整窗锁定
        assert!(step_overlay_hover_level(&mut ov, 1, false));
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (100, 100, 1200, 900)
        );
        assert!(ov.hover_lock_whole);

        // 4. 在同一文件夹内部移动鼠标到 (520, 420)，依然稳稳锁定整个文件夹窗口
        ov.mouse_pt = POINT { x: 520, y: 420 };
        update_overlay_hover_state(&mut ov, 520, 420, false);
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (100, 100, 1200, 900)
        );

        // 5. 按 Tab 键一键切回最小分区
        assert!(step_overlay_hover_level(&mut ov, 0, true));
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (320, 200, 900, 800)
        );
        assert!(!ov.hover_lock_whole);

        // 6. 再按 Tab 键一键切到整窗
        assert!(step_overlay_hover_level(&mut ov, 0, true));
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (100, 100, 1200, 900)
        );

        // 7. 解除锁定后，按住 Shift 悬停或靠左/右/下边缘 (104, 400) 直接命中整窗；而在顶部工具栏 (500, 115) 仍命中顶部栏
        assert!(step_overlay_hover_level(&mut ov, -2, false));
        update_overlay_hover_state(&mut ov, 500, 400, true);
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (100, 100, 1200, 900)
        );
        update_overlay_hover_state(&mut ov, 104, 400, false);
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (100, 100, 1200, 900)
        );
        update_overlay_hover_state(&mut ov, 500, 115, false);
        assert_eq!(
            (ov.hover_win_rc.left, ov.hover_win_rc.top, ov.hover_win_rc.right, ov.hover_win_rc.bottom),
            (100, 100, 1200, 140)
        );
    }
    #[test]
    fn test_ocr_line_hit_and_rect_pick() {
        let lines = vec![
            OcrLine {
                x: 10.0,
                y: 10.0,
                w: 100.0,
                h: 20.0,
                text: "File".into(),
                need_trans: true,
            },
            OcrLine {
                x: 10.0,
                y: 40.0,
                w: 120.0,
                h: 20.0,
                text: "Settings".into(),
                need_trans: true,
            },
        ];
        // 点命中
        assert_eq!(hit_ocr_line(&lines, 30.0, 45.0), 1);
        assert_eq!(hit_ocr_line(&lines, 500.0, 500.0), -1);
        // 框选两行：按 y 排序
        let rc = RECT {
            left: 0,
            top: 5,
            right: 200,
            bottom: 70,
        };
        assert_eq!(lines_in_img_rect(&lines, rc), vec![0usize, 1usize]);
        // 只框到第二行
        let rc2 = RECT {
            left: 0,
            top: 35,
            right: 200,
            bottom: 70,
        };
        assert_eq!(lines_in_img_rect(&lines, rc2), vec![1usize]);
    }

    #[test]
    fn test_pin_sel_text_picks_zh_or_en() {
        let lines = vec![OcrLine {
            x: 5.0,
            y: 5.0,
            w: 80.0,
            h: 18.0,
            text: "Save As".into(),
            need_trans: true,
        }];
        let blocks = vec![SnipBlock {
            x: 0,
            y: 0,
            w: 120,
            h: 30,
            en: "Save As".into(),
            zh: "另存为".into(),
            skip_translate: false,
        }];
        let mut pin = PinEntry {
            hwnd: null_mut(),
            img_w: 200,
            img_h: 100,
            bgra_orig: Vec::new(),
            bgra_zh: Vec::new(),
            show_zh: false,
            translating: false,
            zh_text: String::new(),
            en_text: String::new(),
            lines: lines.clone(),
            blocks: blocks.clone(),
            sel: PinSel::default(),
            zoom: 1.0,
            alpha: 255,
            hover_trans: false,
            dragging: false,
            drag_off: POINT::default(),
        };
        let (t, is_zh) = pin_sel_text(&pin, &[0]);
        assert_eq!((t.as_str(), is_zh), ("Save As", false));
        // 切到译图态：同一行取块级中文
        pin.show_zh = true;
        pin.bgra_zh = vec![0; 200 * 100 * 4];
        let (t2, is_zh2) = pin_sel_text(&pin, &[0]);
        assert_eq!((t2.as_str(), is_zh2), ("另存为", true));
        // 同一块只取一次（两行同块）
        let (t3, _) = pin_sel_text(&pin, &[0, 0]);
        assert_eq!(t3, "另存为");
    }

}
