//! 快译NG 界面层总控：配置模型、热键编码、深色模式探测与子模块导出

pub mod dict_list;
pub mod modern_main;
pub mod modern_settings;
pub mod snip_toast;
pub mod snip_ui;
pub mod win32_ui;

use crate::core::cfg::Config;
use crate::core::str_util::utf8_to_wide;
use crate::core::{FY_FMT_CAMEL, FY_FMT_SNAKE};
use crate::ui::win32_ui::{
    GetKeyNameTextW, MapVirtualKeyW, RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, BYTE, DWORD, ERROR_SUCCESS, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, LONG,
    MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN, REG_DWORD, REG_SZ, UINT, WM_APP,
};
use std::mem::size_of;
use std::path::Path;
use std::ptr::null_mut;

pub const APP_TITLE: &str = "快译NG";
pub const APP_VERSION: &str = "3.1.1";
pub const APP_REPO_URL: &str = "https://github.com/TimKuitu/kuaiyi-ng";

// 避开保留自定义消息区，统一使用 WM_APP + 0x0200..0x021F
pub const WM_APP_TRANSLATED: UINT = WM_APP + 0x0200;
pub const WM_APP_UPDATE: UINT = WM_APP + 0x0201;
pub const WM_APP_DOWNLOAD: UINT = WM_APP + 0x0202;
pub const WM_APP_SNIP_DONE: UINT = WM_APP + 0x0203;
pub const WM_APP_PIN_DONE: UINT = WM_APP + 0x0204;
pub const WM_APP_TRAY: UINT = WM_APP + 0x0205;
pub const WM_APP_OPEN_SETTINGS: UINT = WM_APP + 0x0206;
pub const WM_APP_SHOW_MAIN: UINT = WM_APP + 0x0207;
pub const WM_APP_HIDE_MAIN: UINT = WM_APP + 0x0208;
pub const WM_APP_POPUP_SELECT: UINT = WM_APP + 0x0209;
pub const WM_APP_COPY_NAMING: UINT = WM_APP + 0x020A;
pub const WM_APP_FLASH_HINT: UINT = WM_APP + 0x020B;
pub const WM_APP_PIN_LINES: UINT = WM_APP + 0x020C;
pub const WM_APP_SEL_TRANS: UINT = WM_APP + 0x020D;

// 全局热键 ID
pub const HK_TOGGLE: i32 = 1;
pub const HK_HIST_UP: i32 = 2;
pub const HK_HIST_DOWN: i32 = 3;
pub const HK_COPY: i32 = 4;
pub const HK_SNIP: i32 = 5;
pub const HK_CAPTURE: i32 = 6;
pub const HK_PIN: i32 = 7;

// 热键默认值：(mod_orig 1=Ctrl/2=Shift/4=Alt/8=Win, vk)
pub const DEF_TOGGLE: (i32, i32) = (5, 70); // Ctrl+Alt+F
pub const DEF_HIST_UP: (i32, i32) = (5, 38); // Ctrl+Alt+Up
pub const DEF_HIST_DOWN: (i32, i32) = (5, 40); // Ctrl+Alt+Down
pub const DEF_COPY: (i32, i32) = (5, 67); // Ctrl+Alt+C
pub const DEF_SNIP: (i32, i32) = (5, 68); // Ctrl+Alt+D
pub const DEF_CAPTURE: (i32, i32) = (0, 0x70); // F1
pub const DEF_PIN: (i32, i32) = (0, 0x72); // F3

pub const STYLE_NAMES: [&str; 2] = ["snake_case", "驼峰法"];
pub const STYLE_CODES: [i32; 2] = [FY_FMT_SNAKE, FY_FMT_CAMEL];
pub const KIND_NAMES: [&str; 7] = [
    "局部变量",
    "全局变量",
    "静态变量",
    "指针变量",
    "结构体类型",
    "宏/常量",
    "函数",
];

