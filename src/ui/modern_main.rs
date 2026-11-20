//! 现代无边框圆角主窗口（GDI+ 双缓冲绘制、深浅双色、自绘标题栏与悬浮下拉菜单）

use crate::core::str_util::utf8_to_wide;
use crate::core::{FY_AUTO, FY_EN2ZH, FY_ZH2EN};
use crate::ui::win32_ui::{
    apply_dwm_window_style, apply_richedit_style, argb, ensure_gdiplus, get_window_text,
    gp_draw_round_rect, gp_fill_round_rect, load_app_icon, lparam_xy, rgb, set_window_text,
    wheel_delta, BeginPaint, BitBlt, CallWindowProcW, ClientToScreen, CreateCompatibleBitmap,
    CreateCompatibleDC, CreateFontW, CreateSolidBrush, CreateWindowExW, DefSubclassProc,
    DefWindowProcW, DeleteDC, DeleteObject, DestroyWindow, DrawTextW, EndPaint, FillRect,
    GdipCreateFromHDC, GdipCreatePen1, GdipCreateSolidFill, GdipDeleteBrush, GdipDeleteGraphics,
    GdipDeletePen, GdipDrawLine, GdipFillEllipse, GdipSetSmoothingMode, GetClientRect, GetKeyState,
    GetModuleHandleW, GetWindowRect, InvalidateRect, LoadCursorW, MoveWindow, PostMessageW,
    RegisterClassExW, ScreenToClient, SelectObject, SendMessageW, SetBkMode, SetCapture, SetCursor,
    SetFocus, SetTextColor, SetWindowLongPtrW, SetWindowPos, SetWindowSubclass, ShowWindow,
    TrackMouseEvent, UpdateWindow, CLEARTYPE_QUALITY, COLORREF, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW,
    DEFAULT_CHARSET, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    EC_LEFTMARGIN, EC_RIGHTMARGIN, EM_SETMARGINS, EM_SETSEL, EN_CHANGE, ES_AUTOVSCROLL,
    ES_MULTILINE, ES_READONLY, ES_WANTRETURN, FW_NORMAL, FW_SEMIBOLD, GP_OK,
    GP_SMOOTHING_ANTIALIAS, GP_UNIT_PIXEL, GWLP_USERDATA, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT,
    HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, HWND, HWND_NOTOPMOST,
    HWND_TOP, HWND_TOPMOST, ICON_BIG, ICON_SMALL, IDC_ARROW, LPARAM, LRESULT, MINMAXINFO,
    PAINTSTRUCT, POINT, RECT, SRCCOPY, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_MINIMIZE, SW_RESTORE, SW_SHOW, TME_LEAVE, TRACKMOUSEEVENT, TRANSPARENT, UINT, UINT_PTR,
    VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_SHIFT, WM_ACTIVATE, WM_APP, WM_CAPTURECHANGED, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN,
    WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSELEAVE,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST, WM_NCPAINT, WM_PAINT,
    WM_SETCURSOR, WM_SETFOCUS, WM_SETICON, WM_SIZE, WM_SYSCOMMAND, WNDCLASSEXW, WPARAM, WS_CHILD,
    WS_CLIPCHILDREN, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WS_VISIBLE, WS_VSCROLL,
};
use crate::ui::{
    format_hotkey_text, KIND_NAMES, ROW_HEIGHT_LOGICAL, STYLE_NAMES, WM_APP_COPY_NAMING,
    WM_APP_OPEN_SETTINGS, WM_APP_POPUP_SELECT,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};

pub const MAIN_CLASS_NAME: &str = "KuaiYiNGModernMain";
pub const POPUP_CLASS_NAME: &str = "KuaiYiNGModernPopup";

pub const ID_NONE: i32 = 0;
pub const ID_PIN: i32 = 1;
pub const ID_MIN: i32 = 2;
pub const ID_CLOSE: i32 = 3;
pub const ID_ZH2EN: i32 = 4;
pub const ID_EN2ZH: i32 = 5;
pub const ID_SETTINGS: i32 = 6;
pub const ID_STYLE: i32 = 7;
pub const ID_KIND: i32 = 8;
pub const ID_TRANS: i32 = 9;

const ID_RICH_INPUT: usize = 101;
const ID_RICH_RESULT: usize = 102;

pub const WM_MAIN_CMD_TRANSLATE: UINT = WM_APP + 0x0220;
pub const WM_MAIN_CMD_CLOSE_REQ: UINT = WM_APP + 0x0221;
pub const WM_MAIN_CMD_TOPMOST: UINT = WM_APP + 0x0222;
pub const WM_MAIN_CMD_STYLE_CHANGED: UINT = WM_APP + 0x0223;
pub const WM_MAIN_CMD_KIND_CHANGED: UINT = WM_APP + 0x0224;
pub const WM_MAIN_CMD_INPUT_CHANGED: UINT = WM_APP + 0x0225;

#[derive(Clone, Copy, Default)]
pub struct ModernBtn {
    pub id: i32,
    pub rect: RECT,
    pub label: &'static str,
}

#[derive(Clone, Copy, Default)]
pub struct MainLayout {
    pub win_w: i32,
    pub win_h: i32,
    pub title_rc: RECT,
    pub in_card_rc: RECT,
    pub res_card_rc: RECT,
    pub in_edit_rc: RECT,
    pub res_edit_rc: RECT,
    pub btns: [ModernBtn; 9],
}

#[inline]
pub fn scale_px(v: i32, scale: f64) -> i32 {
    ((v as f64) * scale).round() as i32
}

#[inline]
pub fn modern_title_height(scale: f64) -> i32 {
    scale_px(38, scale).max(34)
}

