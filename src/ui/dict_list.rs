//! 通用虚拟滚动词典列表组件（缩略词 / 忽略词 / 常用词三页共用）

use crate::core::log::log_write;
use crate::core::str_util::utf8_to_wide;
use crate::naming_reducer::{
    builtin_abbreviations, builtin_preferred_pairs, builtin_stop_words, builtin_zh_kind,
    custom_abbr_entries, custom_abbr_pairs, custom_ignore_words, custom_keep_words,
    custom_preferred_entries, custom_preferred_pairs, preferred_merge, save_custom_abbreviations,
    save_custom_ignore, save_custom_preferred,
};
use crate::ui::win32_ui::{
    get_window_text, set_control_theme, set_window_text, wheel_delta, CreateWindowExW,
    DefSubclassProc, MoveWindow, SendMessageW, SetScrollInfo, SetWindowPos, SetWindowSubclass,
    ShowWindow, BOOL, DWORD, DWORD_PTR, EN_CHANGE, ES_AUTOHSCROLL, HWND, HWND_TOP, LPARAM, LRESULT,
    SBS_VERT, SB_CTL, SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP, SB_THUMBPOSITION,
    SB_THUMBTRACK, SCROLLINFO, SIF_ALL, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SW_HIDE, SW_SHOW, UINT, UINT_PTR, WM_MOUSEWHEEL, WM_SETFONT, WPARAM, WS_BORDER, WS_CHILD,
    WS_TABSTOP, WS_VISIBLE,
};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::ptr::null_mut;

pub const DICT_PAD_L: i32 = 20;
pub const DICT_CONT_W: i32 = 544;
pub const DICT_SEARCH_Y: i32 = 92;
pub const DICT_TOP: i32 = 128;
pub const DICT_ROW_H: i32 = 31;
pub const DICT_VIEW_ROWS: usize = 10;

const WHEEL_SUB_ID: usize = 0x4B59;

/// 列数模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictColMode {
    OneCol,
    TwoCol,
}

/// 词表业务类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictKind {
    Abbr,
    Ignore,
    Preferred,
}

impl DictKind {
    pub const fn col_mode(self) -> DictColMode {
        match self {
            DictKind::Ignore => DictColMode::OneCol,
            DictKind::Abbr | DictKind::Preferred => DictColMode::TwoCol,
        }
    }
}

/// 列表项（单列页 val 为空）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictItem {
    pub key: String,
    pub val: String,
}

/// 检查纯 ASCII 字母/数字词条
#[inline]
pub fn is_plain_word(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric())
}

// ---- 三张词表的合并、摘要与差异保存逻辑 ----

/// 合并内置与自定义缩略词（`-` 表示禁用），按原词升序
pub fn abbr_merge() -> Vec<DictItem> {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for (w, a) in builtin_abbreviations() {
        map.insert(w, a);
    }
    for (w, a) in custom_abbr_entries() {
        if a == "-" {
            map.remove(&w);
        } else {
            map.insert(w, a);
        }
    }
    map.into_iter()
        .map(|(k, v)| DictItem { key: k, val: v })
        .collect()
}

pub fn abbr_summary(total: usize) -> String {
    let builtin = builtin_abbreviations().len();
    let custom = custom_abbr_pairs().len();
    let disabled = custom_abbr_entries().len().saturating_sub(custom);
    let mut msg = format!("共 {total} 条：内置 {builtin}，自定义 {custom}");
    if disabled > 0 {
        msg.push_str(&format!("，禁用 {disabled}"));
    }
    msg
}

pub fn save_abbr_items(items: &[DictItem], app_dir: &Path) -> Result<String, String> {
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (i, it) in items.iter().enumerate() {
        let w = it.key.trim().to_ascii_lowercase();
        let a = it.val.trim().to_ascii_lowercase();
        if w.is_empty() && a.is_empty() {
            continue;
        }
        if w.is_empty() || a.is_empty() {
            return Err(format!("第 {} 行只填了一个框，原词和缩写都要填。", i + 1));
        }
        if !is_plain_word(&w) {
            return Err(format!(
                "第 {} 行的原词只能是英文字母/数字（空格、中文不支持）。",
                i + 1
            ));
        }
        if !is_plain_word(&a) {
            return Err(format!("第 {} 行的缩写只能是英文字母/数字。", i + 1));
        }
        if a.len() > 12 {
            return Err(format!("第 {} 行的缩写太长（最多 12 个字符）。", i + 1));
        }
        if !seen.insert(w.clone()) {
            return Err(format!(
                "「{w}」重复了（第 {} 行），同一原词只留一条。",
                i + 1
            ));
        }
        rows.push((w, a));
    }

    let builtin = builtin_abbreviations();
    let bmap: HashMap<String, String> = builtin.iter().cloned().collect();
    let mut lines = vec![
        "# 快译NG 自定义缩写：每行 原词=缩写（设置页保存时自动重写）".to_string(),
        "# 原词=- 表示禁用内置缩写；只记录与内置不同的条目".to_string(),
    ];
    for (w, a) in &rows {
        if bmap.get(w).is_some_and(|v| v == a) {
            continue;
        }
        lines.push(format!("{w}={a}"));
    }
    for (w, _) in &builtin {
        if !seen.contains(w) {
            lines.push(format!("{w}=-"));
        }
    }
    let content = lines.join("\n") + "\n";
    let (_, bad_line) = save_custom_abbreviations(app_dir.join("custom_abbr.txt"), &content);
    if bad_line > 0 {
        return Err(format!("保存失败：第 {bad_line} 行格式不对。"));
    }
    let dis = custom_abbr_entries()
        .len()
        .saturating_sub(custom_abbr_pairs().len());
    log_write(
        "设置",
        &format!("自定义缩写保存 {} 条 禁用 {} 条", rows.len(), dis),
    );
    let mut msg = format!("已保存 {} 条，立即生效", rows.len());
    if dis > 0 {
        msg.push_str(&format!("（禁用 {dis}）"));
    }
    Ok(msg)
}

