//! 快译NG 纯 Rust 主入口：单实例、托盘、全局热键、翻译/更新/截屏后台调度与消息循环

#![cfg_attr(not(test), windows_subsystem = "windows")]

use kuaiyi_ng::core::bing::translate_chain;
use kuaiyi_ng::core::cfg::Config;
use kuaiyi_ng::core::hist::History;
use kuaiyi_ng::core::log::{log_install_crash_filter, log_open, log_write};
use kuaiyi_ng::core::str_util::{has_cjk, utf8_to_wide};
use kuaiyi_ng::core::update::{check_update, download_update};
use kuaiyi_ng::core::{last_engine, last_error, FY_AUTO, FY_FMT_SNAKE};
use kuaiyi_ng::naming_reducer::{
    load_custom_abbreviations, load_custom_ignore, load_custom_preferred,
    reduce_name_with_options, NameKind, NamingOptions, NamingStyle, VerbForm, WordOrder,
};
use kuaiyi_ng::ui::modern_main::{
    create_modern_main_window, focus_and_select_input, get_input_text, get_result_text,
    handle_main_ui_message, hotkey_hint_str, relayout_main_controls, set_input_text,
    set_main_input_dbl_clear, set_main_naming_selection, set_main_title_and_hint, set_main_topmost,
    set_main_translating, set_result_text, sync_main_theme_and_font, ID_KIND, ID_STYLE,
    MAIN_CLASS_NAME, WM_MAIN_CMD_CLOSE_REQ, WM_MAIN_CMD_INPUT_CHANGED, WM_MAIN_CMD_KIND_CHANGED,
    WM_MAIN_CMD_STYLE_CHANGED, WM_MAIN_CMD_TOPMOST, WM_MAIN_CMD_TRANSLATE,
};
use kuaiyi_ng::ui::modern_settings::{
    collect_settings_from_ui, create_settings_window, load_settings_into_ui, open_settings_window,
    refresh_settings_appearance, select_settings_tab, set_about_update_status, show_settings_toast,
    update_hotkey_status_ui, WM_SET_APPLY_HOTKEYS, WM_SET_APPLY_SCALE_OR_THEME,
    WM_SET_APPLY_SETTINGS, WM_SET_CHECK_UPDATE, WM_SET_RESET_HOTKEYS, WM_SET_SNIP_CMD,
};
use kuaiyi_ng::ui::snip_ui::{
    close_all_pins, create_pin_from_clipboard, handle_pin_done_message, handle_pin_lines_message,
    handle_sel_trans_message, handle_snip_done_message,
    has_active_pins, start_snip_capture, start_snip_translate, update_snip_cfg, SnipCfg,
};
use kuaiyi_ng::ui::win32_ui::{
    init_ui_libraries, load_app_icon, set_clipboard_text, shell_open, shutdown_gdiplus,
    work_area_from_point, AppendMenuW, AttachThreadInput, BringWindowToTop, CloseHandle,
    CreateMutexW, CreatePopupMenu, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW,
    FindWindowW, GetCursorPos, GetForegroundWindow, GetLastError, GetMessageW, GetWindowRect,
    GetWindowThreadProcessId, IsWindowVisible, KillTimer, MessageBoxW, PostMessageW,
    PostQuitMessage, RegisterHotKey, SetForegroundWindow, SetTimer, SetWindowPos,
    Shell_NotifyIconW, ShowWindow, TrackPopupMenu, TranslateMessage, UnregisterHotKey,
    ERROR_ALREADY_EXISTS, HANDLE, HWND, HWND_NOTOPMOST, HWND_TOPMOST, IDYES, LPARAM, LRESULT,
    MB_ICONINFORMATION, MB_ICONWARNING, MB_YESNO, MF_CHECKED, MF_SEPARATOR, MF_STRING,
    MOD_NOREPEAT, MSG, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, POINT,
    RECT, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_RESTORE, SW_SHOW,
    TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, UINT, UINT_PTR, WM_CLOSE, WM_DESTROY, WM_HOTKEY,
    WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WM_TIMER, WPARAM,
};
use kuaiyi_ng::ui::{
    compute_def_win_size, orig_to_win, set_auto_start_reg, AppSettings, APP_TITLE, APP_VERSION,
    DEF_CAPTURE, DEF_COPY, DEF_HIST_DOWN, DEF_HIST_UP, DEF_PIN, DEF_SNIP, DEF_TOGGLE, HK_CAPTURE,
    HK_COPY, HK_HIST_DOWN, HK_HIST_UP, HK_PIN, HK_SNIP, HK_TOGGLE, KIND_NAMES, STYLE_CODES,
    STYLE_NAMES, WM_APP_COPY_NAMING, WM_APP_DOWNLOAD, WM_APP_FLASH_HINT, WM_APP_HIDE_MAIN,
    WM_APP_OPEN_SETTINGS, WM_APP_PIN_DONE, WM_APP_PIN_LINES, WM_APP_POPUP_SELECT, WM_APP_SEL_TRANS,
    WM_APP_SHOW_MAIN, WM_APP_SNIP_DONE,
    WM_APP_TRANSLATED, WM_APP_TRAY, WM_APP_UPDATE,
};
use std::cell::RefCell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::mem::{size_of, zeroed};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::thread;

const TIMER_TEST_TRANS_START: UINT_PTR = 65;
const TIMER_TEST_TRANS_TIMEOUT: UINT_PTR = 69;
const TIMER_TEST_SETTINGS: UINT_PTR = 68;
const TIMER_HINT_RESET: UINT_PTR = 70;

struct AppRuntime {
    exe_path: String,
    app_dir: PathBuf,
    cfg: Config,
    hist: Option<History>,
    set: AppSettings,
    hwnd_main: HWND,
    hwnd_set: HWND,
    tray_added: bool,
    single_mutex: HANDLE,

