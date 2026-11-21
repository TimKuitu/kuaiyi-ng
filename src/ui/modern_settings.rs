//! 现代化 Snipaste 风格设置窗口（7 大导航分类、12 个子页面、GDI+ 自绘与动效）

use crate::core::str_util::utf8_to_wide;
use crate::ui::dict_list::{DictKind, DictListView};
use crate::ui::win32_ui::{
    apply_dwm_window_style, apply_font_to_children, apply_uxtheme_dark_app_mode, argb,
    attach_tooltip, browse_for_folder, current_exe_path, ensure_gdiplus, get_window_text,
    gp_draw_glyph, gp_fill_round_rect, gp_round_card, load_app_icon, lparam_xy, rgb,
    set_control_theme, set_window_text, shell_open, wheel_delta, BeginPaint, BitBlt,
    CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush,
    CreateWindowExW, DefSubclassProc, DefWindowProcW, DeleteDC, DeleteObject, DrawTextW, EndPaint,
    FillRect, GdipCreateFromHDC, GdipCreatePen1, GdipCreateSolidFill, GdipDeleteBrush,
    GdipDeleteGraphics, GdipDeletePen, GdipDrawLine, GdipSetSmoothingMode, GetAsyncKeyState,
    GetClientRect, GetTickCount64, GetWindowRect, GpRectF, InvalidateRect, KillTimer, LoadCursorW,
    MoveWindow, PostMessageW, RegisterClassExW, ReleaseCapture, ScreenToClient, SelectObject,
    SendMessageW, SetBkColor, SetBkMode, SetCapture, SetTextColor, SetTimer, SetWindowPos,
    SetWindowSubclass, ShowWindow, TrackMouseEvent, UpdateWindow, CBN_SELCHANGE, CBS_DROPDOWNLIST,
    CBS_HASSTRINGS, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CB_SETITEMHEIGHT, CLEARTYPE_QUALITY,
    COLORREF, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, DEFAULT_CHARSET, DT_CENTER, DT_END_ELLIPSIS,
    DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DWORD, DWORD_PTR, EC_LEFTMARGIN,
    EC_RIGHTMARGIN, EM_SETMARGINS, EM_SETPASSWORDCHAR, EN_CHANGE, ES_AUTOHSCROLL, ES_NUMBER,
    ES_PASSWORD, ES_READONLY, FW_BOLD, FW_MEDIUM, FW_NORMAL, FW_SEMIBOLD, GP_OK,
    GP_SMOOTHING_ANTIALIAS, GP_UNIT_PIXEL, HBRUSH, HDC, HFONT, HTCAPTION, HTCLIENT, HWND, HWND_TOP,
    ICON_BIG, ICON_SMALL, IDC_ARROW, LPARAM, LRESULT, MINMAXINFO, PAINTSTRUCT, POINT, PS_SOLID,
    RECT, SRCCOPY, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE,
    SW_MINIMIZE, SW_RESTORE, SW_SHOW, TME_LEAVE, TRACKMOUSEEVENT, TRANSPARENT, TTM_ACTIVATE,
    TTM_SETMAXTIPWIDTH, UINT, UINT_PTR, VK_BACK, VK_CONTROL, VK_DELETE, VK_ESCAPE, VK_LWIN,
    VK_MENU, VK_RWIN, VK_SHIFT, VK_TAB, WM_APP, WM_CLOSE, WM_COMMAND, WM_CTLCOLOREDIT,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSELEAVE, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_NCACTIVATE, WM_NCCALCSIZE, WM_NCHITTEST, WM_NCPAINT, WM_PAINT, WM_SETFOCUS,
    WM_SETICON, WM_SHOWWINDOW, WM_SYSKEYDOWN, WM_TIMER, WM_VSCROLL, WNDCLASSEXW, WPARAM,
    WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_EX_TOOLWINDOW, WS_MINIMIZEBOX,
    WS_POPUP, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE,
};
use crate::ui::{
    compute_def_win_size, format_hotkey_text, AppSettings, AI_PRESETS, APP_REPO_URL, APP_VERSION,
    THEME_PRESETS, UI_SCALE_PRESETS,
};
use std::cell::RefCell;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

pub const SETTINGS_CLASS_NAME: &str = "KuaiYiNGModernSettings";
pub const PAGE_CLASS_NAME: &str = "KuaiYiNGSettingsPage";

// 逻辑尺寸常数（100% 缩放基准）
pub const SET_WIN_W: i32 = 740;
pub const SET_WIN_H: i32 = 590;
pub const SET_TITLE_H: i32 = 40;
pub const SET_NAV_W: i32 = 156;
pub const SET_SUB_BAR_H: i32 = 44;
pub const SET_PAGE_X: i32 = SET_NAV_W;
pub const SET_PAGE_Y: i32 = SET_TITLE_H + SET_SUB_BAR_H; // 84
pub const SET_PAGE_W: i32 = SET_WIN_W - SET_NAV_W; // 584
pub const SET_PAGE_H: i32 = SET_WIN_H - SET_PAGE_Y; // 506
pub const SET_PAD_L: i32 = 20;
pub const SET_CONT_W: i32 = SET_PAGE_W - SET_PAD_L * 2; // 544
pub const CARD_PAD_X: i32 = 16;
pub const INNER_W: i32 = SET_CONT_W - CARD_PAD_X * 2; // 512

const ANIM_TIMER_ID: UINT_PTR = 0x4D99;
const DEBOUNCE_TIMER_ID: UINT_PTR = 0x4DA0;
const TOAST_TIMER_ID: UINT_PTR = 0x4DA1;

// 设置窗口向主窗口派发的业务同步消息
pub const WM_SET_APPLY_SETTINGS: UINT = WM_APP + 0x0240;
pub const WM_SET_APPLY_SCALE_OR_THEME: UINT = WM_APP + 0x0241; // wp: 0=theme, 1=scale, 2=winsize_reset, 3=winsize_apply
pub const WM_SET_APPLY_HOTKEYS: UINT = WM_APP + 0x0242; // lp=1 来自截屏页同步
pub const WM_SET_RESET_HOTKEYS: UINT = WM_APP + 0x0243;
pub const WM_SET_CHECK_UPDATE: UINT = WM_APP + 0x0244;
pub const WM_SET_SNIP_CMD: UINT = WM_APP + 0x0245; // wp: 1=capture, 2=pin, 3=snip_trans, 4=close_pins

// 控件 ID 表
const ID_CHK_TOP: i32 = 1001;
const ID_CHK_CLOSE_EXIT: i32 = 1002;
const ID_CHK_AUTO_START: i32 = 1003;
const ID_CHK_AUTO_HIDE: i32 = 1004;
const ID_CMB_THEME: i32 = 1005;
const ID_CMB_SCALE: i32 = 1006;
const ID_CHK_ANIM: i32 = 1007;
const ID_EDT_WIN_W: i32 = 1008;
const ID_EDT_WIN_H: i32 = 1009;
const ID_BTN_RESET_SIZE: i32 = 1010;
const ID_CHK_SUMMON: i32 = 1011;
const ID_CHK_SUMMON_CLEAR: i32 = 1012;
const ID_CHK_DBL_CLEAR: i32 = 1013;

const ID_CHK_AI: i32 = 1020;
const ID_CMB_AI_PRESET: i32 = 1021;
const ID_EDT_AI_URL: i32 = 1022;
const ID_EDT_AI_KEY: i32 = 1023;
const ID_BTN_TOGGLE_AI_KEY: i32 = 1024;
const ID_EDT_AI_MODEL: i32 = 1025;
const ID_CHK_BAIDU: i32 = 1026;
const ID_EDT_BAIDU_APPID: i32 = 1027;
const ID_EDT_BAIDU_KEY: i32 = 1028;
const ID_BTN_TOGGLE_BAIDU_KEY: i32 = 1029;
const ID_EDT_BAIDU_URL: i32 = 1030;

const ID_EDT_MAX_WORDS: i32 = 1040;
const ID_EDT_MAX_CHARS: i32 = 1041;
const ID_CHK_SNIP_SUB_WIN: i32 = 1045;

const ID_HK_SNIP_CAPTURE: i32 = 1050;
const ID_HK_SNIP_PIN: i32 = 1051;
const ID_HK_SNIP_TRANS: i32 = 1052;
const ID_EDT_SNIP_DIR: i32 = 1053;
const ID_BTN_BROWSE_SNIP: i32 = 1054;
const ID_BTN_OPEN_SNIP: i32 = 1055;
const ID_BTN_TRY_CAPTURE: i32 = 1056;
const ID_BTN_TRY_PIN: i32 = 1057;
const ID_BTN_TRY_SNIP: i32 = 1058;
const ID_BTN_CLOSE_PINS: i32 = 1059;

const ID_HK_CTRL_BASE: i32 = 1060; // +0..+6 对应 7 路全局热键
const ID_BTN_RESET_HOTKEYS: i32 = 1069;

const ID_EDT_CACHE: i32 = 1070;
const ID_BTN_OPEN_CACHE: i32 = 1071;

const ID_BTN_CHECK_UPDATE: i32 = 1080;
const ID_BTN_OPEN_REPO: i32 = 1081;

const DICT_BASE_ABBR: i32 = 2000;
const DICT_BASE_IGNORE: i32 = 2100;
const DICT_BASE_PREF: i32 = 2200;

/// 左侧 7 大分类定义
#[derive(Debug, Clone, Copy)]
pub struct NavCatDef {
    pub name: &'static str,
    pub icon: u16,
    pub subtitle: &'static str,
    pub sub_names: &'static [&'static str],
    pub page_indices: &'static [usize],
}

pub const DEFAULT_NAV_CATS: [NavCatDef; 7] = [
    NavCatDef {
        name: "常规",
        icon: 0xE713,
        subtitle: "",
        sub_names: &["基本", "界面", "行为"],
        page_indices: &[0, 1, 2],
    },
    NavCatDef {
        name: "翻译",
        icon: 0xE8C1,
        subtitle: "AI → 百度 → 必应 自动回退",
        sub_names: &[],
        page_indices: &[3],
    },
    NavCatDef {
        name: "命名",
        icon: 0xE8EC,
        subtitle: "",
        sub_names: &["缩减预算", "缩略词", "忽略词", "常用词"],
        page_indices: &[4, 5, 6, 7],
    },
    NavCatDef {
        name: "截屏",
        icon: 0xE722,
        subtitle: "快捷键 · 默认保存位置 · 截屏标注与贴图",
        sub_names: &[],
        page_indices: &[8],
    },
    NavCatDef {
        name: "控制",
        icon: 0xE765,
        subtitle: "全局快捷键与冲突检测",
        sub_names: &[],
        page_indices: &[9],
    },
    NavCatDef {
        name: "存储",
        icon: 0xE8B7,
        subtitle: "缓存目录与查询历史",
        sub_names: &[],
        page_indices: &[10],
    },
    NavCatDef {
        name: "关于",
        icon: 0xE946,
        subtitle: "版本信息与检查更新",
        sub_names: &[],
        page_indices: &[11],
    },
];

/// 页面全局索引转换为 `(分类索引, 子标签索引)`
pub fn page_to_cat_sub(page_idx: usize) -> (usize, usize) {
    for (ci, cat) in DEFAULT_NAV_CATS.iter().enumerate() {
        for (si, &pi) in cat.page_indices.iter().enumerate() {
            if pi == page_idx {
                return (ci, si);
            }
        }
    }
    (0, 0)
}