pub const UI_SCALE_PRESETS: [i32; 7] = [75, 100, 110, 125, 150, 175, 200];
pub const THEME_PRESETS: [&str; 3] = ["跟随系统", "白色", "黑色"];
pub const ROW_HEIGHT_LOGICAL: i32 = 32;

/// AI 预设接口描述
#[derive(Debug, Clone, Copy)]
pub struct AiPreset {
    pub name: &'static str,
    pub url: &'static str,
    pub model: &'static str,
}

pub const AI_PRESETS: [AiPreset; 5] = [
    AiPreset {
        name: "（自定义）",
        url: "",
        model: "",
    },
    AiPreset {
        name: "DeepSeek",
        url: "https://api.deepseek.com",
        model: "deepseek-chat",
    },
    AiPreset {
        name: "OpenAI",
        url: "https://api.openai.com",
        model: "gpt-4o-mini",
    },
    AiPreset {
        name: "Moonshot",
        url: "https://api.moonshot.cn",
        model: "moonshot-v1-8k",
    },
    AiPreset {
        name: "Ollama 本地",
        url: "http://localhost:11434",
        model: "qwen2.5:7b",
    },
];

/// 全局运行时设置模型（对应 `fastT/config`）
#[derive(Debug, Clone)]
pub struct AppSettings {
    pub topmost: bool,
    pub close_exit: bool,
    pub auto_start: bool,
    pub auto_hide: bool,
    pub summon_near_mouse: bool,
    pub summon_clear: bool,
    pub input_dbl_clear: bool,
    pub cache_dir: String,
    pub snip_save_dir: String,
    pub word_fmt: i32,
    pub name_kind: i32,
    pub ai_enabled: bool,
    pub ai_url: String,
    pub ai_key: String,
    pub ai_model: String,
    pub baidu_enabled: bool,
    pub baidu_appid: String,
    pub baidu_key: String,
    pub baidu_url: String,
    pub hk_toggle: (i32, i32),
    pub hk_hist_up: (i32, i32),
    pub hk_hist_down: (i32, i32),
    pub hk_copy: (i32, i32),
    pub hk_snip: (i32, i32),
    pub hk_capture: (i32, i32),
    pub hk_pin: (i32, i32),
    pub max_words: usize,
    pub max_chars: usize,
    pub win_w: i32,
    pub win_h: i32,
    pub ui_scale: i32,
    pub theme_mode: i32,
    pub anim_enabled: bool,
    pub snip_sub_window: bool,
}

impl AppSettings {
    /// 构造带目录默认值的初始设置
    pub fn with_defaults(app_dir: &Path) -> Self {
        let app_dir_str = app_dir.to_string_lossy().into_owned();
        let snip_dir_str = app_dir.join("screenshots").to_string_lossy().into_owned();
        Self {
            topmost: false,
            close_exit: false,
            auto_start: false,
            auto_hide: false,
            summon_near_mouse: false,
            summon_clear: false,
            input_dbl_clear: false,
            cache_dir: app_dir_str,
            snip_save_dir: snip_dir_str,
            word_fmt: FY_FMT_CAMEL,
            name_kind: 0,
            ai_enabled: false,
            ai_url: String::new(),
            ai_key: String::new(),
            ai_model: "deepseek-chat".to_string(),
            baidu_enabled: false,
            baidu_appid: String::new(),
            baidu_key: String::new(),
            baidu_url: String::new(),
            hk_toggle: DEF_TOGGLE,
            hk_hist_up: DEF_HIST_UP,
            hk_hist_down: DEF_HIST_DOWN,
            hk_copy: DEF_COPY,
            hk_snip: DEF_SNIP,
            hk_capture: DEF_CAPTURE,
            hk_pin: DEF_PIN,
            max_words: 4,
            max_chars: 40,
            win_w: 0,
            win_h: 0,
            ui_scale: 100,
            theme_mode: 0,
            anim_enabled: true,
            snip_sub_window: true,
        }
    }