/// 合并内置泛词与自定义忽略词（排除 `+词` 保留条目），按字母升序
pub fn ignore_merge() -> Vec<DictItem> {
    let keep: HashSet<String> = custom_keep_words().into_iter().collect();
    let mut words: HashSet<String> = HashSet::new();
    for w in builtin_stop_words() {
        if !keep.contains(&w) {
            words.insert(w);
        }
    }
    for w in custom_ignore_words() {
        if !keep.contains(&w) {
            words.insert(w);
        }
    }
    let mut sorted: Vec<String> = words.into_iter().collect();
    sorted.sort();
    sorted
        .into_iter()
        .map(|k| DictItem {
            key: k,
            val: String::new(),
        })
        .collect()
}

pub fn ignore_summary(total: usize) -> String {
    let builtin = builtin_stop_words().len();
    let custom = custom_ignore_words().len();
    let kept = custom_keep_words().len();
    let mut msg = format!("共 {total} 条：内置 {builtin}，自定义 {custom}");
    if kept > 0 {
        msg.push_str(&format!("，保留 {kept}"));
    }
    msg
}

pub fn save_ignore_items(items: &[DictItem], app_dir: &Path) -> Result<String, String> {
    let mut words: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (i, it) in items.iter().enumerate() {
        let w = it.key.trim().to_ascii_lowercase();
        if w.is_empty() {
            continue;
        }
        if !is_plain_word(&w) {
            return Err(format!(
                "第 {} 行只能是英文字母/数字（中文词请先在缩略页映射）。",
                i + 1
            ));
        }
        if seen.insert(w.clone()) {
            words.push(w);
        }
    }

    let builtin = builtin_stop_words();
    let bset: HashSet<String> = builtin.iter().cloned().collect();
    let mut lines = vec![
        "# 快译NG 自定义忽略词：每行一个英文单词（设置页保存时自动重写）".to_string(),
        "# +原词 表示保留该词（内置泛词里本来会被删掉）".to_string(),
    ];
    for w in &words {
        if !bset.contains(w) {
            lines.push(w.clone());
        }
    }
    for w in &builtin {
        if !seen.contains(w) {
            lines.push(format!("+{w}"));
        }
    }
    let content = lines.join("\n") + "\n";
    let n = save_custom_ignore(app_dir.join("custom_ignore.txt"), &content);
    let kept = custom_keep_words().len();
    log_write("设置", &format!("自定义忽略词保存 {n} 条 保留 {kept} 条"));
    let mut msg = format!("已保存 {} 条，立即生效", words.len());
    if kept > 0 {
        msg.push_str(&format!("（保留 {kept}）"));
    }
    Ok(msg)
}

/// 合并内置与自定义常用词
pub fn pref_merge_items() -> Vec<DictItem> {
    preferred_merge()
        .into_iter()
        .map(|(k, v)| DictItem { key: k, val: v })
        .collect()
}

pub fn pref_summary(total: usize) -> String {
    let builtin = builtin_preferred_pairs().len();
    let custom = custom_preferred_pairs().len();
    let disabled = custom_preferred_entries().len().saturating_sub(custom);
    let mut msg = format!("共 {total} 条：内置 {builtin}，自定义 {custom}");
    if disabled > 0 {
        msg.push_str(&format!("，停用 {disabled}"));
    }
    msg
}

