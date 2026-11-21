//! 截屏体系内的轻量浮层提示：右上角胶囊 toast 与锚点气泡 bubble
//!
//! 设计要点（改动前先读）：
//! - 一个进程内复用**一个**分层窗口，toast 与 bubble 共用同一套绘制/动画，新的直接覆盖旧的
//! - `WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE`：不抢焦点、可点穿，
//!   淡入淡出只改 `SetLayeredWindowAttributes` 的 alpha，整窗一致
//! - 圆角用 `SetWindowRgn(CreateRoundRectRgn)` 裁形状，避免四角露底色
//! - 动画由 16ms `SetTimer` 驱动：淡入 140ms → 停留（toast 2.1s / bubble 3.4s）→ 淡出 420ms
//! - 绝不弹模态框：OCR/翻译失败一律走这里

use crate::core::str_util::utf8_to_wide;
use crate::core::win32::GetTickCount64;
use crate::ui::win32_ui::{
    argb, ensure_gdiplus, gp_draw_text, gp_fill_round_rect, gp_round_card, rgb,
    work_area_from_point, BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
    CreateFontW, CreateRoundRectRgn, CreateSolidBrush, CreateWindowExW, DefWindowProcW, DeleteDC,
    DeleteObject, DrawTextW, EndPaint, FillRect, GdipCreateFromHDC, GdipDeleteGraphics,
    GdipSetSmoothingMode, GetClientRect, GetModuleHandleW, GetWindowRect, HWND, InvalidateRect,
    KillTimer, LoadCursorW, POINT, RegisterClassExW, SelectObject, SetWindowRgn, SetWindowPos,
    SetTimer, ShowWindow, CLEARTYPE_QUALITY, CS_HREDRAW,
    CS_VREDRAW, DEFAULT_CHARSET, DT_CALCRECT, DT_LEFT, DT_WORDBREAK,
    FW_MEDIUM, GP_STRING_ALIGN_CENTER, GP_STRING_ALIGN_NEAR, GP_OK, GP_SMOOTHING_ANTIALIAS,
    GpRectF, HWND_TOPMOST, IDC_ARROW, LPARAM, LRESULT, LWA_ALPHA, PAINTSTRUCT, RECT, SRCCOPY,
    SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, UINT, UINT_PTR, WNDCLASSEXW,
    WPARAM, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
    WS_POPUP,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

const TOAST_CLASS_NAME: &str = "KuaiYiSnipToast";
const TOAST_TIMER: UINT_PTR = 0x5347;

const WM_TIMER_ID: UINT = 0x0113;
const WM_ERASEBKGND_ID: UINT = 0x0014;

const FADE_IN_MS: u64 = 140;
const FADE_OUT_MS: u64 = 420;
const HOLD_TOAST_MS: u64 = 2100;
const HOLD_BUBBLE_MS: u64 = 3400;
const PEAK_ALPHA: u8 = 248;

const PAD_X: i32 = 12;
const PAD_Y: i32 = 6;
const ACCENT_W: i32 = 3;
const RADIUS: i32 = 9;
const MAX_W: i32 = 360;
const FONT_PX: i32 = 12;

const COL_BG: (u8, u8, u8) = (24, 27, 34);
const COL_BORDER: (u8, u8, u8) = (64, 72, 88);
const COL_TEXT: (u8, u8, u8) = (240, 244, 250);
const COL_DIM: (u8, u8, u8) = (148, 163, 184);

/// 提示语气：只影响左侧竖条颜色
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Warn,
    Error,
}