    /// 从 `Config` 读取全部配置，同时结合注册表校验自启动状态
    pub fn load_from_cfg(cfg: &Config, app_dir: &Path) -> Self {
        let mut s = Self::with_defaults(app_dir);
        let cfg_bool = |sec: &str, key: &str, def: bool| -> bool {
            let def_s = if def { "1" } else { "0" };
            cfg.get_str(sec, key, def_s)
                .trim()
                .parse::<i32>()
                .map(|v| v != 0)
                .unwrap_or(def)
        };
        let cfg_int = |sec: &str, key: &str, def: i32| -> i32 {
            cfg.get_str(sec, key, &def.to_string())
                .trim()
                .parse::<i32>()
                .unwrap_or(def)
        };
        let cfg_str = |sec: &str, key: &str, def: &str| -> String {
            cfg.get_str(sec, key, def).trim().to_string()
        };

        s.topmost = cfg_bool("通用设置", "窗口置顶", false);
        s.close_exit = cfg_bool("通用设置", "关闭退出", false);
        s.auto_start = cfg_bool("通用设置", "开机启动", false) || query_auto_start_reg();
        s.auto_hide = cfg_bool("通用设置", "开机隐藏", false);
        s.summon_near_mouse = cfg_bool("通用设置", "鼠标召唤", false);
        s.summon_clear = cfg_bool("通用设置", "召唤清空输入", false);
        s.input_dbl_clear = cfg_bool("通用设置", "双击清空输入", false);

        let cache = cfg_str("通用设置", "缓存目录", &s.cache_dir);
        if !cache.is_empty() {
            s.cache_dir = cache;
        }
        let snip_dir = cfg_str("通用设置", "截图保存目录", &s.snip_save_dir);
        if !snip_dir.is_empty() {
            s.snip_save_dir = snip_dir;
        }
        s.snip_sub_window = cfg_int("通用设置", "截屏识别子分区", 1) != 0;

        let wf = cfg_int("通用设置", "命名格式", FY_FMT_CAMEL);
        s.word_fmt = if wf == FY_FMT_SNAKE || wf == FY_FMT_CAMEL {
            wf
        } else {
            FY_FMT_CAMEL
        };
        s.name_kind = cfg_int("通用设置", "命名对象", 0).clamp(0, 6);

        s.ai_enabled = cfg_bool("AI翻译", "启用", false);
        s.ai_url = cfg_str("AI翻译", "地址", "");
        s.ai_key = cfg_str("AI翻译", "密钥", "");
        s.ai_model = cfg_str("AI翻译", "模型", "deepseek-chat");

        s.baidu_enabled = cfg_bool("百度翻译", "启用", false);
        s.baidu_appid = cfg_str("百度翻译", "AppID", "");
        s.baidu_key = cfg_str("百度翻译", "密钥", "");
        s.baidu_url = cfg_str("百度翻译", "地址", "");

        s.hk_toggle = (
            cfg_int("快捷按键", "隐藏界面1", DEF_TOGGLE.0),
            cfg_int("快捷按键", "隐藏界面2", DEF_TOGGLE.1),
        );
        s.hk_hist_up = (
            cfg_int("快捷按键", "上历史1", DEF_HIST_UP.0),
            cfg_int("快捷按键", "上历史2", DEF_HIST_UP.1),
        );
        s.hk_hist_down = (
            cfg_int("快捷按键", "下历史1", DEF_HIST_DOWN.0),
            cfg_int("快捷按键", "下历史2", DEF_HIST_DOWN.1),
        );
        s.hk_copy = (
            cfg_int("快捷按键", "复制1", DEF_COPY.0),
            cfg_int("快捷按键", "复制2", DEF_COPY.1),
        );
        s.hk_snip = (
            cfg_int("快捷按键", "截屏翻译1", DEF_SNIP.0),
            cfg_int("快捷按键", "截屏翻译2", DEF_SNIP.1),
        );
        s.hk_capture = (
            cfg_int("快捷按键", "截屏1", DEF_CAPTURE.0),
            cfg_int("快捷按键", "截屏2", DEF_CAPTURE.1),
        );
        s.hk_pin = (
            cfg_int("快捷按键", "贴图1", DEF_PIN.0),
            cfg_int("快捷按键", "贴图2", DEF_PIN.1),
        );

        s.max_words = cfg_int("复制单词", "最大词数", 4).max(1) as usize;
        s.max_chars = cfg_int("复制单词", "最大字符数", 40).max(4) as usize;

        s.win_w = cfg_int("通用设置", "窗口宽", 0);
        s.win_h = cfg_int("通用设置", "窗口高", 0);
        s.ui_scale = cfg_int("通用设置", "界面缩放", 100).clamp(75, 200);
        s.theme_mode = cfg_int("通用设置", "主题模式", 0).clamp(0, 2);
        s.anim_enabled = cfg_bool("通用设置", "界面动效", true);

        s
    }