pub fn save_pref_items(items: &[DictItem], app_dir: &Path) -> Result<String, String> {
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (i, it) in items.iter().enumerate() {
        let zh = it.key.trim().to_string();
        let en = it.val.trim().to_string();
        if zh.is_empty() && en.is_empty() {
            continue;
        }
        if zh.is_empty() || en.is_empty() {
            return Err(format!("第 {} 行只填了一个框，中文和英文都要填。", i + 1));
        }
        if zh.contains('=') {
            return Err(format!("第 {} 行的中文里不能有 =。", i + 1));
        }
        if zh.chars().count() > 8 {
            return Err(format!("第 {} 行的中文太长（最多 8 个字）。", i + 1));
        }
        if !zh.bytes().any(|b| b >= 0x80) {
            return Err(format!(
                "第 {} 行的中文要填中文词（纯英文匹配不到词表）。",
                i + 1
            ));
        }
        if builtin_zh_kind(&zh) == 2 {
            return Err(format!(
                "「{zh}」是内置普通词条，不归常用词页管（本页只管词库用词）。"
            ));
        }
        if en != "-" {
            let mut words = 0usize;
            for tok in en.split(' ').filter(|t| !t.is_empty()) {
                words += 1;
                if !is_plain_word(tok) {
                    return Err(format!(
                        "第 {} 行的英文只能是字母/数字（可多词，空格分隔）。",
                        i + 1
                    ));
                }
                if tok.len() > 20 {
                    return Err(format!("第 {} 行的英文单词太长（{tok}）。", i + 1));
                }
            }
            if words == 0 {
                return Err(format!("第 {} 行的英文要填内容。", i + 1));
            }
            if words > 4 {
                return Err(format!("第 {} 行的英文最多 4 个词。", i + 1));
            }
        } else if builtin_zh_kind(&zh) != 1 {
            return Err(format!(
                "「{zh}」不是内置词库词条，用不着停用（直接删行即可）。"
            ));
        }
        if !seen.insert(zh.clone()) {
            return Err(format!(
                "「{zh}」重复了（第 {} 行），同一中文只留一条。",
                i + 1
            ));
        }
        rows.push((zh, en));
    }

    let builtin = builtin_preferred_pairs();
    let bmap: HashMap<String, String> = builtin.iter().cloned().collect();
    let mut lines = vec![
        "# 快译NG 自定义常用词：每行 中文=英文（设置页保存时自动重写）".to_string(),
        "# 英文可多词（空格分隔）；中文=- 表示停用内置映射；只记录与内置不同的条目".to_string(),
    ];
    for (zh, en) in &rows {
        if bmap.get(zh).is_some_and(|v| v == en) {
            continue;
        }
        lines.push(format!("{zh}={en}"));
    }
    for (zh, _) in &builtin {
        if !seen.contains(zh) {
            lines.push(format!("{zh}=-"));
        }
    }
    let content = lines.join("\n") + "\n";
    let (_, bad_line) = save_custom_preferred(app_dir.join("custom_preferred.txt"), &content);
    if bad_line > 0 {
        return Err(format!("保存失败：第 {bad_line} 行格式不对。"));
    }
    let dis = custom_preferred_entries()
        .len()
        .saturating_sub(custom_preferred_pairs().len());
    log_write(
        "设置",
        &format!("自定义常用词保存 {} 条 停用 {} 条", rows.len(), dis),
    );
    let mut msg = format!("已保存 {} 条，立即生效", rows.len());
    if dis > 0 {
        msg.push_str(&format!("（停用 {dis}）"));
    }
    Ok(msg)
}

// ---- 虚拟列表状态机（脱离 HWND 即可测试滚动、筛选、增删） ----

#[derive(Debug, Clone)]
pub struct DictListModel {
    pub kind: DictKind,
    pub items: Vec<DictItem>,
    pub view_idx: Vec<usize>,
    pub filter: String,
    pub scroll_top: i32,
}

impl DictListModel {
    pub fn new(kind: DictKind) -> Self {
        Self {
            kind,
            items: Vec::new(),
            view_idx: Vec::new(),
            filter: String::new(),
            scroll_top: 0,
        }
    }

    pub fn max_scroll(&self) -> i32 {
        (self.view_idx.len() as i32 - DICT_VIEW_ROWS as i32).max(0)
    }

    pub fn set_items(&mut self, items: Vec<DictItem>, preserve_scroll: bool) {
        self.items = items;
        self.rebuild_view(preserve_scroll);
    }

    pub fn set_filter(&mut self, q: &str, preserve_scroll: bool) {
        self.filter = q.trim().to_ascii_lowercase();
        self.rebuild_view(preserve_scroll);
    }

    pub fn rebuild_view(&mut self, preserve_scroll: bool) {
        self.view_idx.clear();
        for (i, it) in self.items.iter().enumerate() {
            if self.filter.is_empty()
                || it.key.to_ascii_lowercase().contains(&self.filter)
                || (self.kind.col_mode() == DictColMode::TwoCol
                    && it.val.to_ascii_lowercase().contains(&self.filter))
            {
                self.view_idx.push(i);
            }
        }
        if preserve_scroll {
            self.scroll_top = self.scroll_top.clamp(0, self.max_scroll());
        } else {
            self.scroll_top = 0;
        }
    }