    hist_idx: isize,
    translating: bool,
    trans_seq: usize,
    last_to_en: bool,
    result_valid: bool,
    exiting: bool,

    test_translate: bool,
    test_tab: isize,
    checking_update: bool,
}

thread_local! {
    static APP_RT: RefCell<Option<AppRuntime>> = const { RefCell::new(None) };
}

struct TransJobResult {
    seq: usize,
    text_out: Option<String>,
    err: String,
    engine: String,
    to_en: bool,
    input_trimmed: String,
}

struct UpdateCheckPayload {
    res: Result<(bool, String, String), String>,
}

struct UpdateDownloadPayload {
    dest: String,
    res: Result<(), String>,
}

fn with_rt<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AppRuntime) -> R,
{
    APP_RT.with(|cell| {
        let mut opt = cell.try_borrow_mut().ok()?;
        opt.as_mut().map(f)
    })
}

fn append_test_log(app_dir: &Path, line: &str) {
    let p = app_dir.join("test.log");
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(p) {
        let _ = writeln!(f, "{line}");
    }
}

/// 强制置前唤醒主窗（跨线程 AttachThreadInput 突破前台窗口焦点锁）
unsafe fn force_foreground(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    let fg = GetForegroundWindow();
    let mut cur_pid = 0u32;
    let cur_tid = GetWindowThreadProcessId(hwnd, &mut cur_pid);
    let fg_tid = if !fg.is_null() {
        GetWindowThreadProcessId(fg, null_mut())
    } else {
        0
    };
    let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(cur_tid, fg_tid, 1) != 0;
    ShowWindow(hwnd, SW_RESTORE);
    ShowWindow(hwnd, SW_SHOW);
    BringWindowToTop(hwnd);
    SetForegroundWindow(hwnd);
    if attached {
        AttachThreadInput(cur_tid, fg_tid, 0);
    }
}

fn show_main_window(by_hotkey: bool) {
    let (hwnd, near_mouse, clear_on_summon, cur_set_hint) = match with_rt(|rt| {
        let mut h_txt = String::new();
        if by_hotkey && rt.set.summon_clear {
            rt.hist_idx = -1;
            rt.result_valid = false;
        }
        if by_hotkey {
            h_txt = hotkey_hint_str(rt.set.hk_copy.0, rt.set.hk_copy.1);
        }
        (
            rt.hwnd_main,
            rt.set.summon_near_mouse,
            rt.set.summon_clear,
            h_txt,
        )
    }) {
        Some(v) => v,
        None => return,
    };

    if by_hotkey && clear_on_summon {
        set_input_text("");
        set_result_text("");
        set_main_title_and_hint(Some(APP_TITLE), Some(&cur_set_hint));
    }

    unsafe {
        if by_hotkey && near_mouse {
            let mut cur = POINT::default();
            GetCursorPos(&mut cur);
            let mut wr = RECT::default();
            GetWindowRect(hwnd, &mut wr);
            let ww = (wr.right - wr.left).max(400);
            let wh = (wr.bottom - wr.top).max(300);
            let wa = work_area_from_point(cur);
            let nx = (cur.x + 12).clamp(wa.left, (wa.right - ww).max(wa.left));
            let ny = (cur.y + 12).clamp(wa.top, (wa.bottom - wh).max(wa.top));
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                nx,
                ny,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        force_foreground(hwnd);
    }
    focus_and_select_input();
}

fn hide_main_window() {
    if let Some(hwnd) = with_rt(|rt| rt.hwnd_main) {
        unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
}

/// 切换主窗口显示或隐藏
fn toggle_main_window() {
    let vis = with_rt(|rt| unsafe { IsWindowVisible(rt.hwnd_main) != 0 }).unwrap_or(false);
    if vis {
        hide_main_window();
    } else {
        show_main_window(true);
    }
}

/// 添加/刷新系统托盘图标
fn add_tray_icon(hwnd: HWND) {
    let icon = load_app_icon(16, 16);
    let mut nid: NOTIFYICONDATAW = unsafe { zeroed() };
    nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_APP_TRAY;
    nid.hIcon = icon;
    let tip = utf8_to_wide(APP_TITLE);
    let cp = tip.len().min(nid.szTip.len() - 1);
    nid.szTip[..cp].copy_from_slice(&tip[..cp]);
    // SAFETY: NOTIFYICONDATAW 已按 Win32 ABI 正确赋值
    let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) != 0 };
    with_rt(|rt| rt.tray_added = ok);
}

fn remove_tray_icon() {
    let (hwnd, added) = with_rt(|rt| {
        let old = rt.tray_added;
        rt.tray_added = false;
        (rt.hwnd_main, old)
    })
    .unwrap_or((null_mut(), false));
    if added && !hwnd.is_null() {
        let mut nid: NOTIFYICONDATAW = unsafe { zeroed() };
        nid.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
    }
}

/// 注册 7 路全局热键并同步状态至设置页
fn register_all_hotkeys() {
    let Some((hwnd, set)) = with_rt(|rt| (rt.hwnd_main, rt.set.clone())) else {
        return;
    };
    if hwnd.is_null() {
        return;
    }
    let mut ok = [true; 7];
    let ids = [
        HK_TOGGLE,
        HK_HIST_UP,
        HK_HIST_DOWN,
        HK_COPY,
        HK_SNIP,
        HK_CAPTURE,
        HK_PIN,
    ];
    for (i, &id) in ids.iter().enumerate() {
        unsafe { UnregisterHotKey(hwnd, id) };
        let (m, vk) = set.hotkey_at(i);
        if vk > 0 {
            let win_m = orig_to_win(m) | MOD_NOREPEAT;
            let res = unsafe { RegisterHotKey(hwnd, id, win_m, vk as UINT) };
            ok[i] = res != 0;
        } else {
            ok[i] = true;
        }
    }
    update_hotkey_status_ui(&ok);
}