/// 纯几何主窗口布局计算（独立于 Win32 HWND，支持单元测试断言零重叠与最小边界）
pub fn compute_main_layout(w: i32, h: i32, scale: f64) -> MainLayout {
    let title_h = modern_title_height(scale);
    let row_h = scale_px(ROW_HEIGHT_LOGICAL, scale).max(26);
    let pad_x = scale_px(10, scale);
    let gap = scale_px(8, scale);
    let card_pad_x = scale_px(10, scale);
    let card_pad_y = scale_px(7, scale);

    let title_rc = RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: title_h,
    };

    let tb_w = scale_px(32, scale);
    let tb_h = scale_px(26, scale);
    let tb_y = (title_h - tb_h) / 2;
    let tb_gap = scale_px(4, scale);
    let close_x = w - pad_x - tb_w;
    let min_x = close_x - tb_gap - tb_w;
    let pin_x = min_x - tb_gap - tb_w;

    let mut btns = [ModernBtn::default(); 9];
    btns[0] = ModernBtn {
        id: ID_PIN,
        rect: RECT {
            left: pin_x,
            top: tb_y,
            right: pin_x + tb_w,
            bottom: tb_y + tb_h,
        },
        label: "",
    };
    btns[1] = ModernBtn {
        id: ID_MIN,
        rect: RECT {
            left: min_x,
            top: tb_y,
            right: min_x + tb_w,
            bottom: tb_y + tb_h,
        },
        label: "",
    };
    btns[2] = ModernBtn {
        id: ID_CLOSE,
        rect: RECT {
            left: close_x,
            top: tb_y,
            right: close_x + tb_w,
            bottom: tb_y + tb_h,
        },
        label: "",
    };

    let fixed_h = title_h + gap * 3 + row_h;
    let base_box_h = scale_px(96, scale);
    let extra = (h - (fixed_h + base_box_h * 2)).max(0);
    let in_h = base_box_h + extra / 2;
    let res_h = base_box_h + (extra - extra / 2);

    let card_w = (w - pad_x * 2).max(100);
    let in_top = title_h;
    let in_card_rc = RECT {
        left: pad_x,
        top: in_top,
        right: pad_x + card_w,
        bottom: in_top + in_h,
    };
    let in_edit_rc = RECT {
        left: pad_x + card_pad_x,
        top: in_top + card_pad_y,
        right: pad_x + card_pad_x + (card_w - card_pad_x * 2).max(20),
        bottom: in_top + card_pad_y + (in_h - card_pad_y * 2).max(20),
    };

    let row_y = in_top + in_h + gap;
    let b_gap = scale_px(5, scale);
    let w_zh = scale_px(74, scale);
    let w_en = scale_px(74, scale);
    let w_set = scale_px(54, scale);
    let w_style = scale_px(118, scale);
    let w_kind = scale_px(116, scale);

    let mut x = pad_x;
    btns[3] = ModernBtn {
        id: ID_ZH2EN,
        rect: RECT {
            left: x,
            top: row_y,
            right: x + w_zh,
            bottom: row_y + row_h,
        },
        label: "中 > 英",
    };
    x += w_zh + b_gap;
    btns[4] = ModernBtn {
        id: ID_EN2ZH,
        rect: RECT {
            left: x,
            top: row_y,
            right: x + w_en,
            bottom: row_y + row_h,
        },
        label: "英 > 中",
    };
    x += w_en + b_gap;
    btns[5] = ModernBtn {
        id: ID_SETTINGS,
        rect: RECT {
            left: x,
            top: row_y,
            right: x + w_set,
            bottom: row_y + row_h,
        },
        label: "设置",
    };
    x += w_set + b_gap;
    btns[6] = ModernBtn {
        id: ID_STYLE,
        rect: RECT {
            left: x,
            top: row_y,
            right: x + w_style,
            bottom: row_y + row_h,
        },
        label: "",
    };
    x += w_style + b_gap;
    btns[7] = ModernBtn {
        id: ID_KIND,
        rect: RECT {
            left: x,
            top: row_y,
            right: x + w_kind,
            bottom: row_y + row_h,
        },
        label: "",
    };
    x += w_kind + b_gap;
    let trans_right = w - pad_x;
    btns[8] = ModernBtn {
        id: ID_TRANS,
        rect: RECT {
            left: x,
            top: row_y,
            right: trans_right.max(x + scale_px(52, scale)),
            bottom: row_y + row_h,
        },
        label: "翻译",
    };

    let res_top = row_y + row_h + gap;
    let res_card_rc = RECT {
        left: pad_x,
        top: res_top,
        right: pad_x + card_w,
        bottom: res_top + res_h,
    };
    let res_edit_rc = RECT {
        left: pad_x + card_pad_x,
        top: res_top + card_pad_y,
        right: pad_x + card_pad_x + (card_w - card_pad_x * 2).max(20),
        bottom: res_top + card_pad_y + (res_h - card_pad_y * 2).max(20),
    };

    MainLayout {
        win_w: w,
        win_h: h,
        title_rc,
        in_card_rc,
        res_card_rc,
        in_edit_rc,
        res_edit_rc,
        btns,
    }
}

#[derive(Default)]
struct MainUiState {
    hwnd_main: HWND,
    hwnd_input: HWND,
    hwnd_result: HWND,
    hwnd_pop: HWND,
    pop_kind: i32,
    pop_hover: i32,
    pop_selected: i32,
    pop_items: Vec<&'static str>,

    layout: MainLayout,
    scale: f64,
    is_dark: bool,
    topmost: bool,
    translating: bool,
    input_focused: bool,
    input_dbl_clear: bool,
    style_idx: usize,
    kind_idx: usize,

    title_str: String,
    hint_str: String,

    hover_id: i32,
    down_id: i32,
    tracking_mouse: bool,
}

thread_local! {
    static MAIN_UI: RefCell<MainUiState> = RefCell::new(MainUiState {
        pop_hover: -1,
        pop_selected: -1,
        title_str: "快译NG".to_string(),
        scale: 1.0,
        ..Default::default()
    });
}

#[inline]
fn pt_in_rect(rc: RECT, x: i32, y: i32) -> bool {
    x >= rc.left && x < rc.right && y >= rc.top && y < rc.bottom
}

fn hit_test_btn(layout: &MainLayout, x: i32, y: i32) -> i32 {
    for b in layout.btns {
        if pt_in_rect(b.rect, x, y) {
            return b.id;
        }
    }
    ID_NONE
}

/// 刷新主窗口界面外观与控件字体
pub fn sync_main_theme_and_font(scale: f64, is_dark: bool) {
    let (hwnd_main, hwnd_in, hwnd_res, hwnd_pop) = MAIN_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.scale = scale;
        s.is_dark = is_dark;
        (s.hwnd_main, s.hwnd_input, s.hwnd_result, s.hwnd_pop)
    });
    if !hwnd_main.is_null() {
        apply_dwm_window_style(hwnd_main, is_dark, true);
    }
    let font_px = (17.0 * scale).round().max(11.0) as f32;
    let bg_card = if is_dark {
        rgb(34, 38, 46)
    } else {
        rgb(255, 255, 255)
    };
    let fg_in = if is_dark {
        rgb(240, 244, 250)
    } else {
        rgb(15, 23, 42)
    };
    let fg_res = if is_dark {
        rgb(224, 232, 245)
    } else {
        rgb(30, 41, 59)
    };
    apply_richedit_style(hwnd_in, "Microsoft YaHei UI", font_px, fg_in, bg_card);
    apply_richedit_style(hwnd_res, "Microsoft YaHei UI", font_px, fg_res, bg_card);
    if !hwnd_pop.is_null() {
        unsafe { InvalidateRect(hwnd_pop, null(), 0) };
    }
    repaint_main();
}

/// 更新主窗口重排布局并同步 RichEdit 控件几何位置
pub fn relayout_main_controls(w: i32, h: i32, scale: f64) {
    let (layout, hwnd_in, hwnd_res) = MAIN_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.scale = scale;
        s.layout = compute_main_layout(w, h, scale);
        (s.layout, s.hwnd_input, s.hwnd_result)
    });
    unsafe {
        if !hwnd_in.is_null() {
            let r = layout.in_edit_rc;
            MoveWindow(
                hwnd_in,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                1,
            );
        }
        if !hwnd_res.is_null() {
            let r = layout.res_edit_rc;
            MoveWindow(
                hwnd_res,
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                1,
            );
        }
    }
    repaint_main();
}

/// 请求重绘主窗口
pub fn repaint_main() {
    let hwnd = MAIN_UI.with(|st| st.borrow().hwnd_main);
    if !hwnd.is_null() {
        // SAFETY: 请求异步客户区重绘
        unsafe { InvalidateRect(hwnd, null(), 0) };
    }
}

pub fn set_main_title_and_hint(title: Option<&str>, hint: Option<&str>) {
    MAIN_UI.with(|st| {
        let mut s = st.borrow_mut();
        if let Some(t) = title {
            s.title_str = t.to_string();
        }
        if let Some(h) = hint {
            s.hint_str = h.to_string();
        }
    });
    repaint_main();
}

pub fn set_main_translating(translating: bool) {
    MAIN_UI.with(|st| st.borrow_mut().translating = translating);
    repaint_main();
}