    pub fn scroll_by(&mut self, notches: i32) {
        self.scroll_top = (self.scroll_top - notches * 3).clamp(0, self.max_scroll());
    }

    pub fn set_scroll_pos(&mut self, pos: i32) {
        self.scroll_top = pos.clamp(0, self.max_scroll());
    }

    /// 添加或校验一行新词条；成功返回新建序号
    pub fn add_item(&mut self, raw_k: &str, raw_v: &str) -> Result<usize, String> {
        let mut k = raw_k.trim().to_string();
        let mut v = raw_v.trim().to_string();
        if self.kind != DictKind::Preferred {
            k = k.to_ascii_lowercase();
            v = v.to_ascii_lowercase();
        }
        match self.kind {
            DictKind::Abbr => {
                if k.is_empty() || v.is_empty() {
                    return Err("原词和缩写都要填。".to_string());
                }
                if !is_plain_word(&k) || !is_plain_word(&v) {
                    return Err("原词和缩写只能是英文字母/数字。".to_string());
                }
            }
            DictKind::Ignore => {
                if k.is_empty() {
                    return Err("请输入要忽略的英文单词。".to_string());
                }
                if !is_plain_word(&k) {
                    return Err("只能填英文字母/数字。".to_string());
                }
            }
            DictKind::Preferred => {
                if k.is_empty() || v.is_empty() {
                    return Err("中文词和目标英文都要填。".to_string());
                }
            }
        }
        if self.items.iter().any(|it| it.key == k) {
            return Err(format!("「{k}」已经在列表里。"));
        }
        self.items.push(DictItem { key: k, val: v });
        self.filter.clear();
        self.rebuild_view(false);
        self.scroll_top = self.max_scroll();
        Ok(self.items.len())
    }

    /// 更新当前视图第 `slot` 行文本
    pub fn update_slot_text(&mut self, slot: usize, is_val: bool, text: &str) {
        let vi = (self.scroll_top as usize) + slot;
        if let Some(&idx) = self.view_idx.get(vi) {
            if let Some(item) = self.items.get_mut(idx) {
                if is_val {
                    item.val = text.to_string();
                } else {
                    item.key = text.to_string();
                }
            }
        }
    }

    /// 删除当前视图第 `slot` 行
    pub fn delete_slot(&mut self, slot: usize) -> bool {
        let vi = (self.scroll_top as usize) + slot;
        if let Some(&idx) = self.view_idx.get(vi) {
            if idx < self.items.len() {
                self.items.remove(idx);
                self.rebuild_view(true);
                return true;
            }
        }
        false
    }

    pub fn load_merged(&mut self, preserve_scroll: bool) {
        let merged = match self.kind {
            DictKind::Abbr => abbr_merge(),
            DictKind::Ignore => ignore_merge(),
            DictKind::Preferred => pref_merge_items(),
        };
        self.set_items(merged, preserve_scroll);
    }

    pub fn summary(&self) -> String {
        let total = self.items.len();
        match self.kind {
            DictKind::Abbr => abbr_summary(total),
            DictKind::Ignore => ignore_summary(total),
            DictKind::Preferred => pref_summary(total),
        }
    }

    pub fn save_to_disk(&self, app_dir: &Path) -> Result<String, String> {
        match self.kind {
            DictKind::Abbr => save_abbr_items(&self.items, app_dir),
            DictKind::Ignore => save_ignore_items(&self.items, app_dir),
            DictKind::Preferred => save_pref_items(&self.items, app_dir),
        }
    }
}

// ---- Win32 词典列表组件视图 ----

/// 列表操作回调消息由宿主页统一分发，这里定义结构化的控件 ID 段：
/// 每张词典页占 100 个 ID 槽：`base + 0..9` key edit，`base + 10..19` val edit，`base + 20..29` del btn，
/// `base + 30` search edit，`base + 31` clear btn，`base + 32` add key，`base + 33` add val，
/// `base + 34` add btn，`base + 35` apply btn，`base + 36` scrollbar
pub struct DictListView {
    pub base_id: i32,
    pub model: DictListModel,
    pub parent: HWND,
    pub edt_search: HWND,
    pub btn_clear: HWND,
    pub scrollbar: HWND,
    pub key_edits: [HWND; DICT_VIEW_ROWS],
    pub val_edits: [HWND; DICT_VIEW_ROWS],
    pub del_btns: [HWND; DICT_VIEW_ROWS],
    pub edt_add_k: HWND,
    pub edt_add_v: HWND,
    pub lbl_eq: HWND,
    pub btn_add: HWND,
    pub lbl_status: HWND,
    pub btn_apply: HWND,
    pub tip_texts: (String, String),
    pub sync_guard: bool,
    pub kw: i32,
    pub vw: i32,
}