fn sync_snip_cfg_from_rt() {
    if let Some(cfg) = with_rt(|rt| SnipCfg {
        hwnd_main: rt.hwnd_main as isize,
        ai_url: if rt.set.ai_enabled {
            rt.set.ai_url.clone()
        } else {
            String::new()
        },
        ai_key: if rt.set.ai_enabled {
            rt.set.ai_key.clone()
        } else {
            String::new()
        },
        ai_model: if rt.set.ai_enabled {
            rt.set.ai_model.clone()
        } else {
            String::new()
        },
        baidu_appid: if rt.set.baidu_enabled {
            rt.set.baidu_appid.clone()
        } else {
            String::new()
        },
        baidu_secret: if rt.set.baidu_enabled {
            rt.set.baidu_key.clone()
        } else {
            String::new()
        },
        baidu_url: if rt.set.baidu_enabled {
            rt.set.baidu_url.clone()
        } else {
            String::new()
        },
        cache_dir: rt.set.cache_dir.clone(),
        save_dir: rt.set.snip_save_dir.clone(),
        pin_mods: rt.set.hk_pin.0,
        pin_vk: rt.set.hk_pin.1,
        sub_window: rt.set.snip_sub_window,
    }) {
        update_snip_cfg(cfg);
    }
}

/// 发起后台翻译任务
fn do_translate(mode: i32) {
    let raw = get_input_text();
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        return;
    }

    let Some((hwnd_main, seq, set)) = with_rt(|rt| {
        rt.translating = true;
        rt.trans_seq += 1;
        (rt.hwnd_main, rt.trans_seq, rt.set.clone())
    }) else {
        return;
    };

    set_main_translating(true);
    set_result_text("翻译中...");
    log_write(
        "翻译",
        &format!("发起翻译 seq={seq} len={} mode={mode}", trimmed.len()),
    );

    let to_en = if mode == kuaiyi_ng::core::FY_ZH2EN {
        true
    } else if mode == kuaiyi_ng::core::FY_EN2ZH {
        false
    } else {
        has_cjk(&trimmed)
    };

    let hwnd_raw = hwnd_main as usize;
    thread::spawn(move || {
        let ai_url = if set.ai_enabled {
            set.ai_url.as_str()
        } else {
            ""
        };
        let ai_key = if set.ai_enabled {
            set.ai_key.as_str()
        } else {
            ""
        };
        let ai_model = if set.ai_enabled {
            set.ai_model.as_str()
        } else {
            ""
        };
        let b_id = if set.baidu_enabled {
            set.baidu_appid.as_str()
        } else {
            ""
        };
        let b_sec = if set.baidu_enabled {
            set.baidu_key.as_str()
        } else {
            ""
        };
        let b_url = if set.baidu_enabled {
            set.baidu_url.as_str()
        } else {
            ""
        };

        let text_out =
            translate_chain(&trimmed, mode, ai_url, ai_key, ai_model, b_id, b_sec, b_url);
        let err = if text_out.is_none() {
            last_error()
        } else {
            String::new()
        };
        let engine = last_engine();
        let payload = Box::into_raw(Box::new(TransJobResult {
            seq,
            text_out,
            err,
            engine,
            to_en,
            input_trimmed: trimmed,
        }));
        unsafe { PostMessageW(hwnd_raw as HWND, WM_APP_TRANSLATED, 0, payload as LPARAM) };
    });
}

/// 在主窗口提示栏临时显示状态消息，2.8 秒后自动恢复默认提示
fn flash_main_hint(hwnd_main: HWND, msg: &str) {
    set_main_title_and_hint(None, Some(msg));
    if !hwnd_main.is_null() {
        unsafe { SetTimer(hwnd_main, TIMER_HINT_RESET, 2800, null()) };
    }
}

/// 根据当前命名风格和命名对象缩减短语并写入剪贴板
fn perform_naming_copy(from_result: bool) {
    let in_txt = get_input_text();
    let res_txt = get_result_text();

    let Some((
        word_fmt,
        name_kind,
        max_w,
        max_c,
        word_order,
        verb_form,
        last_to_en,
        res_ok,
        hwnd_main,
        copy_hk,
    )) = with_rt(|rt| {
        (
            rt.set.word_fmt,
            rt.set.name_kind,
            rt.set.max_words,
            rt.set.max_chars,
            rt.set.word_order,
            rt.set.verb_form,
            rt.last_to_en,
            rt.result_valid,
            rt.hwnd_main,
            rt.set.hk_copy,
        )
    })
    else {
        return;
    };

    // 选择源文本：若双击结果区或通过快捷键且上次为中译英且结果有效，则优先结果首行，否则输入框
    let mut source = if (from_result && res_ok)
        || (!from_result && res_ok && last_to_en && in_txt.trim().is_empty())
    {
        res_txt.clone()
    } else {
        in_txt.clone()
    };
    if source.starts_with('（') {
        if let Some(nl) = source.find('\n') {
            if source[..nl].contains("已用必应翻译") {
                source = source[nl + 1..].to_string();
            }
        }
    }
    let first_line = source.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return;
    }

    let style = if word_fmt == FY_FMT_SNAKE {
        NamingStyle::Snake
    } else {
        NamingStyle::Camel
    };
    let kind = NameKind::from_index(name_kind);
    let opts = NamingOptions {
        word_order: WordOrder::from_index(word_order),
        verb_form: VerbForm::from_index(verb_form),
    };
    let r = reduce_name_with_options(first_line, style, kind, max_w, max_c, opts);
    if r.ok {
        set_clipboard_text(&r.name);
        set_result_text(&r.name);
        flash_main_hint(hwnd_main, &format!("已复制命名：{}", r.name));
        log_write("命名", &format!("复制标识符 {}", r.name));
    } else {
        let hint = if r.reason.is_empty() {
            "无法生成合法标识符".to_string()
        } else {
            format!("命名失败：{}", r.reason)
        };
        flash_main_hint(hwnd_main, &hint);
    }
    let _ = copy_hk;
}