    /// 将当前全量配置写入 `Config` 并保存到磁盘
    pub fn save_to_cfg(&self, cfg: &mut Config) -> std::io::Result<()> {
        let b = |v: bool| if v { "1" } else { "0" };
        cfg.set("通用设置", "窗口置顶", b(self.topmost));
        cfg.set("通用设置", "关闭退出", b(self.close_exit));
        cfg.set("通用设置", "开机启动", b(self.auto_start));
        cfg.set("通用设置", "开机隐藏", b(self.auto_hide));
        cfg.set("通用设置", "鼠标召唤", b(self.summon_near_mouse));
        cfg.set("通用设置", "召唤清空输入", b(self.summon_clear));
        cfg.set("通用设置", "双击清空输入", b(self.input_dbl_clear));
        cfg.set("通用设置", "缓存目录", &self.cache_dir);
        cfg.set("通用设置", "截图保存目录", &self.snip_save_dir);
        cfg.set("通用设置", "截屏识别子分区", b(self.snip_sub_window));
        cfg.set("通用设置", "命名格式", &self.word_fmt.to_string());
        cfg.set("通用设置", "命名对象", &self.name_kind.to_string());
        cfg.set("通用设置", "窗口宽", &self.win_w.to_string());
        cfg.set("通用设置", "窗口高", &self.win_h.to_string());
        cfg.set("通用设置", "界面缩放", &self.ui_scale.to_string());
        cfg.set("通用设置", "主题模式", &self.theme_mode.to_string());
        cfg.set("通用设置", "界面动效", b(self.anim_enabled));

        cfg.set("AI翻译", "启用", b(self.ai_enabled));
        cfg.set("AI翻译", "地址", &self.ai_url);
        cfg.set("AI翻译", "密钥", &self.ai_key);
        cfg.set("AI翻译", "模型", &self.ai_model);

        cfg.set("百度翻译", "启用", b(self.baidu_enabled));
        cfg.set("百度翻译", "AppID", &self.baidu_appid);
        cfg.set("百度翻译", "密钥", &self.baidu_key);
        cfg.set("百度翻译", "地址", &self.baidu_url);

        cfg.set("快捷按键", "隐藏界面1", &self.hk_toggle.0.to_string());
        cfg.set("快捷按键", "隐藏界面2", &self.hk_toggle.1.to_string());
        cfg.set("快捷按键", "上历史1", &self.hk_hist_up.0.to_string());
        cfg.set("快捷按键", "上历史2", &self.hk_hist_up.1.to_string());
        cfg.set("快捷按键", "下历史1", &self.hk_hist_down.0.to_string());
        cfg.set("快捷按键", "下历史2", &self.hk_hist_down.1.to_string());
        cfg.set("快捷按键", "复制1", &self.hk_copy.0.to_string());
        cfg.set("快捷按键", "复制2", &self.hk_copy.1.to_string());
        cfg.set("快捷按键", "截屏翻译1", &self.hk_snip.0.to_string());
        cfg.set("快捷按键", "截屏翻译2", &self.hk_snip.1.to_string());
        cfg.set("快捷按键", "截屏1", &self.hk_capture.0.to_string());
        cfg.set("快捷按键", "截屏2", &self.hk_capture.1.to_string());
        cfg.set("快捷按键", "贴图1", &self.hk_pin.0.to_string());
        cfg.set("快捷按键", "贴图2", &self.hk_pin.1.to_string());

        cfg.set("复制单词", "最大词数", &self.max_words.to_string());
        cfg.set("复制单词", "最大字符数", &self.max_chars.to_string());

        cfg.save()
    }