#[derive(Debug, Clone, Copy)]
pub struct LogicalRect {
    pub hwnd_slot: usize,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BtnKind {
    Normal,
    Primary,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRole {
    Normal,
    Hint,
    Title,
    Danger,
    Success,
}

#[derive(Clone)]
struct CtrlStyleMeta {
    hwnd: HWND,
    page_idx: usize,
    in_card: bool,
    role: TextRole,
    btn_kind: BtnKind,
    is_edit_or_hk: bool,
    hover: bool,
    down: bool,
    checked: bool,
    anim_t: f32,
    border_err: bool,
    logic_rect: (i32, i32, i32, i32),
}

impl Default for CtrlStyleMeta {
    fn default() -> Self {
        Self {
            hwnd: null_mut(),
            page_idx: 0,
            in_card: true,
            role: TextRole::Normal,
            btn_kind: BtnKind::Normal,
            is_edit_or_hk: false,
            hover: false,
            down: false,
            checked: false,
            anim_t: 0.0,
            border_err: false,
            logic_rect: (0, 0, 0, 0),
        }
    }
}

#[derive(Default, Clone, Copy)]
struct HotkeyBoxState {
    hwnd: HWND,
    mod_orig: i32,
    vk: i32,
    is_snip_mirror: bool,
    hk_idx: usize,
}

/// 每个子页的卡片逻辑矩形
pub fn page_cards_logical(page_idx: usize) -> Vec<(i32, i32, i32, i32)> {
    match page_idx {
        0 => vec![
            (SET_PAD_L, 14, SET_CONT_W, 128),
            (SET_PAD_L, 156, SET_CONT_W, 128),
        ],
        1 => vec![
            (SET_PAD_L, 14, SET_CONT_W, 174),
            (SET_PAD_L, 202, SET_CONT_W, 114),
        ],
        2 => vec![(SET_PAD_L, 14, SET_CONT_W, 174)],
        3 => vec![
            (SET_PAD_L, 14, SET_CONT_W, 142),
            (SET_PAD_L, 168, SET_CONT_W, 146),
            (SET_PAD_L, 326, SET_CONT_W, 108),
        ],
        4 => vec![
            (SET_PAD_L, 14, SET_CONT_W, 152),
            (SET_PAD_L, 180, SET_CONT_W, 128),
        ],
        5..=7 => vec![(SET_PAD_L, 8, SET_CONT_W, 76)],
        8 => vec![
            (SET_PAD_L, 12, SET_CONT_W, 148),
            (SET_PAD_L, 168, SET_CONT_W, 106),
            (SET_PAD_L, 282, SET_CONT_W, 244),
        ],
        9 => vec![(SET_PAD_L, 12, SET_CONT_W, 436)],
        10 => vec![(SET_PAD_L, 14, SET_CONT_W, 150)],
        11 => vec![(SET_PAD_L, 14, SET_CONT_W, 186)],
        _ => Vec::new(),
    }
}

/// 计算各子页面的实际内容逻辑总高度（用于上下滑动垂直滚动）
pub fn page_content_logical_height(page_idx: usize) -> i32 {
    let cards = page_cards_logical(page_idx);
    let max_card_bottom = cards.iter().map(|(_, y, _, h)| y + h).max().unwrap_or(0);
    // 留出 14px 的底部呼吸感内边距
    (max_card_bottom + 14).max(SET_PAGE_H)
}

#[derive(Default)]
struct SettingsUiState {
    hwnd_set: HWND,
    hwnd_main: HWND,
    hwnd_tip: HWND,
    page_hwnds: [HWND; 12],
    cur_page: usize,
    page_scroll_y: [i32; 12],
    scroll_dragging: bool,
    drag_start_y: i32,
    drag_start_scroll: i32,
    scale: f64,
    is_dark: bool,
    anim_enabled: bool,
    loading_guard: bool,
    app_dir: PathBuf,

    hfont_main: HFONT,
    br_page: HBRUSH,
    br_card: HBRUSH,
    br_edit: HBRUSH,

    // 导航栏与标题栏互动状态
    nav_hover: i32,
    sub_hover: i32,
    close_hover: bool,
    min_hover: bool,
    nav_pill_y: f32,
    nav_pill_target_y: f32,
    sub_pill_x: f32,
    sub_pill_w: f32,
    sub_pill_tx: f32,
    sub_pill_tw: f32,

    // 底部 Toast
    toast_text: String,
    toast_ok: bool,
    toast_alpha: f32,
    toast_expire_ms: u64,

    // 注册控件样式与位置
    ctrls: Vec<CtrlStyleMeta>,
    hotkeys: Vec<HotkeyBoxState>,
    combos: Vec<(HWND, (i32, i32, i32, i32))>,

    // 命名控件句柄
    chk_top: HWND,
    chk_close_exit: HWND,
    chk_auto_start: HWND,
    chk_auto_hide: HWND,
    cmb_theme: HWND,
    cmb_scale: HWND,
    chk_anim: HWND,
    edt_win_w: HWND,
    edt_win_h: HWND,
    chk_summon: HWND,
    chk_summon_clear: HWND,
    chk_dbl_clear: HWND,

    chk_ai: HWND,
    cmb_ai_preset: HWND,
    edt_ai_url: HWND,
    edt_ai_key: HWND,
    btn_toggle_ai_key: HWND,
    ai_key_shown: bool,
    edt_ai_model: HWND,
    chk_baidu: HWND,
    edt_baidu_appid: HWND,
    edt_baidu_key: HWND,
    btn_toggle_baidu_key: HWND,
    baidu_key_shown: bool,
    edt_baidu_url: HWND,

    edt_max_words: HWND,
    edt_max_chars: HWND,

    chk_snip_sub_win: HWND,
    edt_snip_dir: HWND,
    lbl_snip_hk_status: [HWND; 3],
    lbl_hk_status: [HWND; 7],
    btn_try_capture: HWND,
    btn_try_pin: HWND,

    edt_cache: HWND,
    lbl_update_status: HWND,
    btn_check_update: HWND,

    // 最新状态快照
    cur_settings: Option<AppSettings>,
}

thread_local! {
    static SET_UI: RefCell<SettingsUiState> = RefCell::new(SettingsUiState {
        scale: 1.0,
        is_dark: true,
        anim_enabled: true,
        nav_hover: -1,
        sub_hover: -1,
        ..Default::default()
    });
    // 词表组件独立存放，避免 sync_slots/MoveWindow 同步回调 page_wnd_proc 时与 SET_UI 发生 RefCell 重入冲突
    static DICT_VIEWS: RefCell<[Option<DictListView>; 3]> = const { RefCell::new([None, None, None]) };
}

#[inline]
fn spx(v: i32, sc: f64) -> i32 {
    ((v as f64) * sc).round() as i32
}

#[inline]
fn col_bg_win(dark: bool) -> COLORREF {
    if dark {
        rgb(24, 26, 32)
    } else {
        rgb(243, 245, 249)
    }
}

#[inline]
fn col_bg_nav(dark: bool) -> COLORREF {
    if dark {
        rgb(20, 22, 28)
    } else {
        rgb(235, 238, 245)
    }
}

#[inline]
fn col_bg_card(dark: bool) -> COLORREF {
    if dark {
        rgb(34, 38, 46)
    } else {
        rgb(255, 255, 255)
    }
}

#[inline]
fn col_bg_edit(dark: bool) -> COLORREF {
    if dark {
        rgb(26, 30, 38)
    } else {
        rgb(255, 255, 255)
    }
}

#[inline]
fn col_fg(dark: bool, role: TextRole) -> COLORREF {
    match role {
        TextRole::Danger => {
            if dark {
                rgb(248, 113, 113)
            } else {
                rgb(220, 38, 38)
            }
        }
        TextRole::Success => {
            if dark {
                rgb(74, 222, 128)
            } else {
                rgb(22, 163, 74)
            }
        }
        TextRole::Hint => {
            if dark {
                rgb(148, 163, 184)
            } else {
                rgb(100, 116, 139)
            }
        }
        TextRole::Title => {
            if dark {
                rgb(240, 244, 250)
            } else {
                rgb(15, 23, 42)
            }
        }
        TextRole::Normal => {
            if dark {
                rgb(224, 232, 244)
            } else {
                rgb(30, 41, 59)
            }
        }
    }
}

pub fn nav_item_rect(idx: usize, sc: f64) -> RECT {
    let nav_w = spx(SET_NAV_W, sc);
    let top = spx(SET_TITLE_H + 10, sc);
    let item_h = spx(40, sc);
    let gap = spx(4, sc);
    let y = top + (idx as i32) * (item_h + gap);
    RECT {
        left: spx(10, sc),
        top: y,
        right: nav_w - spx(10, sc),
        bottom: y + item_h,
    }
}

pub fn sub_tab_rects(cat_idx: usize, sc: f64) -> Vec<RECT> {
    let mut out = Vec::new();
    let Some(cat) = DEFAULT_NAV_CATS.get(cat_idx) else {
        return out;
    };
    let mut x = spx(SET_NAV_W + SET_PAD_L, sc);
    let y = spx(SET_TITLE_H + 8, sc);
    let h = spx(28, sc);
    for sub in cat.sub_names {
        let chars = sub.chars().count() as i32;
        let w = spx(chars * 16 + 28, sc).max(spx(64, sc));
        out.push(RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        });
        x += w + spx(8, sc);
    }
    out
}

fn title_btn_rects(sc: f64) -> (RECT, RECT) {
    let win_w = spx(SET_WIN_W, sc);
    let title_h = spx(SET_TITLE_H, sc);
    let bw = spx(34, sc);
    let bh = spx(26, sc);
    let by = (title_h - bh) / 2;
    let close_rc = RECT {
        left: win_w - spx(10, sc) - bw,
        top: by,
        right: win_w - spx(10, sc),
        bottom: by + bh,
    };
    let min_rc = RECT {
        left: close_rc.left - spx(4, sc) - bw,
        top: by,
        right: close_rc.left - spx(4, sc),
        bottom: by + bh,
    };
    (min_rc, close_rc)
}

#[inline]
fn pt_in(rc: RECT, x: i32, y: i32) -> bool {
    x >= rc.left && x < rc.right && y >= rc.top && y < rc.bottom
}

unsafe fn rebuild_brushes_and_font(s: &mut SettingsUiState) {
    if !s.br_page.is_null() {
        DeleteObject(s.br_page);
    }
    if !s.br_card.is_null() {
        DeleteObject(s.br_card);
    }
    if !s.br_edit.is_null() {
        DeleteObject(s.br_edit);
    }
    if !s.hfont_main.is_null() {
        DeleteObject(s.hfont_main);
    }
    s.br_page = CreateSolidBrush(col_bg_win(s.is_dark));
    s.br_card = CreateSolidBrush(col_bg_card(s.is_dark));
    s.br_edit = CreateSolidBrush(col_bg_edit(s.is_dark));
    let px = (14.5 * s.scale).round().max(11.0) as i32;
    let wf = utf8_to_wide("Microsoft YaHei UI");
    s.hfont_main = CreateFontW(
        -px,
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
}

// ---- 自绘按钮与开关子类 ----

unsafe extern "system" fn modern_btn_subclass(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
    _id: UINT_PTR,
    _ref: DWORD_PTR,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_MOUSEMOVE => {
            let changed = SET_UI.with(|st| {
                let mut s = st.borrow_mut();
                if let Some(m) = s.ctrls.iter_mut().find(|c| c.hwnd == hwnd) {
                    if !m.hover {
                        m.hover = true;
                        return true;
                    }
                }
                false
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
            SET_UI.with(|st| {
                if let Some(m) = st.borrow_mut().ctrls.iter_mut().find(|c| c.hwnd == hwnd) {
                    m.hover = false;
                    m.down = false;
                }
            });
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            SET_UI.with(|st| {
                if let Some(m) = st.borrow_mut().ctrls.iter_mut().find(|c| c.hwnd == hwnd) {
                    m.down = true;
                }
            });
            InvalidateRect(hwnd, null(), 0);
            DefSubclassProc(hwnd, msg, wp, lp)
        }
        WM_LBUTTONUP => {
            SET_UI.with(|st| {
                if let Some(m) = st.borrow_mut().ctrls.iter_mut().find(|c| c.hwnd == hwnd) {
                    m.down = false;
                }
            });
            InvalidateRect(hwnd, null(), 0);
            DefSubclassProc(hwnd, msg, wp, lp)
        }
        WM_PAINT => {
            paint_modern_btn(hwnd);
            0
        }
        WM_MOUSEWHEEL => {
            let parent = crate::ui::win32_ui::GetParent(hwnd);
            if !parent.is_null() {
                PostMessageW(parent, WM_MOUSEWHEEL, wp, lp);
            }
            0
        }
        _ => DefSubclassProc(hwnd, msg, wp, lp),
    }
}

unsafe fn paint_modern_btn(hwnd: HWND) {
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
    let (scale, is_dark, meta) = SET_UI.with(|st| {
        let s = st.borrow();
        let m = s
            .ctrls
            .iter()
            .find(|c| c.hwnd == hwnd)
            .cloned()
            .unwrap_or_default();
        (s.scale, s.is_dark, m)
    });

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm = CreateCompatibleBitmap(hdc, w, h);
    let old = SelectObject(hdc_buf, hbm);

    let bg_parent = CreateSolidBrush(if meta.in_card {
        col_bg_card(is_dark)
    } else {
        col_bg_win(is_dark)
    });
    FillRect(hdc_buf, &rc, bg_parent);
    DeleteObject(bg_parent);

    ensure_gdiplus();
    let mut g: *mut c_void = null_mut();
    if GdipCreateFromHDC(hdc_buf, &mut g) == GP_OK && !g.is_null() {
        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);
        let (fill, border, fg) = match meta.btn_kind {
            BtnKind::Primary => (
                if meta.down {
                    argb(42, 108, 220, 255)
                } else if meta.hover {
                    argb(79, 145, 250, 255)
                } else {
                    argb(59, 130, 246, 255)
                },
                0u32,
                rgb(255, 255, 255),
            ),
            BtnKind::Danger => {
                if meta.down {
                    (
                        argb(175, 45, 45, 255),
                        argb(200, 62, 62, 255),
                        rgb(255, 255, 255),
                    )
                } else if meta.hover {
                    (
                        argb(200, 62, 62, 255),
                        argb(215, 72, 72, 255),
                        rgb(255, 255, 255),
                    )
                } else if is_dark {
                    (
                        argb(54, 42, 46, 255),
                        argb(92, 58, 62, 255),
                        rgb(248, 113, 113),
                    )
                } else {
                    (
                        argb(254, 242, 242, 255),
                        argb(252, 165, 165, 255),
                        rgb(220, 38, 38),
                    )
                }
            }
            BtnKind::Normal => {
                if meta.down {
                    if is_dark {
                        (
                            argb(44, 50, 62, 255),
                            argb(76, 86, 104, 255),
                            rgb(240, 244, 250),
                        )
                    } else {
                        (
                            argb(226, 232, 240, 255),
                            argb(148, 163, 184, 255),
                            rgb(15, 23, 42),
                        )
                    }
                } else if meta.hover {
                    if is_dark {
                        (
                            argb(66, 74, 88, 255),
                            argb(92, 104, 124, 255),
                            rgb(255, 255, 255),
                        )
                    } else {
                        (
                            argb(241, 245, 249, 255),
                            argb(148, 163, 184, 255),
                            rgb(15, 23, 42),
                        )
                    }
                } else if is_dark {
                    (
                        argb(50, 56, 68, 255),
                        argb(72, 80, 96, 255),
                        rgb(232, 238, 248),
                    )
                } else {
                    (
                        argb(255, 255, 255, 255),
                        argb(203, 213, 225, 255),
                        rgb(30, 41, 59),
                    )
                }
            }
        };
        gp_round_card(
            g,
            0.5,
            0.5,
            (w - 1) as f32,
            (h - 1) as f32,
            spx(6, scale) as f32,
            fill,
            border,
            1.0,
        );
        GdipDeleteGraphics(g);

        let text = get_window_text(hwnd);
        let px = (13.5 * scale).round().max(11.0) as i32;
        let wf = utf8_to_wide("Microsoft YaHei UI");
        let hf = CreateFontW(
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
        );
        let old_f = SelectObject(hdc_buf, hf);
        SetBkMode(hdc_buf, TRANSPARENT);
        SetTextColor(hdc_buf, fg);
        let mut tr = rc;
        let wt = utf8_to_wide(&text);
        DrawTextW(
            hdc_buf,
            wt.as_ptr(),
            -1,
            &mut tr,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(hdc_buf, old_f);
        DeleteObject(hf);
    }

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old);
    DeleteObject(hbm);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

// ---- iOS 风格滑动开关子类 ----

unsafe extern "system" fn modern_chk_subclass(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
    _id: UINT_PTR,
    _ref: DWORD_PTR,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_MOUSEMOVE => {
            SET_UI.with(|st| {
                if let Some(m) = st.borrow_mut().ctrls.iter_mut().find(|c| c.hwnd == hwnd) {
                    m.hover = true;
                }
            });
            let mut tme = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tme);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_MOUSELEAVE => {
            SET_UI.with(|st| {
                if let Some(m) = st.borrow_mut().ctrls.iter_mut().find(|c| c.hwnd == hwnd) {
                    m.hover = false;
                }
            });
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => 0,
        WM_LBUTTONUP => {
            let mut rc = RECT::default();
            GetClientRect(hwnd, &mut rc);
            let (x, y) = lparam_xy(lp);
            if pt_in(rc, x, y) {
                let (new_chk, ctrl_id, page_hwnd) = SET_UI.with(|st| {
                    let mut s = st.borrow_mut();
                    let mut nxt = false;
                    let mut pid = 0usize;
                    if let Some(m) = s.ctrls.iter_mut().find(|c| c.hwnd == hwnd) {
                        m.checked = !m.checked;
                        m.anim_t = if m.checked { 1.0 } else { 0.0 };
                        nxt = m.checked;
                        pid = m.page_idx;
                    }
                    let cid = crate::ui::win32_ui::GetDlgCtrlID(hwnd);
                    (nxt, cid, s.page_hwnds[pid])
                });
                let _ = new_chk;
                InvalidateRect(hwnd, null(), 0);
                if !page_hwnd.is_null() {
                    PostMessageW(page_hwnd, WM_COMMAND, ctrl_id as WPARAM, hwnd as LPARAM);
                }
            }
            0
        }
        WM_PAINT => {
            paint_modern_chk(hwnd);
            0
        }
        WM_MOUSEWHEEL => {
            let parent = crate::ui::win32_ui::GetParent(hwnd);
            if !parent.is_null() {
                PostMessageW(parent, WM_MOUSEWHEEL, wp, lp);
            }
            0
        }
        _ => DefSubclassProc(hwnd, msg, wp, lp),
    }
}

unsafe fn paint_modern_chk(hwnd: HWND) {
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

    let (scale, is_dark, meta) = SET_UI.with(|st| {
        let s = st.borrow();
        let m = s
            .ctrls
            .iter()
            .find(|c| c.hwnd == hwnd)
            .cloned()
            .unwrap_or_default();
        (s.scale, s.is_dark, m)
    });

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm = CreateCompatibleBitmap(hdc, w, h);
    let old = SelectObject(hdc_buf, hbm);

    let bg = CreateSolidBrush(if meta.in_card {
        col_bg_card(is_dark)
    } else {
        col_bg_win(is_dark)
    });
    FillRect(hdc_buf, &rc, bg);
    DeleteObject(bg);

    let sw_w = spx(36, scale) as f32;
    let sw_h = spx(20, scale) as f32;
    let sw_y = ((h as f32) - sw_h) * 0.5;
    let t = meta.anim_t;

    ensure_gdiplus();
    let mut g: *mut c_void = null_mut();
    if GdipCreateFromHDC(hdc_buf, &mut g) == GP_OK && !g.is_null() {
        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);
        let track_col = if meta.checked {
            if meta.hover {
                argb(79, 145, 250, 255)
            } else {
                argb(59, 130, 246, 255)
            }
        } else if meta.hover {
            if is_dark {
                argb(76, 86, 104, 255)
            } else {
                argb(176, 188, 204, 255)
            }
        } else if is_dark {
            argb(56, 64, 78, 255)
        } else {
            argb(203, 213, 225, 255)
        };
        gp_fill_round_rect(g, 0.5, sw_y, sw_w, sw_h, sw_h * 0.5, track_col);

        let pad = spx(3, scale) as f32;
        let knob_d = sw_h - pad * 2.0;
        let knob_x = pad + (sw_w - pad * 2.0 - knob_d) * t;
        let mut br_k: *mut c_void = null_mut();
        if GdipCreateSolidFill(argb(255, 255, 255, 255), &mut br_k) == GP_OK && !br_k.is_null() {
            crate::ui::win32_ui::GdipFillEllipse(g, br_k, knob_x, sw_y + pad, knob_d, knob_d);
            GdipDeleteBrush(br_k);
        }
        GdipDeleteGraphics(g);
    }

    let txt = get_window_text(hwnd);
    let px = (14.0 * scale).round().max(11.0) as i32;
    let wf = utf8_to_wide("Microsoft YaHei UI");
    let hf = CreateFontW(
        -px,
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
    let old_f = SelectObject(hdc_buf, hf);
    SetBkMode(hdc_buf, TRANSPARENT);
    SetTextColor(hdc_buf, col_fg(is_dark, TextRole::Normal));
    let mut tr = RECT {
        left: spx(46, scale),
        top: 0,
        right: w,
        bottom: h,
    };
    let wt = utf8_to_wide(&txt);
    DrawTextW(
        hdc_buf,
        wt.as_ptr(),
        -1,
        &mut tr,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
    );
    SelectObject(hdc_buf, old_f);
    DeleteObject(hf);

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old);
    DeleteObject(hbm);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

// ---- 自绘热键录入子类（只读 EDIT + 按键截获，支持单键 F1~F12 与组合键） ----

unsafe extern "system" fn hotkey_edit_subclass(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
    _id: UINT_PTR,
    _ref: DWORD_PTR,
) -> LRESULT {
    match msg {
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let vk = (wp & 0xFF) as i32;
            // 跳过纯修饰键本身与 Tab 焦点切换
            if vk == VK_SHIFT
                || vk == VK_CONTROL
                || vk == VK_MENU
                || vk == VK_LWIN
                || vk == VK_RWIN
                || vk == VK_TAB
            {
                if vk == VK_TAB {
                    return DefSubclassProc(hwnd, msg, wp, lp);
                }
                return 0;
            }
            let (new_m, new_vk) = if vk == VK_ESCAPE || vk == VK_BACK || vk == VK_DELETE {
                (0, 0)
            } else {
                let mut m = 0;
                if (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0 {
                    m |= 1;
                }
                if (GetAsyncKeyState(VK_SHIFT) as u16 & 0x8000) != 0 {
                    m |= 2;
                }
                if (GetAsyncKeyState(VK_MENU) as u16 & 0x8000) != 0 {
                    m |= 4;
                }
                if ((GetAsyncKeyState(VK_LWIN) as u16 | GetAsyncKeyState(VK_RWIN) as u16) & 0x8000)
                    != 0
                {
                    m |= 8;
                }
                (m, vk)
            };

            let (hk_idx, is_mirror, hwnd_main, hwnd_set) = SET_UI.with(|st| {
                let mut s = st.borrow_mut();
                let mut idx = 0usize;
                let mut mir = false;
                if let Some(hk) = s.hotkeys.iter_mut().find(|h| h.hwnd == hwnd) {
                    hk.mod_orig = new_m;
                    hk.vk = new_vk;
                    idx = hk.hk_idx;
                    mir = hk.is_snip_mirror;
                }
                if let Some(cfg) = s.cur_settings.as_mut() {
                    cfg.set_hotkey_at(idx, (new_m, new_vk));
                }
                // 双向同步同序号的两处热键控件（控制页 <-> 截屏页）
                for h in &mut s.hotkeys {
                    if h.hk_idx == idx {
                        h.mod_orig = new_m;
                        h.vk = new_vk;
                    }
                }
                (idx, mir, s.hwnd_main, s.hwnd_set)
            });

            // 更新同步后的热键框文字
            let label = format_hotkey_text(new_m, new_vk);
            let hwnds: Vec<HWND> = SET_UI.with(|st| {
                st.borrow()
                    .hotkeys
                    .iter()
                    .filter(|h| h.hk_idx == hk_idx)
                    .map(|h| h.hwnd)
                    .collect()
            });
            for h in hwnds {
                set_window_text(h, &label);
            }
            if !hwnd_set.is_null() {
                InvalidateRect(hwnd_set, null(), 0);
            }
            if !hwnd_main.is_null() {
                PostMessageW(
                    hwnd_main,
                    WM_SET_APPLY_HOTKEYS,
                    0,
                    if is_mirror { 1 } else { 0 },
                );
            }
            return 0;
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            let p = crate::ui::win32_ui::GetParent(hwnd);
            if !p.is_null() {
                InvalidateRect(p, null(), 0);
            }
        }
        WM_MOUSEWHEEL => {
            let parent = crate::ui::win32_ui::GetParent(hwnd);
            if !parent.is_null() {
                PostMessageW(parent, WM_MOUSEWHEEL, wp, lp);
            }
            return 0;
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe extern "system" fn edit_focus_subclass(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
    _id: UINT_PTR,
    _ref: DWORD_PTR,
) -> LRESULT {
    if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
        let p = crate::ui::win32_ui::GetParent(hwnd);
        if !p.is_null() {
            InvalidateRect(p, null(), 0);
        }
    } else if msg == WM_MOUSEWHEEL {
        let parent = crate::ui::win32_ui::GetParent(hwnd);
        if !parent.is_null() {
            PostMessageW(parent, WM_MOUSEWHEEL, wp, lp);
        }
        return 0;
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

// ---- 工具方法：同步热键状态、刷新页签与显示 Toast ----

pub fn show_settings_toast(msg: &str, ok: bool) {
    let hwnd_set = SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.toast_text = msg.to_string();
        s.toast_ok = ok;
        s.toast_alpha = 1.0;
        s.toast_expire_ms = unsafe { GetTickCount64() } + 2400;
        s.hwnd_set
    });
    if !hwnd_set.is_null() {
        unsafe {
            SetTimer(hwnd_set, TOAST_TIMER_ID, 40, null());
            InvalidateRect(hwnd_set, null(), 0);
        }
    }
}

/// 由主宿主线程回调：更新 7 路热键冲突标记及文案
pub fn update_hotkey_status_ui(ok_flags: &[bool; 7]) {
    let (h_main_labels, h_snip_labels, cur_set, ctrls_hwnds) = SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        let mut hk_to_err: Vec<(HWND, bool)> = Vec::new();
        for hk in &s.hotkeys {
            let is_ok = ok_flags.get(hk.hk_idx).copied().unwrap_or(true);
            hk_to_err.push((hk.hwnd, !is_ok));
        }
        for (h, err) in &hk_to_err {
            if let Some(m) = s.ctrls.iter_mut().find(|c| c.hwnd == *h) {
                m.border_err = *err;
            }
        }
        for (i, &ok) in ok_flags.iter().enumerate() {
            let lbl_h = s.lbl_hk_status[i];
            if let Some(m) = s.ctrls.iter_mut().find(|c| c.hwnd == lbl_h) {
                m.role = if ok {
                    TextRole::Success
                } else {
                    TextRole::Danger
                };
            }
        }
        for (si, &hidx) in [5usize, 6usize, 4usize].iter().enumerate() {
            let lbl_h = s.lbl_snip_hk_status[si];
            if let Some(m) = s.ctrls.iter_mut().find(|c| c.hwnd == lbl_h) {
                m.role = if ok_flags[hidx] {
                    TextRole::Success
                } else {
                    TextRole::Danger
                };
            }
        }
        (
            s.lbl_hk_status,
            s.lbl_snip_hk_status,
            s.cur_settings.clone(),
            (s.btn_try_capture, s.btn_try_pin),
        )
    });

    if let Some(cfg) = cur_set {
        for i in 0..7 {
            let (m, vk) = cfg.hotkey_at(i);
            let txt = if vk <= 0 {
                "未设置"
            } else if ok_flags[i] {
                "已就绪"
            } else {
                "已被占用"
            };
            let _ = m;
            set_window_text(h_main_labels[i], txt);
            unsafe { InvalidateRect(h_main_labels[i], null(), 0) };
        }
        let snip_map = [5usize, 6usize, 4usize];
        for (si, &idx) in snip_map.iter().enumerate() {
            let (_, vk) = cfg.hotkey_at(idx);
            let txt = if vk <= 0 {
                "未设置"
            } else if ok_flags[idx] {
                "已就绪"
            } else {
                "已被占用"
            };
            set_window_text(h_snip_labels[si], txt);
            unsafe { InvalidateRect(h_snip_labels[si], null(), 0) };
        }
        let cap_lbl = format_hotkey_text(cfg.hk_capture.0, cfg.hk_capture.1);
        let pin_lbl = format_hotkey_text(cfg.hk_pin.0, cfg.hk_pin.1);
        set_window_text(ctrls_hwnds.0, &format!("立即截屏 ({cap_lbl})"));
        set_window_text(ctrls_hwnds.1, &format!("剪贴板贴图 ({pin_lbl})"));
    }
}

pub fn set_about_update_status(text: &str, checking: bool) {
    let (lbl, btn) = SET_UI.with(|st| {
        let s = st.borrow();
        (s.lbl_update_status, s.btn_check_update)
    });
    if !lbl.is_null() {
        set_window_text(lbl, text);
    }
    if !btn.is_null() {
        unsafe { crate::ui::win32_ui::EnableWindow(btn, if checking { 0 } else { 1 }) };
        set_window_text(
            btn,
            if checking {
                "检查中..."
            } else {
                "检查更新"
            },
        );
    }
}

/// 从 `AppSettings` 将配置同步回填设置界面
pub fn load_settings_into_ui(cfg: &AppSettings) {
    SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.loading_guard = true;
        s.cur_settings = Some(cfg.clone());
        s.anim_enabled = cfg.anim_enabled;

        let set_chk = |ctrls: &mut Vec<CtrlStyleMeta>, h: HWND, val: bool| {
            if let Some(m) = ctrls.iter_mut().find(|c| c.hwnd == h) {
                m.checked = val;
                m.anim_t = if val { 1.0 } else { 0.0 };
            }
        };
        let chk_handles = [
            (s.chk_top, cfg.topmost),
            (s.chk_close_exit, cfg.close_exit),
            (s.chk_auto_start, cfg.auto_start),
            (s.chk_auto_hide, cfg.auto_hide),
            (s.chk_anim, cfg.anim_enabled),
            (s.chk_summon, cfg.summon_near_mouse),
            (s.chk_summon_clear, cfg.summon_clear),
            (s.chk_dbl_clear, cfg.input_dbl_clear),
            (s.chk_ai, cfg.ai_enabled),
            (s.chk_baidu, cfg.baidu_enabled),
            (s.chk_snip_sub_win, cfg.snip_sub_window),
        ];
        for (h, v) in chk_handles {
            set_chk(&mut s.ctrls, h, v);
        }

        for hk in &mut s.hotkeys {
            let (m, vk) = cfg.hotkey_at(hk.hk_idx);
            hk.mod_orig = m;
            hk.vk = vk;
        }
    });

    // 更新各个原生控件内容（在释放借用后操作 HWND）
    let handles = SET_UI.with(|st| {
        let s = st.borrow();
        let hks: Vec<(HWND, i32, i32)> = s
            .hotkeys
            .iter()
            .map(|h| (h.hwnd, h.mod_orig, h.vk))
            .collect();
        (
            s.cmb_theme,
            s.cmb_scale,
            s.edt_win_w,
            s.edt_win_h,
            s.cmb_ai_preset,
            s.edt_ai_url,
            s.edt_ai_key,
            s.edt_ai_model,
            s.edt_baidu_appid,
            s.edt_baidu_key,
            s.edt_baidu_url,
            s.edt_max_words,
            s.edt_max_chars,
            s.edt_snip_dir,
            s.edt_cache,
            hks,
            s.page_hwnds,
        )
    });

    unsafe {
        SendMessageW(handles.0, CB_SETCURSEL, cfg.theme_mode as WPARAM, 0);
        let scale_idx = UI_SCALE_PRESETS
            .iter()
            .position(|&p| p == cfg.ui_scale)
            .unwrap_or(1);
        SendMessageW(handles.1, CB_SETCURSEL, scale_idx, 0);

        let (def_w, def_h) = compute_def_win_size(cfg.ui_scale);
        let show_w = if cfg.win_w > 0 { cfg.win_w } else { def_w };
        let show_h = if cfg.win_h > 0 { cfg.win_h } else { def_h };
        set_window_text(handles.2, &show_w.to_string());
        set_window_text(handles.3, &show_h.to_string());

        let preset_idx = AI_PRESETS
            .iter()
            .position(|p| !p.url.is_empty() && p.url == cfg.ai_url)
            .unwrap_or(0);
        SendMessageW(handles.4, CB_SETCURSEL, preset_idx, 0);

        set_window_text(handles.5, &cfg.ai_url);
        set_window_text(handles.6, &cfg.ai_key);
        set_window_text(handles.7, &cfg.ai_model);
        set_window_text(handles.8, &cfg.baidu_appid);
        set_window_text(handles.9, &cfg.baidu_key);
        set_window_text(handles.10, &cfg.baidu_url);
        set_window_text(handles.11, &cfg.max_words.to_string());
        set_window_text(handles.12, &cfg.max_chars.to_string());
        set_window_text(handles.13, &cfg.snip_save_dir);
        set_window_text(handles.14, &cfg.cache_dir);

        for (h, m, vk) in handles.15 {
            set_window_text(h, &format_hotkey_text(m, vk));
        }
        for p in handles.16 {
            if !p.is_null() {
                InvalidateRect(p, null(), 0);
            }
        }
    }

    SET_UI.with(|st| {
        st.borrow_mut().loading_guard = false;
    });
    DICT_VIEWS.with(|dv| {
        if let Ok(mut views) = dv.try_borrow_mut() {
            for dl in views.iter_mut().flatten() {
                dl.model.load_merged(false);
                unsafe { dl.sync_slots() };
            }
        }
    });
}

/// 从界面控件采集用户当前填写并更新 `AppSettings`
pub fn collect_settings_from_ui(base: &mut AppSettings) {
    let snap = SET_UI.with(|st| {
        let s = st.borrow();
        let is_chk = |h: HWND| -> bool {
            s.ctrls
                .iter()
                .find(|c| c.hwnd == h)
                .map(|m| m.checked)
                .unwrap_or(false)
        };
        (
            is_chk(s.chk_top),
            is_chk(s.chk_close_exit),
            is_chk(s.chk_auto_start),
            is_chk(s.chk_auto_hide),
            is_chk(s.chk_anim),
            is_chk(s.chk_summon),
            is_chk(s.chk_summon_clear),
            is_chk(s.chk_dbl_clear),
            is_chk(s.chk_ai),
            is_chk(s.chk_baidu),
            s.edt_ai_url,
            s.edt_ai_key,
            s.edt_ai_model,
            s.edt_baidu_appid,
            s.edt_baidu_key,
            s.edt_baidu_url,
            s.edt_max_words,
            s.edt_max_chars,
            s.edt_snip_dir,
            s.edt_cache,
            s.edt_win_w,
            s.edt_win_h,
            s.cmb_theme,
            s.cmb_scale,
            s.hotkeys.clone(),
            is_chk(s.chk_snip_sub_win),
        )
    });

    base.topmost = snap.0;
    base.close_exit = snap.1;
    base.auto_start = snap.2;
    base.auto_hide = snap.3;
    base.anim_enabled = snap.4;
    base.summon_near_mouse = snap.5;
    base.summon_clear = snap.6;
    base.input_dbl_clear = snap.7;
    base.ai_enabled = snap.8;
    base.baidu_enabled = snap.9;

    base.ai_url = get_window_text(snap.10).trim().to_string();
    base.ai_key = get_window_text(snap.11).trim().to_string();
    let m = get_window_text(snap.12).trim().to_string();
    base.ai_model = if m.is_empty() {
        "deepseek-chat".to_string()
    } else {
        m
    };

    base.baidu_appid = get_window_text(snap.13).trim().to_string();
    base.baidu_key = get_window_text(snap.14).trim().to_string();
    base.baidu_url = get_window_text(snap.15).trim().to_string();

    if let Ok(mw) = get_window_text(snap.16).trim().parse::<usize>() {
        base.max_words = mw.clamp(1, 16);
    }
    if let Ok(mc) = get_window_text(snap.17).trim().parse::<usize>() {
        base.max_chars = mc.clamp(8, 128);
    }

    let snip_dir = get_window_text(snap.18).trim().to_string();
    if !snip_dir.is_empty() {
        base.snip_save_dir = snip_dir;
    }
    base.snip_sub_window = snap.25;
    let cache_dir = get_window_text(snap.19).trim().to_string();
    if !cache_dir.is_empty() {
        base.cache_dir = cache_dir;
    }
    if let Ok(ww) = get_window_text(snap.20).trim().parse::<i32>() {
        if ww >= 400 {
            base.win_w = ww;
        }
    }
    if let Ok(wh) = get_window_text(snap.21).trim().parse::<i32>() {
        if wh >= 300 {
            base.win_h = wh;
        }
    }

    unsafe {
        let tidx = SendMessageW(snap.22, CB_GETCURSEL, 0, 0) as i32;
        if (0..=2).contains(&tidx) {
            base.theme_mode = tidx;
        }
        let sidx = SendMessageW(snap.23, CB_GETCURSEL, 0, 0) as usize;
        if let Some(&pct) = UI_SCALE_PRESETS.get(sidx) {
            base.ui_scale = pct;
        }
    }

    for hk in snap.24 {
        base.set_hotkey_at(hk.hk_idx, (hk.mod_orig, hk.vk));
    }

    SET_UI.with(|st| st.borrow_mut().cur_settings = Some(base.clone()));
}

/// 对指定子页面应用垂直滚动偏移，并同步调整其上所有子控件的物理坐标
pub unsafe fn apply_page_scroll(page_idx: usize, new_scroll: i32) {
    let (page_hwnd, sc, ctrls_to_move, combos_to_move) = SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        let max_s = (page_content_logical_height(page_idx) - SET_PAGE_H).max(0);
        let clamped = new_scroll.clamp(0, max_s);
        if s.page_scroll_y[page_idx] == clamped {
            return (null_mut(), 1.0, Vec::new(), Vec::new());
        }
        s.page_scroll_y[page_idx] = clamped;
        let phwnd = s.page_hwnds[page_idx];
        let scale = s.scale;
        let c_list: Vec<(HWND, (i32, i32, i32, i32))> = s
            .ctrls
            .iter()
            .filter(|c| c.page_idx == page_idx)
            .map(|c| (c.hwnd, c.logic_rect))
            .collect();
        let cmb_list: Vec<(HWND, (i32, i32, i32, i32))> = s
            .combos
            .iter()
            .filter(|(h, _)| crate::ui::win32_ui::GetParent(*h) == phwnd)
            .cloned()
            .collect();
        (phwnd, scale, c_list, cmb_list)
    });

    if page_hwnd.is_null() {
        return;
    }

    let scroll_y = SET_UI.with(|st| st.borrow().page_scroll_y[page_idx]);
    for (hwnd, (lx, ly, lw, lh)) in ctrls_to_move {
        if !hwnd.is_null() && lw > 0 {
            SetWindowPos(
                hwnd,
                null_mut(),
                spx(lx, sc),
                spx(ly - scroll_y, sc),
                spx(lw, sc),
                spx(lh, sc),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    for (hwnd, (lx, ly, lw, lh)) in combos_to_move {
        if !hwnd.is_null() && lw > 0 {
            SetWindowPos(
                hwnd,
                null_mut(),
                spx(lx, sc),
                spx(ly - scroll_y, sc),
                spx(lw, sc),
                spx(lh, sc),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    InvalidateRect(page_hwnd, null(), 0);
}

/// 切换设置窗口显示页（支持直接带动画跳转）
pub fn select_settings_tab(target_page: usize) {
    let target = target_page.min(11);
    let (hwnd_set, old_page, pages, anim, sc, cur_nav_y) = SET_UI.with(|st| {
        let s = st.borrow();
        (
            s.hwnd_set,
            s.cur_page,
            s.page_hwnds,
            s.anim_enabled,
            s.scale,
            s.nav_pill_y,
        )
    });
    if hwnd_set.is_null() || target == old_page {
        return;
    }

    let (old_cat, _) = page_to_cat_sub(old_page);
    let (cat_idx, sub_idx) = page_to_cat_sub(target);
    let nav_rc = nav_item_rect(cat_idx, sc);
    let sub_rects = sub_tab_rects(cat_idx, sc);
    let (sub_tx, sub_tw) = sub_rects
        .get(sub_idx)
        .map(|r| (r.left as f32, (r.right - r.left) as f32))
        .unwrap_or((0.0, 0.0));

    // 先显示目标子页再隐藏旧页，避免 WS_CLIPCHILDREN 下切换瞬间露出父窗底色或产生黑缝
    if !pages[target].is_null() {
        unsafe {
            ShowWindow(pages[target], SW_SHOW);
            InvalidateRect(pages[target], null(), 0);
        }
    }
    for (i, &p) in pages.iter().enumerate() {
        if i != target && !p.is_null() {
            unsafe { ShowWindow(p, SW_HIDE) };
        }
    }

    SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.cur_page = target;
        s.nav_pill_target_y = nav_rc.top as f32;
        if !anim || cur_nav_y <= 0.1 {
            s.nav_pill_y = nav_rc.top as f32;
        }
        s.sub_pill_tx = sub_tx;
        s.sub_pill_tw = sub_tw;
        if !anim || s.sub_pill_w <= 0.1 || old_cat != cat_idx {
            s.sub_pill_x = sub_tx;
            s.sub_pill_w = sub_tw;
        }
    });
    if (5..=7).contains(&target) {
        DICT_VIEWS.with(|dv| {
            if let Ok(mut views) = dv.try_borrow_mut() {
                if let Some(dl) = views[target - 5].as_mut() {
                    unsafe { dl.sync_slots() };
                }
            }
        });
    }

    unsafe {
        if anim {
            crate::ui::win32_ui::timeBeginPeriod(1);
            SetTimer(hwnd_set, ANIM_TIMER_ID, 16, null());
        }
        let nav_w = spx(SET_NAV_W, sc) + 2;
        let top_h = spx(SET_PAGE_Y, sc);
        let mut r1 = RECT { left: 0, top: 0, right: nav_w, bottom: 2000 };
        let mut r2 = RECT { left: 0, top: 0, right: 2000, bottom: top_h };
        InvalidateRect(hwnd_set, &mut r1, 0);
        InvalidateRect(hwnd_set, &mut r2, 0);
    }
}

/// 显示或前置弹出设置窗口，支持直接打开指定子页
pub fn open_settings_window(tab_idx: Option<usize>) {
    let hwnd_set = SET_UI.with(|st| st.borrow().hwnd_set);
    if hwnd_set.is_null() {
        return;
    }
    if let Some(t) = tab_idx {
        select_settings_tab(t);
    }
    unsafe {
        ShowWindow(hwnd_set, SW_RESTORE);
        ShowWindow(hwnd_set, SW_SHOW);
        crate::ui::win32_ui::SetForegroundWindow(hwnd_set);
    }
}

/// 隐藏设置窗口
pub fn hide_settings_window() {
    let hwnd_set = SET_UI.with(|st| st.borrow().hwnd_set);
    if !hwnd_set.is_null() {
        unsafe { ShowWindow(hwnd_set, SW_HIDE) };
    }
}

/// 实时切换设置窗口与所有子控件的主题（深/浅）及字体缩放（75%~200%）
pub fn refresh_settings_appearance(scale: f64, is_dark: bool, anim_enabled: bool) {
    apply_uxtheme_dark_app_mode(is_dark);
    let (hwnd_set, pages, hfont, ctrls_and_combos) = SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.scale = scale;
        s.is_dark = is_dark;
        s.anim_enabled = anim_enabled;
        unsafe { rebuild_brushes_and_font(&mut s) };

        let (cat_idx, sub_idx) = page_to_cat_sub(s.cur_page);
        s.nav_pill_y = nav_item_rect(cat_idx, scale).top as f32;
        s.nav_pill_target_y = s.nav_pill_y;
        let sub_r = sub_tab_rects(cat_idx, scale);
        if let Some(sr) = sub_r.get(sub_idx) {
            s.sub_pill_x = sr.left as f32;
            s.sub_pill_w = (sr.right - sr.left) as f32;
            s.sub_pill_tx = s.sub_pill_x;
            s.sub_pill_tw = s.sub_pill_w;
        }
        let hf_main = s.hfont_main;
        let ctrl_rects: Vec<(HWND, usize, (i32, i32, i32, i32))> =
            s.ctrls.iter().map(|c| (c.hwnd, c.page_idx, c.logic_rect)).collect();
        let cmb_rects = s.combos.clone();
        (s.hwnd_set, s.page_hwnds, hf_main, (ctrl_rects, cmb_rects))
    });

    DICT_VIEWS.with(|dv| {
        if let Ok(mut views) = dv.try_borrow_mut() {
            for dl in views.iter_mut().flatten() {
                unsafe {
                    dl.relayout(scale);
                    dl.apply_font_and_theme(hfont, is_dark);
                }
            }
        }
    });

    if hwnd_set.is_null() {
        return;
    }

    apply_dwm_window_style(hwnd_set, is_dark, true);
    let win_w = spx(SET_WIN_W, scale);
    let win_h = spx(SET_WIN_H, scale);
    let px = spx(SET_PAGE_X, scale);
    let py = spx(SET_PAGE_Y, scale);
    let pw = spx(SET_PAGE_W, scale);
    let ph = spx(SET_PAGE_H, scale);

    unsafe {
        // 先解除 minmax 再设新宽高
        SetWindowPos(
            hwnd_set,
            HWND_TOP,
            0,
            0,
            win_w,
            win_h,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        for p in pages {
            if !p.is_null() {
                MoveWindow(p, px, py, pw, ph, 1);
                apply_font_to_children(p, hfont);
                InvalidateRect(p, null(), 1);
            }
        }
        let scroll_arr = SET_UI.with(|st| st.borrow().page_scroll_y);
        for (h, p_idx, (lx, ly, lw, lh)) in ctrls_and_combos.0 {
            if !h.is_null() && lw > 0 {
                let sy = scroll_arr.get(p_idx).copied().unwrap_or(0);
                MoveWindow(
                    h,
                    spx(lx, scale),
                    spx(ly - sy, scale),
                    spx(lw, scale),
                    spx(lh, scale),
                    1,
                );
            }
        }
        for (h, (lx, ly, lw, lh)) in ctrls_and_combos.1 {
            if !h.is_null() {
                let parent = crate::ui::win32_ui::GetParent(h);
                let p_idx = pages.iter().position(|&p| p == parent).unwrap_or(0);
                let sy = scroll_arr.get(p_idx).copied().unwrap_or(0);
                MoveWindow(
                    h,
                    spx(lx, scale),
                    spx(ly - sy, scale),
                    spx(lw, scale),
                    spx(lh, scale),
                    1,
                );
                SendMessageW(
                    h,
                    CB_SETITEMHEIGHT,
                    usize::MAX,
                    (spx(22, scale)).max(16) as LPARAM,
                );
                set_control_theme(h, if is_dark { "DarkMode_CFD" } else { "CFD" });
            }
        }
        InvalidateRect(hwnd_set, null(), 1);
        UpdateWindow(hwnd_set);
    }
}

// ---- 绘制设置窗口与子页面 ----

unsafe fn paint_settings_window(hwnd: HWND) {
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
        scale,
        is_dark,
        cur_page,
        nav_hover,
        sub_hover,
        min_hover,
        close_hover,
        nav_pill_y,
        sub_pill_x,
        sub_pill_w,
        toast_text,
        toast_ok,
        toast_alpha,
    ) = SET_UI.with(|st| {
        let s = st.borrow();
        (
            s.scale,
            s.is_dark,
            s.cur_page,
            s.nav_hover,
            s.sub_hover,
            s.min_hover,
            s.close_hover,
            s.nav_pill_y,
            s.sub_pill_x,
            s.sub_pill_w,
            s.toast_text.clone(),
            s.toast_ok,
            s.toast_alpha,
        )
    });

    let hdc_buf = CreateCompatibleDC(hdc);
    let hbm_buf = CreateCompatibleBitmap(hdc, w, h);
    let old_bm = SelectObject(hdc_buf, hbm_buf);

    let bg = CreateSolidBrush(col_bg_win(is_dark));
    FillRect(hdc_buf, &rc, bg);
    DeleteObject(bg);

    let nav_w = spx(SET_NAV_W, scale);
    let title_h = spx(SET_TITLE_H, scale);
    let nav_rc = RECT {
        left: 0,
        top: 0,
        right: nav_w,
        bottom: h,
    };
    let br_nav = CreateSolidBrush(col_bg_nav(is_dark));
    FillRect(hdc_buf, &nav_rc, br_nav);
    DeleteObject(br_nav);

    // 导航分隔竖线与副标题底线（画在 nav_w - 1 避免被右侧子页面窗口遮挡）
    let sep_col = if is_dark {
        rgb(40, 46, 56)
    } else {
        rgb(216, 222, 232)
    };
    let pen_sep = CreatePen(PS_SOLID, 1, sep_col);
    let old_p = SelectObject(hdc_buf, pen_sep);
    let sep_x = (nav_w - 1).max(0);
    crate::ui::win32_ui::MoveToEx(hdc_buf, sep_x, 0, null_mut());
    crate::ui::win32_ui::LineTo(hdc_buf, sep_x, h);
    crate::ui::win32_ui::MoveToEx(hdc_buf, nav_w, spx(SET_PAGE_Y, scale) - 1, null_mut());
    crate::ui::win32_ui::LineTo(hdc_buf, w, spx(SET_PAGE_Y, scale) - 1);
    SelectObject(hdc_buf, old_p);
    DeleteObject(pen_sep);

    let (cur_cat, cur_sub) = page_to_cat_sub(cur_page);

    ensure_gdiplus();
    let mut g: *mut c_void = null_mut();
    if GdipCreateFromHDC(hdc_buf, &mut g) == GP_OK && !g.is_null() {
        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);

        // 左侧标题小蓝点
        let mut br_dot: *mut c_void = null_mut();
        if GdipCreateSolidFill(argb(59, 130, 246, 255), &mut br_dot) == GP_OK && !br_dot.is_null() {
            let ds = spx(7, scale) as f32;
            crate::ui::win32_ui::GdipFillEllipse(
                g,
                br_dot,
                spx(14, scale) as f32,
                (title_h as f32) * 0.5 - ds * 0.5,
                ds,
                ds,
            );
            GdipDeleteBrush(br_dot);
        }

        // 导航项悬停与选中胶囊
        for (i, cat) in DEFAULT_NAV_CATS.iter().enumerate() {
            let r = nav_item_rect(i, scale);
            if i as i32 == nav_hover && i != cur_cat {
                gp_fill_round_rect(
                    g,
                    r.left as f32,
                    r.top as f32,
                    (r.right - r.left) as f32,
                    (r.bottom - r.top) as f32,
                    spx(7, scale) as f32,
                    if is_dark {
                        argb(38, 44, 54, 255)
                    } else {
                        argb(224, 229, 238, 255)
                    },
                );
            }
            let _ = cat;
        }
        let sel_rc = nav_item_rect(cur_cat, scale);
        gp_fill_round_rect(
            g,
            sel_rc.left as f32,
            nav_pill_y,
            (sel_rc.right - sel_rc.left) as f32,
            (sel_rc.bottom - sel_rc.top) as f32,
            spx(7, scale) as f32,
            if is_dark {
                argb(59, 130, 246, 46)
            } else {
                argb(59, 130, 246, 28)
            },
        );
        gp_fill_round_rect(
            g,
            sel_rc.left as f32,
            nav_pill_y + spx(9, scale) as f32,
            spx(3, scale) as f32,
            (sel_rc.bottom - sel_rc.top - spx(18, scale)) as f32,
            1.5,
            argb(59, 130, 246, 255),
        );

        // 导航 MDL2 图标与文字
        for (i, cat) in DEFAULT_NAV_CATS.iter().enumerate() {
            let r = nav_item_rect(i, scale);
            let active = i == cur_cat;
            let icon_col = if active {
                if is_dark {
                    argb(147, 197, 253, 255)
                } else {
                    argb(37, 99, 235, 255)
                }
            } else if is_dark {
                argb(170, 184, 204, 255)
            } else {
                argb(100, 116, 139, 255)
            };
            let g_rc = GpRectF {
                x: (r.left + spx(10, scale)) as f32,
                y: r.top as f32,
                width: spx(22, scale) as f32,
                height: (r.bottom - r.top) as f32,
            };
            gp_draw_glyph(
                g,
                cat.icon,
                &g_rc,
                "Segoe MDL2 Assets",
                (15.0 * scale) as f32,
                icon_col,
            );
        }

        // 顶部子标签胶囊
        let sub_r = sub_tab_rects(cur_cat, scale);
        for (si, r) in sub_r.iter().enumerate() {
            if si as i32 == sub_hover && si != cur_sub {
                gp_fill_round_rect(
                    g,
                    r.left as f32,
                    r.top as f32,
                    (r.right - r.left) as f32,
                    (r.bottom - r.top) as f32,
                    spx(6, scale) as f32,
                    if is_dark {
                        argb(40, 46, 56, 255)
                    } else {
                        argb(226, 232, 240, 255)
                    },
                );
            }
        }
        if !sub_r.is_empty() && sub_pill_w > 1.0 {
            let sr0 = sub_r[0];
            gp_fill_round_rect(
                g,
                sub_pill_x,
                sr0.top as f32,
                sub_pill_w,
                (sr0.bottom - sr0.top) as f32,
                spx(6, scale) as f32,
                if is_dark {
                    argb(59, 130, 246, 54)
                } else {
                    argb(59, 130, 246, 28)
                },
            );
        }

        // 右上角 最小化 / 关闭 按钮
        let (min_rc, close_rc) = title_btn_rects(scale);
        let r_tb = spx(5, scale) as f32;
        if min_hover {
            gp_fill_round_rect(
                g,
                min_rc.left as f32,
                min_rc.top as f32,
                (min_rc.right - min_rc.left) as f32,
                (min_rc.bottom - min_rc.top) as f32,
                r_tb,
                if is_dark {
                    argb(54, 60, 72, 255)
                } else {
                    argb(226, 232, 240, 255)
                },
            );
        }
        if close_hover {
            gp_fill_round_rect(
                g,
                close_rc.left as f32,
                close_rc.top as f32,
                (close_rc.right - close_rc.left) as f32,
                (close_rc.bottom - close_rc.top) as f32,
                r_tb,
                argb(200, 62, 62, 255),
            );
        }
        // 画线图标
        let mut pen_tb: *mut c_void = null_mut();
        let line_col = if is_dark {
            argb(200, 210, 224, 255)
        } else {
            argb(71, 85, 105, 255)
        };
        if GdipCreatePen1(line_col, 1.5, GP_UNIT_PIXEL, &mut pen_tb) == GP_OK && !pen_tb.is_null() {
            let mx = (min_rc.left + min_rc.right) as f32 * 0.5;
            let my = (min_rc.top + min_rc.bottom) as f32 * 0.5;
            let u = spx(5, scale) as f32;
            GdipDrawLine(g, pen_tb, mx - u, my, mx + u, my);
            GdipDeletePen(pen_tb);
        }
        let mut pen_cl: *mut c_void = null_mut();
        let cl_col = if close_hover {
            argb(255, 255, 255, 255)
        } else {
            line_col
        };
        if GdipCreatePen1(cl_col, 1.5, GP_UNIT_PIXEL, &mut pen_cl) == GP_OK && !pen_cl.is_null() {
            let cx = (close_rc.left + close_rc.right) as f32 * 0.5;
            let cy = (close_rc.top + close_rc.bottom) as f32 * 0.5;
            let u = spx(4, scale) as f32;
            GdipDrawLine(g, pen_cl, cx - u, cy - u, cx + u, cy + u);
            GdipDrawLine(g, pen_cl, cx + u, cy - u, cx - u, cy + u);
            GdipDeletePen(pen_cl);
        }

        // 左下角 Toast 状态条（如果有）
        if !toast_text.is_empty() && toast_alpha > 0.02 {
            let a = (240.0 * toast_alpha).round() as u8;
            let fill = if toast_ok {
                if is_dark {
                    argb(22, 101, 52, a)
                } else {
                    argb(220, 252, 231, a)
                }
            } else if is_dark {
                argb(127, 29, 29, a)
            } else {
                argb(254, 226, 226, a)
            };
            let tr = RECT {
                left: spx(10, scale),
                top: h - spx(48, scale),
                right: nav_w - spx(10, scale),
                bottom: h - spx(12, scale),
            };
            gp_fill_round_rect(
                g,
                tr.left as f32,
                tr.top as f32,
                (tr.right - tr.left) as f32,
                (tr.bottom - tr.top) as f32,
                spx(6, scale) as f32,
                fill,
            );
        }

        GdipDeleteGraphics(g);
    }

    // 文本绘制：标题、分类名称、子标签与副标题、Toast
    let wf = utf8_to_wide("Microsoft YaHei UI");
    let hf_title = CreateFontW(
        -(spx(14, scale).max(12)),
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
        wf.as_ptr(),
    );
    let old_f = SelectObject(hdc_buf, hf_title);
    SetBkMode(hdc_buf, TRANSPARENT);
    SetTextColor(hdc_buf, col_fg(is_dark, TextRole::Title));
    let mut t_rc = RECT {
        left: spx(28, scale),
        top: 0,
        right: nav_w - spx(6, scale),
        bottom: title_h,
    };
    let wt_set = utf8_to_wide("快译NG 设置");
    DrawTextW(
        hdc_buf,
        wt_set.as_ptr(),
        -1,
        &mut t_rc,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );

    // 分类文字
    for (i, cat) in DEFAULT_NAV_CATS.iter().enumerate() {
        let mut r = nav_item_rect(i, scale);
        r.left += spx(38, scale);
        let active = i == cur_cat;
        SetTextColor(
            hdc_buf,
            if active {
                if is_dark {
                    rgb(147, 197, 253)
                } else {
                    rgb(37, 99, 235)
                }
            } else {
                col_fg(is_dark, TextRole::Normal)
            },
        );
        let wn = utf8_to_wide(cat.name);
        DrawTextW(
            hdc_buf,
            wn.as_ptr(),
            -1,
            &mut r,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }

    // 右侧顶部大标题 + 副标题或子标签栏
    let cat_def = &DEFAULT_NAV_CATS[cur_cat];
    let mut cat_hdr = RECT {
        left: spx(SET_NAV_W + SET_PAD_L, scale),
        top: 0,
        right: w - spx(96, scale),
        bottom: title_h,
    };
    SetTextColor(hdc_buf, col_fg(is_dark, TextRole::Title));
    let wcat = utf8_to_wide(cat_def.name);
    DrawTextW(
        hdc_buf,
        wcat.as_ptr(),
        -1,
        &mut cat_hdr,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );

    let sub_r = sub_tab_rects(cur_cat, scale);
    if !sub_r.is_empty() {
        for (si, sub_name) in cat_def.sub_names.iter().enumerate() {
            let mut r = sub_r[si];
            let active = si == cur_sub;
            SetTextColor(
                hdc_buf,
                if active {
                    if is_dark {
                        rgb(147, 197, 253)
                    } else {
                        rgb(37, 99, 235)
                    }
                } else {
                    col_fg(is_dark, TextRole::Hint)
                },
            );
            let ws = utf8_to_wide(sub_name);
            DrawTextW(
                hdc_buf,
                ws.as_ptr(),
                -1,
                &mut r,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    } else if !cat_def.subtitle.is_empty() {
        let mut sr = RECT {
            left: spx(SET_NAV_W + SET_PAD_L, scale),
            top: spx(SET_TITLE_H, scale),
            right: w - spx(SET_PAD_L, scale),
            bottom: spx(SET_PAGE_Y, scale),
        };
        SetTextColor(hdc_buf, col_fg(is_dark, TextRole::Hint));
        let wsub = utf8_to_wide(cat_def.subtitle);
        DrawTextW(
            hdc_buf,
            wsub.as_ptr(),
            -1,
            &mut sr,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
    }

    // Toast 文字
    if !toast_text.is_empty() && toast_alpha > 0.02 {
        let mut tr = RECT {
            left: spx(16, scale),
            top: h - spx(48, scale),
            right: nav_w - spx(16, scale),
            bottom: h - spx(12, scale),
        };
        SetTextColor(
            hdc_buf,
            if toast_ok {
                col_fg(is_dark, TextRole::Success)
            } else {
                col_fg(is_dark, TextRole::Danger)
            },
        );
        let wt_msg = utf8_to_wide(&toast_text);
        DrawTextW(
            hdc_buf,
            wt_msg.as_ptr(),
            -1,
            &mut tr,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }

    SelectObject(hdc_buf, old_f);
    DeleteObject(hf_title);

    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
    SelectObject(hdc_buf, old_bm);
    DeleteObject(hbm_buf);
    DeleteDC(hdc_buf);
    EndPaint(hwnd, &ps);
}

unsafe fn page_wnd_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc = RECT::default();
            GetClientRect(hwnd, &mut rc);
            if let Some((page_idx, sc, dark, scroll_y, edits)) = SET_UI.with(|st| {
                let s = st.try_borrow().ok()?;
                let idx = s.page_hwnds.iter().position(|&p| p == hwnd).unwrap_or(0);
                let ed: Vec<(HWND, bool)> = s
                    .ctrls
                    .iter()
                    .filter(|c| c.page_idx == idx && c.is_edit_or_hk)
                    .map(|c| (c.hwnd, c.border_err))
                    .collect();
                Some((idx, s.scale, s.is_dark, s.page_scroll_y[idx], ed))
            }) {
                let w = rc.right - rc.left;
                let h = rc.bottom - rc.top;
                if w > 0 && h > 0 {
                    let hdc_buf = CreateCompatibleDC(hdc);
                    let hbm = CreateCompatibleBitmap(hdc, w, h);
                    let old = SelectObject(hdc_buf, hbm);

                    let bg = CreateSolidBrush(col_bg_win(dark));
                    FillRect(hdc_buf, &rc, bg);
                    DeleteObject(bg);

                    ensure_gdiplus();
                    let mut g: *mut c_void = null_mut();
                    if GdipCreateFromHDC(hdc_buf, &mut g) == GP_OK && !g.is_null() {
                        GdipSetSmoothingMode(g, GP_SMOOTHING_ANTIALIAS);
                        let card_bg = if dark {
                            argb(34, 38, 46, 255)
                        } else {
                            argb(255, 255, 255, 255)
                        };
                        let card_bd = if dark {
                            argb(48, 54, 66, 255)
                        } else {
                            argb(226, 232, 240, 255)
                        };
                        for (cx, cy, cw, ch) in page_cards_logical(page_idx) {
                            gp_round_card(
                                g,
                                spx(cx, sc) as f32,
                                spx(cy - scroll_y, sc) as f32,
                                spx(cw, sc) as f32,
                                spx(ch, sc) as f32,
                                spx(8, sc) as f32,
                                card_bg,
                                card_bd,
                                1.0,
                            );
                        }

                        // 绘制非系统边框输入框外框（支持焦点蓝色高亮与热键冲突红框）
                        let focus_h = crate::ui::win32_ui::GetFocus();
                        for (eh, is_err) in edits {
                            if eh.is_null() || crate::ui::win32_ui::IsWindowVisible(eh) == 0 {
                                continue;
                            }
                            let mut er = RECT::default();
                            GetWindowRect(eh, &mut er);
                            let mut pt1 = POINT {
                                x: er.left,
                                y: er.top,
                            };
                            let mut pt2 = POINT {
                                x: er.right,
                                y: er.bottom,
                            };
                            ScreenToClient(hwnd, &mut pt1);
                            ScreenToClient(hwnd, &mut pt2);
                            let focused = focus_h == eh;
                            let bd = if is_err {
                                argb(239, 68, 68, 255)
                            } else if focused {
                                argb(59, 130, 246, 255)
                            } else if dark {
                                argb(66, 74, 90, 255)
                            } else {
                                argb(203, 213, 225, 255)
                            };
                            let bg_e = if dark {
                                argb(26, 30, 38, 255)
                            } else {
                                argb(255, 255, 255, 255)
                            };
                            gp_round_card(
                                g,
                                (pt1.x - 1) as f32,
                                (pt1.y - 1) as f32,
                                (pt2.x - pt1.x + 2) as f32,
                                (pt2.y - pt1.y + 2) as f32,
                                spx(6, sc) as f32,
                                bg_e,
                                bd,
                                if focused || is_err { 1.4 } else { 1.0 },
                            );
                        }

                        let total_h = page_content_logical_height(page_idx);
                        if total_h > SET_PAGE_H && !(5..=7).contains(&page_idx) {
                            let max_s = total_h - SET_PAGE_H;
                            let track_w = spx(4, sc).max(3);
                            let track_x = (rc.right - rc.left) - track_w - spx(3, sc);
                            let thumb_h = ((h as f32) * (SET_PAGE_H as f32) / (total_h as f32)).round() as i32;
                            let thumb_h = thumb_h.clamp(spx(24, sc), h - spx(8, sc));
                            let travel = (h - thumb_h).max(1);
                            let thumb_y = ((travel as f32) * (scroll_y as f32) / (max_s as f32)).round() as i32;

                            let bar_col = if dark {
                                argb(140, 160, 190, 80)
                            } else {
                                argb(100, 120, 150, 80)
                            };
                            gp_round_card(
                                g,
                                track_x as f32,
                                thumb_y as f32,
                                track_w as f32,
                                thumb_h as f32,
                                (track_w as f32) / 2.0,
                                bar_col,
                                0,
                                0.0,
                            );
                        }

                        GdipDeleteGraphics(g);
                    }

                    BitBlt(hdc, 0, 0, w, h, hdc_buf, 0, 0, SRCCOPY);
                    SelectObject(hdc_buf, old);
                    DeleteObject(hbm);
                    DeleteDC(hdc_buf);
                }
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_CTLCOLORSTATIC => {
            let hdc = wp as HDC;
            let ctrl = lp as HWND;
            let (is_dark, role, in_card, br_c, br_p) = SET_UI.with(|st| {
                let Ok(s) = st.try_borrow() else {
                    return (true, TextRole::Normal, true, null_mut(), null_mut());
                };
                let m = s.ctrls.iter().find(|c| c.hwnd == ctrl);
                (
                    s.is_dark,
                    m.map(|x| x.role).unwrap_or(TextRole::Normal),
                    m.map(|x| x.in_card).unwrap_or(true),
                    s.br_card,
                    s.br_page,
                )
            });
            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, col_fg(is_dark, role));
            (if in_card { br_c } else { br_p }) as LRESULT
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let hdc = wp as HDC;
            let (is_dark, br_e) = SET_UI.with(|st| {
                st.try_borrow()
                    .map(|s| (s.is_dark, s.br_edit))
                    .unwrap_or((true, null_mut()))
            });
            SetBkColor(hdc, col_bg_edit(is_dark));
            SetTextColor(hdc, col_fg(is_dark, TextRole::Normal));
            br_e as LRESULT
        }
        WM_MOUSEWHEEL => {
            let page_idx = SET_UI.with(|st| {
                st.try_borrow()
                    .ok()
                    .and_then(|s| s.page_hwnds.iter().position(|&p| p == hwnd))
                    .unwrap_or(0)
            });
            if (5..=7).contains(&page_idx) {
                DICT_VIEWS.with(|dv| {
                    if let Ok(mut views) = dv.try_borrow_mut() {
                        if let Some(dl) = views[page_idx - 5].as_mut() {
                            dl.handle_wheel(wp);
                        }
                    }
                });
                return 0;
            }
            let delta = (wp >> 16) as i16;
            let step = if delta > 0 { -36 } else { 36 };
            let cur_s = SET_UI.with(|st| st.borrow().page_scroll_y[page_idx]);
            apply_page_scroll(page_idx, cur_s + step);
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            let mut rc = RECT::default();
            GetClientRect(hwnd, &mut rc);
            let (idx, sc, cur_s) = SET_UI.with(|st| {
                let s = st.borrow();
                let i = s.page_hwnds.iter().position(|&p| p == hwnd).unwrap_or(0);
                (i, s.scale, s.page_scroll_y[i])
            });
            if !(5..=7).contains(&idx) {
                let total_h = page_content_logical_height(idx);
                if total_h > SET_PAGE_H {
                    let track_w = spx(14, sc).max(10);
                    let track_x = (rc.right - rc.left) - track_w;
                    if x >= track_x {
                        crate::ui::win32_ui::SetCapture(hwnd);
                        SET_UI.with(|st| {
                            let mut s = st.borrow_mut();
                            s.scroll_dragging = true;
                            s.drag_start_y = y;
                            s.drag_start_scroll = cur_s;
                        });
                        return 0;
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_MOUSEMOVE => {
            let is_drag = SET_UI.with(|st| st.borrow().scroll_dragging);
            if is_drag {
                let (_, y) = lparam_xy(lp);
                let (idx, sc, start_y, start_s) = SET_UI.with(|st| {
                    let s = st.borrow();
                    let i = s.page_hwnds.iter().position(|&p| p == hwnd).unwrap_or(0);
                    (i, s.scale, s.drag_start_y, s.drag_start_scroll)
                });
                let mut rc = RECT::default();
                GetClientRect(hwnd, &mut rc);
                let view_h = rc.bottom - rc.top;
                let total_h = page_content_logical_height(idx);
                let max_s = (total_h - SET_PAGE_H).max(1);
                let thumb_h = ((view_h as f32) * (SET_PAGE_H as f32) / (total_h as f32)).round() as i32;
                let thumb_h = thumb_h.clamp(spx(24, sc), view_h - spx(8, sc));
                let travel = (view_h - thumb_h).max(1);

                let dy = y - start_y;
                let d_scroll = ((dy as f32) * (max_s as f32) / (travel as f32)).round() as i32;
                apply_page_scroll(idx, start_s + d_scroll);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_LBUTTONUP => {
            let is_drag = SET_UI.with(|st| {
                let mut s = st.borrow_mut();
                let was = s.scroll_dragging;
                s.scroll_dragging = false;
                was
            });
            if is_drag {
                crate::ui::win32_ui::ReleaseCapture();
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_VSCROLL => {
            let page_idx = SET_UI.with(|st| {
                st.try_borrow()
                    .ok()
                    .and_then(|s| s.page_hwnds.iter().position(|&p| p == hwnd))
                    .unwrap_or(0)
            });
            if (5..=7).contains(&page_idx) {
                DICT_VIEWS.with(|dv| {
                    if let Ok(mut views) = dv.try_borrow_mut() {
                        if let Some(dl) = views[page_idx - 5].as_mut() {
                            dl.handle_vscroll(wp);
                        }
                    }
                });
            }
            0
        }
        WM_COMMAND => {
            handle_page_command(hwnd, wp, lp);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn handle_page_command(_page_hwnd: HWND, wp: WPARAM, _lp: LPARAM) {
    let id = (wp & 0xFFFF) as i32;
    let code = ((wp >> 16) & 0xFFFF) as u16;

    let (loading, app_dir, hwnd_set, hwnd_main) = SET_UI.with(|st| {
        st.try_borrow()
            .map(|s| (s.loading_guard, s.app_dir.clone(), s.hwnd_set, s.hwnd_main))
            .unwrap_or((true, PathBuf::new(), null_mut(), null_mut()))
    });
    if loading {
        return;
    }

    // 处理 3 张词典页组件命令（通过 try_borrow_mut 免疫 sync_slots 触发 EN_CHANGE 重入）
    if (2000..2300).contains(&id) {
        let res = DICT_VIEWS.with(|dv| {
            let Ok(mut views) = dv.try_borrow_mut() else {
                return None;
            };
            for dl in views.iter_mut().flatten() {
                let (h, r) = dl.handle_command(id, code, &app_dir);
                if h {
                    return r;
                }
            }
            None
        });
        if let Some(r) = res {
            match r {
                Ok(msg) => show_settings_toast(&msg, true),
                Err(err) => show_settings_toast(&err, false),
            }
        }
        return;
    }

    match id {
        ID_CHK_TOP | ID_CHK_CLOSE_EXIT | ID_CHK_AUTO_START | ID_CHK_AUTO_HIDE | ID_CHK_SUMMON
        | ID_CHK_SUMMON_CLEAR | ID_CHK_DBL_CLEAR | ID_CHK_AI | ID_CHK_BAIDU | ID_CHK_SNIP_SUB_WIN => {
            if id == ID_CHK_SNIP_SUB_WIN {
                let val = SET_UI.with(|st| {
                    let s = st.borrow();
                    s.ctrls
                        .iter()
                        .find(|c| c.hwnd == s.chk_snip_sub_win)
                        .map(|m| m.checked)
                        .unwrap_or(true)
                });
                crate::ui::snip_ui::update_snip_cfg_sub_win(val);
            }
            if !hwnd_main.is_null() {
                SetTimer(hwnd_set, DEBOUNCE_TIMER_ID, 200, null());
            }
        }
        ID_CHK_ANIM => {
            if !hwnd_main.is_null() {
                SetTimer(hwnd_set, DEBOUNCE_TIMER_ID, 200, null());
            }
        }
        ID_CMB_THEME => {
            if code == CBN_SELCHANGE && !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_APPLY_SCALE_OR_THEME, 0, 0);
            }
        }
        ID_CMB_SCALE => {
            if code == CBN_SELCHANGE && !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_APPLY_SCALE_OR_THEME, 1, 0);
            }
        }
        ID_BTN_RESET_SIZE => {
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_APPLY_SCALE_OR_THEME, 2, 0);
            }
        }
        ID_CMB_AI_PRESET => {
            if code == CBN_SELCHANGE {
                let (cmb, u, m) = SET_UI.with(|st| {
                    let s = st.borrow();
                    (s.cmb_ai_preset, s.edt_ai_url, s.edt_ai_model)
                });
                let idx = SendMessageW(cmb, CB_GETCURSEL, 0, 0) as usize;
                if let Some(p) = AI_PRESETS.get(idx) {
                    if !p.url.is_empty() {
                        set_window_text(u, p.url);
                        set_window_text(m, p.model);
                        if !hwnd_main.is_null() {
                            PostMessageW(hwnd_main, WM_SET_APPLY_SETTINGS, 0, 0);
                        }
                    }
                }
            }
        }
        ID_BTN_TOGGLE_AI_KEY => {
            let (edt, btn, shown) = SET_UI.with(|st| {
                let mut s = st.borrow_mut();
                s.ai_key_shown = !s.ai_key_shown;
                (s.edt_ai_key, s.btn_toggle_ai_key, s.ai_key_shown)
            });
            SendMessageW(
                edt,
                EM_SETPASSWORDCHAR,
                if shown { 0 } else { '*' as WPARAM },
                0,
            );
            set_window_text(btn, if shown { "隐藏" } else { "显示" });
            InvalidateRect(edt, null(), 1);
        }
        ID_BTN_TOGGLE_BAIDU_KEY => {
            let (edt, btn, shown) = SET_UI.with(|st| {
                let mut s = st.borrow_mut();
                s.baidu_key_shown = !s.baidu_key_shown;
                (s.edt_baidu_key, s.btn_toggle_baidu_key, s.baidu_key_shown)
            });
            SendMessageW(
                edt,
                EM_SETPASSWORDCHAR,
                if shown { 0 } else { '*' as WPARAM },
                0,
            );
            set_window_text(btn, if shown { "隐藏" } else { "显示" });
            InvalidateRect(edt, null(), 1);
        }
        ID_EDT_WIN_W | ID_EDT_WIN_H | ID_EDT_AI_URL | ID_EDT_AI_KEY | ID_EDT_AI_MODEL
        | ID_EDT_BAIDU_APPID | ID_EDT_BAIDU_KEY | ID_EDT_BAIDU_URL | ID_EDT_MAX_WORDS
        | ID_EDT_MAX_CHARS | ID_EDT_SNIP_DIR | ID_EDT_CACHE => {
            if code == EN_CHANGE && !hwnd_set.is_null() {
                SetTimer(hwnd_set, DEBOUNCE_TIMER_ID, 500, null());
            }
        }
        ID_BTN_BROWSE_SNIP => {
            let cur = get_window_text(SET_UI.with(|st| st.borrow().edt_snip_dir));
            if let Some(dir) = browse_for_folder(hwnd_set, "选择截图默认保存目录", &cur) {
                let edt = SET_UI.with(|st| st.borrow().edt_snip_dir);
                set_window_text(edt, &dir);
                if !hwnd_main.is_null() {
                    PostMessageW(hwnd_main, WM_SET_APPLY_SETTINGS, 0, 0);
                }
            }
        }
        ID_BTN_OPEN_SNIP => {
            let dir = get_window_text(SET_UI.with(|st| st.borrow().edt_snip_dir));
            if !dir.is_empty() {
                let _ = std::fs::create_dir_all(&dir);
                shell_open(&dir);
            }
        }
        ID_BTN_OPEN_CACHE => {
            let dir = get_window_text(SET_UI.with(|st| st.borrow().edt_cache));
            if !dir.is_empty() {
                let _ = std::fs::create_dir_all(&dir);
                shell_open(&dir);
            }
        }
        ID_BTN_TRY_CAPTURE => {
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_SNIP_CMD, 1, 0);
            }
        }
        ID_BTN_TRY_PIN => {
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_SNIP_CMD, 2, 0);
            }
        }
        ID_BTN_TRY_SNIP => {
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_SNIP_CMD, 3, 0);
            }
        }
        ID_BTN_CLOSE_PINS => {
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_SNIP_CMD, 4, 0);
            }
        }
        ID_BTN_RESET_HOTKEYS => {
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_RESET_HOTKEYS, 0, 0);
            }
        }
        ID_BTN_CHECK_UPDATE => {
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_CHECK_UPDATE, 0, 0);
            }
        }
        ID_BTN_OPEN_REPO => {
            shell_open(APP_REPO_URL);
        }
        _ => {}
    }
}