fn browse_history(delta: isize) {
    let text_opt = with_rt(|rt| {
        let hist = rt.hist.as_ref()?;
        let cnt = hist.count() as isize;
        if cnt <= 0 {
            return None;
        }
        let next = if rt.hist_idx < 0 {
            if delta < 0 {
                cnt - 1
            } else {
                0
            }
        } else {
            (rt.hist_idx + delta).clamp(0, cnt - 1)
        };
        rt.hist_idx = next;
        hist.get(next as usize).map(str::to_string)
    })
    .flatten();

    if let Some(txt) = text_opt {
        set_input_text(&txt);
        focus_and_select_input();
    }
}

fn trigger_check_update(quiet: bool) {
    let (already, hwnd_main) = with_rt(|rt| {
        if rt.checking_update {
            return (true, rt.hwnd_main);
        }
        rt.checking_update = true;
        (false, rt.hwnd_main)
    })
    .unwrap_or((true, null_mut()));
    if already {
        return;
    }
    if !quiet {
        set_about_update_status("正在连接 GitHub 检查最新版本...", true);
    }
    let hwnd_raw = hwnd_main as usize;
    let wp_flag = if quiet { 1usize } else { 0usize };
    thread::spawn(move || {
        let res = check_update(APP_VERSION);
        let payload = Box::into_raw(Box::new(UpdateCheckPayload { res }));
        unsafe { PostMessageW(hwnd_raw as HWND, WM_APP_UPDATE, wp_flag, payload as LPARAM) };
    });
}

fn show_tray_context_menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let vis = IsWindowVisible(hwnd) != 0;
        let add = |id: usize, txt: &str, flags: UINT| {
            let wt = utf8_to_wide(txt);
            AppendMenuW(menu, flags, id, wt.as_ptr());
        };
        add(
            1,
            if vis {
                "隐藏主窗口"
            } else {
                "显示主窗口"
            },
            MF_STRING,
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        add(2, "区域截屏", MF_STRING);
        add(3, "剪贴板贴图", MF_STRING);
        add(4, "直接截图翻译 (Ctrl+Alt+D)", MF_STRING);
        if has_active_pins() {
            add(5, "关闭所有贴图", MF_STRING);
        }
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        add(6, "设置...", MF_STRING);
        let topmost = with_rt(|rt| rt.set.topmost).unwrap_or(false);
        add(
            7,
            "窗口置顶",
            MF_STRING | if topmost { MF_CHECKED } else { 0 },
        );
        add(8, "检查更新", MF_STRING);
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        add(9, "退出", MF_STRING);

        let mut pt = POINT::default();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
            pt.x,
            pt.y,
            0,
            hwnd,
            null(),
        );
        DestroyMenu(menu);
        match cmd {
            1 => toggle_main_window(),
            2 => start_snip_capture(),
            3 => create_pin_from_clipboard(),
            4 => start_snip_translate(),
            5 => close_all_pins(),
            6 => open_settings_window(None),
            7 => {
                PostMessageW(hwnd, WM_MAIN_CMD_TOPMOST, 0, 0);
            }
            8 => trigger_check_update(false),
            9 => quit_app(),
            _ => {}
        }
    }
}

fn quit_app() {
    let mut main_hwnd = null_mut();
    let mut set_hwnd = null_mut();
    let mut mutex_h = null_mut();

    with_rt(|rt| {
        if rt.exiting {
            return;
        }
        rt.exiting = true;
        main_hwnd = rt.hwnd_main;
        set_hwnd = rt.hwnd_set;
        mutex_h = rt.single_mutex;

        let mut cur_w = 0;
        let mut cur_h = 0;
        if !main_hwnd.is_null() {
            let mut wr = RECT::default();
            if unsafe { GetWindowRect(main_hwnd, &mut wr) != 0 } {
                cur_w = wr.right - wr.left;
                cur_h = wr.bottom - wr.top;
            }
        }
        rt.set.save_exit_state(&mut rt.cfg, cur_w, cur_h);
    });

    close_all_pins();
    remove_tray_icon();
    if !main_hwnd.is_null() {
        for id in [
            HK_TOGGLE,
            HK_HIST_UP,
            HK_HIST_DOWN,
            HK_COPY,
            HK_SNIP,
            HK_CAPTURE,
            HK_PIN,
        ] {
            unsafe { UnregisterHotKey(main_hwnd, id) };
        }
    }
    unsafe {
        if !set_hwnd.is_null() {
            DestroyWindow(set_hwnd);
        }
        if !main_hwnd.is_null() {
            DestroyWindow(main_hwnd);
        }
        if !mutex_h.is_null() {
            CloseHandle(mutex_h);
        }
        shutdown_gdiplus();
        PostQuitMessage(0);
    }
}