impl Tone {
    fn accent(self) -> (u8, u8, u8) {
        match self {
            Tone::Info => (59, 130, 246),
            Tone::Warn => (245, 158, 11),
            Tone::Error => (239, 68, 68),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// 右上角胶囊，跟随锚点窗口
    Toast,
    /// 锚在屏幕坐标矩形上的气泡（划选翻译结果）
    Bubble,
}

struct ToastState {
    hwnd: HWND,
    registered: bool,
    kind: Kind,
    tone: Tone,
    /// 多行文本：两行以上时首行用弱色（原文），其余高亮（译文）
    lines: Vec<String>,
    /// 每行折行后的高度
    line_hs: Vec<i32>,
    anchor: HWND,
    /// bubble 的屏幕锚点矩形
    anchor_rc: RECT,
    w: i32,
    h: i32,
    alpha: u8,
    t0: u64,
    hold_ms: u64,
}

impl Default for ToastState {
    fn default() -> Self {
        Self {
            hwnd: null_mut(),
            registered: false,
            kind: Kind::Toast,
            tone: Tone::Info,
            lines: Vec::new(),
            line_hs: Vec::new(),
            anchor: null_mut(),
            anchor_rc: RECT::default(),
            w: 0,
            h: 0,
            alpha: 0,
            t0: 0,
            hold_ms: HOLD_TOAST_MS,
        }
    }
}

thread_local! {
    static TOAST: RefCell<ToastState> = RefCell::new(ToastState::default());
}

unsafe fn make_font(px: i32) -> *mut c_void {
    let wf = utf8_to_wide("Microsoft YaHei UI");
    CreateFontW(
        -px,
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
    )
}

/// 按 `MAX_W` 折行并量出每行高度与最宽行宽
unsafe fn measure(lines: &[String]) -> (i32, Vec<i32>) {
    let max_text_w = MAX_W - ACCENT_W - PAD_X * 2;
    let hdc = CreateCompatibleDC(null_mut());
    if hdc.is_null() {
        return (max_text_w, vec![16; lines.len()]);
    }
    let font = make_font(FONT_PX);
    let old = SelectObject(hdc, font);
    let mut widest = 0i32;
    let mut hs = Vec::with_capacity(lines.len());
    for ln in lines {
        let wl = utf8_to_wide(ln);
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: max_text_w,
            bottom: 0,
        };
        DrawTextW(
            hdc,
            wl.as_ptr(),
            -1,
            &mut rc,
            DT_CALCRECT | DT_LEFT | DT_WORDBREAK,
        );
        widest = widest.max(rc.right - rc.left);
        hs.push((rc.bottom - rc.top).max(15));
    }
    SelectObject(hdc, old);
    DeleteObject(font);
    DeleteDC(hdc);
    (widest.clamp(24, max_text_w), hs)
}

unsafe fn ensure_class() {
    if TOAST.with(|t| t.borrow().registered) {
        return;
    }
    let h_inst = GetModuleHandleW(null());
    let cls = utf8_to_wide(TOAST_CLASS_NAME);
    let mut wc: WNDCLASSEXW = zeroed();
    wc.cbSize = size_of::<WNDCLASSEXW>() as u32;
    wc.style = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc = Some(toast_wnd_proc);
    wc.hInstance = h_inst;
    wc.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
    wc.lpszClassName = cls.as_ptr();
    RegisterClassExW(&wc);
    TOAST.with(|t| t.borrow_mut().registered = true);
}

unsafe fn ensure_window() -> HWND {
    let existing = TOAST.with(|t| t.borrow().hwnd);
    if !existing.is_null() {
        return existing;
    }
    ensure_class();
    let h_inst = GetModuleHandleW(null());
    let cls = utf8_to_wide(TOAST_CLASS_NAME);
    let hwnd = CreateWindowExW(
        WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_LAYERED
            | WS_EX_TRANSPARENT
            | WS_EX_NOACTIVATE,
        cls.as_ptr(),
        null(),
        WS_POPUP,
        0,
        0,
        10,
        10,
        null_mut(),
        null_mut(),
        h_inst,
        null_mut(),
    );
    TOAST.with(|t| t.borrow_mut().hwnd = hwnd);
    hwnd
}

/// 计算并应用窗口位置（屏幕坐标），同时重设圆角裁形区域
unsafe fn place(st: &mut ToastState) {
    let (w, h) = (st.w, st.h);
    let hwnd = st.hwnd;
    if hwnd.is_null() || w <= 0 || h <= 0 {
        return;
    }
    let (mut x, mut y) = match st.kind {
        Kind::Toast => {
            if st.anchor.is_null() {
                return;
            }
            let mut wr = RECT::default();
            if GetWindowRect(st.anchor, &mut wr) == 0 {
                return;
            }
            if wr.right - wr.left < w + 24 {
                // 贴图太窄，放右上角会整块盖住它，改到下方
                (wr.left, wr.bottom + 8)
            } else {
                (wr.right - w - 10, wr.top + 10)
            }
        }
        Kind::Bubble => {
            let rc = st.anchor_rc;
            let below = rc.bottom + 8;
            let wa = work_area_from_point(POINT { x: rc.left, y: below });
            if below + h > wa.bottom - 6 {
                (rc.left, (rc.top - h - 8).max(wa.top + 6))
            } else {
                (rc.left, below)
            }
        }
    };
    let wa = work_area_from_point(POINT { x, y });
    x = x.clamp(wa.left + 4, (wa.right - w - 4).max(wa.left + 4));
    y = y.clamp(wa.top + 4, (wa.bottom - h - 4).max(wa.top + 4));
    SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        y,
        w,
        h,
        SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    // 圆角裁形：region 交给系统接管，旧 region 自动释放
    let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, RADIUS * 2, RADIUS * 2);
    if !rgn.is_null() {
        SetWindowRgn(hwnd, rgn, 1);
    }
}