#[inline]
fn s_px(v: i32, scale: f64) -> i32 {
    ((v as f64) * scale).round() as i32
}

unsafe extern "system" fn dict_wheel_subclass(
    hwnd: HWND,
    msg: UINT,
    wp: WPARAM,
    lp: LPARAM,
    _id: UINT_PTR,
    parent_hwnd_raw: DWORD_PTR,
) -> LRESULT {
    if msg == WM_MOUSEWHEEL && parent_hwnd_raw != 0 {
        crate::ui::win32_ui::PostMessageW(parent_hwnd_raw as HWND, WM_MOUSEWHEEL, wp, lp);
        return 0;
    }
    DefSubclassProc(hwnd, msg, wp, lp)
}

unsafe fn hook_wheel(hwnd: HWND, page_hwnd: HWND) {
    if !hwnd.is_null() {
        SetWindowSubclass(
            hwnd,
            dict_wheel_subclass,
            WHEEL_SUB_ID,
            page_hwnd as DWORD_PTR,
        );
    }
}

impl DictListView {
    /// 创建并排布词典页全部 Win32 子控件
    ///
    /// # Safety
    /// `parent` 必须为有效的父窗口句柄，所有子控件消息回传至该父窗口。
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn create(
        parent: HWND,
        base_id: i32,
        kind: DictKind,
        tip1: &str,
        tip2: &str,
        search_ph: &str,
        add_ph_k: &str,
        add_ph_v: &str,
        scale: f64,
        is_dark: bool,
    ) -> Self {
        let mut view = Self {
            base_id,
            model: DictListModel::new(kind),
            parent,
            edt_search: null_mut(),
            btn_clear: null_mut(),
            scrollbar: null_mut(),
            key_edits: [null_mut(); DICT_VIEW_ROWS],
            val_edits: [null_mut(); DICT_VIEW_ROWS],
            del_btns: [null_mut(); DICT_VIEW_ROWS],
            edt_add_k: null_mut(),
            edt_add_v: null_mut(),
            lbl_eq: null_mut(),
            btn_add: null_mut(),
            lbl_status: null_mut(),
            btn_apply: null_mut(),
            tip_texts: (tip1.to_string(), tip2.to_string()),
            sync_guard: false,
            kw: if kind.col_mode() == DictColMode::TwoCol {
                192
            } else {
                474
            },
            vw: if kind.col_mode() == DictColMode::TwoCol {
                274
            } else {
                0
            },
        };

        let cls_edit = utf8_to_wide("EDIT");
        let cls_btn = utf8_to_wide("BUTTON");
        let cls_static = utf8_to_wide("STATIC");
        let cls_sb = utf8_to_wide("SCROLLBAR");
        let empty = utf8_to_wide("");
        let style_edit = WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL;

        let make_ctrl = |cls: &[u16], txt: &str, style: DWORD, id: i32| -> HWND {
            let wt = utf8_to_wide(txt);
            let h = CreateWindowExW(
                0,
                cls.as_ptr(),
                wt.as_ptr(),
                style,
                0,
                0,
                10,
                10,
                parent,
                id as _,
                null_mut(),
                null_mut(),
            );
            hook_wheel(h, parent);
            h
        };

        // 搜索框与清除按钮
        view.edt_search = make_ctrl(&cls_edit, "", style_edit, base_id + 30);
        // 设置 CueBanner 提示
        let wph = utf8_to_wide(search_ph);
        SendMessageW(view.edt_search, 0x1501, 1, wph.as_ptr() as LPARAM);
        view.btn_clear = make_ctrl(&cls_btn, "×", WS_CHILD | WS_VISIBLE | 0x0B, base_id + 31);

        // 滚动条
        view.scrollbar = make_ctrl(&cls_sb, "", WS_CHILD | WS_VISIBLE | SBS_VERT, base_id + 36);
        set_control_theme(
            view.scrollbar,
            if is_dark {
                "DarkMode_Explorer"
            } else {
                "Explorer"
            },
        );

        // 10 行槽位
        for slot in 0..DICT_VIEW_ROWS {
            view.key_edits[slot] = make_ctrl(&cls_edit, "", style_edit, base_id + (slot as i32));
            if kind.col_mode() == DictColMode::TwoCol {
                view.val_edits[slot] =
                    make_ctrl(&cls_edit, "", style_edit, base_id + 10 + (slot as i32));
            }
            view.del_btns[slot] = make_ctrl(
                &cls_btn,
                "删",
                WS_CHILD | WS_VISIBLE | 0x0B,
                base_id + 20 + (slot as i32),
            );
        }

        // 底部新增与保存操作条
        view.edt_add_k = make_ctrl(&cls_edit, "", style_edit, base_id + 32);
        let wk = utf8_to_wide(add_ph_k);
        SendMessageW(view.edt_add_k, 0x1501, 1, wk.as_ptr() as LPARAM);

        if kind.col_mode() == DictColMode::TwoCol {
            view.lbl_eq = make_ctrl(&cls_static, "=", WS_CHILD | WS_VISIBLE, 0);
            view.edt_add_v = make_ctrl(&cls_edit, "", style_edit, base_id + 33);
            let wv = utf8_to_wide(add_ph_v);
            SendMessageW(view.edt_add_v, 0x1501, 1, wv.as_ptr() as LPARAM);
        }
        view.btn_add = make_ctrl(&cls_btn, "添加", WS_CHILD | WS_VISIBLE | 0x0B, base_id + 34);
        view.lbl_status = make_ctrl(&cls_static, "", WS_CHILD | WS_VISIBLE, 0);
        let _ = empty;
        view.btn_apply = make_ctrl(
            &cls_btn,
            "应用本页",
            WS_CHILD | WS_VISIBLE | 0x0B,
            base_id + 35,
        );

        hook_wheel(parent, parent);
        view.model.load_merged(false);
        view.relayout(scale);
        view.sync_slots();
        view
    }