    /// 退出前保存动态状态（置顶/命名风格/命名对象/主窗尺寸）
    pub fn save_exit_state(&self, cfg: &mut Config, cur_w: i32, cur_h: i32) {
        cfg.set("通用设置", "窗口置顶", if self.topmost { "1" } else { "0" });
        cfg.set("通用设置", "命名格式", &self.word_fmt.to_string());
        cfg.set("通用设置", "命名对象", &self.name_kind.to_string());
        if cur_w > 0 && cur_h > 0 {
            cfg.set("通用设置", "窗口宽", &cur_w.to_string());
            cfg.set("通用设置", "窗口高", &cur_h.to_string());
        }
        let _ = cfg.save();
    }

    /// 按索引读取 7 个全局热键 `(mod_orig, vk)`
    pub fn hotkey_at(&self, idx: usize) -> (i32, i32) {
        match idx {
            0 => self.hk_toggle,
            1 => self.hk_hist_up,
            2 => self.hk_hist_down,
            3 => self.hk_copy,
            4 => self.hk_snip,
            5 => self.hk_capture,
            6 => self.hk_pin,
            _ => (0, 0),
        }
    }

    /// 按索引更新 7 个全局热键
    pub fn set_hotkey_at(&mut self, idx: usize, val: (i32, i32)) {
        match idx {
            0 => self.hk_toggle = val,
            1 => self.hk_hist_up = val,
            2 => self.hk_hist_down = val,
            3 => self.hk_copy = val,
            4 => self.hk_snip = val,
            5 => self.hk_capture = val,
            6 => self.hk_pin = val,
            _ => {}
        }
    }

    /// 是否采用暗色界面
    pub fn resolved_dark(&self) -> bool {
        match self.theme_mode {
            1 => false,
            2 => true,
            _ => is_system_dark_theme(),
        }
    }
}

/// 根据字体缩放比例计算主窗口默认逻辑宽高（与 modern-ui 保持一致）
pub fn compute_def_win_size(ui_scale_pct: i32) -> (i32, i32) {
    let sc = (ui_scale_pct.clamp(75, 200) as f64) / 100.0;
    let w = (550.0 * sc).round().max(546.0) as i32;
    let h = (392.0 * sc).round().max(350.0) as i32;
    (w, h)
}

/// 内部修饰位（`1=Ctrl, 2=Shift, 4=Alt, 8=Win`）转 Win32 `RegisterHotKey` 修饰位
#[inline]
pub const fn orig_to_win(m: i32) -> UINT {
    let mut r = 0;
    if (m & 1) != 0 {
        r |= MOD_CONTROL;
    }
    if (m & 2) != 0 {
        r |= MOD_SHIFT;
    }
    if (m & 4) != 0 {
        r |= MOD_ALT;
    }
    if (m & 8) != 0 {
        r |= MOD_WIN;
    }
    r
}