/// 推进一帧：算 alpha，结束后自毁
unsafe fn tick() {
    let hwnd = TOAST.with(|t| {
        let mut st = t.borrow_mut();
        if st.hwnd.is_null() {
            return null_mut();
        }
        let elapsed = GetTickCount64().saturating_sub(st.t0);
        let out_start = FADE_IN_MS + st.hold_ms;
        let total = out_start + FADE_OUT_MS;
        let a = if elapsed < FADE_IN_MS {
            ((elapsed as f64 / FADE_IN_MS as f64) * PEAK_ALPHA as f64) as u8
        } else if elapsed < out_start {
            PEAK_ALPHA
        } else if elapsed < total {
            (((total - elapsed) as f64 / FADE_OUT_MS as f64) * PEAK_ALPHA as f64) as u8
        } else {
            0
        };
        st.alpha = a;
        if elapsed >= total {
            st.lines.clear();
            st.line_hs.clear();
            st.anchor = null_mut();
            ShowWindow(st.hwnd, SW_HIDE);
            KillTimer(st.hwnd, TOAST_TIMER);
            return null_mut();
        }
        st.hwnd
    });
    if !hwnd.is_null() {
        let a = TOAST.with(|t| t.borrow().alpha);
        crate::ui::win32_ui::SetLayeredWindowAttributes(hwnd, 0, a, LWA_ALPHA);
    }
}

unsafe extern "system" fn toast_wnd_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    const WM_PAINT_ID: UINT = 0x000F;
    match msg {
        WM_PAINT_ID => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND_ID => 1,
        WM_TIMER_ID => {
            tick();
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let mut rc = RECT::default();
    GetClientRect(hwnd, &mut rc);
    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
    let (tone, lines, line_hs) = TOAST.with(|t| {
        let st = t.borrow();
        (st.tone, st.lines.clone(), st.line_hs.clone())
    });
    if w <= 0 || h <= 0 || lines.is_empty() {
        EndPaint(hwnd, &ps);
        return;
    }

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm = CreateCompatibleBitmap(hdc, w, h);
    let old = SelectObject(hdc_buf, hbm);
    let bg = CreateSolidBrush(rgb(COL_BG.0, COL_BG.1, COL_BG.2));
    FillRect(hdc_buf, &rc, bg);
    DeleteObject(bg);

    if ensure_gdiplus() {
        let mut g: *mut c_void = null_mut();
        if GdipCreateFromHDC(hdc_buf, &mut g) == GP_OK && !g.is_null() {
            GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);
            let fw = w as f32;
            let fh = h as f32;
            gp_round_card(
                g,
                fw / 2.0,
                fh / 2.0,
                fw,
                fh,
                RADIUS as f32,
                argb(COL_BG.0, COL_BG.1, COL_BG.2, 255),
                argb(COL_BORDER.0, COL_BORDER.1, COL_BORDER.2, 255),
                1.0,
            );
            let (ar, ag, ab) = tone.accent();
            gp_fill_round_rect(
                g,
                ACCENT_W as f32 / 2.0,
                fh / 2.0,
                ACCENT_W as f32,
                fh,
                ACCENT_W as f32 / 2.0,
                argb(ar, ag, ab, 255),
            );
            let mut ty = PAD_Y;
            for (i, ln) in lines.iter().enumerate() {
                let lh = line_hs.get(i).copied().unwrap_or(16);
                let (tr, tg, tb) = if i == 0 && lines.len() > 1 {
                    COL_DIM
                } else {
                    COL_TEXT
                };
                let grc = GpRectF {
                    x: (ACCENT_W + PAD_X) as f32,
                    y: ty as f32,
                    width: (w - ACCENT_W - PAD_X * 2) as f32,
                    height: lh as f32,
                };
                gp_draw_text(
                    g,
                    ln,
                    &grc,
                    "Microsoft YaHei UI",
                    FONT_PX as f32,
                    false,
                    argb(tr, tg, tb, 255),
                    GP_STRING_ALIGN_NEAR,
                    GP_STRING_ALIGN_CENTER,
                );
                ty += lh;
            }
            GdipDeleteGraphics(g);
        }
    } else {
        // GDI+ 不可用的兜底：PAD 内直接用 GDI 文本
        let font = make_font(FONT_PX);
        let old_f = SelectObject(hdc_buf, font);
        crate::ui::win32_ui::SetBkMode(hdc_buf, crate::ui::win32_ui::TRANSPARENT);
        crate::ui::win32_ui::SetTextColor(
            hdc_buf,
            rgb(COL_TEXT.0, COL_TEXT.1, COL_TEXT.2),
        );
        let mut ty = PAD_Y;
        for (i, ln) in lines.iter().enumerate() {
            let lh = line_hs.get(i).copied().unwrap_or(16);
            let wl = utf8_to_wide(ln);
            let mut drc = RECT {
                left: ACCENT_W + PAD_X,
                top: ty,
                right: w - PAD_X,
                bottom: ty + lh,
            };
            DrawTextW(
                hdc_buf,
                wl.as_ptr(),
                -1,
                &mut drc,
                DT_LEFT | DT_WORDBREAK,
            );
            ty += lh;
        }
        let _ = i32::default();
        SelectObject(hdc_buf, old_f);
        DeleteObject(font);
    }

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old);
    DeleteObject(hbm);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