    /// 按缩放系数重新计算并移动列表页全部子控件位置
    ///
    /// # Safety
    /// 调用方须保证内部持有的所有控件 `HWND` 均处于有效生命周期内。
    pub unsafe fn relayout(&self, scale: f64) {
        let pad_l = s_px(DICT_PAD_L, scale);
        let cont_w = s_px(DICT_CONT_W, scale);
        let sb_w = s_px(16, scale);
        let search_y = s_px(DICT_SEARCH_Y, scale);
        let clr_w = s_px(28, scale);
        let row_h = s_px(DICT_ROW_H, scale);
        let top = s_px(DICT_TOP, scale);
        let eh = s_px(24, scale);
        let del_w = s_px(42, scale);
        let gap = s_px(6, scale);

        MoveWindow(
            self.edt_search,
            pad_l,
            search_y,
            cont_w - sb_w - gap - clr_w - s_px(4, scale),
            s_px(25, scale),
            1,
        );
        MoveWindow(
            self.btn_clear,
            pad_l + cont_w - sb_w - clr_w - s_px(4, scale),
            search_y,
            clr_w,
            s_px(25, scale),
            1,
        );

        let sb_x = pad_l + cont_w - sb_w;
        MoveWindow(
            self.scrollbar,
            sb_x,
            top,
            sb_w,
            (DICT_VIEW_ROWS as i32) * row_h - s_px(5, scale),
            1,
        );

        let kw = s_px(self.kw, scale);
        let vw = s_px(self.vw, scale);
        for slot in 0..DICT_VIEW_ROWS {
            let y = top + (slot as i32) * row_h;
            MoveWindow(self.key_edits[slot], pad_l, y, kw, eh, 1);
            let del_x = if self.model.kind.col_mode() == DictColMode::TwoCol {
                let vx = pad_l + kw + gap;
                MoveWindow(self.val_edits[slot], vx, y, vw, eh, 1);
                vx + vw + gap
            } else {
                pad_l + kw + gap
            };
            MoveWindow(self.del_btns[slot], del_x, y, del_w, eh, 1);
        }

        let bot_y = s_px(456, scale);
        let add_h = s_px(26, scale);
        let add_btn_w = s_px(60, scale);
        let apply_w = s_px(88, scale);
        if self.model.kind.col_mode() == DictColMode::TwoCol {
            let w_k = s_px(108, scale);
            let w_eq = s_px(14, scale);
            let w_v = s_px(116, scale);
            MoveWindow(self.edt_add_k, pad_l, bot_y, w_k, add_h, 1);
            MoveWindow(
                self.lbl_eq,
                pad_l + w_k + s_px(2, scale),
                bot_y + s_px(4, scale),
                w_eq,
                s_px(18, scale),
                1,
            );
            let vx = pad_l + w_k + w_eq + s_px(2, scale);
            MoveWindow(self.edt_add_v, vx, bot_y, w_v, add_h, 1);
            let ax = vx + w_v + gap;
            MoveWindow(self.btn_add, ax, bot_y, add_btn_w, add_h, 1);
            let sx = ax + add_btn_w + s_px(8, scale);
            MoveWindow(
                self.lbl_status,
                sx,
                bot_y + s_px(4, scale),
                (pad_l + cont_w - apply_w - gap - sx).max(20),
                s_px(20, scale),
                1,
            );
        } else {
            let w_k = s_px(180, scale);
            MoveWindow(self.edt_add_k, pad_l, bot_y, w_k, add_h, 1);
            let ax = pad_l + w_k + gap;
            MoveWindow(self.btn_add, ax, bot_y, add_btn_w, add_h, 1);
            let sx = ax + add_btn_w + s_px(8, scale);
            MoveWindow(
                self.lbl_status,
                sx,
                bot_y + s_px(4, scale),
                (pad_l + cont_w - apply_w - gap - sx).max(20),
                s_px(20, scale),
                1,
            );
        }
        MoveWindow(
            self.btn_apply,
            pad_l + cont_w - apply_w,
            bot_y,
            apply_w,
            add_h,
            1,
        );
    }