/// 同步常规与接口设置并立即生效存盘
fn apply_general_and_trans_settings() {
    let mut cfg_copy = match with_rt(|rt| rt.set.clone()) {
        Some(s) => s,
        None => return,
    };
    let old_cache = cfg_copy.cache_dir.clone();
    let old_auto_start = cfg_copy.auto_start;
    let old_auto_hide = cfg_copy.auto_hide;

    collect_settings_from_ui(&mut cfg_copy);

    let exe_path = with_rt(|rt| rt.exe_path.clone()).unwrap_or_default();
    if cfg_copy.auto_start != old_auto_start || cfg_copy.auto_hide != old_auto_hide {
        set_auto_start_reg(cfg_copy.auto_start, cfg_copy.auto_hide, &exe_path);
    }

    let mut need_hist_reopen = false;
    if cfg_copy.cache_dir != old_cache && !cfg_copy.cache_dir.trim().is_empty() {
        let _ = fs::create_dir_all(&cfg_copy.cache_dir);
        need_hist_reopen = true;
    }

    set_main_topmost(cfg_copy.topmost);
    set_main_input_dbl_clear(cfg_copy.input_dbl_clear);
    let hk_hint = hotkey_hint_str(cfg_copy.hk_copy.0, cfg_copy.hk_copy.1);
    set_main_title_and_hint(None, Some(&hk_hint));

    with_rt(|rt| {
        rt.set = cfg_copy.clone();
        if need_hist_reopen {
            if let Some(h) = History::open(&rt.set.cache_dir) {
                rt.hist = Some(h);
                rt.hist_idx = -1;
            }
        }
        if let Err(e) = rt.set.save_to_cfg(&mut rt.cfg) {
            log_write("配置", &format!("保存设置失败: {e}"));
        }
    });
    sync_snip_cfg_from_rt();
}