/// Win32 `RegisterHotKey` 修饰位转内部修饰位
#[inline]
pub const fn win_to_orig(m: UINT) -> i32 {
    let mut r = 0;
    if (m & MOD_CONTROL) != 0 {
        r |= 1;
    }
    if (m & MOD_SHIFT) != 0 {
        r |= 2;
    }
    if (m & MOD_ALT) != 0 {
        r |= 4;
    }
    if (m & MOD_WIN) != 0 {
        r |= 8;
    }
    r
}

/// 将内部热键 `(mod_orig, vk)` 格式化为人类可读快捷键标签（如 `Ctrl+Alt+D`、`F1`、`无`）
pub fn format_hotkey_text(mod_orig: i32, vk: i32) -> String {
    if vk <= 0 {
        return "无".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    if (mod_orig & 1) != 0 {
        parts.push("Ctrl".to_string());
    }
    if (mod_orig & 2) != 0 {
        parts.push("Shift".to_string());
    }
    if (mod_orig & 4) != 0 {
        parts.push("Alt".to_string());
    }
    if (mod_orig & 8) != 0 {
        parts.push("Win".to_string());
    }
    parts.push(vk_to_name(vk));
    parts.join("+")
}

/// 将虚拟键码转换为简洁键名
pub fn vk_to_name(vk: i32) -> String {
    match vk {
        0 => "无".to_string(),
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x1B => "Esc".to_string(),
        0x20 => "Space".to_string(),
        0x21 => "PageUp".to_string(),
        0x22 => "PageDown".to_string(),
        0x23 => "End".to_string(),
        0x24 => "Home".to_string(),
        0x25 => "Left".to_string(),
        0x26 => "Up".to_string(),
        0x27 => "Right".to_string(),
        0x28 => "Down".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x30..=0x39 => ((vk as u8) as char).to_string(),
        0x41..=0x5A => ((vk as u8) as char).to_string(),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0xBA => ";".to_string(),
        0xBB => "=".to_string(),
        0xBC => ",".to_string(),
        0xBD => "-".to_string(),
        0xBE => ".".to_string(),
        0xBF => "/".to_string(),
        0xC0 => "`".to_string(),
        _ => {
            // 兜底查询系统键位名称
            // SAFETY: MapVirtualKeyW 与 GetKeyNameTextW 安全读入栈缓冲区
            unsafe {
                let scan = MapVirtualKeyW(vk as UINT, 0);
                if scan != 0 {
                    let mut buf = [0u16; 32];
                    let n = GetKeyNameTextW((scan << 16) as LONG, buf.as_mut_ptr(), 32);
                    if n > 0 {
                        return crate::core::str_util::wide_to_utf8(&buf[..n as usize]);
                    }
                }
            }
            format!("VK_{vk:02X}")
        }
    }
}

/// 查询注册表 Run 键是否存在当前自启动项
pub fn query_auto_start_reg() -> bool {
    let sub = utf8_to_wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let name = utf8_to_wide(APP_TITLE);
    let mut hkey: HKEY = null_mut();
    // SAFETY: 读取当前用户 Run 键是否存在
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut hkey) != ERROR_SUCCESS {
            return false;
        }
        let exists = RegQueryValueExW(
            hkey,
            name.as_ptr(),
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
        ) == ERROR_SUCCESS;
        RegCloseKey(hkey);
        exists
    }
}

/// 写入或删除开机自启动注册表项
pub fn set_auto_start_reg(enable: bool, hide_on_start: bool, exe_path: &str) {
    let sub = utf8_to_wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let name = utf8_to_wide(APP_TITLE);
    let mut hkey: HKEY = null_mut();
    // SAFETY: 仅修改当前用户 HKCU Run 键，失败静默返回
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_WRITE, &mut hkey) != ERROR_SUCCESS
        {
            return;
        }
        if enable && !exe_path.is_empty() {
            let cmd = if hide_on_start {
                format!("\"{exe_path}\" -hide")
            } else {
                format!("\"{exe_path}\"")
            };
            let wcmd = utf8_to_wide(&cmd);
            let bytes = (wcmd.len() * 2) as DWORD;
            let _ = RegSetValueExW(
                hkey,
                name.as_ptr(),
                0,
                REG_SZ,
                wcmd.as_ptr().cast::<BYTE>(),
                bytes,
            );
        } else {
            let _ = RegDeleteValueW(hkey, name.as_ptr());
        }
        RegCloseKey(hkey);
    }
}