    /// 将当前滚动视口的 10 行数据同步写入控件并刷新滚动条范围
    ///
    /// # Safety
    /// 调用方须保证在拥有窗口控件的 UI 线程内调用且所有 `HWND` 有效。
    pub unsafe fn sync_slots(&mut self) {
        self.sync_guard = true;
        let top = self.model.scroll_top as usize;
        let total_view = self.model.view_idx.len();
        for slot in 0..DICT_VIEW_ROWS {
            let vi = top + slot;
            if let Some(&idx) = self.model.view_idx.get(vi) {
                let item = &self.model.items[idx];
                set_window_text(self.key_edits[slot], &item.key);
                ShowWindow(self.key_edits[slot], SW_SHOW);
                if !self.val_edits[slot].is_null() {
                    set_window_text(self.val_edits[slot], &item.val);
                    ShowWindow(self.val_edits[slot], SW_SHOW);
                }
                ShowWindow(self.del_btns[slot], SW_SHOW);
            } else {
                ShowWindow(self.key_edits[slot], SW_HIDE);
                if !self.val_edits[slot].is_null() {
                    ShowWindow(self.val_edits[slot], SW_HIDE);
                }
                ShowWindow(self.del_btns[slot], SW_HIDE);
            }
        }

        let max_scroll = self.model.max_scroll();
        let si = SCROLLINFO {
            cbSize: std::mem::size_of::<SCROLLINFO>() as UINT,
            fMask: SIF_ALL,
            nMin: 0,
            nMax: if total_view == 0 {
                0
            } else {
                (total_view as i32 - 1).max(0)
            },
            nPage: DICT_VIEW_ROWS as UINT,
            nPos: self.model.scroll_top,
            nTrackPos: 0,
        };
        SetScrollInfo(self.scrollbar, SB_CTL, &si, 1);
        ShowWindow(
            self.scrollbar,
            if max_scroll > 0 { SW_SHOW } else { SW_HIDE },
        );
        set_window_text(self.lbl_status, &self.model.summary());
        self.sync_guard = false;
    }