pub fn set_main_topmost(topmost: bool) {
    let hwnd = MAIN_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.topmost = topmost;
        s.hwnd_main
    });
    if !hwnd.is_null() {
        // SAFETY: 调整窗口置顶 Z-Order
        unsafe {
            SetWindowPos(
                hwnd,
                if topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
    }
    repaint_main();
}

pub fn set_main_naming_selection(style_idx: usize, kind_idx: usize) {
    MAIN_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.style_idx = style_idx.min(STYLE_NAMES.len() - 1);
        s.kind_idx = kind_idx.min(KIND_NAMES.len() - 1);
    });
    repaint_main();
}

pub fn set_main_input_dbl_clear(dbl_clear: bool) {
    MAIN_UI.with(|st| st.borrow_mut().input_dbl_clear = dbl_clear);
}

pub fn get_main_hwnd() -> HWND {
    MAIN_UI.with(|st| st.borrow().hwnd_main)
}

pub fn get_input_text() -> String {
    let h = MAIN_UI.with(|st| st.borrow().hwnd_input);
    get_window_text(h)
}

pub fn set_input_text(text: &str) {
    let h = MAIN_UI.with(|st| st.borrow().hwnd_input);
    set_window_text(h, text);
}

pub fn set_result_text(text: &str) {
    let h = MAIN_UI.with(|st| st.borrow().hwnd_result);
    set_window_text(h, text);
}

pub fn get_result_text() -> String {
    let h = MAIN_UI.with(|st| st.borrow().hwnd_result);
    get_window_text(h)
}

pub fn focus_and_select_input() {
    let h = MAIN_UI.with(|st| st.borrow().hwnd_input);
    if !h.is_null() {
        // SAFETY: 聚焦并全选输入区，便于二次输入直接覆盖
        unsafe {
            SetFocus(h);
            SendMessageW(h, EM_SETSEL, 0, -1);
        }
    }
}

/// 生成主窗口快捷键显示提示文案
pub fn hotkey_hint_str(mod_copy: i32, vk_copy: i32) -> String {
    let label = format_hotkey_text(mod_copy, vk_copy);
    if label == "无" {
        "双击输入/结果区复制命名".to_string()
    } else {
        format!("命名复制 {label}")
    }
}

// ---- 自绘圆角悬浮下拉菜单窗口 ----

fn close_popup() {
    let hwnd_pop = MAIN_UI.with(|st| {
        let mut s = st.borrow_mut();
        let h = s.hwnd_pop;
        s.hwnd_pop = null_mut();
        s.pop_kind = ID_NONE;
        h
    });
    if !hwnd_pop.is_null() {
        // SAFETY: 安全销毁现有弹出菜单
        unsafe { DestroyWindow(hwnd_pop) };
    }
    repaint_main();
}