/// 查询 Windows 个性化设置判断是否启用系统深色应用主题
pub fn is_system_dark_theme() -> bool {
    let sub = utf8_to_wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let val_name = utf8_to_wide("AppsUseLightTheme");
    let mut hkey: HKEY = null_mut();
    // SAFETY: 只读查询 AppsUseLightTheme DWORD
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut hkey) != ERROR_SUCCESS {
            return true;
        }
        let mut val: DWORD = 1;
        let mut ty: DWORD = 0;
        let mut sz: DWORD = size_of::<DWORD>() as DWORD;
        let ok = RegQueryValueExW(
            hkey,
            val_name.as_ptr(),
            null_mut(),
            &mut ty,
            (&mut val as *mut DWORD).cast::<BYTE>(),
            &mut sz,
        ) == ERROR_SUCCESS;
        RegCloseKey(hkey);
        if ok && ty == REG_DWORD {
            val == 0
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_hotkey_mod_conversions_and_labels() {
        for m in 0..16 {
            let w = orig_to_win(m);
            assert_eq!(win_to_orig(w), m);
        }
        assert_eq!(orig_to_win(5), MOD_CONTROL | MOD_ALT);
        assert_eq!(format_hotkey_text(5, 68), "Ctrl+Alt+D");
        assert_eq!(format_hotkey_text(0, 0x70), "F1");
        assert_eq!(format_hotkey_text(0, 0), "无");
    }

    #[test]
    fn test_app_settings_cfg_roundtrip() {
        let dir = std::env::temp_dir().join("ky_rust_ui_settings_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let cfg_path = dir.join("config");

        let mut s = AppSettings::with_defaults(&dir);
        s.topmost = true;
        s.close_exit = true;
        s.word_fmt = FY_FMT_SNAKE;
        s.name_kind = 4;
        s.ai_enabled = true;
        s.ai_url = "https://api.deepseek.com".to_string();
        s.ai_model = "deepseek-chat".to_string();
        s.baidu_enabled = true;
        s.baidu_appid = "2024001".to_string();
        s.hk_snip = (3, 0x74); // Ctrl+Shift+F5
        s.ui_scale = 125;
        s.theme_mode = 2;
        s.anim_enabled = false;
        s.snip_sub_window = false;
        s.win_w = 640;
        s.win_h = 460;

        {
            let mut cfg = Config::open(&cfg_path);
            s.save_to_cfg(&mut cfg).expect("save cfg");
        }

        let cfg2 = Config::open(&cfg_path);
        let loaded = AppSettings::load_from_cfg(&cfg2, &dir);
        assert!(loaded.topmost);
        assert!(loaded.close_exit);
        assert_eq!(loaded.word_fmt, FY_FMT_SNAKE);
        assert_eq!(loaded.name_kind, 4);
        assert!(loaded.ai_enabled);
        assert_eq!(loaded.ai_url, "https://api.deepseek.com");
        assert!(loaded.baidu_enabled);
        assert_eq!(loaded.baidu_appid, "2024001");
        assert_eq!(loaded.hk_snip, (3, 0x74));
        assert_eq!(loaded.ui_scale, 125);
        assert_eq!(loaded.theme_mode, 2);
        assert!(!loaded.anim_enabled);
        assert!(!loaded.snip_sub_window);
        assert_eq!(loaded.win_w, 640);
        assert_eq!(loaded.win_h, 460);

        let _ = fs::remove_dir_all(&dir);
    }
}