unsafe extern "system" fn main_wnd_proc(hwnd: HWND, msg: UINT, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // 先尝试交由 modern_main UI 层处理自绘/鼠标/无边框缩放消息
    if let Some(res) = handle_main_ui_message(hwnd, msg, wp, lp) {
        return res;
    }

    match msg {
        WM_CLOSE => {
            let close_exit = with_rt(|rt| rt.set.close_exit).unwrap_or(false);
            if close_exit {
                quit_app();
            } else {
                hide_main_window();
            }
            0
        }
        WM_MAIN_CMD_CLOSE_REQ => {
            let close_exit = with_rt(|rt| rt.set.close_exit).unwrap_or(false);
            if close_exit {
                quit_app();
            } else {
                hide_main_window();
            }
            0
        }
        WM_MAIN_CMD_TOPMOST => {
            let new_top = with_rt(|rt| {
                rt.set.topmost = !rt.set.topmost;
                let _ = rt.set.save_to_cfg(&mut rt.cfg);
                rt.set.topmost
            })
            .unwrap_or(false);
            set_main_topmost(new_top);
            if let Some(s) = with_rt(|rt| rt.set.clone()) {
                load_settings_into_ui(&s);
            }
            0
        }
        WM_MAIN_CMD_TRANSLATE => {
            let mode = wp as i32;
            let is_trans_btn = lp == 1;
            let was_translating = with_rt(|rt| rt.translating).unwrap_or(false);
            if is_trans_btn && was_translating {
                with_rt(|rt| {
                    rt.translating = false;
                    rt.trans_seq += 1;
                });
                set_main_translating(false);
                set_result_text("已取消翻译");
            } else {
                do_translate(mode);
            }
            0
        }
        WM_MAIN_CMD_STYLE_CHANGED => {
            let idx = wp;
            if let Some(&code) = STYLE_CODES.get(idx) {
                with_rt(|rt| {
                    rt.set.word_fmt = code;
                    let _ = rt.set.save_to_cfg(&mut rt.cfg);
                });
                set_main_naming_selection(
                    idx,
                    with_rt(|rt| rt.set.name_kind as usize).unwrap_or(0),
                );
            }
            0
        }
        WM_MAIN_CMD_KIND_CHANGED => {
            let idx = wp;
            if idx < KIND_NAMES.len() {
                with_rt(|rt| {
                    rt.set.name_kind = idx as i32;
                    let _ = rt.set.save_to_cfg(&mut rt.cfg);
                });
                let sidx = with_rt(|rt| {
                    if rt.set.word_fmt == FY_FMT_SNAKE {
                        0
                    } else {
                        1
                    }
                })
                .unwrap_or(1);
                set_main_naming_selection(sidx, idx);
            }
            0
        }
        WM_APP_POPUP_SELECT => {
            let kind = wp as i32;
            let idx = lp as usize;
            if kind == ID_STYLE {
                PostMessageW(hwnd, WM_MAIN_CMD_STYLE_CHANGED, idx, 0);
            } else if kind == ID_KIND {
                PostMessageW(hwnd, WM_MAIN_CMD_KIND_CHANGED, idx, 0);
            }
            0
        }
        WM_APP_COPY_NAMING => {
            perform_naming_copy(wp != 0);
            0
        }
        WM_APP_OPEN_SETTINGS => {
            open_settings_window(None);
            0
        }
        WM_APP_SHOW_MAIN => {
            show_main_window(false);
            0
        }
        WM_APP_HIDE_MAIN => {
            hide_main_window();
            0
        }
        WM_MAIN_CMD_INPUT_CHANGED => {
            with_rt(|rt| rt.hist_idx = -1);
            0
        }
        WM_APP_TRANSLATED => {
            if lp != 0 {
                let res = Box::from_raw(lp as *mut TransJobResult);
                let accepted = with_rt(|rt| {
                    if res.seq != rt.trans_seq {
                        return false;
                    }
                    rt.translating = false;
                    true
                })
                .unwrap_or(false);
                if accepted {
                    set_main_translating(false);
                    match res.text_out {
                        Some(out) => {
                            set_result_text(&out);
                            let test_mode = with_rt(|rt| {
                                rt.last_to_en = res.to_en;
                                rt.result_valid = true;
                                if let Some(h) = rt.hist.as_mut() {
                                    h.push(&res.input_trimmed);
                                    rt.hist_idx = -1;
                                }
                                rt.test_translate
                            })
                            .unwrap_or(false);
                            log_write(
                                "翻译",
                                &format!("完成 引擎={} 结果{}字节", res.engine, out.len()),
                            );
                            if test_mode {
                                if let Some(dir) = with_rt(|rt| rt.app_dir.clone()) {
                                    append_test_log(&dir, &format!("TEST-RESULT: OK {out}"));
                                }
                                quit_app();
                            }
                        }
                        None => {
                            let err = if res.err.is_empty() {
                                "翻译失败（请检查网络或接口配置）".to_string()
                            } else {
                                res.err
                            };
                            set_result_text(&err);
                            let test_mode = with_rt(|rt| {
                                rt.result_valid = false;
                                rt.test_translate
                            })
                            .unwrap_or(false);
                            log_write("翻译", &format!("失败: {err}"));
                            if test_mode {
                                if let Some(dir) = with_rt(|rt| rt.app_dir.clone()) {
                                    append_test_log(&dir, &format!("TEST-RESULT: FAIL {err}"));
                                }
                                quit_app();
                            }
                        }
                    }
                }
            }
            0
        }
        WM_APP_SNIP_DONE => {
            handle_snip_done_message(wp, lp);
            0
        }
        WM_APP_PIN_DONE => {
            handle_pin_done_message(wp, lp);
            0
        }
        WM_APP_PIN_LINES => {
            handle_pin_lines_message(lp);
            0
        }
        WM_APP_SEL_TRANS => {
            handle_sel_trans_message(lp);
            0
        }
        WM_APP_FLASH_HINT => {
            if lp != 0 {
                let msg = Box::from_raw(lp as *mut String);
                flash_main_hint(hwnd, &msg);
            }
            0
        }
        WM_APP_UPDATE => {
            if lp != 0 {
                let payload = Box::from_raw(lp as *mut UpdateCheckPayload);
                let quiet = wp != 0;
                with_rt(|rt| rt.checking_update = false);
                match payload.res {
                    Ok((newer, tag, url)) => {
                        if newer {
                            set_about_update_status(
                                &format!("发现新版本 {tag}！（当前 v{APP_VERSION}）"),
                                false,
                            );
                            let tip = utf8_to_wide(&format!(
                                "检测到快译NG 新版本 {tag}（当前 v{APP_VERSION}）。\n\n是否立即下载安装包并自动安装？"
                            ));
                            let cap = utf8_to_wide("检查更新 - 快译NG");
                            if MessageBoxW(
                                hwnd,
                                tip.as_ptr(),
                                cap.as_ptr(),
                                MB_YESNO | MB_ICONINFORMATION,
                            ) == IDYES
                            {
                                set_about_update_status(&format!("正在下载 {tag}..."), true);
                                let dest = std::env::temp_dir()
                                    .join(format!("KuaiYiNG_{tag}_Setup.exe"))
                                    .to_string_lossy()
                                    .into_owned();
                                let hwnd_raw = hwnd as usize;
                                thread::spawn(move || {
                                    let res = download_update(&url, &dest, None);
                                    let p = Box::into_raw(Box::new(UpdateDownloadPayload {
                                        dest,
                                        res,
                                    }));
                                    unsafe {
                                        PostMessageW(
                                            hwnd_raw as HWND,
                                            WM_APP_DOWNLOAD,
                                            0,
                                            p as LPARAM,
                                        )
                                    };
                                });
                            }
                        } else if !quiet {
                            set_about_update_status(&format!("已是最新版本 v{APP_VERSION}"), false);
                            show_settings_toast("已是最新版本", true);
                        }
                    }
                    Err(e) => {
                        set_about_update_status(
                            &format!("当前版本 v{APP_VERSION}（检查失败：{e}）"),
                            false,
                        );
                        if !quiet {
                            show_settings_toast("检查更新失败", false);
                        }
                    }
                }
            }
            0
        }
        WM_APP_DOWNLOAD => {
            if lp != 0 {
                let payload = Box::from_raw(lp as *mut UpdateDownloadPayload);
                match payload.res {
                    Ok(()) => {
                        set_about_update_status("安装包已下载，正在启动安装...", false);
                        shell_open(&payload.dest);
                        quit_app();
                    }
                    Err(e) => {
                        set_about_update_status(&format!("下载失败：{e}"), false);
                        let wt = utf8_to_wide(&format!("下载失败：{e}"));
                        let cap = utf8_to_wide(APP_TITLE);
                        MessageBoxW(hwnd, wt.as_ptr(), cap.as_ptr(), MB_ICONWARNING);
                    }
                }
            }
            0
        }
        WM_APP_TRAY => {
            let ev = (lp & 0xFFFF) as UINT;
            if ev == WM_LBUTTONUP || ev == WM_LBUTTONDBLCLK {
                toggle_main_window();
            } else if ev == WM_RBUTTONUP {
                show_tray_context_menu(hwnd);
            }
            0
        }
        WM_HOTKEY => {
            match wp as i32 {
                HK_TOGGLE => toggle_main_window(),
                HK_HIST_UP => browse_history(-1),
                HK_HIST_DOWN => browse_history(1),
                HK_COPY => perform_naming_copy(false),
                HK_SNIP => start_snip_translate(),
                HK_CAPTURE => start_snip_capture(),
                HK_PIN => create_pin_from_clipboard(),
                _ => {}
            }
            0
        }
        WM_SET_APPLY_SETTINGS => {
            apply_general_and_trans_settings();
            0
        }
        WM_SET_APPLY_SCALE_OR_THEME => {
            let old_scale = with_rt(|rt| rt.set.ui_scale).unwrap_or(100);
            apply_general_and_trans_settings();
            if let Some(set) = with_rt(|rt| rt.set.clone()) {
                let dark = set.resolved_dark();
                let sc = (set.ui_scale as f64) / 100.0;
                match wp {
                    0 => {
                        // 仅主题切换
                        sync_main_theme_and_font(sc, dark);
                        refresh_settings_appearance(sc, dark, set.anim_enabled);
                    }
                    1 => {
                        // 缩放比例切换：同时按比例缩放主窗口与设置窗口
                        let (def_w, def_h) = compute_def_win_size(set.ui_scale);
                        let ratio = (set.ui_scale as f64) / (old_scale.max(75) as f64);
                        let mut wr = RECT::default();
                        GetWindowRect(hwnd, &mut wr);
                        let cur_w = wr.right - wr.left;
                        let cur_h = wr.bottom - wr.top;
                        let nw = ((cur_w as f64) * ratio).round().max(def_w as f64) as i32;
                        let nh = ((cur_h as f64) * ratio).round().max(def_h as f64) as i32;
                        SetWindowPos(
                            hwnd,
                            HWND_NOTOPMOST,
                            0,
                            0,
                            nw,
                            nh,
                            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                        sync_main_theme_and_font(sc, dark);
                        relayout_main_controls(nw, nh, sc);
                        refresh_settings_appearance(sc, dark, set.anim_enabled);
                        with_rt(|rt| {
                            rt.set.win_w = nw;
                            rt.set.win_h = nh;
                            let _ = rt.set.save_to_cfg(&mut rt.cfg);
                        });
                        if let Some(s2) = with_rt(|rt| rt.set.clone()) {
                            load_settings_into_ui(&s2);
                        }
                    }
                    2 => {
                        // 恢复主窗口默认尺寸
                        let (def_w, def_h) = compute_def_win_size(set.ui_scale);
                        SetWindowPos(
                            hwnd,
                            HWND_NOTOPMOST,
                            0,
                            0,
                            def_w,
                            def_h,
                            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                        relayout_main_controls(def_w, def_h, sc);
                        with_rt(|rt| {
                            rt.set.win_w = def_w;
                            rt.set.win_h = def_h;
                            let _ = rt.set.save_to_cfg(&mut rt.cfg);
                        });
                        if let Some(s2) = with_rt(|rt| rt.set.clone()) {
                            load_settings_into_ui(&s2);
                        }
                        show_settings_toast("已恢复默认大小", true);
                    }
                    // 应用手填窗口大小
                    3 if set.win_w >= 400 && set.win_h >= 300 => {
                        SetWindowPos(
                            hwnd,
                            HWND_NOTOPMOST,
                            0,
                            0,
                            set.win_w,
                            set.win_h,
                            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                        relayout_main_controls(set.win_w, set.win_h, sc);
                    }
                    _ => {}
                }
            }
            0
        }
        WM_SET_APPLY_HOTKEYS => {
            apply_general_and_trans_settings();
            register_all_hotkeys();
            0
        }
        WM_SET_RESET_HOTKEYS => {
            with_rt(|rt| {
                rt.set.hk_toggle = DEF_TOGGLE;
                rt.set.hk_hist_up = DEF_HIST_UP;
                rt.set.hk_hist_down = DEF_HIST_DOWN;
                rt.set.hk_copy = DEF_COPY;
                rt.set.hk_snip = DEF_SNIP;
                rt.set.hk_capture = DEF_CAPTURE;
                rt.set.hk_pin = DEF_PIN;
                let _ = rt.set.save_to_cfg(&mut rt.cfg);
            });
            if let Some(s) = with_rt(|rt| rt.set.clone()) {
                load_settings_into_ui(&s);
                let hk_hint = hotkey_hint_str(s.hk_copy.0, s.hk_copy.1);
                set_main_title_and_hint(None, Some(&hk_hint));
            }
            register_all_hotkeys();
            show_settings_toast("热键已恢复默认", true);
            0
        }
        WM_SET_CHECK_UPDATE => {
            trigger_check_update(false);
            0
        }
        WM_SET_SNIP_CMD => {
            match wp {
                1 => start_snip_capture(),
                2 => create_pin_from_clipboard(),
                3 => start_snip_translate(),
                4 => close_all_pins(),
                _ => {}
            }
            0
        }
        WM_TIMER => {
            match wp {
                TIMER_TEST_TRANS_START => {
                    KillTimer(hwnd, TIMER_TEST_TRANS_START);
                    set_input_text("The quick brown fox jumps over the lazy dog.");
                    do_translate(FY_AUTO);
                }
                TIMER_TEST_TRANS_TIMEOUT => {
                    KillTimer(hwnd, TIMER_TEST_TRANS_TIMEOUT);
                    if let Some(dir) = with_rt(|rt| {
                        if rt.exiting {
                            None
                        } else {
                            Some(rt.app_dir.clone())
                        }
                    })
                    .flatten()
                    {
                        append_test_log(&dir, "TEST-RESULT: TIMEOUT");
                        log_write("翻译", "自测超时退出");
                        quit_app();
                    }
                }
                TIMER_TEST_SETTINGS => {
                    KillTimer(hwnd, TIMER_TEST_SETTINGS);
                    let tab = with_rt(|rt| rt.test_tab).unwrap_or(-1);
                    open_settings_window(if tab >= 0 { Some(tab as usize) } else { None });
                }
                TIMER_HINT_RESET => {
                    KillTimer(hwnd, TIMER_HINT_RESET);
                    if let Some(h) =
                        with_rt(|rt| hotkey_hint_str(rt.set.hk_copy.0, rt.set.hk_copy.1))
                    {
                        set_main_title_and_hint(None, Some(&h));
                    }
                }
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn parse_cli_args() -> (bool, bool, bool, isize) {
    let mut hide = false;
    let mut test_trans = false;
    let mut test_open_set = false;
    let mut test_tab = -1isize;
    for arg in std::env::args().skip(1) {
        if arg == "-hide" {
            hide = true;
        } else if arg == "-testtranslate" {
            test_trans = true;
        } else if arg == "-testopensettings" {
            test_open_set = true;
        } else if let Some(rest) = arg.strip_prefix("-testtab=") {
            if let Ok(t) = rest.parse::<isize>() {
                test_tab = t.clamp(0, 11);
            }
        }
    }
    (hide, test_trans, test_open_set, test_tab)
}

fn main() {
    init_ui_libraries();
    let (hide_flag, test_trans, test_open_set, test_tab) = parse_cli_args();

    // 非测试模式下检查单实例互斥体并激活已有窗口
    let mut single_mutex: HANDLE = null_mut();
    if !test_trans {
        let mut wmut = utf8_to_wide("KuaiYiNG_SingleInstance");
        // SAFETY: 以名称创建单实例互斥体
        unsafe {
            single_mutex = CreateMutexW(null(), 1, wmut.as_ptr());
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let cls = utf8_to_wide(MAIN_CLASS_NAME);
                let old_hwnd = FindWindowW(cls.as_ptr(), null());
                if !old_hwnd.is_null() {
                    PostMessageW(old_hwnd, WM_APP_SHOW_MAIN, 0, 0);
                    if !single_mutex.is_null() {
                        CloseHandle(single_mutex);
                    }
                    return;
                }
                let _ = &mut wmut;
            }
        }
    }

    let exe_path = kuaiyi_ng::ui::win32_ui::current_exe_path();
    let exe_dir = Path::new(&exe_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let app_dir = exe_dir.join("fastT");
    let _ = fs::create_dir_all(&app_dir);

    log_open(app_dir.join("run.log"));
    log_install_crash_filter();
    log_write("启动", &format!("快译NG v{APP_VERSION} 纯Rust版启动"));

    let cfg = Config::open(app_dir.join("config"));
    let mut set = AppSettings::load_from_cfg(&cfg, &app_dir);

    let hist_dir = if !set.cache_dir.trim().is_empty() {
        PathBuf::from(&set.cache_dir)
    } else {
        app_dir.clone()
    };
    let hist = History::open(&hist_dir);

    let n_abbr = load_custom_abbreviations(app_dir.join("custom_abbr.txt"));
    let n_ign = load_custom_ignore(app_dir.join("custom_ignore.txt"));
    let n_pref = load_custom_preferred(app_dir.join("custom_preferred.txt"));
    log_write(
        "词表",
        &format!("加载自定义 缩略={n_abbr} 忽略={n_ign} 常用={n_pref}"),
    );

    let sc = (set.ui_scale as f64) / 100.0;
    let is_dark = set.resolved_dark();
    let (def_w, def_h) = compute_def_win_size(set.ui_scale);
    let win_w = if set.win_w >= 546 { set.win_w } else { def_w };
    let win_h = if set.win_h >= 350 { set.win_h } else { def_h };
    set.win_w = win_w;
    set.win_h = win_h;

    let hwnd_main = create_modern_main_window(
        main_wnd_proc,
        APP_TITLE,
        win_w,
        win_h,
        sc,
        is_dark,
        set.topmost,
    );
    if hwnd_main.is_null() {
        log_write("错误", "创建主窗口失败");
        return;
    }
    log_write("启动", "主窗口创建完成");

    let style_idx = if set.word_fmt == FY_FMT_SNAKE { 0 } else { 1 };
    set_main_naming_selection(style_idx, set.name_kind as usize);
    set_main_input_dbl_clear(set.input_dbl_clear);
    let hk_hint = hotkey_hint_str(set.hk_copy.0, set.hk_copy.1);
    set_main_title_and_hint(Some(APP_TITLE), Some(&hk_hint));

    let hwnd_set = create_settings_window(hwnd_main, &app_dir, &set, is_dark);
    log_write(
        "启动",
        &format!("设置窗口创建完成 valid={}", !hwnd_set.is_null()),
    );

    APP_RT.with(|cell| {
        *cell.borrow_mut() = Some(AppRuntime {
            exe_path,
            app_dir,
            cfg,
            hist,
            set,
            hwnd_main,
            hwnd_set,
            tray_added: false,
            single_mutex,
            hist_idx: -1,
            translating: false,
            trans_seq: 0,
            last_to_en: false,
            result_valid: false,
            exiting: false,
            test_translate: test_trans,
            test_tab,
            checking_update: false,
        });
    });

    sync_snip_cfg_from_rt();
    add_tray_icon(hwnd_main);
    register_all_hotkeys();
    log_write("启动", "托盘与热键注册完成");

    if !hide_flag && !test_trans {
        unsafe {
            ShowWindow(hwnd_main, SW_SHOW);
            force_foreground(hwnd_main);
        }
        focus_and_select_input();
    } else if test_trans {
        unsafe {
            SetTimer(hwnd_main, TIMER_TEST_TRANS_START, 500, null());
            SetTimer(hwnd_main, TIMER_TEST_TRANS_TIMEOUT, 15000, null());
        }
    }

    if test_open_set || test_tab >= 0 {
        unsafe { SetTimer(hwnd_main, TIMER_TEST_SETTINGS, 600, null()) };
    }

    // 标准 Win32 消息循环
    let mut msg: MSG = unsafe { zeroed() };
    unsafe {
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    let _ = (select_settings_tab, STYLE_NAMES);
}