unsafe extern "system" fn popup_wnd_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_MOUSEMOVE => {
            let (_x, y) = lparam_xy(lp);
            let (scale, items_len) = MAIN_UI.with(|st| {
                let s = st.borrow();
                (s.scale, s.pop_items.len())
            });
            let pad_y = scale_px(6, scale);
            let item_h = scale_px(30, scale).max(24);
            let idx = (y - pad_y) / item_h;
            let new_hover = if y >= pad_y && idx >= 0 && (idx as usize) < items_len {
                idx
            } else {
                -1
            };
            let changed = MAIN_UI.with(|st| {
                let mut s = st.borrow_mut();
                if s.pop_hover != new_hover {
                    s.pop_hover = new_hover;
                    true
                } else {
                    false
                }
            });
            if changed {
                InvalidateRect(hwnd, null(), 0);
            }
            let mut tme = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tme);
            0
        }
        WM_MOUSELEAVE => {
            MAIN_UI.with(|st| st.borrow_mut().pop_hover = -1);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONUP => {
            let (_x, y) = lparam_xy(lp);
            let (hwnd_main, pop_kind, scale, len) = MAIN_UI.with(|st| {
                let s = st.borrow();
                (s.hwnd_main, s.pop_kind, s.scale, s.pop_items.len())
            });
            let pad_y = scale_px(6, scale);
            let item_h = scale_px(30, scale).max(24);
            let idx = (y - pad_y) / item_h;
            close_popup();
            if y >= pad_y && idx >= 0 && (idx as usize) < len && !hwnd_main.is_null() {
                PostMessageW(
                    hwnd_main,
                    WM_APP_POPUP_SELECT,
                    pop_kind as WPARAM,
                    idx as LPARAM,
                );
            }
            0
        }
        WM_KEYDOWN => {
            if wp as i32 == VK_ESCAPE {
                close_popup();
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_KILLFOCUS | WM_ACTIVATE => {
            if msg == WM_KILLFOCUS || (msg == WM_ACTIVATE && (wp & 0xFFFF) == 0) {
                close_popup();
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_PAINT => {
            paint_popup(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn paint_popup(hwnd: HWND) {
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

    let (scale, is_dark, items, pop_selected, pop_hover) = MAIN_UI.with(|st| {
        let s = st.borrow();
        (
            s.scale,
            s.is_dark,
            s.pop_items.clone(),
            s.pop_selected,
            s.pop_hover,
        )
    });

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm_buf = CreateCompatibleBitmap(hdc, w, h);
    let old_bm = SelectObject(hdc_buf, hbm_buf);

    let bg = CreateSolidBrush(if is_dark {
        rgb(32, 36, 44)
    } else {
        rgb(255, 255, 255)
    });
    FillRect(hdc_buf, &rc, bg);
    DeleteObject(bg);

    ensure_gdiplus();
    let mut g: *mut c_void = null_mut();
    if GdipCreateFromHDC(hdc_buf, &mut g) == GP_OK && !g.is_null() {
        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);
        let pad_x = scale_px(6, scale);
        let pad_y = scale_px(6, scale);
        let item_h = scale_px(30, scale).max(24);
        let rad = scale_px(6, scale) as f32;

        gp_draw_round_rect(
            g,
            0.0,
            0.0,
            (w - 1) as f32,
            (h - 1) as f32,
            scale_px(8, scale) as f32,
            if is_dark {
                argb(66, 74, 90, 255)
            } else {
                argb(203, 213, 225, 255)
            },
            1.0,
        );

        for (i, _) in items.iter().enumerate() {
            let iy = pad_y + (i as i32) * item_h;
            let sel = pop_selected == i as i32;
            let hov = pop_hover == i as i32;
            if sel {
                gp_fill_round_rect(
                    g,
                    pad_x as f32,
                    iy as f32,
                    (w - pad_x * 2) as f32,
                    item_h as f32,
                    rad,
                    if is_dark {
                        argb(59, 130, 246, 56)
                    } else {
                        argb(59, 130, 246, 30)
                    },
                );
            } else if hov {
                gp_fill_round_rect(
                    g,
                    pad_x as f32,
                    iy as f32,
                    (w - pad_x * 2) as f32,
                    item_h as f32,
                    rad,
                    if is_dark {
                        argb(58, 66, 80, 255)
                    } else {
                        argb(241, 245, 249, 255)
                    },
                );
            }
        }
        GdipDeleteGraphics(g);
    }

    let font_px = (14.0 * scale).round().max(12.0) as i32;
    let wfont = utf8_to_wide("Microsoft YaHei UI");
    let hf = CreateFontW(
        -font_px,
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
        wfont.as_ptr(),
    );
    let old_f = SelectObject(hdc_buf, hf);
    SetBkMode(hdc_buf, TRANSPARENT);
    let pad_x = scale_px(6, scale);
    let pad_y = scale_px(6, scale);
    let item_h = scale_px(30, scale).max(24);
    for (i, text) in items.iter().enumerate() {
        let sel = pop_selected == i as i32;
        let iy = pad_y + (i as i32) * item_h;
        let mut tr = RECT {
            left: pad_x + scale_px(10, scale),
            top: iy,
            right: w - pad_x - scale_px(8, scale),
            bottom: iy + item_h,
        };
        let col = if sel {
            if is_dark {
                rgb(147, 197, 253)
            } else {
                rgb(37, 99, 235)
            }
        } else if is_dark {
            rgb(236, 242, 250)
        } else {
            rgb(30, 41, 59)
        };
        SetTextColor(hdc_buf, col);
        let wt = utf8_to_wide(text);
        DrawTextW(
            hdc_buf,
            wt.as_ptr(),
            -1,
            &mut tr,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }

    SelectObject(hdc_buf, old_f);
    DeleteObject(hf);

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old_bm);
    DeleteObject(hbm_buf);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

fn show_dropdown_popup(kind_id: i32, btn_rc: RECT) {
    let was_same = MAIN_UI.with(|st| {
        let s = st.borrow();
        !s.hwnd_pop.is_null() && s.pop_kind == kind_id
    });
    if was_same {
        close_popup();
        return;
    }
    close_popup();

    let (hwnd_main, scale, items, selected) = MAIN_UI.with(|st| {
        let s = st.borrow();
        let items: Vec<&'static str> = if kind_id == ID_STYLE {
            STYLE_NAMES.to_vec()
        } else {
            KIND_NAMES.to_vec()
        };
        let sel = if kind_id == ID_STYLE {
            s.style_idx as i32
        } else {
            s.kind_idx as i32
        };
        (s.hwnd_main, s.scale, items, sel)
    });
    if hwnd_main.is_null() || items.is_empty() {
        return;
    }

    let pad_y = scale_px(6, scale);
    let item_h = scale_px(30, scale).max(24);
    let pop_w = (btn_rc.right - btn_rc.left).max(scale_px(124, scale));
    let pop_h = pad_y * 2 + (items.len() as i32) * item_h;

    let mut pt = POINT {
        x: btn_rc.left,
        y: btn_rc.bottom + scale_px(4, scale),
    };
    unsafe { ClientToScreen(hwnd_main, &mut pt) };

    let wa = crate::ui::win32_ui::work_area_from_point(pt);
    if pt.y + pop_h > wa.bottom {
        let mut pt_top = POINT {
            x: btn_rc.left,
            y: btn_rc.top - pop_h - scale_px(4, scale),
        };
        unsafe { ClientToScreen(hwnd_main, &mut pt_top) };
        if pt_top.y >= wa.top {
            pt.y = pt_top.y;
        }
    }

    let cls = utf8_to_wide(POPUP_CLASS_NAME);
    let empty = utf8_to_wide("");
    // SAFETY: 主窗为父，顶层工具弹出窗带 DWM 圆角
    let hwnd_pop = unsafe {
        let h_inst = GetModuleHandleW(null());
        CreateWindowExW(
            crate::ui::win32_ui::WS_EX_TOOLWINDOW | crate::ui::win32_ui::WS_EX_TOPMOST,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN,
            pt.x,
            pt.y,
            pop_w,
            pop_h,
            hwnd_main,
            null_mut(),
            h_inst,
            null_mut(),
        )
    };
    if hwnd_pop.is_null() {
        return;
    }

    MAIN_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.hwnd_pop = hwnd_pop;
        s.pop_kind = kind_id;
        s.pop_items = items;
        s.pop_selected = selected;
        s.pop_hover = -1;
    });

    apply_dwm_window_style(hwnd_pop, MAIN_UI.with(|st| st.borrow().is_dark), true);
    unsafe {
        ShowWindow(hwnd_pop, SW_SHOW);
        SetFocus(hwnd_pop);
    }
    repaint_main();
}

// ---- GDI+ 标题栏与主界面绘制 ----

unsafe fn draw_pin_icon(g: *mut c_void, rc: RECT, scale: f64, is_dark: bool, active: bool) {
    let cx = (rc.left + rc.right) as f32 * 0.5;
    let cy = (rc.top + rc.bottom) as f32 * 0.5;
    let col = if active {
        argb(255, 255, 255, 255)
    } else if is_dark {
        argb(170, 182, 200, 255)
    } else {
        argb(100, 116, 139, 255)
    };
    let mut pen: *mut c_void = null_mut();
    if GdipCreatePen1(col, (1.4 * scale).max(1.3) as f32, GP_UNIT_PIXEL, &mut pen) == GP_OK
        && !pen.is_null()
    {
        let u = scale_px(4, scale) as f32;
        GdipDrawLine(
            g,
            pen,
            cx - u * 0.7,
            cy - u * 0.9,
            cx + u * 0.7,
            cy - u * 0.9,
        );
        GdipDrawLine(
            g,
            pen,
            cx - u * 0.5,
            cy - u * 0.9,
            cx - u * 0.5,
            cy + u * 0.1,
        );
        GdipDrawLine(
            g,
            pen,
            cx + u * 0.5,
            cy - u * 0.9,
            cx + u * 0.5,
            cy + u * 0.1,
        );
        GdipDrawLine(
            g,
            pen,
            cx - u * 0.9,
            cy + u * 0.1,
            cx + u * 0.9,
            cy + u * 0.1,
        );
        GdipDrawLine(g, pen, cx, cy + u * 0.1, cx, cy + u * 1.1);
        GdipDeletePen(pen);
    }
}

unsafe fn draw_min_icon(g: *mut c_void, rc: RECT, scale: f64, is_dark: bool) {
    let cx = (rc.left + rc.right) as f32 * 0.5;
    let cy = (rc.top + rc.bottom) as f32 * 0.5;
    let col = if is_dark {
        argb(190, 200, 215, 255)
    } else {
        argb(71, 85, 105, 255)
    };
    let mut pen: *mut c_void = null_mut();
    if GdipCreatePen1(col, (1.5 * scale).max(1.4) as f32, GP_UNIT_PIXEL, &mut pen) == GP_OK
        && !pen.is_null()
    {
        let u = scale_px(5, scale) as f32;
        GdipDrawLine(g, pen, cx - u, cy, cx + u, cy);
        GdipDeletePen(pen);
    }
}

unsafe fn draw_close_icon(g: *mut c_void, rc: RECT, scale: f64, is_dark: bool, hover: bool) {
    let cx = (rc.left + rc.right) as f32 * 0.5;
    let cy = (rc.top + rc.bottom) as f32 * 0.5;
    let col = if hover {
        argb(255, 255, 255, 255)
    } else if is_dark {
        argb(190, 200, 215, 255)
    } else {
        argb(71, 85, 105, 255)
    };
    let mut pen: *mut c_void = null_mut();
    if GdipCreatePen1(col, (1.5 * scale).max(1.4) as f32, GP_UNIT_PIXEL, &mut pen) == GP_OK
        && !pen.is_null()
    {
        let u = scale_px(4, scale) as f32;
        GdipDrawLine(g, pen, cx - u, cy - u, cx + u, cy + u);
        GdipDrawLine(g, pen, cx + u, cy - u, cx - u, cy + u);
        GdipDeletePen(pen);
    }
}

unsafe fn draw_arrow_down(g: *mut c_void, rc: RECT, scale: f64, is_dark: bool) {
    let ax = (rc.right - scale_px(13, scale)) as f32;
    let ay = (rc.top + rc.bottom) as f32 * 0.5 - scale_px(1, scale) as f32;
    let col = if is_dark {
        argb(175, 190, 210, 255)
    } else {
        argb(100, 116, 139, 255)
    };
    let mut pen: *mut c_void = null_mut();
    if GdipCreatePen1(col, (1.4 * scale).max(1.3) as f32, GP_UNIT_PIXEL, &mut pen) == GP_OK
        && !pen.is_null()
    {
        let u = scale_px(3, scale) as f32;
        GdipDrawLine(g, pen, ax - u, ay - u * 0.5, ax, ay + u * 0.6);
        GdipDrawLine(g, pen, ax, ay + u * 0.6, ax + u, ay - u * 0.5);
        GdipDeletePen(pen);
    }
}

unsafe fn paint_main_window(hwnd: HWND) {
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

    let (
        layout,
        scale,
        is_dark,
        topmost,
        translating,
        in_focused,
        hover_id,
        down_id,
        pop_open_kind,
        style_idx,
        kind_idx,
        title_str,
        hint_str,
    ) = MAIN_UI.with(|st| {
        let Ok(s) = st.try_borrow() else {
            return (
                MainLayout::default(),
                1.0,
                true,
                false,
                false,
                false,
                ID_NONE,
                ID_NONE,
                ID_NONE,
                0,
                0,
                String::new(),
                String::new(),
            );
        };
        let pop_k = if !s.hwnd_pop.is_null() {
            s.pop_kind
        } else {
            ID_NONE
        };
        (
            s.layout,
            s.scale,
            s.is_dark,
            s.topmost,
            s.translating,
            s.input_focused,
            s.hover_id,
            s.down_id,
            pop_k,
            s.style_idx,
            s.kind_idx,
            s.title_str.clone(),
            s.hint_str.clone(),
        )
    });

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm_buf = CreateCompatibleBitmap(hdc, w, h);
    let old_bm = SelectObject(hdc_buf, hbm_buf);

    let bg = CreateSolidBrush(if is_dark {
        rgb(24, 26, 32)
    } else {
        rgb(243, 245, 249)
    });
    FillRect(hdc_buf, &rc, bg);
    DeleteObject(bg);

    ensure_gdiplus();
    let mut g: *mut c_void = null_mut();
    if GdipCreateFromHDC(hdc_buf, &mut g) == GP_OK && !g.is_null() {
        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);

        // 微弱外边框
        gp_draw_round_rect(
            g,
            0.0,
            0.0,
            (w - 1) as f32,
            (h - 1) as f32,
            scale_px(8, scale) as f32,
            if is_dark {
                argb(46, 52, 64, 255)
            } else {
                argb(203, 213, 225, 255)
            },
            1.0,
        );

        // 标题栏左侧品牌蓝点
        let mut br_dot: *mut c_void = null_mut();
        if GdipCreateSolidFill(argb(59, 130, 246, 255), &mut br_dot) == GP_OK && !br_dot.is_null() {
            let ds = scale_px(7, scale) as f32;
            let dy = (layout.title_rc.bottom as f32) * 0.5 - ds * 0.5;
            GdipFillEllipse(g, br_dot, scale_px(12, scale) as f32, dy, ds, ds);
            GdipDeleteBrush(br_dot);
        }

        // 右侧 3 个标题栏按钮
        let top_hover = if is_dark {
            argb(54, 60, 72, 255)
        } else {
            argb(226, 232, 240, 255)
        };
        let r_tb = scale_px(5, scale) as f32;
        for b in &layout.btns[0..3] {
            let hov = hover_id == b.id;
            let dwn = down_id == b.id;
            let bw = (b.rect.right - b.rect.left) as f32;
            let bh = (b.rect.bottom - b.rect.top) as f32;
            match b.id {
                ID_PIN => {
                    if topmost {
                        gp_fill_round_rect(
                            g,
                            b.rect.left as f32,
                            b.rect.top as f32,
                            bw,
                            bh,
                            r_tb,
                            if hov {
                                argb(79, 145, 250, 255)
                            } else {
                                argb(59, 130, 246, 255)
                            },
                        );
                    } else if hov || dwn {
                        gp_fill_round_rect(
                            g,
                            b.rect.left as f32,
                            b.rect.top as f32,
                            bw,
                            bh,
                            r_tb,
                            top_hover,
                        );
                    }
                    draw_pin_icon(g, b.rect, scale, is_dark, topmost);
                }
                ID_MIN => {
                    if hov || dwn {
                        gp_fill_round_rect(
                            g,
                            b.rect.left as f32,
                            b.rect.top as f32,
                            bw,
                            bh,
                            r_tb,
                            top_hover,
                        );
                    }
                    draw_min_icon(g, b.rect, scale, is_dark);
                }
                ID_CLOSE => {
                    if hov || dwn {
                        gp_fill_round_rect(
                            g,
                            b.rect.left as f32,
                            b.rect.top as f32,
                            bw,
                            bh,
                            r_tb,
                            argb(200, 62, 62, 255),
                        );
                    }
                    draw_close_icon(g, b.rect, scale, is_dark, hov || dwn);
                }
                _ => {}
            }
        }

        // 输入与结果卡片
        let card_bg = if is_dark {
            argb(34, 38, 46, 255)
        } else {
            argb(255, 255, 255, 255)
        };
        let card_border = if is_dark {
            argb(48, 54, 66, 255)
        } else {
            argb(226, 232, 240, 255)
        };
        let in_border = if in_focused {
            argb(59, 130, 246, 255)
        } else {
            card_border
        };
        let r_card = scale_px(8, scale) as f32;
        gp_fill_round_rect(
            g,
            layout.in_card_rc.left as f32,
            layout.in_card_rc.top as f32,
            (layout.in_card_rc.right - layout.in_card_rc.left) as f32,
            (layout.in_card_rc.bottom - layout.in_card_rc.top) as f32,
            r_card,
            card_bg,
        );
        gp_draw_round_rect(
            g,
            layout.in_card_rc.left as f32,
            layout.in_card_rc.top as f32,
            (layout.in_card_rc.right - layout.in_card_rc.left) as f32,
            (layout.in_card_rc.bottom - layout.in_card_rc.top) as f32,
            r_card,
            in_border,
            1.0,
        );
        gp_fill_round_rect(
            g,
            layout.res_card_rc.left as f32,
            layout.res_card_rc.top as f32,
            (layout.res_card_rc.right - layout.res_card_rc.left) as f32,
            (layout.res_card_rc.bottom - layout.res_card_rc.top) as f32,
            r_card,
            card_bg,
        );
        gp_draw_round_rect(
            g,
            layout.res_card_rc.left as f32,
            layout.res_card_rc.top as f32,
            (layout.res_card_rc.right - layout.res_card_rc.left) as f32,
            (layout.res_card_rc.bottom - layout.res_card_rc.top) as f32,
            r_card,
            card_border,
            1.0,
        );

        // 工具栏 6 个扁平圆角按钮
        let r_btn = scale_px(6, scale) as f32;
        for b in &layout.btns[3..9] {
            let hov = hover_id == b.id;
            let dwn = down_id == b.id;
            let disabled = translating && (b.id == ID_ZH2EN || b.id == ID_EN2ZH);
            let mut fill_col = if is_dark {
                argb(54, 60, 72, 255)
            } else {
                argb(255, 255, 255, 255)
            };
            let mut border_col = if is_dark {
                argb(68, 76, 92, 255)
            } else {
                argb(203, 213, 225, 255)
            };

            if b.id == ID_TRANS {
                fill_col = if translating {
                    if hov {
                        argb(218, 74, 74, 255)
                    } else {
                        argb(200, 62, 62, 255)
                    }
                } else if dwn {
                    argb(42, 108, 220, 255)
                } else if hov {
                    argb(79, 145, 250, 255)
                } else {
                    argb(59, 130, 246, 255)
                };
                border_col = fill_col;
            } else if disabled {
                fill_col = if is_dark {
                    argb(40, 44, 54, 255)
                } else {
                    argb(241, 245, 249, 255)
                };
                border_col = if is_dark {
                    argb(48, 54, 66, 255)
                } else {
                    argb(226, 232, 240, 255)
                };
            } else if (b.id == ID_STYLE || b.id == ID_KIND) && pop_open_kind == b.id {
                fill_col = if is_dark {
                    argb(62, 70, 84, 255)
                } else {
                    argb(239, 246, 255, 255)
                };
                border_col = argb(59, 130, 246, 255);
            } else if dwn {
                fill_col = if is_dark {
                    argb(45, 50, 60, 255)
                } else {
                    argb(226, 232, 240, 255)
                };
            } else if hov {
                fill_col = if is_dark {
                    argb(68, 76, 92, 255)
                } else {
                    argb(241, 245, 249, 255)
                };
                border_col = if is_dark {
                    argb(88, 100, 120, 255)
                } else {
                    argb(148, 163, 184, 255)
                };
            }

            let bw = (b.rect.right - b.rect.left) as f32;
            let bh = (b.rect.bottom - b.rect.top) as f32;
            gp_fill_round_rect(
                g,
                b.rect.left as f32,
                b.rect.top as f32,
                bw,
                bh,
                r_btn,
                fill_col,
            );
            if b.id != ID_TRANS {
                gp_draw_round_rect(
                    g,
                    b.rect.left as f32,
                    b.rect.top as f32,
                    bw,
                    bh,
                    r_btn,
                    border_col,
                    1.0,
                );
            }
            if b.id == ID_STYLE || b.id == ID_KIND {
                draw_arrow_down(g, b.rect, scale, is_dark);
            }
        }
        GdipDeleteGraphics(g);
    }

    // 标题、提示与按钮文本
    let wfont = utf8_to_wide("Microsoft YaHei UI");
    let title_px = (14.0 * scale).round().max(12.0) as i32;
    let hf_title = CreateFontW(
        -title_px,
        0,
        0,
        0,
        FW_SEMIBOLD,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        0,
        0,
        CLEARTYPE_QUALITY,
        0,
        wfont.as_ptr(),
    );
    let old_f = SelectObject(hdc_buf, hf_title);
    SetBkMode(hdc_buf, TRANSPARENT);
    SetTextColor(
        hdc_buf,
        if is_dark {
            rgb(230, 236, 246)
        } else {
            rgb(30, 41, 59)
        },
    );
    let mut tr_title = RECT {
        left: scale_px(24, scale),
        top: 0,
        right: scale_px(110, scale),
        bottom: layout.title_rc.bottom,
    };
    let wtitle = utf8_to_wide(&title_str);
    DrawTextW(
        hdc_buf,
        wtitle.as_ptr(),
        -1,
        &mut tr_title,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    SelectObject(hdc_buf, old_f);
    DeleteObject(hf_title);

    if !hint_str.is_empty() {
        let hint_px = (13.0 * scale).round().max(11.0) as i32;
        let hf_hint = CreateFontW(
            -hint_px,
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
            wfont.as_ptr(),
        );
        let old_h = SelectObject(hdc_buf, hf_hint);
        SetTextColor(
            hdc_buf,
            if is_dark {
                rgb(147, 197, 253)
            } else {
                rgb(37, 99, 235)
            },
        );
        let mut tr_hint = RECT {
            left: scale_px(92, scale),
            top: 0,
            right: layout.btns[0].rect.left - scale_px(8, scale),
            bottom: layout.title_rc.bottom,
        };
        let whint = utf8_to_wide(&hint_str);
        DrawTextW(
            hdc_buf,
            whint.as_ptr(),
            -1,
            &mut tr_hint,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        SelectObject(hdc_buf, old_h);
        DeleteObject(hf_hint);
    }

    let btn_px = (14.0 * scale).round().max(12.0) as i32;
    let hf_btn = CreateFontW(
        -btn_px,
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
        wfont.as_ptr(),
    );
    let old_b = SelectObject(hdc_buf, hf_btn);
    for b in &layout.btns[3..9] {
        let txt = if b.id == ID_TRANS {
            if translating {
                "取消"
            } else {
                "翻译"
            }
        } else if b.id == ID_STYLE {
            STYLE_NAMES
                .get(style_idx)
                .copied()
                .unwrap_or(STYLE_NAMES[0])
        } else if b.id == ID_KIND {
            KIND_NAMES.get(kind_idx).copied().unwrap_or(KIND_NAMES[0])
        } else {
            b.label
        };

        let disabled = translating && (b.id == ID_ZH2EN || b.id == ID_EN2ZH);
        let fg: COLORREF = if disabled {
            if is_dark {
                rgb(120, 128, 142)
            } else {
                rgb(148, 163, 184)
            }
        } else if b.id == ID_TRANS {
            rgb(255, 255, 255)
        } else if is_dark {
            rgb(240, 244, 250)
        } else {
            rgb(30, 41, 59)
        };
        SetTextColor(hdc_buf, fg);

        let mut tr = b.rect;
        let wt = utf8_to_wide(txt);
        if b.id == ID_STYLE || b.id == ID_KIND {
            tr.left += scale_px(10, scale);
            tr.right -= scale_px(20, scale);
            DrawTextW(
                hdc_buf,
                wt.as_ptr(),
                -1,
                &mut tr,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
            );
        } else {
            DrawTextW(
                hdc_buf,
                wt.as_ptr(),
                -1,
                &mut tr,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
        }
    }
    SelectObject(hdc_buf, old_b);
    DeleteObject(hf_btn);

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old_bm);
    DeleteObject(hbm_buf);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

// ---- RichEdit 子类化（焦点环、Enter 翻译、双击命名复制/清空） ----

unsafe extern "system" fn richedit_subclass_proc(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
    _id: UINT_PTR,
    is_result_flag: usize,
) -> LRESULT {
    let is_res = is_result_flag != 0;
    match msg {
        WM_SETFOCUS | WM_KILLFOCUS => {
            if !is_res {
                MAIN_UI.with(|st| st.borrow_mut().input_focused = msg == WM_SETFOCUS);
                repaint_main();
            }
        }
        WM_KEYDOWN => {
            if !is_res && wp as i32 == VK_RETURN {
                let shift = (GetKeyState(VK_SHIFT) as u16 & 0x8000) != 0;
                let ctrl = (GetKeyState(VK_CONTROL) as u16 & 0x8000) != 0;
                if !shift && !ctrl {
                    let hwnd_main = MAIN_UI.with(|st| st.borrow().hwnd_main);
                    if !hwnd_main.is_null() {
                        PostMessageW(hwnd_main, WM_MAIN_CMD_TRANSLATE, FY_AUTO as WPARAM, 0);
                    }
                    return 0;
                }
            }
        }
        WM_LBUTTONDBLCLK => {
            let (hwnd_main, dbl_clear) = MAIN_UI.with(|st| {
                let s = st.borrow();
                (s.hwnd_main, s.input_dbl_clear)
            });
            if !is_res && dbl_clear {
                set_window_text(hwnd, "");
                return 0;
            }
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_APP_COPY_NAMING, if is_res { 1 } else { 0 }, 0);
                return 0;
            }
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe fn frame_hit_test(hwnd: HWND, lp: LPARAM) -> LRESULT {
    let mut rc = RECT::default();
    GetWindowRect(hwnd, &mut rc);
    let (x, y) = lparam_xy(lp);
    let (scale, layout) = MAIN_UI.with(|st| {
        st.try_borrow()
            .map(|s| (s.scale, s.layout))
            .unwrap_or((1.0, MainLayout::default()))
    });
    let b = scale_px(6, scale);
    let left = x < rc.left + b;
    let right = x >= rc.right - b;
    let top = y < rc.top + b;
    let bottom = y >= rc.bottom - b;
    if top && left {
        return HTTOPLEFT;
    }
    if top && right {
        return HTTOPRIGHT;
    }
    if bottom && left {
        return HTBOTTOMLEFT;
    }
    if bottom && right {
        return HTBOTTOMRIGHT;
    }
    if left {
        return HTLEFT;
    }
    if right {
        return HTRIGHT;
    }
    if top {
        return HTTOP;
    }
    if bottom {
        return HTBOTTOM;
    }

    let mut pt = POINT { x, y };
    ScreenToClient(hwnd, &mut pt);
    if pt.y < layout.title_rc.bottom && hit_test_btn(&layout, pt.x, pt.y) == ID_NONE {
        return HTCAPTION;
    }
    HTCLIENT
}

unsafe fn handle_btn_click(hwnd: HWND, id: i32) {
    match id {
        ID_PIN => {
            PostMessageW(hwnd, WM_MAIN_CMD_TOPMOST, 0, 0);
        }
        ID_MIN => {
            ShowWindow(hwnd, SW_MINIMIZE);
        }
        ID_CLOSE => {
            PostMessageW(hwnd, WM_MAIN_CMD_CLOSE_REQ, 0, 0);
        }
        ID_ZH2EN => {
            PostMessageW(hwnd, WM_MAIN_CMD_TRANSLATE, FY_ZH2EN as WPARAM, 0);
        }
        ID_EN2ZH => {
            PostMessageW(hwnd, WM_MAIN_CMD_TRANSLATE, FY_EN2ZH as WPARAM, 0);
        }
        ID_TRANS => {
            PostMessageW(hwnd, WM_MAIN_CMD_TRANSLATE, FY_AUTO as WPARAM, 1);
        }
        ID_SETTINGS => {
            PostMessageW(hwnd, WM_APP_OPEN_SETTINGS, 0, 0);
        }
        ID_STYLE | ID_KIND => {
            let btn_rc = MAIN_UI.with(|st| {
                let s = st.borrow();
                s.layout
                    .btns
                    .iter()
                    .find(|b| b.id == id)
                    .map(|b| b.rect)
                    .unwrap_or_default()
            });
            show_dropdown_popup(id, btn_rc);
        }
        _ => {}
    }
}

unsafe fn handle_wheel_on_dropdown(hwnd: HWND, wp: WPARAM, lp: LPARAM) -> bool {
    let (mut pt_x, mut pt_y) = lparam_xy(lp);
    let mut pt = POINT { x: pt_x, y: pt_y };
    ScreenToClient(hwnd, &mut pt);
    pt_x = pt.x;
    pt_y = pt.y;
    let layout = MAIN_UI.with(|st| st.borrow().layout);
    let hit = hit_test_btn(&layout, pt_x, pt_y);
    if hit != ID_STYLE && hit != ID_KIND {
        return false;
    }
    let delta = wheel_delta(wp);
    if delta == 0 {
        return true;
    }
    let step = if delta > 0 { -1isize } else { 1isize };
    if hit == ID_STYLE {
        let cur = MAIN_UI.with(|st| st.borrow().style_idx as isize);
        let nxt = (cur + step).clamp(0, (STYLE_NAMES.len() - 1) as isize) as usize;
        if nxt as isize != cur {
            MAIN_UI.with(|st| st.borrow_mut().style_idx = nxt);
            PostMessageW(hwnd, WM_MAIN_CMD_STYLE_CHANGED, nxt, 0);
            repaint_main();
        }
    } else {
        let cur = MAIN_UI.with(|st| st.borrow().kind_idx as isize);
        let nxt = (cur + step).clamp(0, (KIND_NAMES.len() - 1) as isize) as usize;
        if nxt as isize != cur {
            MAIN_UI.with(|st| st.borrow_mut().kind_idx = nxt);
            PostMessageW(hwnd, WM_MAIN_CMD_KIND_CHANGED, nxt, 0);
            repaint_main();
        }
    }
    true
}

/// 分派主窗口基础 UI 消息；若属于应用层业务消息返回 `None` 交由主循环宿主处理
///
/// # Safety
/// `hwnd`、`wp`、`lp` 必须来自有效的 Win32 窗口消息回调，且部分消息中 `lp` 指针指向有效结构体。
pub unsafe fn handle_main_ui_message(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
) -> Option<LRESULT> {
    match msg {
        WM_NCCALCSIZE => {
            if wp != 0 {
                return Some(0);
            }
            None
        }
        WM_NCPAINT => Some(0),
        WM_NCACTIVATE => Some(1),
        WM_NCHITTEST => Some(frame_hit_test(hwnd, lp)),
        WM_ERASEBKGND => Some(1),
        WM_DPICHANGED => Some(0),
        WM_GETMINMAXINFO => {
            if lp != 0 {
                let mmi = &mut *(lp as *mut MINMAXINFO);
                mmi.ptMinTrackSize.x = 546;
                mmi.ptMinTrackSize.y = 350;
            }
            Some(0)
        }
        WM_SIZE => {
            let w = (lp as usize & 0xFFFF) as i16 as i32;
            let h = ((lp as usize >> 16) & 0xFFFF) as i16 as i32;
            if w > 0 && h > 0 {
                let sc = MAIN_UI.with(|st| st.borrow().scale);
                relayout_main_controls(w, h, sc);
            }
            Some(0)
        }
        WM_PAINT => {
            paint_main_window(hwnd);
            Some(0)
        }
        WM_SETCURSOR => {
            if (lp & 0xFFFF) as LRESULT == HTCLIENT {
                SetCursor(LoadCursorW(null_mut(), IDC_ARROW));
                return Some(1);
            }
            None
        }
        WM_MOUSEMOVE => {
            let (x, y) = lparam_xy(lp);
            let need_repaint = MAIN_UI.with(|st| {
                let mut s = st.borrow_mut();
                let new_h = hit_test_btn(&s.layout, x, y);
                let ch = s.hover_id != new_h;
                s.hover_id = new_h;
                if !s.tracking_mouse {
                    s.tracking_mouse = true;
                }
                ch
            });
            let mut tme = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tme);
            if need_repaint {
                repaint_main();
            }
            Some(0)
        }
        WM_MOUSELEAVE => {
            MAIN_UI.with(|st| {
                let mut s = st.borrow_mut();
                s.hover_id = ID_NONE;
                s.tracking_mouse = false;
            });
            repaint_main();
            Some(0)
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            let hit = MAIN_UI.with(|st| {
                let mut s = st.borrow_mut();
                let h = hit_test_btn(&s.layout, x, y);
                if h != ID_NONE {
                    s.down_id = h;
                }
                h
            });
            if hit != ID_NONE {
                SetCapture(hwnd);
                repaint_main();
                return Some(0);
            }
            Some(DefWindowProcW(hwnd, msg, wp, lp))
        }
        WM_LBUTTONUP => {
            let (x, y) = lparam_xy(lp);
            let down = MAIN_UI.with(|st| {
                let mut s = st.borrow_mut();
                let d = s.down_id;
                s.down_id = ID_NONE;
                d
            });
            if down != ID_NONE {
                crate::ui::win32_ui::ReleaseCapture();
                let hit = MAIN_UI.with(|st| hit_test_btn(&st.borrow().layout, x, y));
                repaint_main();
                if hit == down {
                    handle_btn_click(hwnd, down);
                }
                return Some(0);
            }
            Some(DefWindowProcW(hwnd, msg, wp, lp))
        }
        WM_CAPTURECHANGED => {
            MAIN_UI.with(|st| st.borrow_mut().down_id = ID_NONE);
            repaint_main();
            Some(0)
        }
        WM_MOUSEWHEEL => {
            if handle_wheel_on_dropdown(hwnd, wp, lp) {
                return Some(0);
            }
            None
        }
        WM_COMMAND => {
            let ctrl_id = wp & 0xFFFF;
            let code = ((wp >> 16) & 0xFFFF) as u16;
            if ctrl_id == ID_RICH_INPUT && code == EN_CHANGE {
                PostMessageW(hwnd, WM_MAIN_CMD_INPUT_CHANGED, 0, 0);
                return Some(0);
            }
            None
        }
        _ => None,
    }
}

/// 注册主窗体和弹出菜单窗口类，并创建主窗口及其 RichEdit 输入/结果子控件
pub fn create_modern_main_window(
    wnd_proc: crate::ui::win32_ui::WNDPROC,
    title: &str,
    w: i32,
    h: i32,
    scale: f64,
    is_dark: bool,
    topmost: bool,
) -> HWND {
    unsafe {
        let h_inst = GetModuleHandleW(null());
        let icon_big = load_app_icon(32, 32);
        let icon_sm = load_app_icon(16, 16);

        let cls_main = utf8_to_wide(MAIN_CLASS_NAME);
        let wc_main = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_inst,
            hIcon: icon_big,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: cls_main.as_ptr(),
            hIconSm: icon_sm,
        };
        RegisterClassExW(&wc_main);

        let cls_pop = utf8_to_wide(POPUP_CLASS_NAME);
        let wc_pop = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(popup_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_inst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: cls_pop.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&wc_pop);

        let wa = crate::ui::win32_ui::primary_work_area();
        let x = wa.left + ((wa.right - wa.left) - w).max(0) / 2;
        let y = wa.top + ((wa.bottom - wa.top) - h).max(0) / 2;

        let wtitle = utf8_to_wide(title);
        let hwnd = CreateWindowExW(
            if topmost {
                crate::ui::win32_ui::WS_EX_TOPMOST
            } else {
                0
            },
            cls_main.as_ptr(),
            wtitle.as_ptr(),
            WS_POPUP | WS_THICKFRAME | WS_MINIMIZEBOX | WS_SYSMENU | WS_CLIPCHILDREN,
            x,
            y,
            w,
            h,
            null_mut(),
            null_mut(),
            h_inst,
            null_mut(),
        );
        if hwnd.is_null() {
            return null_mut();
        }
        if !icon_big.is_null() {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG, icon_big as LPARAM);
        }
        if !icon_sm.is_null() {
            SendMessageW(hwnd, WM_SETICON, ICON_SMALL, icon_sm as LPARAM);
        }

        // 创建两个无边框 RichEdit 控件（输入区 + 只读结果区）
        let cls_rich = utf8_to_wide("RICHEDIT50W");
        let empty = utf8_to_wide("");
        let style_in =
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_WANTRETURN;
        let hwnd_in = CreateWindowExW(
            0,
            cls_rich.as_ptr(),
            empty.as_ptr(),
            style_in,
            0,
            0,
            100,
            80,
            hwnd,
            ID_RICH_INPUT as _,
            h_inst,
            null_mut(),
        );
        let style_res =
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY;
        let hwnd_res = CreateWindowExW(
            0,
            cls_rich.as_ptr(),
            empty.as_ptr(),
            style_res,
            0,
            0,
            100,
            80,
            hwnd,
            ID_RICH_RESULT as _,
            h_inst,
            null_mut(),
        );

        if !hwnd_in.is_null() {
            SendMessageW(
                hwnd_in,
                EM_SETMARGINS,
                EC_LEFTMARGIN | EC_RIGHTMARGIN,
                (4 << 16) | 4,
            );
            SetWindowSubclass(hwnd_in, richedit_subclass_proc, 0x5249, 0);
        }
        if !hwnd_res.is_null() {
            SendMessageW(
                hwnd_res,
                EM_SETMARGINS,
                EC_LEFTMARGIN | EC_RIGHTMARGIN,
                (4 << 16) | 4,
            );
            SetWindowSubclass(hwnd_res, richedit_subclass_proc, 0x524A, 1);
        }

        MAIN_UI.with(|st| {
            let mut s = st.borrow_mut();
            s.hwnd_main = hwnd;
            s.hwnd_input = hwnd_in;
            s.hwnd_result = hwnd_res;
            s.scale = scale;
            s.is_dark = is_dark;
            s.topmost = topmost;
            s.title_str = title.to_string();
        });

        // 触发 WM_NCCALCSIZE 并建立 DWM 圆角
        SetWindowPos(
            hwnd,
            if topmost { HWND_TOPMOST } else { HWND_TOP },
            x,
            y,
            w,
            h,
            SWP_FRAMECHANGED | SWP_NOACTIVATE,
        );
        sync_main_theme_and_font(scale, is_dark);
        relayout_main_controls(w, h, scale);
        let _ = (
            CALL_KEEP,
            WM_CLOSE,
            WM_CREATE,
            WM_DESTROY,
            WM_MOUSEACTIVATE,
            WM_SYSCOMMAND,
            SW_RESTORE,
            SetWindowLongPtrW,
            GWLP_USERDATA,
            CallWindowProcW,
            UpdateWindow,
        );
        hwnd
    }
}

const CALL_KEEP: usize = 0;

#[cfg(test)]
mod tests {
    use super::*;

    fn rects_overlap(a: RECT, b: RECT) -> bool {
        a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
    }

    #[test]
    fn test_main_layout_no_overlap_across_scales() {
        for pct in [75, 100, 125, 150, 200] {
            let sc = (pct as f64) / 100.0;
            let (w, h) = crate::ui::compute_def_win_size(pct);
            let lay = compute_main_layout(w, h, sc);
            assert!(lay.in_card_rc.top >= lay.title_rc.bottom);
            assert!(lay.res_card_rc.bottom <= h);
            assert!(lay.btns[8].rect.right <= w);
            assert!(!rects_overlap(lay.in_card_rc, lay.res_card_rc));
            for i in 0..lay.btns.len() {
                for j in (i + 1)..lay.btns.len() {
                    assert!(
                        !rects_overlap(lay.btns[i].rect, lay.btns[j].rect),
                        "按钮 {i} 与 {j} 在 {pct}% 下重叠"
                    );
                }
            }
            for b in &lay.btns[3..9] {
                assert!(!rects_overlap(b.rect, lay.in_card_rc));
                assert!(!rects_overlap(b.rect, lay.res_card_rc));
            }
        }
    }
}