/// 显示/刷新右上角胶囊提示
///
/// `anchor` 为目标窗口（结果窗/贴图窗），提示固定在它的客户区右上角。
pub fn show_toast(anchor: HWND, text: &str, tone: Tone) {
    show(
        anchor,
        Kind::Toast,
        vec![text.to_string()],
        tone,
        RECT::default(),
    );
}

/// 在屏幕矩形下方（放不下则上方）显示气泡；`lines[0]` 用弱色，其余高亮
pub fn show_bubble(anchor_rc: RECT, lines: Vec<String>, tone: Tone) {
    show(null_mut(), Kind::Bubble, lines, tone, anchor_rc);
}

fn show(anchor: HWND, kind: Kind, lines: Vec<String>, tone: Tone, anchor_rc: RECT) {
    if lines.is_empty() {
        return;
    }
    unsafe {
        let hwnd = ensure_window();
        if hwnd.is_null() {
            return;
        }
        let (widest, line_hs) = measure(&lines);
        let w = (widest + ACCENT_W + PAD_X * 2).clamp(56, MAX_W);
        let h = (line_hs.iter().sum::<i32>() + PAD_Y * 2).clamp(26, 140);
        TOAST.with(|t| {
            let mut st = t.borrow_mut();
            st.kind = kind;
            st.tone = tone;
            st.lines = lines;
            st.line_hs = line_hs;
            st.anchor = anchor;
            st.anchor_rc = anchor_rc;
            st.w = w;
            st.h = h;
            st.alpha = 0;
            st.t0 = GetTickCount64();
            st.hold_ms = if kind == Kind::Bubble {
                HOLD_BUBBLE_MS
            } else {
                HOLD_TOAST_MS
            };
        });
        TOAST.with(|t| {
            let mut st = t.borrow_mut();
            place(&mut st);
        });
        crate::ui::win32_ui::SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
        ShowWindow(hwnd, SW_SHOW);
        SetTimer(hwnd, TOAST_TIMER, 16, null());
        InvalidateRect(hwnd, null(), 0);
    }
}

/// 目标窗口移动/缩放后重算位置（贴图拖动时调用）
pub fn follow(anchor: HWND) {
    unsafe {
        let active = TOAST.with(|t| {
            let st = t.borrow();
            !st.hwnd.is_null() && st.kind == Kind::Toast && st.anchor == anchor && !st.lines.is_empty()
        });
        if !active {
            return;
        }
        TOAST.with(|t| {
            let mut st = t.borrow_mut();
            place(&mut st);
        });
    }
}

/// 立刻隐藏（窗口关闭、开始新操作时调用）
pub fn hide_all() {
    unsafe {
        let hwnd = TOAST.with(|t| {
            let mut st = t.borrow_mut();
            st.lines.clear();
            st.line_hs.clear();
            st.anchor = null_mut();
            st.hwnd
        });
        if !hwnd.is_null() {
            KillTimer(hwnd, TOAST_TIMER);
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_accents_are_distinct() {
        assert_ne!(Tone::Info.accent(), Tone::Warn.accent());
        assert_ne!(Tone::Warn.accent(), Tone::Error.accent());
        assert_ne!(Tone::Info.accent(), Tone::Error.accent());
    }

    #[test]
    fn accent_order_is_bright() {
        // Info 蓝、Warn 琥珀、Error 红：用于区分语气
        let (r, g, b) = Tone::Error.accent();
        assert!(r > g && r > b);
        let (r, g, b) = Tone::Warn.accent();
        assert!(r > b && g > b);
    }
}