    /// 更新词表页各子控件的字体与滚动条深浅主题
    ///
    /// # Safety
    /// `hfont` 须为有效字体句柄且组件子控件尚未销毁。
    pub unsafe fn apply_font_and_theme(&self, hfont: crate::ui::win32_ui::HFONT, is_dark: bool) {
        let theme = if is_dark {
            "DarkMode_Explorer"
        } else {
            "Explorer"
        };
        set_control_theme(self.scrollbar, theme);
        let apply = |h: HWND| {
            if !h.is_null() {
                SendMessageW(h, WM_SETFONT, hfont as WPARAM, 1);
            }
        };
        apply(self.edt_search);
        apply(self.btn_clear);
        apply(self.edt_add_k);
        apply(self.edt_add_v);
        apply(self.lbl_eq);
        apply(self.btn_add);
        apply(self.lbl_status);
        apply(self.btn_apply);
        for slot in 0..DICT_VIEW_ROWS {
            apply(self.key_edits[slot]);
            apply(self.val_edits[slot]);
            apply(self.del_btns[slot]);
        }
        SetWindowPos(
            self.scrollbar,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }

    /// 处理子控件命令与输入修改，返回 `(handled, Option<toast_or_status>)`
    ///
    /// # Safety
    /// 调用方须在 UI 线程调用并保证相关控件句柄有效。
    pub unsafe fn handle_command(
        &mut self,
        ctrl_id: i32,
        code: u16,
        app_dir: &Path,
    ) -> (bool, Option<Result<String, String>>) {
        let rel = ctrl_id - self.base_id;
        if !(0..=36).contains(&rel) {
            return (false, None);
        }
        if self.sync_guard {
            return (true, None);
        }
        match rel {
            0..=9 => {
                if code == EN_CHANGE {
                    let slot = rel as usize;
                    let txt = get_window_text(self.key_edits[slot]);
                    self.model.update_slot_text(slot, false, &txt);
                }
                (true, None)
            }
            10..=19 => {
                if code == EN_CHANGE {
                    let slot = (rel - 10) as usize;
                    let txt = get_window_text(self.val_edits[slot]);
                    self.model.update_slot_text(slot, true, &txt);
                }
                (true, None)
            }
            20..=29 => {
                let slot = (rel - 20) as usize;
                if self.model.delete_slot(slot) {
                    self.sync_slots();
                }
                (true, None)
            }
            30 => {
                if code == EN_CHANGE {
                    let q = get_window_text(self.edt_search);
                    self.model.set_filter(&q, false);
                    self.sync_slots();
                }
                (true, None)
            }
            31 => {
                set_window_text(self.edt_search, "");
                self.model.set_filter("", false);
                self.sync_slots();
                (true, None)
            }
            34 => {
                let k = get_window_text(self.edt_add_k);
                let v = if self.edt_add_v.is_null() {
                    String::new()
                } else {
                    get_window_text(self.edt_add_v)
                };
                match self.model.add_item(&k, &v) {
                    Ok(_) => {
                        set_window_text(self.edt_add_k, "");
                        if !self.edt_add_v.is_null() {
                            set_window_text(self.edt_add_v, "");
                        }
                        set_window_text(self.edt_search, "");
                        self.sync_slots();
                        (true, None)
                    }
                    Err(e) => {
                        set_window_text(self.lbl_status, &e);
                        (true, Some(Err(e)))
                    }
                }
            }
            35 => {
                let res = self.model.save_to_disk(app_dir);
                match &res {
                    Ok(msg) => {
                        self.model.load_merged(true);
                        self.sync_slots();
                        set_window_text(self.lbl_status, msg);
                    }
                    Err(err) => {
                        set_window_text(self.lbl_status, &format!("未保存：{err}"));
                    }
                }
                (true, Some(res))
            }
            _ => (false, None),
        }
    }

    /// 响应列表滚动条 `WM_VSCROLL` 消息并同步视口
    ///
    /// # Safety
    /// 必须在拥有控件的 UI 线程上调用。
    pub unsafe fn handle_vscroll(&mut self, wp: WPARAM) {
        let sb_code = (wp & 0xFFFF) as u16;
        let track_pos = ((wp >> 16) & 0xFFFF) as i16 as i32;
        let mut pos = self.model.scroll_top;
        match sb_code {
            SB_LINEUP => pos -= 1,
            SB_LINEDOWN => pos += 1,
            SB_PAGEUP => pos -= DICT_VIEW_ROWS as i32,
            SB_PAGEDOWN => pos += DICT_VIEW_ROWS as i32,
            SB_THUMBTRACK | SB_THUMBPOSITION => pos = track_pos,
            _ => return,
        }
        self.model.set_scroll_pos(pos);
        self.sync_slots();
    }

    /// 响应鼠标滚轮消息（每格滚动 3 行）
    ///
    /// # Safety
    /// 必须在拥有控件的 UI 线程上调用。
    pub unsafe fn handle_wheel(&mut self, wp: WPARAM) {
        let notches = wheel_delta(wp) / 120;
        if notches != 0 {
            self.model.scroll_by(notches);
            self.sync_slots();
        }
    }
}

pub fn check_dict_wheel_unused_keep() -> BOOL {
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_dict_model_filter_scroll_add_delete() {
        let mut m = DictListModel::new(DictKind::Abbr);
        let mut init = Vec::new();
        for i in 0..25 {
            init.push(DictItem {
                key: format!("word{i:02}"),
                val: format!("w{i:02}"),
            });
        }
        m.set_items(init, false);
        assert_eq!(m.view_idx.len(), 25);
        assert_eq!(m.max_scroll(), 15);

        m.scroll_by(-1); // 向下滚 3 行
        assert_eq!(m.scroll_top, 3);
        m.set_filter("word1", false);
        assert_eq!(m.view_idx.len(), 10);
        assert_eq!(m.scroll_top, 0);

        m.set_filter("", false);
        assert!(m.delete_slot(0));
        assert_eq!(m.items.len(), 24);
        assert_eq!(m.items[0].key, "word01");

        assert!(m.add_item("customKey", "ck").is_ok());
        assert_eq!(m.items.last().map(|x| x.key.as_str()), Some("customkey"));
        assert!(m.add_item("customKey", "dup").is_err());
        assert!(m.add_item("bad word", "bw").is_err());
    }

    #[test]
    fn test_save_abbr_ignore_preferred_validation() {
        let _global_guard = crate::core::TEST_GLOBAL_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join("ky_rust_dict_save_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");

        // 缩写校验与保存
        let bad_abbr = vec![DictItem {
            key: "bad word".into(),
            val: "bw".into(),
        }];
        assert!(save_abbr_items(&bad_abbr, &dir).is_err());

        let mut abbr = abbr_merge();
        abbr.push(DictItem {
            key: "telemetry".into(),
            val: "tlm".into(),
        });
        let ok = save_abbr_items(&abbr, &dir).expect("save abbr");
        assert!(ok.contains("已保存"));
        // 清理测试污染出的自定义缩写全局内存表
        let _ = save_custom_abbreviations(dir.join("custom_abbr.txt"), "");

        // 常用词校验
        let bad_pref = vec![DictItem {
            key: "englishonly".into(),
            val: "abc".into(),
        }];
        assert!(save_pref_items(&bad_pref, &dir).is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