unsafe fn settings_wnd_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_NCCALCSIZE => {
            if wp != 0 {
                return 0;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_NCPAINT => 0,
        WM_NCACTIVATE => 1,
        WM_ERASEBKGND => 1,
        WM_DPICHANGED => 0,
        WM_GETMINMAXINFO => {
            if lp != 0 {
                let sc = SET_UI.with(|st| st.try_borrow().map(|s| s.scale).unwrap_or(1.0));
                let w = spx(SET_WIN_W, sc);
                let h = spx(SET_WIN_H, sc);
                let mmi = &mut *(lp as *mut MINMAXINFO);
                mmi.ptMinTrackSize.x = w;
                mmi.ptMinTrackSize.y = h;
                mmi.ptMaxTrackSize.x = w;
                mmi.ptMaxTrackSize.y = h;
            }
            0
        }
        WM_NCHITTEST => {
            let (x, y) = lparam_xy(lp);
            let mut pt = POINT { x, y };
            ScreenToClient(hwnd, &mut pt);
            let sc = SET_UI.with(|st| st.try_borrow().map(|s| s.scale).unwrap_or(1.0));
            let (min_rc, close_rc) = title_btn_rects(sc);
            if pt.y < spx(SET_TITLE_H, sc)
                && !pt_in(min_rc, pt.x, pt.y)
                && !pt_in(close_rc, pt.x, pt.y)
            {
                return HTCAPTION;
            }
            HTCLIENT
        }
        WM_MOUSEMOVE => {
            let (x, y) = lparam_xy(lp);
            let (sc, cur_cat) = SET_UI.with(|st| {
                let s = st.borrow();
                (s.scale, page_to_cat_sub(s.cur_page).0)
            });
            let (min_rc, close_rc) = title_btn_rects(sc);
            let mut nav_h = -1i32;
            for i in 0..DEFAULT_NAV_CATS.len() {
                if pt_in(nav_item_rect(i, sc), x, y) {
                    nav_h = i as i32;
                    break;
                }
            }
            let mut sub_h = -1i32;
            for (si, r) in sub_tab_rects(cur_cat, sc).iter().enumerate() {
                if pt_in(*r, x, y) {
                    sub_h = si as i32;
                    break;
                }
            }
            let mh = pt_in(min_rc, x, y);
            let ch = pt_in(close_rc, x, y);
            let changed = SET_UI.with(|st| {
                let mut s = st.borrow_mut();
                let diff = s.nav_hover != nav_h
                    || s.sub_hover != sub_h
                    || s.min_hover != mh
                    || s.close_hover != ch;
                s.nav_hover = nav_h;
                s.sub_hover = sub_h;
                s.min_hover = mh;
                s.close_hover = ch;
                diff
            });
            let mut tme = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            TrackMouseEvent(&mut tme);
            if changed {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        WM_MOUSELEAVE => {
            SET_UI.with(|st| {
                let mut s = st.borrow_mut();
                s.nav_hover = -1;
                s.sub_hover = -1;
                s.min_hover = false;
                s.close_hover = false;
            });
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            let (sc, cur_page) = SET_UI.with(|st| {
                let s = st.borrow();
                (s.scale, s.cur_page)
            });
            let (min_rc, close_rc) = title_btn_rects(sc);
            if pt_in(min_rc, x, y) {
                ShowWindow(hwnd, SW_MINIMIZE);
                return 0;
            }
            if pt_in(close_rc, x, y) {
                KillTimer(hwnd, DEBOUNCE_TIMER_ID);
                let hwnd_main = SET_UI.with(|st| st.borrow().hwnd_main);
                if !hwnd_main.is_null() {
                    PostMessageW(hwnd_main, WM_SET_APPLY_SETTINGS, 0, 0);
                }
                ShowWindow(hwnd, SW_HIDE);
                return 0;
            }
            for (ci, cat) in DEFAULT_NAV_CATS.iter().enumerate() {
                if pt_in(nav_item_rect(ci, sc), x, y) {
                    if let Some(&first_p) = cat.page_indices.first() {
                        select_settings_tab(first_p);
                    }
                    return 0;
                }
            }
            let (cur_cat, _) = page_to_cat_sub(cur_page);
            for (si, r) in sub_tab_rects(cur_cat, sc).iter().enumerate() {
                if pt_in(*r, x, y) {
                    if let Some(&p) = DEFAULT_NAV_CATS[cur_cat].page_indices.get(si) {
                        select_settings_tab(p);
                    }
                    return 0;
                }
            }
            0
        }
        WM_CLOSE => {
            KillTimer(hwnd, DEBOUNCE_TIMER_ID);
            let hwnd_main = SET_UI.with(|st| st.borrow().hwnd_main);
            if !hwnd_main.is_null() {
                PostMessageW(hwnd_main, WM_SET_APPLY_SETTINGS, 0, 0);
            }
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_TIMER => {
            if wp == DEBOUNCE_TIMER_ID {
                KillTimer(hwnd, DEBOUNCE_TIMER_ID);
                let hwnd_main = SET_UI.with(|st| st.borrow().hwnd_main);
                if !hwnd_main.is_null() {
                    PostMessageW(hwnd_main, WM_SET_APPLY_SETTINGS, 0, 0);
                    PostMessageW(hwnd_main, WM_SET_APPLY_SCALE_OR_THEME, 3, 0);
                }
            } else if wp == TOAST_TIMER_ID {
                let now = GetTickCount64();
                let keep = SET_UI.with(|st| {
                    let mut s = st.borrow_mut();
                    if now >= s.toast_expire_ms {
                        s.toast_alpha -= 0.12;
                        if s.toast_alpha <= 0.0 {
                            s.toast_text.clear();
                            s.toast_alpha = 0.0;
                            return false;
                        }
                    }
                    true
                });
                if !keep {
                    KillTimer(hwnd, TOAST_TIMER_ID);
                }
                InvalidateRect(hwnd, null(), 0);
            } else if wp == ANIM_TIMER_ID {
                let more = SET_UI.with(|st| {
                    let mut s = st.borrow_mut();
                    let dy = s.nav_pill_target_y - s.nav_pill_y;
                    let dx = s.sub_pill_tx - s.sub_pill_x;
                    let dw = s.sub_pill_tw - s.sub_pill_w;
                    let mut active = false;
                    if dy.abs() > 0.4 {
                        s.nav_pill_y += dy * 0.32;
                        active = true;
                    } else {
                        s.nav_pill_y = s.nav_pill_target_y;
                    }
                    if dx.abs() > 0.4 || dw.abs() > 0.4 {
                        s.sub_pill_x += dx * 0.32;
                        s.sub_pill_w += dw * 0.32;
                        active = true;
                    } else {
                        s.sub_pill_x = s.sub_pill_tx;
                        s.sub_pill_w = s.sub_pill_tw;
                    }
                    active
                });
                let sc = SET_UI.with(|st| st.borrow().scale);
                let nav_w = spx(SET_NAV_W, sc) + 2;
                let top_h = spx(SET_PAGE_Y, sc);
                let mut r1 = RECT { left: 0, top: 0, right: nav_w, bottom: 2000 };
                let mut r2 = RECT { left: 0, top: 0, right: 2000, bottom: top_h };
                InvalidateRect(hwnd, &mut r1, 0);
                InvalidateRect(hwnd, &mut r2, 0);
                if !more {
                    KillTimer(hwnd, ANIM_TIMER_ID);
                    crate::ui::win32_ui::timeEndPeriod(1);
                }
            }
            0
        }
        WM_PAINT => {
            paint_settings_window(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// 初始化并预建现代化设置窗口（默认隐藏，打开即秒显）
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn create_settings_window(
    hwnd_main: HWND,
    app_dir: &Path,
    cfg: &AppSettings,
    is_dark: bool,
) -> HWND {
    let sc = (cfg.ui_scale as f64) / 100.0;
    apply_uxtheme_dark_app_mode(is_dark);

    unsafe {
        let h_inst = crate::ui::win32_ui::GetModuleHandleW(null());
        let icon_big = load_app_icon(32, 32);
        let icon_sm = load_app_icon(16, 16);

        let cls_set = utf8_to_wide(SETTINGS_CLASS_NAME);
        let wc_set = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
            lpfnWndProc: Some(settings_wnd_proc_trampoline),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_inst,
            hIcon: icon_big,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: cls_set.as_ptr(),
            hIconSm: icon_sm,
        };
        RegisterClassExW(&wc_set);

        let cls_page = utf8_to_wide(PAGE_CLASS_NAME);
        let wc_page = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(page_wnd_proc_trampoline),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_inst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: cls_page.as_ptr(),
            hIconSm: null_mut(),
        };
        RegisterClassExW(&wc_page);

        let win_w = spx(SET_WIN_W, sc);
        let win_h = spx(SET_WIN_H, sc);
        let wa = crate::ui::win32_ui::primary_work_area();
        let x = wa.left + ((wa.right - wa.left) - win_w).max(0) / 2;
        let y = wa.top + ((wa.bottom - wa.top) - win_h).max(0) / 2;

        let wtitle = utf8_to_wide("设置 - 快译NG");
        let hwnd_set = CreateWindowExW(
            0,
            cls_set.as_ptr(),
            wtitle.as_ptr(),
            WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU | WS_CLIPCHILDREN,
            x,
            y,
            win_w,
            win_h,
            hwnd_main,
            null_mut(),
            h_inst,
            null_mut(),
        );
        if hwnd_set.is_null() {
            return null_mut();
        }
        if !icon_big.is_null() {
            SendMessageW(hwnd_set, WM_SETICON, ICON_BIG, icon_big as LPARAM);
        }
        if !icon_sm.is_null() {
            SendMessageW(hwnd_set, WM_SETICON, ICON_SMALL, icon_sm as LPARAM);
        }

        // Tooltip 提示窗
        let cls_tip = utf8_to_wide(crate::ui::win32_ui::TOOLTIPS_CLASSW_STR);
        let empty = utf8_to_wide("");
        let hwnd_tip = CreateWindowExW(
            0,
            cls_tip.as_ptr(),
            empty.as_ptr(),
            WS_POPUP | 0x01 | 0x02,
            0,
            0,
            0,
            0,
            hwnd_set,
            null_mut(),
            h_inst,
            null_mut(),
        );
        if !hwnd_tip.is_null() {
            SendMessageW(hwnd_tip, TTM_ACTIVATE, 1, 0);
            SendMessageW(hwnd_tip, TTM_SETMAXTIPWIDTH, 0, 280);
        }

        // 创建 12 个子页面面板
        let mut page_hwnds = [null_mut(); 12];
        let px = spx(SET_PAGE_X, sc);
        let py = spx(SET_PAGE_Y, sc);
        let pw = spx(SET_PAGE_W, sc);
        let ph = spx(SET_PAGE_H, sc);
        for (i, slot) in page_hwnds.iter_mut().enumerate() {
            *slot = CreateWindowExW(
                0,
                cls_page.as_ptr(),
                empty.as_ptr(),
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | if i == 0 { WS_VISIBLE } else { 0 },
                px,
                py,
                pw,
                ph,
                hwnd_set,
                null_mut(),
                h_inst,
                null_mut(),
            );
        }

        SET_UI.with(|st| {
            let mut s = st.borrow_mut();
            s.hwnd_set = hwnd_set;
            s.hwnd_main = hwnd_main;
            s.hwnd_tip = hwnd_tip;
            s.page_hwnds = page_hwnds;
            s.scale = sc;
            s.is_dark = is_dark;
            s.anim_enabled = cfg.anim_enabled;
            s.app_dir = app_dir.to_path_buf();
            rebuild_brushes_and_font(&mut s);
        });

        build_all_pages(&page_hwnds, hwnd_tip, sc, is_dark);

        load_settings_into_ui(cfg);
        refresh_settings_appearance(sc, is_dark, cfg.anim_enabled);
        SetWindowPos(
            hwnd_set,
            HWND_TOP,
            x,
            y,
            win_w,
            win_h,
            SWP_FRAMECHANGED | SWP_NOACTIVATE,
        );
        ShowWindow(hwnd_set, SW_HIDE);
        let _ = (
            WS_CAPTION,
            WS_EX_TOOLWINDOW,
            SetCapture,
            ReleaseCapture,
            wheel_delta,
            argb,
            FW_BOLD,
            current_exe_path,
            WM_SHOWWINDOW,
        );
        hwnd_set
    }
}

unsafe extern "system" fn settings_wnd_proc_trampoline(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    settings_wnd_proc(hwnd, msg, wp, lp)
}

unsafe extern "system" fn page_wnd_proc_trampoline(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    page_wnd_proc(hwnd, msg, wp, lp)
}

// ---- 12 页控件工厂与构造 ----

struct PageBuilder {
    hwnd: HWND,
    page_idx: usize,
    sc: f64,
    is_dark: bool,
    hwnd_tip: HWND,
    ctrls: Vec<CtrlStyleMeta>,
    hotkeys: Vec<HotkeyBoxState>,
    combos: Vec<(HWND, (i32, i32, i32, i32))>,
}

impl PageBuilder {
    fn new(hwnd: HWND, page_idx: usize, sc: f64, is_dark: bool, hwnd_tip: HWND) -> Self {
        Self {
            hwnd,
            page_idx,
            sc,
            is_dark,
            hwnd_tip,
            ctrls: Vec::new(),
            hotkeys: Vec::new(),
            combos: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn static_txt(
        &mut self,
        txt: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        in_card: bool,
        role: TextRole,
    ) -> HWND {
        let cls = utf8_to_wide("STATIC");
        let wt = utf8_to_wide(txt);
        let ctrl = CreateWindowExW(
            0,
            cls.as_ptr(),
            wt.as_ptr(),
            WS_CHILD | WS_VISIBLE | 0x2000, // SS_NOPREFIX 避免 & 被识别成快捷键
            spx(x, self.sc),
            spx(y, self.sc),
            spx(w, self.sc),
            spx(h, self.sc),
            self.hwnd,
            null_mut(),
            null_mut(),
            null_mut(),
        );
        self.ctrls.push(CtrlStyleMeta {
            hwnd: ctrl,
            page_idx: self.page_idx,
            in_card,
            role,
            logic_rect: (x, y, w, h),
            ..Default::default()
        });
        ctrl
    }

    unsafe fn sec_title(&mut self, txt: &str, y: i32) {
        self.static_txt(
            &format!("▍ {txt}"),
            SET_PAD_L + CARD_PAD_X,
            y,
            320,
            24,
            true,
            TextRole::Title,
        );
    }

    unsafe fn chk(
        &mut self,
        txt: &str,
        id: i32,
        x: i32,
        y: i32,
        w: i32,
        tip: Option<&str>,
    ) -> HWND {
        let cls = utf8_to_wide("BUTTON");
        let wt = utf8_to_wide(txt);
        let ctrl = CreateWindowExW(
            0,
            cls.as_ptr(),
            wt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x03,
            spx(x, self.sc),
            spx(y, self.sc),
            spx(w, self.sc),
            spx(26, self.sc),
            self.hwnd,
            id as _,
            null_mut(),
            null_mut(),
        );
        SetWindowSubclass(ctrl, modern_chk_subclass, 0x4348, 0);
        if let Some(t) = tip {
            attach_tooltip(self.hwnd_tip, ctrl, t);
        }
        self.ctrls.push(CtrlStyleMeta {
            hwnd: ctrl,
            page_idx: self.page_idx,
            in_card: true,
            logic_rect: (x, y, w, 26),
            ..Default::default()
        });
        ctrl
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn btn(
        &mut self,
        txt: &str,
        id: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        kind: BtnKind,
    ) -> HWND {
        let cls = utf8_to_wide("BUTTON");
        let wt = utf8_to_wide(txt);
        let ctrl = CreateWindowExW(
            0,
            cls.as_ptr(),
            wt.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x0B,
            spx(x, self.sc),
            spx(y, self.sc),
            spx(w, self.sc),
            spx(h, self.sc),
            self.hwnd,
            id as _,
            null_mut(),
            null_mut(),
        );
        SetWindowSubclass(ctrl, modern_btn_subclass, 0x4254, 0);
        self.ctrls.push(CtrlStyleMeta {
            hwnd: ctrl,
            page_idx: self.page_idx,
            in_card: true,
            btn_kind: kind,
            logic_rect: (x, y, w, h),
            ..Default::default()
        });
        ctrl
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn edit(
        &mut self,
        id: i32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        extra_style: DWORD,
        tip: Option<&str>,
    ) -> HWND {
        let cls = utf8_to_wide("EDIT");
        let empty = utf8_to_wide("");
        let ctrl = CreateWindowExW(
            0,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | extra_style,
            spx(x, self.sc),
            spx(y, self.sc),
            spx(w, self.sc),
            spx(h, self.sc),
            self.hwnd,
            id as _,
            null_mut(),
            null_mut(),
        );
        SendMessageW(
            ctrl,
            EM_SETMARGINS,
            EC_LEFTMARGIN | EC_RIGHTMARGIN,
            (6 << 16) | 6,
        );
        SetWindowSubclass(ctrl, edit_focus_subclass, 0x4544, 0);
        if let Some(t) = tip {
            attach_tooltip(self.hwnd_tip, ctrl, t);
        }
        self.ctrls.push(CtrlStyleMeta {
            hwnd: ctrl,
            page_idx: self.page_idx,
            in_card: true,
            is_edit_or_hk: true,
            logic_rect: (x, y, w, h),
            ..Default::default()
        });
        ctrl
    }

    unsafe fn hotkey_box(
        &mut self,
        id: i32,
        x: i32,
        y: i32,
        w: i32,
        hk_idx: usize,
        is_mirror: bool,
    ) -> HWND {
        let cls = utf8_to_wide("EDIT");
        let empty = utf8_to_wide("");
        let ctrl = CreateWindowExW(
            0,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_READONLY | 0x01, // ES_CENTER
            spx(x, self.sc),
            spx(y, self.sc),
            spx(w, self.sc),
            spx(28, self.sc),
            self.hwnd,
            id as _,
            null_mut(),
            null_mut(),
        );
        SetWindowSubclass(ctrl, hotkey_edit_subclass, 0x484B, 0);
        self.ctrls.push(CtrlStyleMeta {
            hwnd: ctrl,
            page_idx: self.page_idx,
            in_card: true,
            is_edit_or_hk: true,
            logic_rect: (x, y, w, 28),
            ..Default::default()
        });
        self.hotkeys.push(HotkeyBoxState {
            hwnd: ctrl,
            mod_orig: 0,
            vk: 0,
            is_snip_mirror: is_mirror,
            hk_idx,
        });
        ctrl
    }

    unsafe fn combo(
        &mut self,
        id: i32,
        x: i32,
        y: i32,
        w: i32,
        items: &[&str],
        tip: Option<&str>,
    ) -> HWND {
        let cls = utf8_to_wide("COMBOBOX");
        let empty = utf8_to_wide("");
        let ctrl = CreateWindowExW(
            0,
            cls.as_ptr(),
            empty.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST | CBS_HASSTRINGS,
            spx(x, self.sc),
            spx(y, self.sc),
            spx(w, self.sc),
            spx(200, self.sc),
            self.hwnd,
            id as _,
            null_mut(),
            null_mut(),
        );
        for it in items {
            let wi = utf8_to_wide(it);
            SendMessageW(ctrl, CB_ADDSTRING, 0, wi.as_ptr() as LPARAM);
        }
        set_control_theme(ctrl, if self.is_dark { "DarkMode_CFD" } else { "CFD" });
        if let Some(t) = tip {
            attach_tooltip(self.hwnd_tip, ctrl, t);
        }
        self.combos.push((ctrl, (x, y, w, 200)));
        ctrl
    }

    fn flush(self) {
        SET_UI.with(|st| {
            let mut s = st.borrow_mut();
            s.ctrls.extend(self.ctrls);
            s.hotkeys.extend(self.hotkeys);
            s.combos.extend(self.combos);
        });
    }
}

unsafe fn build_all_pages(pages: &[HWND; 12], hwnd_tip: HWND, sc: f64, is_dark: bool) {
    let cx = SET_PAD_L + CARD_PAD_X;

    // ---- Page 0: 常规 - 基本 ----
    let mut p0 = PageBuilder::new(pages[0], 0, sc, is_dark, hwnd_tip);
    p0.sec_title("窗口与退出", 26);
    let chk_top = p0.chk(
        "软件窗口永远在最上面",
        ID_CHK_TOP,
        cx,
        58,
        INNER_W,
        Some("开启后翻译主窗口始终置顶显示在其他窗口上方"),
    );
    let chk_close_exit = p0.chk(
        "点击关闭直接退出程序",
        ID_CHK_CLOSE_EXIT,
        cx,
        94,
        INNER_W,
        Some("开启：点右上角关闭直接退出；关闭：隐藏到系统托盘"),
    );
    p0.sec_title("启动与托盘", 168);
    let chk_auto_start = p0.chk(
        "开机自动启动快译NG",
        ID_CHK_AUTO_START,
        cx,
        200,
        INNER_W,
        Some("写入当前用户启动项，登录 Windows 时自动运行"),
    );
    let chk_auto_hide = p0.chk(
        "开机启动后隐藏到托盘",
        ID_CHK_AUTO_HIDE,
        cx,
        236,
        INNER_W,
        Some("仅在开启“开机自动启动”后生效，开机后直接静默隐藏到托盘"),
    );
    p0.static_txt(
        "所有设置修改后立即生效并自动保存，无需点击保存按钮。",
        SET_PAD_L + 4,
        300,
        SET_CONT_W - 8,
        22,
        false,
        TextRole::Hint,
    );
    p0.flush();

    // ---- Page 1: 常规 - 界面 ----
    let mut p1 = PageBuilder::new(pages[1], 1, sc, is_dark, hwnd_tip);
    p1.sec_title("外观与缩放", 26);
    p1.static_txt("界面主题", cx, 62, 82, 22, true, TextRole::Normal);
    let cmb_theme = p1.combo(ID_CMB_THEME, cx + 90, 58, 148, &THEME_PRESETS, None);
    p1.static_txt("字体缩放", cx, 104, 82, 22, true, TextRole::Normal);
    let scale_labels: Vec<String> = UI_SCALE_PRESETS.iter().map(|p| format!("{p}%")).collect();
    let scale_refs: Vec<&str> = scale_labels.iter().map(String::as_str).collect();
    let cmb_scale = p1.combo(
        ID_CMB_SCALE,
        cx + 90,
        100,
        148,
        &scale_refs,
        Some("独立于系统 DPI，整窗按所选百分比等比缩放"),
    );
    p1.static_txt(
        "实时等比缩放翻译窗与设置窗",
        cx + 252,
        104,
        250,
        22,
        true,
        TextRole::Hint,
    );
    let chk_anim = p1.chk(
        "启用界面切换与悬停动效",
        ID_CHK_ANIM,
        cx,
        144,
        INNER_W,
        Some("关闭后切页、开关和悬停将立即响应，无过渡动画"),
    );
    p1.sec_title("主窗口尺寸", 214);
    p1.static_txt("窗口大小", cx, 252, 82, 22, true, TextRole::Normal);
    let edt_win_w = p1.edit(ID_EDT_WIN_W, cx + 90, 248, 74, 28, ES_NUMBER, None);
    p1.static_txt("×", cx + 172, 252, 16, 22, true, TextRole::Hint);
    let edt_win_h = p1.edit(ID_EDT_WIN_H, cx + 194, 248, 74, 28, ES_NUMBER, None);
    p1.static_txt("像素", cx + 276, 252, 44, 22, true, TextRole::Hint);
    p1.btn(
        "恢复默认",
        ID_BTN_RESET_SIZE,
        cx + 332,
        248,
        96,
        28,
        BtnKind::Normal,
    );
    p1.flush();

    // ---- Page 2: 常规 - 行为 ----
    let mut p2 = PageBuilder::new(pages[2], 2, sc, is_dark, hwnd_tip);
    p2.sec_title("召唤与输入行为", 26);
    let chk_summon = p2.chk(
        "热键召唤窗口时出现在鼠标旁边",
        ID_CHK_SUMMON,
        cx,
        60,
        INNER_W,
        Some("按全局热键显示窗口时，自动定位到当前鼠标光标附近"),
    );
    let chk_summon_clear = p2.chk(
        "热键召唤窗口时清空输入区和结果区",
        ID_CHK_SUMMON_CLEAR,
        cx,
        98,
        INNER_W,
        Some("按全局热键召唤主窗口时，自动清空上次的输入与翻译结果"),
    );
    let chk_dbl_clear = p2.chk(
        "双击输入区＝清空输入区",
        ID_CHK_DBL_CLEAR,
        cx,
        136,
        INNER_W,
        Some("开启：双击输入区清空内容；关闭：双击直接按命名规则生成并复制变量名"),
    );
    p2.flush();

    // ---- Page 3: 翻译接口 ----
    let mut p3 = PageBuilder::new(pages[3], 3, sc, is_dark, hwnd_tip);
    let chk_ai = p3.chk("AI 接口（OpenAI 格式）", ID_CHK_AI, cx, 26, 228, None);
    let preset_names: Vec<&str> = AI_PRESETS.iter().map(|p| p.name).collect();
    let cmb_ai_preset = p3.combo(ID_CMB_AI_PRESET, cx + 236, 24, 142, &preset_names, None);
    p3.static_txt("地址可带 /v1", cx + 386, 28, 120, 22, true, TextRole::Hint);
    let edt_ai_url = p3.edit(ID_EDT_AI_URL, cx, 64, 250, 28, 0, None);
    p3.static_txt("密钥", cx + 258, 68, 36, 22, true, TextRole::Normal);
    let edt_ai_key = p3.edit(ID_EDT_AI_KEY, cx + 298, 64, 150, 28, ES_PASSWORD, None);
    let btn_toggle_ai_key = p3.btn(
        "显示",
        ID_BTN_TOGGLE_AI_KEY,
        cx + 456,
        64,
        56,
        28,
        BtnKind::Normal,
    );
    p3.static_txt("模型", cx, 108, 40, 22, true, TextRole::Normal);
    let edt_ai_model = p3.edit(ID_EDT_AI_MODEL, cx + 46, 104, 204, 28, 0, None);

    let chk_baidu = p3.chk(
        "百度翻译开放平台（fanyi-api.baidu.com）",
        ID_CHK_BAIDU,
        cx,
        180,
        INNER_W,
        None,
    );
    p3.static_txt("AppID", cx, 220, 56, 22, true, TextRole::Normal);
    let edt_baidu_appid = p3.edit(ID_EDT_BAIDU_APPID, cx + 60, 216, 150, 28, 0, None);
    p3.static_txt("密钥", cx + 218, 220, 38, 22, true, TextRole::Normal);
    let edt_baidu_key = p3.edit(ID_EDT_BAIDU_KEY, cx + 260, 216, 188, 28, ES_PASSWORD, None);
    let btn_toggle_baidu_key = p3.btn(
        "显示",
        ID_BTN_TOGGLE_BAIDU_KEY,
        cx + 456,
        216,
        56,
        28,
        BtnKind::Normal,
    );
    p3.static_txt(
        "接口地址（可选，默认官方）",
        cx,
        260,
        226,
        22,
        true,
        TextRole::Hint,
    );
    let edt_baidu_url = p3.edit(ID_EDT_BAIDU_URL, cx + 234, 256, INNER_W - 234, 28, 0, None);

    p3.static_txt(
        "翻译顺序：AI → 百度 → 必应免费（未启用或失败自动回退）。",
        cx,
        338,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p3.static_txt(
        "单词词典始终走必应；百度免费版每秒 1 次、每月 5 万字符。",
        cx,
        366,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p3.static_txt(
        "Ollama 本地无需密钥；百度需开通“通用文本翻译”权限。",
        cx,
        394,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p3.flush();

    // ---- Page 4: 命名 - 缩减预算 ----
    let mut p4 = PageBuilder::new(pages[4], 4, sc, is_dark, hwnd_tip);
    p4.sec_title("命名缩减预算", 26);
    p4.static_txt("最大词数", cx, 64, 96, 22, true, TextRole::Normal);
    let edt_max_words = p4.edit(ID_EDT_MAX_WORDS, cx + 104, 60, 72, 28, ES_NUMBER, None);
    p4.static_txt(
        "默认 4，超出自动按优先级裁剪（1~16）",
        cx + 190,
        64,
        316,
        22,
        true,
        TextRole::Hint,
    );
    p4.static_txt("最大字符数", cx, 106, 96, 22, true, TextRole::Normal);
    let edt_max_chars = p4.edit(ID_EDT_MAX_CHARS, cx + 104, 102, 72, 28, ES_NUMBER, None);
    p4.static_txt(
        "默认 40，超长时优先触发词尾缩写（8~128）",
        cx + 190,
        106,
        316,
        22,
        true,
        TextRole::Hint,
    );
    p4.sec_title("规则优先级说明", 192);
    p4.static_txt(
        "优先级：自定义忽略词 ＞ 自定义缩略词 ＞ 常用词库 ＞ 内置规则",
        cx,
        226,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p4.static_txt(
        "命名风格（snake_case / 驼峰法）与命名对象请在主窗口工具栏直接切换。",
        cx,
        256,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p4.flush();

    // ---- Pages 5/6/7: 缩略词 / 忽略词 / 常用词 ----
    let mut p5 = PageBuilder::new(pages[5], 5, sc, is_dark, hwnd_tip);
    p5.static_txt(
        "原词 → 缩写，例 Initialization → Init（内置已列出）",
        cx,
        18,
        INNER_W,
        22,
        true,
        TextRole::Normal,
    );
    p5.static_txt(
        "可改可删：删行＝该词不再缩写；空白行忽略，点击「应用本页」生效。",
        cx,
        46,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p5.flush();
    let dl_abbr = DictListView::create(
        pages[5],
        DICT_BASE_ABBR,
        DictKind::Abbr,
        "",
        "",
        "搜索原词或缩写…",
        "原词",
        "缩写",
        sc,
        is_dark,
    );
    SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        for b in [dl_abbr.btn_clear, dl_abbr.btn_add, dl_abbr.btn_apply] {
            SetWindowSubclass(b, modern_btn_subclass, 0x4254, 0);
            s.ctrls.push(CtrlStyleMeta {
                hwnd: b,
                page_idx: 5,
                in_card: false,
                btn_kind: if b == dl_abbr.btn_apply {
                    BtnKind::Primary
                } else {
                    BtnKind::Normal
                },
                ..Default::default()
            });
        }
        for b in dl_abbr.del_btns {
            SetWindowSubclass(b, modern_btn_subclass, 0x4254, 0);
            s.ctrls.push(CtrlStyleMeta {
                hwnd: b,
                page_idx: 5,
                in_card: false,
                btn_kind: BtnKind::Danger,
                ..Default::default()
            });
        }
    });
    DICT_VIEWS.with(|dv| dv.borrow_mut()[0] = Some(dl_abbr));

    let mut p6 = PageBuilder::new(pages[6], 6, sc, is_dark, hwnd_tip);
    p6.static_txt(
        "每行一个英文单词，生成命名时直接删掉（内置泛词已列出）",
        cx,
        18,
        INNER_W,
        22,
        true,
        TextRole::Normal,
    );
    p6.static_txt(
        "删掉某行＝保留该词；中文先经内置词表转英文再判断，点击「应用本页」生效。",
        cx,
        46,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p6.flush();
    let dl_ignore = DictListView::create(
        pages[6],
        DICT_BASE_IGNORE,
        DictKind::Ignore,
        "",
        "",
        "搜索忽略词…",
        "英文单词",
        "",
        sc,
        is_dark,
    );
    SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        for b in [dl_ignore.btn_clear, dl_ignore.btn_add, dl_ignore.btn_apply] {
            SetWindowSubclass(b, modern_btn_subclass, 0x4254, 0);
            s.ctrls.push(CtrlStyleMeta {
                hwnd: b,
                page_idx: 6,
                in_card: false,
                btn_kind: if b == dl_ignore.btn_apply {
                    BtnKind::Primary
                } else {
                    BtnKind::Normal
                },
                ..Default::default()
            });
        }
        for b in dl_ignore.del_btns {
            SetWindowSubclass(b, modern_btn_subclass, 0x4254, 0);
            s.ctrls.push(CtrlStyleMeta {
                hwnd: b,
                page_idx: 6,
                in_card: false,
                btn_kind: BtnKind::Danger,
                ..Default::default()
            });
        }
    });
    DICT_VIEWS.with(|dv| dv.borrow_mut()[1] = Some(dl_ignore));

    let mut p7 = PageBuilder::new(pages[7], 7, sc, is_dark, hwnd_tip);
    p7.static_txt(
        "每行一条：中文 → 英文词库，例 串口 → uart（内置已列出）",
        cx,
        18,
        INNER_W,
        22,
        true,
        TextRole::Normal,
    );
    p7.static_txt(
        "改英文＝覆盖内置映射，删行＝停用；可加全新词条，点击「应用本页」生效。",
        cx,
        46,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    p7.flush();
    let dl_pref = DictListView::create(
        pages[7],
        DICT_BASE_PREF,
        DictKind::Preferred,
        "",
        "",
        "搜索中文或英文…",
        "中文词",
        "英文词",
        sc,
        is_dark,
    );
    SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        for b in [dl_pref.btn_clear, dl_pref.btn_add, dl_pref.btn_apply] {
            SetWindowSubclass(b, modern_btn_subclass, 0x4254, 0);
            s.ctrls.push(CtrlStyleMeta {
                hwnd: b,
                page_idx: 7,
                in_card: false,
                btn_kind: if b == dl_pref.btn_apply {
                    BtnKind::Primary
                } else {
                    BtnKind::Normal
                },
                ..Default::default()
            });
        }
        for b in dl_pref.del_btns {
            SetWindowSubclass(b, modern_btn_subclass, 0x4254, 0);
            s.ctrls.push(CtrlStyleMeta {
                hwnd: b,
                page_idx: 7,
                in_card: false,
                btn_kind: BtnKind::Danger,
                ..Default::default()
            });
        }
    });
    DICT_VIEWS.with(|dv| dv.borrow_mut()[2] = Some(dl_pref));

    // ---- Page 8: 截屏与贴图 ----
    let mut p8 = PageBuilder::new(pages[8], 8, sc, is_dark, hwnd_tip);
    p8.sec_title("截屏与贴图快捷键", 22);
    let snip_specs = [
        ("区域截屏（含工具栏）", 50, ID_HK_SNIP_CAPTURE, 5usize),
        ("贴图到屏幕", 84, ID_HK_SNIP_PIN, 6usize),
        ("直接截屏翻译", 118, ID_HK_SNIP_TRANS, 4usize),
    ];
    let mut lbl_snip_hk_status = [null_mut(); 3];
    for (i, (label, y, cid, hidx)) in snip_specs.into_iter().enumerate() {
        p8.static_txt(label, cx, y + 4, 166, 22, true, TextRole::Normal);
        p8.hotkey_box(cid, cx + 172, y, 206, hidx, true);
        lbl_snip_hk_status[i] = p8.static_txt("", cx + 388, y + 4, 120, 22, true, TextRole::Hint);
    }
    p8.sec_title("截图默认保存位置", 178);
    p8.static_txt(
        "F2 翻译选区 · Ctrl+S 另存 · Ctrl+Shift+S 快速保存 · Ctrl+C 复制 · 贴图内 Ctrl+左键 划字复制",
        cx,
        206,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    let edt_snip_dir = p8.edit(
        ID_EDT_SNIP_DIR,
        cx,
        234,
        INNER_W - 196,
        28,
        0,
        Some("按 Ctrl+Shift+S 时截图将直接保存到此目录"),
    );
    p8.btn(
        "选择目录",
        ID_BTN_BROWSE_SNIP,
        cx + INNER_W - 186,
        234,
        88,
        28,
        BtnKind::Normal,
    );
    p8.btn(
        "打开目录",
        ID_BTN_OPEN_SNIP,
        cx + INNER_W - 90,
        234,
        90,
        28,
        BtnKind::Normal,
    );
    p8.sec_title("截屏选项与操作说明", 292);
    let chk_snip_sub_win = p8.chk(
        "自动识别窗口子分区（顶部工具栏/分区与整窗切换）",
        ID_CHK_SNIP_SUB_WIN,
        cx,
        318,
        INNER_W,
        Some("启用后截屏可自动识别窗口内部工具栏与各子分区，随时可通过滚轮或Tab键切换整窗；取消勾选则只选取大窗口"),
    );
    p8.static_txt(
        "• 截屏选区：自动识窗、8向调框、放大镜取色（单按 C 复制色值）",
        cx,
        344,
        INNER_W,
        20,
        true,
        TextRole::Hint,
    );
    p8.static_txt(
        "• 快捷操作：滚轮或Tab切换整窗与分区、Ctrl+C复制、Ctrl+S保存",
        cx,
        364,
        INNER_W,
        20,
        true,
        TextRole::Hint,
    );
    p8.static_txt(
        "• 标注工具：框/圆/线/箭头/画笔/荧光笔/序号/文字/马赛克/模糊",
        cx,
        384,
        INNER_W,
        20,
        true,
        TextRole::Hint,
    );
    p8.static_txt(
        "• 屏幕贴图：支持图文贴图、多图置顶、滚轮缩放与透明度调节",
        cx,
        404,
        INNER_W,
        20,
        true,
        TextRole::Hint,
    );
    let btn_try_capture = p8.btn(
        "立即截屏 (F1)",
        ID_BTN_TRY_CAPTURE,
        cx,
        438,
        200,
        32,
        BtnKind::Primary,
    );
    let btn_try_pin = p8.btn(
        "剪贴板贴图 (F3)",
        ID_BTN_TRY_PIN,
        cx + 214,
        438,
        200,
        32,
        BtnKind::Normal,
    );
    p8.btn(
        "直接截屏翻译",
        ID_BTN_TRY_SNIP,
        cx,
        478,
        200,
        32,
        BtnKind::Normal,
    );
    p8.btn(
        "关闭所有贴图",
        ID_BTN_CLOSE_PINS,
        cx + 214,
        478,
        200,
        32,
        BtnKind::Danger,
    );
    p8.flush();

    // ---- Page 9: 控制（全局快捷键） ----
    let mut p9 = PageBuilder::new(pages[9], 9, sc, is_dark, hwnd_tip);
    let hk_specs = [
        ("显示/隐藏快译", 24),
        ("上一个历史记录", 66),
        ("下一个历史记录", 108),
        ("复制单词", 150),
        ("直接截屏翻译", 192),
        ("截屏（含工具栏）", 234),
        ("贴图到屏幕", 276),
    ];
    let mut lbl_hk_status = [null_mut(); 7];
    for (i, (label, y)) in hk_specs.into_iter().enumerate() {
        p9.static_txt(label, cx, y + 4, 166, 24, true, TextRole::Normal);
        p9.hotkey_box(ID_HK_CTRL_BASE + (i as i32), cx + 172, y, 206, i, false);
        lbl_hk_status[i] = p9.static_txt("", cx + 388, y + 4, 120, 24, true, TextRole::Hint);
    }
    p9.static_txt(
        "按下组合键立即生效，按 Esc/Backspace 清空，红框表示被占用。",
        cx,
        322,
        INNER_W,
        24,
        true,
        TextRole::Hint,
    );
    p9.btn(
        "恢复默认热键",
        ID_BTN_RESET_HOTKEYS,
        cx + 172,
        358,
        128,
        32,
        BtnKind::Normal,
    );
    p9.flush();

    // ---- Page 10: 存储 ----
    let mut p10 = PageBuilder::new(pages[10], 10, sc, is_dark, hwnd_tip);
    p10.sec_title("缓存目录与查询历史", 26);
    p10.static_txt(
        "查询历史（上限 200 条）和运行日志保存在此目录，修改后立即生效。",
        cx,
        56,
        INNER_W,
        22,
        true,
        TextRole::Hint,
    );
    let edt_cache = p10.edit(
        ID_EDT_CACHE,
        cx,
        92,
        INNER_W - 108,
        28,
        0,
        Some("存放查询历史（history.txt）与运行日志（run.log）的目录"),
    );
    p10.btn(
        "打开目录",
        ID_BTN_OPEN_CACHE,
        cx + INNER_W - 96,
        92,
        96,
        28,
        BtnKind::Normal,
    );
    p10.flush();

    // ---- Page 11: 关于 ----
    let mut p11 = PageBuilder::new(pages[11], 11, sc, is_dark, hwnd_tip);
    p11.sec_title("快译NG", 28);
    let lbl_update_status = p11.static_txt(
        &format!("当前版本 v{APP_VERSION}"),
        cx,
        62,
        INNER_W,
        24,
        true,
        TextRole::Normal,
    );
    p11.static_txt(
        "轻量级 Windows 单文件翻译与编程命名工具（纯 Rust 原生实现）。",
        cx,
        94,
        INNER_W,
        24,
        true,
        TextRole::Hint,
    );
    let btn_check_update = p11.btn(
        "检查更新",
        ID_BTN_CHECK_UPDATE,
        cx,
        142,
        104,
        32,
        BtnKind::Primary,
    );
    p11.btn(
        "GitHub 主页",
        ID_BTN_OPEN_REPO,
        cx + 116,
        142,
        116,
        32,
        BtnKind::Normal,
    );
    p11.flush();

    SET_UI.with(|st| {
        let mut s = st.borrow_mut();
        s.chk_top = chk_top;
        s.chk_close_exit = chk_close_exit;
        s.chk_auto_start = chk_auto_start;
        s.chk_auto_hide = chk_auto_hide;
        s.cmb_theme = cmb_theme;
        s.cmb_scale = cmb_scale;
        s.chk_anim = chk_anim;
        s.edt_win_w = edt_win_w;
        s.edt_win_h = edt_win_h;
        s.chk_summon = chk_summon;
        s.chk_summon_clear = chk_summon_clear;
        s.chk_dbl_clear = chk_dbl_clear;
        s.chk_ai = chk_ai;
        s.cmb_ai_preset = cmb_ai_preset;
        s.edt_ai_url = edt_ai_url;
        s.edt_ai_key = edt_ai_key;
        s.btn_toggle_ai_key = btn_toggle_ai_key;
        s.edt_ai_model = edt_ai_model;
        s.chk_baidu = chk_baidu;
        s.edt_baidu_appid = edt_baidu_appid;
        s.edt_baidu_key = edt_baidu_key;
        s.btn_toggle_baidu_key = btn_toggle_baidu_key;
        s.edt_baidu_url = edt_baidu_url;
        s.edt_max_words = edt_max_words;
        s.edt_max_chars = edt_max_chars;
        s.chk_snip_sub_win = chk_snip_sub_win;
        s.edt_snip_dir = edt_snip_dir;
        s.lbl_snip_hk_status = lbl_snip_hk_status;
        s.lbl_hk_status = lbl_hk_status;
        s.btn_try_capture = btn_try_capture;
        s.btn_try_pin = btn_try_pin;
        s.edt_cache = edt_cache;
        s.lbl_update_status = lbl_update_status;
        s.btn_check_update = btn_check_update;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_nav_and_card_bounds() {
        for p in 0..12 {
            let (cat, sub) = page_to_cat_sub(p);
            assert_eq!(DEFAULT_NAV_CATS[cat].page_indices[sub], p);
            for (x, y, w, h) in page_cards_logical(p) {
                assert!(x >= SET_PAD_L);
                assert!(x + w <= SET_PAGE_W);
                assert!(y >= 8);
                let max_h = page_content_logical_height(p);
                assert!(y + h <= max_h, "page {p} card exceeds page content height");
            }
        }
        let r0 = nav_item_rect(0, 1.0);
        let r6 = nav_item_rect(6, 1.0);
        assert!(r6.bottom < SET_WIN_H);
        assert!(r0.right < SET_NAV_W);
    }
}
