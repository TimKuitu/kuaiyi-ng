//! 快译NG 截屏 OCR 与原图中文替换渲染
//!
//! - OCR：动态加载 `combase.dll` 调用 Windows 10/11 原生 `Windows.Media.Ocr`，零外部依赖
//! - 翻译：复用 `translate_chain_ex(FY_EN2ZH)`，多块合并单次请求，行数不一致时逐块并发兜底
//! - 渲染：采样周边背景色擦除原英文，按框尺寸自适应字号绘制微软雅黑中文

use crate::core::bing::translate_chain_ex;
use crate::core::log::log_write;
use crate::core::str_util::{to_c_bytes, utf8_to_wide, wide_to_utf8};
use crate::core::win32::{get_b_value, get_g_value, get_r_value, rgb};
use crate::core::win32::{
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DeleteDC, DeleteObject,
    ExtTextOutW, FillRect, GdiFlush, GetDC, GetProcAddress, GetTextExtentPoint32W, GetTextMetricsW,
    LoadLibraryA, ReleaseDC, SelectObject, SetBkMode, SetBrushOrgEx, SetStretchBltMode,
    SetTextColor, StretchBlt, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CLEARTYPE_QUALITY, COLORREF,
    DEFAULT_CHARSET, DIB_RGB_COLORS, DWORD, ETO_CLIPPED, FW_NORMAL, GUID, HALFTONE, HBRUSH, HDC,
    HFONT, HMODULE, HRESULT, RECT, SIZE, SRCCOPY, TEXTMETRICW, TRANSPARENT, UINT, WCHAR,
};
use crate::core::{FY_CHAIN_NO_DICT, FY_CHAIN_PREFER_BAIDU, FY_EN2ZH};
use std::collections::HashMap;
use std::ffi::c_void;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};

/// 截屏 OCR 翻译输出结构
#[derive(Debug, Clone)]
pub struct SnipResult {
    /// 是否成功
    pub ok: bool,
    /// 图像宽度（像素）
    pub width: i32,
    /// 图像高度（像素）
    pub height: i32,
    /// 32-bit BGRA top-down 渲染后图像数据
    pub bgra_out: Vec<u8>,
    /// 全部译文（UTF-8，换行分隔）
    pub zh_text: String,
    /// 全部原文（UTF-8，换行分隔）
    pub en_text: String,
    /// 失败时的错误信息（UTF-8）
    pub err: String,
}

impl SnipResult {
    fn fail(width: i32, height: i32, bgra_out: Vec<u8>, err: &str) -> Self {
        Self {
            ok: false,
            width,
            height,
            bgra_out,
            zh_text: String::new(),
            en_text: String::new(),
            err: err.to_string(),
        }
    }
}

// ---- WinRT 最小 COM ABI 声明（避免引入 winrt/windows 大型 crate） ----

type FyHString = *mut c_void;

#[repr(C)]
struct FyHStringHeader {
    reserved: *mut c_void,
    data: [u8; 24],
}

impl Default for FyHStringHeader {
    fn default() -> Self {
        Self {
            reserved: null_mut(),
            data: [0; 24],
        }
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FyWinRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

type PfnRoInitialize = unsafe extern "system" fn(i32) -> HRESULT;
type PfnRoUninitialize = unsafe extern "system" fn();
type PfnRoGetActivationFactory =
    unsafe extern "system" fn(FyHString, *const GUID, *mut *mut c_void) -> HRESULT;
type PfnWindowsCreateStringReference =
    unsafe extern "system" fn(*const WCHAR, UINT, *mut FyHStringHeader, *mut FyHString) -> HRESULT;
type PfnWindowsGetStringRawBuffer = unsafe extern "system" fn(FyHString, *mut UINT) -> *const WCHAR;
type PfnWindowsDeleteString = unsafe extern "system" fn(FyHString) -> HRESULT;

#[repr(C)]
struct FyIInspectableVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_iids: unsafe extern "system" fn(*mut c_void, *mut u32, *mut *mut GUID) -> HRESULT,
    get_runtime_class_name: unsafe extern "system" fn(*mut c_void, *mut FyHString) -> HRESULT,
    get_trust_level: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
}

const FY_IID_IASYNC_INFO: GUID = GUID {
    Data1: 0x0000_0036,
    Data2: 0x0000,
    Data3: 0x0000,
    Data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

#[repr(C)]
struct FyIAsyncInfoVtbl {
    base: FyIInspectableVtbl,
    get_id: unsafe extern "system" fn(*mut c_void, *mut UINT) -> HRESULT,
    get_status: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    get_error_code: unsafe extern "system" fn(*mut c_void, *mut HRESULT) -> HRESULT,
    cancel: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    close: unsafe extern "system" fn(*mut c_void) -> HRESULT,
}

#[repr(C)]
struct FyIAsyncOperationVtbl {
    base: FyIInspectableVtbl,
    put_completed: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    get_completed: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    get_results: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

const FY_IID_ILANGUAGE_FACTORY: GUID = GUID {
    Data1: 0x9B02_52AC,
    Data2: 0x0C27,
    Data3: 0x44F8,
    Data4: [0xB7, 0x92, 0x97, 0x93, 0xFB, 0x66, 0xC6, 0x3E],
};

#[repr(C)]
struct FyILanguageFactoryVtbl {
    base: FyIInspectableVtbl,
    create_language: unsafe extern "system" fn(*mut c_void, FyHString, *mut *mut c_void) -> HRESULT,
}

const FY_IID_IOCR_ENGINE_STATICS: GUID = GUID {
    Data1: 0x5BFF_A85A,
    Data2: 0x3384,
    Data3: 0x3540,
    Data4: [0x99, 0x40, 0x69, 0x91, 0x20, 0xD4, 0x28, 0xA8],
};

#[repr(C)]
struct FyIOcrEngineStaticsVtbl {
    base: FyIInspectableVtbl,
    get_max_image_dimension: unsafe extern "system" fn(*mut c_void, *mut UINT) -> HRESULT,
    get_available_recognizer_languages:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    is_language_supported: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut u8) -> HRESULT,
    try_create_from_language:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
    try_create_from_user_profile_languages:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
struct FyIOcrEngineVtbl {
    base: FyIInspectableVtbl,
    recognize_async:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
    get_recognizer_language: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

const FY_IID_ISOFTWARE_BITMAP_STATICS: GUID = GUID {
    Data1: 0xDF03_85DB,
    Data2: 0x672F,
    Data3: 0x4A9D,
    Data4: [0x80, 0x6E, 0xC2, 0x44, 0x2F, 0x34, 0x3E, 0x86],
};

#[repr(C)]
struct FyISoftwareBitmapStaticsVtbl {
    base: FyIInspectableVtbl,
    copy: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
    convert: unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut *mut c_void) -> HRESULT,
    convert_with_alpha:
        unsafe extern "system" fn(*mut c_void, *mut c_void, i32, i32, *mut *mut c_void) -> HRESULT,
    create_copy_from_buffer: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        i32,
        i32,
        i32,
        *mut *mut c_void,
    ) -> HRESULT,
}

const FY_IID_IBUFFER_FACTORY: GUID = GUID {
    Data1: 0x71AF_914D,
    Data2: 0xC10F,
    Data3: 0x484B,
    Data4: [0xBC, 0x50, 0x14, 0xBC, 0x62, 0x3B, 0x3A, 0x27],
};

#[repr(C)]
struct FyIBufferFactoryVtbl {
    base: FyIInspectableVtbl,
    create: unsafe extern "system" fn(*mut c_void, UINT, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
struct FyIBufferVtbl {
    base: FyIInspectableVtbl,
    get_capacity: unsafe extern "system" fn(*mut c_void, *mut UINT) -> HRESULT,
    get_length: unsafe extern "system" fn(*mut c_void, *mut UINT) -> HRESULT,
    put_length: unsafe extern "system" fn(*mut c_void, UINT) -> HRESULT,
}

const FY_IID_IBUFFER_BYTE_ACCESS: GUID = GUID {
    Data1: 0x905A_0FEF,
    Data2: 0xBC53,
    Data3: 0x11DF,
    Data4: [0x8C, 0x49, 0x00, 0x1E, 0x4F, 0xC6, 0x86, 0xDA],
};

#[repr(C)]
struct FyIBufferByteAccessVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    buffer: unsafe extern "system" fn(*mut c_void, *mut *mut u8) -> HRESULT,
}

#[repr(C)]
struct FyIOcrResultVtbl {
    base: FyIInspectableVtbl,
    get_lines: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    get_text_angle: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    get_text: unsafe extern "system" fn(*mut c_void, *mut FyHString) -> HRESULT,
}

#[repr(C)]
struct FyIVectorViewVtbl {
    base: FyIInspectableVtbl,
    get_at: unsafe extern "system" fn(*mut c_void, UINT, *mut *mut c_void) -> HRESULT,
    get_size: unsafe extern "system" fn(*mut c_void, *mut UINT) -> HRESULT,
}

#[repr(C)]
struct FyIOcrLineVtbl {
    base: FyIInspectableVtbl,
    get_words: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    get_text: unsafe extern "system" fn(*mut c_void, *mut FyHString) -> HRESULT,
}

#[repr(C)]
struct FyIOcrWordVtbl {
    base: FyIInspectableVtbl,
    get_bounding_rect: unsafe extern "system" fn(*mut c_void, *mut FyWinRect) -> HRESULT,
    get_text: unsafe extern "system" fn(*mut c_void, *mut FyHString) -> HRESULT,
}

#[inline]
fn succeeded(hr: HRESULT) -> bool {
    hr >= 0
}

/// 轻量 COM 智能指针，确保任何错误或早退路径都不泄漏引用计数
struct ComPtr(*mut c_void);

impl ComPtr {
    fn from_raw(ptr: *mut c_void) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self(ptr))
        }
    }

    fn as_ptr(&self) -> *mut c_void {
        self.0
    }

    fn add_ref_clone(&self) -> Self {
        // SAFETY: self.0 为非空 COM 接口指针，首字段是指向 FyIInspectableVtbl 的虚表指针
        unsafe {
            let vtbl = *(self.0 as *const *const FyIInspectableVtbl);
            ((*vtbl).add_ref)(self.0);
        }
        Self(self.0)
    }
}

impl Drop for ComPtr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 为有效 COM 对象指针，Release 释放一次引用
            unsafe {
                let vtbl = *(self.0 as *const *const FyIInspectableVtbl);
                ((*vtbl).release)(self.0);
            }
            self.0 = null_mut();
        }
    }
}

struct HStringGuard {
    hs: FyHString,
    del: PfnWindowsDeleteString,
}

impl Drop for HStringGuard {
    fn drop(&mut self) {
        if !self.hs.is_null() {
            // SAFETY: hs 由 WinRT getter 分配且未释放，匹配传入的 WindowsDeleteString
            unsafe {
                let _ = (self.del)(self.hs);
            }
        }
    }
}

// ---- 语言判断与文字块结构 ----

#[derive(Debug, Clone)]
struct OcrLineBox {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    text: String,
    need_trans: bool,
}

#[derive(Debug, Clone)]
struct TextBlock {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    line_h: i32,
    n_lines: i32,
    skip_translate: bool,
    en: String,
    zh: String,
}

fn is_cjk_cp(cp: u32) -> bool {
    (0x4E00..=0x9FFF).contains(&cp)
        || (0x3400..=0x4DBF).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0x20000..=0x2A6DF).contains(&cp)
}

fn is_cjk_or_fullwidth_w(c: u16) -> bool {
    (0x4E00..=0x9FFF).contains(&c)
        || (0x3400..=0x4DBF).contains(&c)
        || (0xF900..=0xFAFF).contains(&c)
        || (0x3000..=0x303F).contains(&c)
        || (0xFF00..=0xFFEF).contains(&c)
}

/// 判断文本块是否需要英译中（纯中文/纯数字/单字母跳过）
pub fn should_translate_block(utf8: &str) -> bool {
    if utf8.is_empty() {
        return false;
    }
    let mut cjk_cnt = 0usize;
    let mut latin_cnt = 0usize;
    for ch in utf8.chars() {
        let cp = ch as u32;
        if (b'A' as u32..=b'Z' as u32).contains(&cp)
            || (b'a' as u32..=b'z' as u32).contains(&cp)
            || (0x00C0..=0x024F).contains(&cp)
        {
            latin_cnt += 1;
        } else if is_cjk_cp(cp) {
            cjk_cnt += 1;
        }
    }
    if latin_cnt < 2 {
        return false;
    }
    if cjk_cnt > 0 && cjk_cnt * 2 >= latin_cnt {
        return false;
    }
    true
}

fn has_any_cjk(utf8: &str) -> bool {
    utf8.chars().any(|ch| is_cjk_cp(ch as u32))
}

fn cmp_line_box(a: &OcrLineBox, b: &OcrLineBox) -> std::cmp::Ordering {
    let min_h = a.h.min(b.h);
    let y_diff = a.y - b.y;
    if y_diff.abs() <= min_h * 0.45 {
        a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
    } else if a.y < b.y {
        std::cmp::Ordering::Less
    } else {
        std::cmp::Ordering::Greater
    }
}

/// 对 32-bit BGRA 位图做 2x 放大（小图提升 OCR 识别率）
fn upscale_bgra_2x(src: &[u8], w: i32, h: i32) -> Option<(Vec<u8>, i32, i32)> {
    let nw = w * 2;
    let nh = h * 2;
    // SAFETY: GetDC(NULL) 获取屏幕兼容 DC 上下文，全程确保配对释放 GDI 对象
    unsafe {
        let hdc_scr = GetDC(null_mut());
        let hdc_s = CreateCompatibleDC(hdc_scr);
        let hdc_d = CreateCompatibleDC(hdc_scr);

        let bmi_s = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as DWORD,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut bmi_d = bmi_s;
        bmi_d.bmiHeader.biWidth = nw;
        bmi_d.bmiHeader.biHeight = -nh;

        let mut bits_s: *mut c_void = null_mut();
        let mut bits_d: *mut c_void = null_mut();
        let hbm_s = CreateDIBSection(hdc_s, &bmi_s, DIB_RGB_COLORS, &mut bits_s, null_mut(), 0);
        let hbm_d = CreateDIBSection(hdc_d, &bmi_d, DIB_RGB_COLORS, &mut bits_d, null_mut(), 0);

        if hbm_s.is_null() || hbm_d.is_null() || bits_s.is_null() || bits_d.is_null() {
            if !hbm_s.is_null() {
                let _ = DeleteObject(hbm_s);
            }
            if !hbm_d.is_null() {
                let _ = DeleteObject(hbm_d);
            }
            if !hdc_s.is_null() {
                let _ = DeleteDC(hdc_s);
            }
            if !hdc_d.is_null() {
                let _ = DeleteDC(hdc_d);
            }
            let _ = ReleaseDC(null_mut(), hdc_scr);
            return None;
        }

        let src_bytes = (w as usize) * (h as usize) * 4;
        std::ptr::copy_nonoverlapping(src.as_ptr(), bits_s.cast(), src_bytes);

        let old_s = SelectObject(hdc_s, hbm_s);
        let old_d = SelectObject(hdc_d, hbm_d);
        let _ = SetStretchBltMode(hdc_d, HALFTONE);
        let _ = SetBrushOrgEx(hdc_d, 0, 0, null_mut());
        let _ = StretchBlt(hdc_d, 0, 0, nw, nh, hdc_s, 0, 0, w, h, SRCCOPY);
        let _ = GdiFlush();

        let dst_bytes = (nw as usize) * (nh as usize) * 4;
        let mut out = vec![0u8; dst_bytes];
        std::ptr::copy_nonoverlapping(bits_d.cast::<u8>(), out.as_mut_ptr(), dst_bytes);

        let _ = SelectObject(hdc_s, old_s);
        let _ = SelectObject(hdc_d, old_d);
        let _ = DeleteObject(hbm_s);
        let _ = DeleteObject(hbm_d);
        let _ = DeleteDC(hdc_s);
        let _ = DeleteDC(hdc_d);
        let _ = ReleaseDC(null_mut(), hdc_scr);
        Some((out, nw, nh))
    }
}

// ---- 全局缓存的 WinRT OCR 上下文 ----

struct OcrCtx {
    ready: bool,
    combase: HMODULE,
    p_ro_init: Option<PfnRoInitialize>,
    p_ro_uninit: Option<PfnRoUninitialize>,
    p_ro_factory: Option<PfnRoGetActivationFactory>,
    p_win_str_ref: Option<PfnWindowsCreateStringReference>,
    p_win_str_buf: Option<PfnWindowsGetStringRawBuffer>,
    p_win_str_del: Option<PfnWindowsDeleteString>,
    ocr_engine: Option<ComPtr>,
    zh_engine: Option<ComPtr>,
    buf_factory: Option<ComPtr>,
    sb_statics: Option<ComPtr>,
}

// SAFETY: WinRT MTA 对象与 combase 函数指针在进程 MTA 生命周期内跨线程安全调用
unsafe impl Send for OcrCtx {}

static OCR_CTX: LazyLock<Mutex<OcrCtx>> = LazyLock::new(|| {
    Mutex::new(OcrCtx {
        ready: false,
        combase: null_mut(),
        p_ro_init: None,
        p_ro_uninit: None,
        p_ro_factory: None,
        p_win_str_ref: None,
        p_win_str_buf: None,
        p_win_str_del: None,
        ocr_engine: None,
        zh_engine: None,
        buf_factory: None,
        sb_statics: None,
    })
});

impl OcrCtx {
    fn reset_locked(&mut self) {
        self.sb_statics = None;
        self.buf_factory = None;
        self.zh_engine = None;
        self.ocr_engine = None;
        self.ready = false;
    }
}

fn load_proc<T>(module: HMODULE, name: &str) -> Option<T> {
    let cname = to_c_bytes(name);
    // SAFETY: module 为有效已加载 DLL 句柄，cname 以 '\0' 结尾
    let ptr = unsafe { GetProcAddress(module, cname.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        // SAFETY: 调用方传入与 Win32 官方函数原型完全匹配的函数指针类型 T
        Some(unsafe { std::mem::transmute_copy::<*const c_void, T>(&ptr) })
    }
}

fn make_hstring_ref(
    p_ref: PfnWindowsCreateStringReference,
    text: &str,
    header: &mut FyHStringHeader,
) -> Option<(Vec<u16>, FyHString)> {
    let wide = utf8_to_wide(text);
    let wlen = wide.len().saturating_sub(1) as UINT;
    let mut hs: FyHString = null_mut();
    // SAFETY: wide 以 '\0' 结尾且在返回值持有期间保留，header 占满所需空间
    let hr = unsafe { p_ref(wide.as_ptr(), wlen, header, &mut hs) };
    if succeeded(hr) && !hs.is_null() {
        Some((wide, hs))
    } else {
        None
    }
}

fn try_create_ocr_lang(
    ocr_statics: &ComPtr,
    lang_factory: &ComPtr,
    p_ref: PfnWindowsCreateStringReference,
    tag: &str,
) -> Option<ComPtr> {
    let mut hh = FyHStringHeader::default();
    let (_w, hs) = make_hstring_ref(p_ref, tag, &mut hh)?;
    let mut lang_raw: *mut c_void = null_mut();
    // SAFETY: lang_factory 为有效 ILanguageFactory，hs 为绑定到 _w 的有效 HSTRING
    let hr_lang = unsafe {
        let vtbl = *(lang_factory.as_ptr() as *const *const FyILanguageFactoryVtbl);
        ((*vtbl).create_language)(lang_factory.as_ptr(), hs, &mut lang_raw)
    };
    let lang = if succeeded(hr_lang) {
        ComPtr::from_raw(lang_raw)
    } else {
        None
    }?;

    let mut eng_raw: *mut c_void = null_mut();
    // SAFETY: ocr_statics 为有效 IOcrEngineStatics，lang 为有效 Language 对象
    let hr_eng = unsafe {
        let vtbl = *(ocr_statics.as_ptr() as *const *const FyIOcrEngineStaticsVtbl);
        ((*vtbl).try_create_from_language)(ocr_statics.as_ptr(), lang.as_ptr(), &mut eng_raw)
    };
    if succeeded(hr_eng) {
        ComPtr::from_raw(eng_raw)
    } else {
        None
    }
}

fn init_ocr_ctx_locked(ctx: &mut OcrCtx) -> Result<(), String> {
    if ctx.ready {
        return Ok(());
    }
    if ctx.combase.is_null() {
        let cname = b"combase.dll\0";
        // SAFETY: 加载系统 combase.dll
        let mod_h = unsafe { LoadLibraryA(cname.as_ptr()) };
        if mod_h.is_null() {
            return Err("当前系统不支持 Windows OCR（缺少 combase.dll）".to_string());
        }
        ctx.combase = mod_h;
        ctx.p_ro_init = load_proc(mod_h, "RoInitialize");
        ctx.p_ro_uninit = load_proc(mod_h, "RoUninitialize");
        ctx.p_ro_factory = load_proc(mod_h, "RoGetActivationFactory");
        ctx.p_win_str_ref = load_proc(mod_h, "WindowsCreateStringReference");
        ctx.p_win_str_buf = load_proc(mod_h, "WindowsGetStringRawBuffer");
        ctx.p_win_str_del = load_proc(mod_h, "WindowsDeleteString");
    }

    let (Some(p_ro_init), Some(p_ro_factory), Some(p_ref), Some(_), Some(_)) = (
        ctx.p_ro_init,
        ctx.p_ro_factory,
        ctx.p_win_str_ref,
        ctx.p_win_str_buf,
        ctx.p_win_str_del,
    ) else {
        return Err("当前系统版本过低，不支持 Windows OCR（需 Win10/11）".to_string());
    };

    // 保留一份 MTA 引用不 Uninit，确保缓存的 WinRT 对象不因计数归零被拆掉
    // SAFETY: 1 = RO_INIT_MULTITHREADED
    unsafe {
        let _ = p_ro_init(1);
    }

    let get_factory = |class_name: &str, iid: &GUID| -> Option<ComPtr> {
        let mut hh = FyHStringHeader::default();
        let (_w, hs) = make_hstring_ref(p_ref, class_name, &mut hh)?;
        let mut ptr: *mut c_void = null_mut();
        // SAFETY: hs 有效，iid 为对应接口 GUID
        let hr = unsafe { p_ro_factory(hs, iid, &mut ptr) };
        if succeeded(hr) {
            ComPtr::from_raw(ptr)
        } else {
            None
        }
    };

    let Some(ocr_statics) = get_factory("Windows.Media.Ocr.OcrEngine", &FY_IID_IOCR_ENGINE_STATICS)
    else {
        ctx.reset_locked();
        return Err("无法初始化 Windows OCR 引擎".to_string());
    };

    let mut main_lang = "user-profile";
    let mut zh_lang = "none";

    if let Some(lang_factory) =
        get_factory("Windows.Globalization.Language", &FY_IID_ILANGUAGE_FACTORY)
    {
        if let Some(en_eng) = try_create_ocr_lang(&ocr_statics, &lang_factory, p_ref, "en-US") {
            ctx.ocr_engine = Some(en_eng);
            main_lang = "en-US";
        }
        let mut zh_eng = try_create_ocr_lang(&ocr_statics, &lang_factory, p_ref, "zh-Hans-CN");
        if zh_eng.is_some() {
            zh_lang = "zh-Hans-CN";
        } else {
            zh_eng = try_create_ocr_lang(&ocr_statics, &lang_factory, p_ref, "zh-CN");
            if zh_eng.is_some() {
                zh_lang = "zh-CN";
            }
        }
        if ctx.ocr_engine.is_none() && zh_eng.is_some() {
            ctx.ocr_engine = zh_eng;
            main_lang = zh_lang;
            zh_lang = "same-as-main";
        } else {
            ctx.zh_engine = zh_eng;
        }
    }

    if ctx.ocr_engine.is_none() {
        let mut eng_raw: *mut c_void = null_mut();
        // SAFETY: ocr_statics 为有效 IOcrEngineStatics
        let hr = unsafe {
            let vtbl = *(ocr_statics.as_ptr() as *const *const FyIOcrEngineStaticsVtbl);
            ((*vtbl).try_create_from_user_profile_languages)(ocr_statics.as_ptr(), &mut eng_raw)
        };
        if succeeded(hr) {
            ctx.ocr_engine = ComPtr::from_raw(eng_raw);
        }
    }

    if ctx.ocr_engine.is_none() {
        ctx.reset_locked();
        return Err("系统未安装 OCR 语言包，请在 Windows 设置中安装中文或英语识别".to_string());
    }
    log_write(
        "OCR",
        &format!("已初始化 Windows OCR 引擎 (main={main_lang}, zh_mask={zh_lang})"),
    );

    let Some(buf_factory) = get_factory("Windows.Storage.Streams.Buffer", &FY_IID_IBUFFER_FACTORY)
    else {
        ctx.reset_locked();
        return Err("无法创建 OCR 图像缓冲".to_string());
    };
    let Some(sb_statics) = get_factory(
        "Windows.Graphics.Imaging.SoftwareBitmap",
        &FY_IID_ISOFTWARE_BITMAP_STATICS,
    ) else {
        ctx.reset_locked();
        return Err("无法初始化 SoftwareBitmap".to_string());
    };

    ctx.buf_factory = Some(buf_factory);
    ctx.sb_statics = Some(sb_statics);
    ctx.ready = true;
    Ok(())
}

fn hstring_to_trimmed_utf16(
    hs: FyHString,
    p_buf: PfnWindowsGetStringRawBuffer,
) -> Option<Vec<u16>> {
    if hs.is_null() {
        return None;
    }
    let mut len: UINT = 0;
    // SAFETY: hs 为有效 WinRT HSTRING
    let ptr = unsafe { p_buf(hs, &mut len) };
    if ptr.is_null() || len == 0 {
        return None;
    }
    // SAFETY: ptr 指向长度为 len 的有效只读 UTF-16 切片
    let raw = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
    let mut s = 0usize;
    let mut e = raw.len();
    while s < e && (raw[s] == b' ' as u16 || raw[s] == b'\t' as u16) {
        s += 1;
    }
    while e > s && (raw[e - 1] == b' ' as u16 || raw[e - 1] == b'\t' as u16) {
        e -= 1;
    }
    if s >= e {
        None
    } else {
        Some(raw[s..e].to_vec())
    }
}

fn recognize_lines_with_engine(
    engine: &ComPtr,
    sb: &ComPtr,
    inv_scale: f32,
    p_buf: PfnWindowsGetStringRawBuffer,
    p_del: PfnWindowsDeleteString,
) -> Result<Vec<OcrLineBox>, String> {
    let mut async_op_raw: *mut c_void = null_mut();
    // SAFETY: engine 为有效 IOcrEngine，sb 为有效 SoftwareBitmap
    let hr_rec = unsafe {
        let vtbl = *(engine.as_ptr() as *const *const FyIOcrEngineVtbl);
        ((*vtbl).recognize_async)(engine.as_ptr(), sb.as_ptr(), &mut async_op_raw)
    };
    let async_op = ComPtr::from_raw(async_op_raw)
        .filter(|_| succeeded(hr_rec))
        .ok_or_else(|| "OCR 识别启动失败".to_string())?;

    let mut async_info_raw: *mut c_void = null_mut();
    // SAFETY: async_op 为有效 IAsyncOperation，查询 IAsyncInfo 接口
    let hr_qi = unsafe {
        let vtbl = *(async_op.as_ptr() as *const *const FyIInspectableVtbl);
        ((*vtbl).query_interface)(async_op.as_ptr(), &FY_IID_IASYNC_INFO, &mut async_info_raw)
    };
    let async_info = ComPtr::from_raw(async_info_raw)
        .filter(|_| succeeded(hr_qi))
        .ok_or_else(|| "OCR 状态查询失败".to_string())?;

    let mut status: i32 = 0;
    for tries in 0..2000 {
        // SAFETY: async_info 为有效 IAsyncInfo
        unsafe {
            let vtbl = *(async_info.as_ptr() as *const *const FyIAsyncInfoVtbl);
            let _ = ((*vtbl).get_status)(async_info.as_ptr(), &mut status);
        }
        if status != 0 {
            break;
        }
        thread::sleep(Duration::from_millis(if tries < 20 { 1 } else { 5 }));
    }
    if status != 1 {
        return Err("OCR 识别超时或失败".to_string());
    }

    let mut ocr_res_raw: *mut c_void = null_mut();
    // SAFETY: async_op 已完成(status == 1)，调用 GetResults 取识别结果
    let hr_res = unsafe {
        let vtbl = *(async_op.as_ptr() as *const *const FyIAsyncOperationVtbl);
        ((*vtbl).get_results)(async_op.as_ptr(), &mut ocr_res_raw)
    };
    let ocr_res = ComPtr::from_raw(ocr_res_raw)
        .filter(|_| succeeded(hr_res))
        .ok_or_else(|| "获取 OCR 结果失败".to_string())?;

    let mut lines_raw: *mut c_void = null_mut();
    // SAFETY: ocr_res 为有效 IOcrResult
    let hr_lines = unsafe {
        let vtbl = *(ocr_res.as_ptr() as *const *const FyIOcrResultVtbl);
        ((*vtbl).get_lines)(ocr_res.as_ptr(), &mut lines_raw)
    };
    let lines_vec = ComPtr::from_raw(lines_raw)
        .filter(|_| succeeded(hr_lines))
        .ok_or_else(|| "读取 OCR 行列表失败".to_string())?;

    let mut n_lines: UINT = 0;
    // SAFETY: lines_vec 为有效 IVectorView<OcrLine>
    unsafe {
        let vtbl = *(lines_vec.as_ptr() as *const *const FyIVectorViewVtbl);
        let _ = ((*vtbl).get_size)(lines_vec.as_ptr(), &mut n_lines);
    }

    let mut out = Vec::with_capacity(n_lines as usize);
    for i in 0..n_lines {
        let mut line_raw: *mut c_void = null_mut();
        // SAFETY: 按合法索引 i < n_lines 读取 OcrLine
        let hr_line = unsafe {
            let vtbl = *(lines_vec.as_ptr() as *const *const FyIVectorViewVtbl);
            ((*vtbl).get_at)(lines_vec.as_ptr(), i, &mut line_raw)
        };
        let Some(line) = ComPtr::from_raw(line_raw).filter(|_| succeeded(hr_line)) else {
            continue;
        };

        let mut min_x = 1e9f32;
        let mut min_y = 1e9f32;
        let mut max_x = -1e9f32;
        let mut max_y = -1e9f32;
        let mut wbuf: Vec<u16> = Vec::new();
        let mut prev_last: u16 = 0;
        let mut prev_right: f32 = 0.0;
        let mut prev_h: f32 = 0.0;
        let mut has_prev = false;

        let mut words_raw: *mut c_void = null_mut();
        // SAFETY: line 为有效 IOcrLine
        let hr_words = unsafe {
            let vtbl = *(line.as_ptr() as *const *const FyIOcrLineVtbl);
            ((*vtbl).get_words)(line.as_ptr(), &mut words_raw)
        };
        if let Some(words_vec) = ComPtr::from_raw(words_raw).filter(|_| succeeded(hr_words)) {
            let mut n_words: UINT = 0;
            // SAFETY: words_vec 为有效 IVectorView<OcrWord>
            unsafe {
                let vtbl = *(words_vec.as_ptr() as *const *const FyIVectorViewVtbl);
                let _ = ((*vtbl).get_size)(words_vec.as_ptr(), &mut n_words);
            }
            for j in 0..n_words {
                let mut word_raw: *mut c_void = null_mut();
                // SAFETY: 按合法索引 j < n_words 读取 OcrWord
                let hr_w = unsafe {
                    let vtbl = *(words_vec.as_ptr() as *const *const FyIVectorViewVtbl);
                    ((*vtbl).get_at)(words_vec.as_ptr(), j, &mut word_raw)
                };
                let Some(word) = ComPtr::from_raw(word_raw).filter(|_| succeeded(hr_w)) else {
                    continue;
                };
                let mut wr = FyWinRect::default();
                // SAFETY: word 为有效 IOcrWord
                let hr_rc = unsafe {
                    let vtbl = *(word.as_ptr() as *const *const FyIOcrWordVtbl);
                    ((*vtbl).get_bounding_rect)(word.as_ptr(), &mut wr)
                };
                let got_rc = succeeded(hr_rc);
                if got_rc {
                    min_x = min_x.min(wr.x);
                    min_y = min_y.min(wr.y);
                    max_x = max_x.max(wr.x + wr.width);
                    max_y = max_y.max(wr.y + wr.height);
                }

                let mut wt: FyHString = null_mut();
                // SAFETY: word 为有效 IOcrWord
                let hr_wt = unsafe {
                    let vtbl = *(word.as_ptr() as *const *const FyIOcrWordVtbl);
                    ((*vtbl).get_text)(word.as_ptr(), &mut wt)
                };
                let _wt_guard = HStringGuard { hs: wt, del: p_del };
                if succeeded(hr_wt) {
                    if let Some(wts) = hstring_to_trimmed_utf16(wt, p_buf) {
                        let mut need_sp = false;
                        if has_prev {
                            let cur_first = wts[0];
                            let prev_cjk = is_cjk_or_fullwidth_w(prev_last);
                            let cur_cjk = is_cjk_or_fullwidth_w(cur_first);
                            let gap = if got_rc { wr.x - prev_right } else { 0.0 };
                            let ref_h = if prev_h > 1.0 {
                                prev_h
                            } else if got_rc {
                                wr.height
                            } else {
                                14.0
                            };
                            if !prev_cjk && !cur_cjk {
                                let tight_cur = matches!(
                                    cur_first,
                                    0x2C | 0x2E
                                        | 0x3B
                                        | 0x3A
                                        | 0x21
                                        | 0x3F
                                        | 0x29
                                        | 0x5D
                                        | 0x7D
                                        | 0x27
                                ); // , . ; : ! ? ) ] } '
                                let tight_prev =
                                    matches!(prev_last, 0x28 | 0x5B | 0x7B | 0x2F | 0x2D | 0x27); // ( [ { / - '
                                if !got_rc {
                                    if !tight_cur && !tight_prev {
                                        need_sp = true;
                                    }
                                } else if gap >= ref_h * 0.14
                                    && !((tight_cur || tight_prev) && gap < ref_h * 0.25)
                                {
                                    need_sp = true;
                                }
                            } else if prev_cjk && cur_cjk {
                                if gap > ref_h * 0.70 {
                                    need_sp = true;
                                }
                            } else if gap > ref_h * 0.25 {
                                need_sp = true;
                            }
                        }
                        if need_sp {
                            wbuf.push(b' ' as u16);
                        }
                        wbuf.extend_from_slice(&wts);
                        prev_last = *wts.last().unwrap_or(&0);
                        if got_rc {
                            prev_right = wr.x + wr.width;
                            prev_h = wr.height;
                        }
                        has_prev = true;
                    }
                }
            }
        }

        let utf8 = if !wbuf.is_empty() {
            wide_to_utf8(&wbuf)
        } else {
            let mut lt: FyHString = null_mut();
            // SAFETY: line 为有效 IOcrLine
            unsafe {
                let vtbl = *(line.as_ptr() as *const *const FyIOcrLineVtbl);
                let _ = ((*vtbl).get_text)(line.as_ptr(), &mut lt);
            }
            let _lt_guard = HStringGuard { hs: lt, del: p_del };
            hstring_to_trimmed_utf16(lt, p_buf)
                .map(|v| wide_to_utf8(&v))
                .unwrap_or_default()
        };

        if !utf8.is_empty() && max_x > min_x && max_y > min_y {
            let need_trans = should_translate_block(&utf8);
            out.push(OcrLineBox {
                x: min_x * inv_scale,
                y: min_y * inv_scale,
                w: (max_x - min_x) * inv_scale,
                h: (max_y - min_y) * inv_scale,
                text: utf8,
                need_trans,
            });
        }
    }

    Ok(out)
}

fn box_overlaps_zh(a: &OcrLineBox, b: &OcrLineBox) -> bool {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.w).min(b.x + b.w);
    let y2 = (a.y + a.h).min(b.y + b.h);
    if x2 <= x1 || y2 <= y1 {
        return false;
    }
    let inter_w = x2 - x1;
    let inter_h = y2 - y1;
    let min_h = a.h.min(b.h);
    if min_h <= 0.0 || inter_h < min_h * 0.45 {
        return false;
    }
    a.w > 0.0 && inter_w >= a.w * 0.35
}

fn run_winrt_ocr(bgra: &[u8], w: i32, h: i32) -> Result<Vec<OcrLineBox>, String> {
    let (ocr_engine, zh_engine, buf_factory, sb_statics, p_ro_init, p_ro_uninit, p_buf, p_del) = {
        let mut ctx = OCR_CTX.lock().unwrap_or_else(|e| e.into_inner());
        init_ocr_ctx_locked(&mut ctx)?;
        (
            ctx.ocr_engine
                .as_ref()
                .map(ComPtr::add_ref_clone)
                .ok_or_else(|| "OCR 引擎未就绪".to_string())?,
            ctx.zh_engine.as_ref().map(ComPtr::add_ref_clone),
            ctx.buf_factory
                .as_ref()
                .map(ComPtr::add_ref_clone)
                .ok_or_else(|| "BufferFactory 未就绪".to_string())?,
            ctx.sb_statics
                .as_ref()
                .map(ComPtr::add_ref_clone)
                .ok_or_else(|| "SoftwareBitmapStatics 未就绪".to_string())?,
            ctx.p_ro_init
                .ok_or_else(|| "RoInitialize 缺失".to_string())?,
            ctx.p_ro_uninit,
            ctx.p_win_str_buf
                .ok_or_else(|| "WindowsGetStringRawBuffer 缺失".to_string())?,
            ctx.p_win_str_del
                .ok_or_else(|| "WindowsDeleteString 缺失".to_string())?,
        )
    };

    // SAFETY: 1 = RO_INIT_MULTITHREADED
    let hr_init = unsafe { p_ro_init(1) };
    let need_uninit = hr_init == 0 || hr_init == 1; // S_OK || S_FALSE
    struct RoGuard(bool, Option<PfnRoUninitialize>);
    impl Drop for RoGuard {
        fn drop(&mut self) {
            if self.0 {
                if let Some(f) = self.1 {
                    // SAFETY: 与前面成功的 RoInitialize 配对调用
                    unsafe { f() };
                }
            }
        }
    }
    let _ro_guard = RoGuard(need_uninit, p_ro_uninit);

    let mut ocr_w = w;
    let mut ocr_h = h;
    let mut inv_scale = 1.0f32;
    let scaled_storage: Option<Vec<u8>>;
    let ocr_src: &[u8] = if (w < 400 || h < 120) && w * 2 <= 2400 && h * 2 <= 2400 {
        if let Some((buf2x, nw, nh)) = upscale_bgra_2x(bgra, w, h) {
            ocr_w = nw;
            ocr_h = nh;
            inv_scale = 0.5;
            scaled_storage = Some(buf2x);
            scaled_storage.as_deref().unwrap_or(bgra)
        } else {
            bgra
        }
    } else {
        bgra
    };

    let byte_sz = (ocr_w as UINT) * (ocr_h as UINT) * 4;
    let mut buf_raw: *mut c_void = null_mut();
    // SAFETY: buf_factory 为有效 IBufferFactory
    let hr_buf = unsafe {
        let vtbl = *(buf_factory.as_ptr() as *const *const FyIBufferFactoryVtbl);
        ((*vtbl).create)(buf_factory.as_ptr(), byte_sz, &mut buf_raw)
    };
    let buf = ComPtr::from_raw(buf_raw)
        .filter(|_| succeeded(hr_buf))
        .ok_or_else(|| "分配 OCR 缓冲失败".to_string())?;

    // SAFETY: buf 为有效 IBuffer，设置长度并查询 IBufferByteAccess 写入像素
    unsafe {
        let vtbl = *(buf.as_ptr() as *const *const FyIBufferVtbl);
        let _ = ((*vtbl).put_length)(buf.as_ptr(), byte_sz);
    }

    let mut byte_acc_raw: *mut c_void = null_mut();
    // SAFETY: 查询 IBufferByteAccess 接口
    let hr_acc = unsafe {
        let vtbl = *(buf.as_ptr() as *const *const FyIInspectableVtbl);
        ((*vtbl).query_interface)(buf.as_ptr(), &FY_IID_IBUFFER_BYTE_ACCESS, &mut byte_acc_raw)
    };
    let byte_acc = ComPtr::from_raw(byte_acc_raw)
        .filter(|_| succeeded(hr_acc))
        .ok_or_else(|| "访问 OCR 缓冲失败".to_string())?;

    let mut dst_ptr: *mut u8 = null_mut();
    // SAFETY: byte_acc 为有效 IBufferByteAccess，取得内部 byte_sz 字节可写缓冲后拷贝 BGRA 像素
    unsafe {
        let vtbl = *(byte_acc.as_ptr() as *const *const FyIBufferByteAccessVtbl);
        let _ = ((*vtbl).buffer)(byte_acc.as_ptr(), &mut dst_ptr);
        if dst_ptr.is_null() {
            return Err("获取 OCR 缓冲指针失败".to_string());
        }
        std::ptr::copy_nonoverlapping(ocr_src.as_ptr(), dst_ptr, byte_sz as usize);
    }

    let mut sb_raw: *mut c_void = null_mut();
    // SAFETY: sb_statics 为有效 ISoftwareBitmapStatics，87 = BitmapPixelFormat::Bgra8
    let hr_sb = unsafe {
        let vtbl = *(sb_statics.as_ptr() as *const *const FyISoftwareBitmapStaticsVtbl);
        ((*vtbl).create_copy_from_buffer)(
            sb_statics.as_ptr(),
            buf.as_ptr(),
            87,
            ocr_w,
            ocr_h,
            &mut sb_raw,
        )
    };
    let sb = ComPtr::from_raw(sb_raw)
        .filter(|_| succeeded(hr_sb))
        .ok_or_else(|| "构造 OCR 图像失败".to_string())?;

    let mut lines = recognize_lines_with_engine(&ocr_engine, &sb, inv_scale, p_buf, p_del)?;

    if let Some(ref zh_eng) = zh_engine {
        if let Ok(zh_lines) = recognize_lines_with_engine(zh_eng, &sb, inv_scale, p_buf, p_del) {
            for ln in &mut lines {
                if !ln.need_trans {
                    continue;
                }
                for zh in &zh_lines {
                    if !zh.need_trans && has_any_cjk(&zh.text) && box_overlaps_zh(ln, zh) {
                        ln.need_trans = false;
                        break;
                    }
                }
            }
            for mut zh in zh_lines {
                if !zh.need_trans && has_any_cjk(&zh.text) {
                    zh.need_trans = false;
                    lines.push(zh);
                }
            }
        }
    }

    if lines.len() > 1 {
        lines.sort_by(cmp_line_box);
    }
    Ok(lines)
}

// ---- 段落聚合与译文清理 ----

fn should_merge_lines(blk: &TextBlock, ln: &OcrLineBox) -> bool {
    if blk.skip_translate != (!ln.need_trans) {
        return false;
    }
    let prev_bottom = blk.y + blk.h;
    let gap = (ln.y as i32) - prev_bottom;
    let ln_h = ln.h as i32;
    let ref_h = blk.line_h.max(ln_h);
    let min_h = blk.line_h.min(ln_h);
    if ref_h <= 0 || min_h <= 0 {
        return false;
    }
    if gap > ((ref_h as f32) * 0.65) as i32 || gap < -(((min_h as f32) * 0.3) as i32) {
        return false;
    }
    if (min_h as f32) / (ref_h as f32) < 0.68 {
        return false;
    }
    let left_diff = ((ln.x as i32) - blk.x).abs();
    left_diff <= ((ref_h as f32) * 1.4) as i32
}

fn group_lines(lines: &[OcrLineBox]) -> Vec<TextBlock> {
    let mut blks: Vec<TextBlock> = Vec::with_capacity(lines.len());
    for ln in lines {
        if let Some(last) = blks.last_mut() {
            if should_merge_lines(last, ln) {
                let nx1 = last.x.min(ln.x as i32);
                let ny1 = last.y.min(ln.y as i32);
                let nx2 = (last.x + last.w).max((ln.x + ln.w) as i32);
                let ny2 = (last.y + last.h).max((ln.y + ln.h) as i32);
                last.x = nx1;
                last.y = ny1;
                last.w = nx2 - nx1;
                last.h = ny2 - ny1;
                last.line_h = (last.line_h * last.n_lines + (ln.h as i32)) / (last.n_lines + 1);
                last.n_lines += 1;
                last.en.push(' ');
                last.en.push_str(&ln.text);
                last.skip_translate = !should_translate_block(&last.en);
                continue;
            }
        }
        let h_px = ln.h.ceil() as i32;
        blks.push(TextBlock {
            x: ln.x.floor() as i32,
            y: ln.y.floor() as i32,
            w: ln.w.ceil() as i32,
            h: h_px,
            line_h: if h_px > 0 { h_px } else { 14 },
            n_lines: 1,
            skip_translate: !ln.need_trans,
            en: ln.text.clone(),
            zh: String::new(),
        });
    }
    blks
}

/// 清理译文中的词典音标/拼音附加行及回退提示，适合直接覆盖到截图原图上
pub fn clean_zh_for_overlay(raw: &str, single_line: bool) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut p = raw;
    if p.starts_with('（') {
        if let Some(nl) = p.find('\n') {
            if p[..nl].contains("已用必应翻译") {
                p = &p[nl + 1..];
            }
        }
    }

    let mut out_lines = Vec::new();
    for raw_line in p.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty()
            || line.starts_with("拼音：")
            || line.starts_with("美[")
            || line.starts_with("英[")
            || line.starts_with('[')
        {
            continue;
        }
        out_lines.push(line);
        if single_line {
            break;
        }
    }
    if out_lines.is_empty() {
        p.to_string()
    } else {
        out_lines.join("\n")
    }
}

// ---- 本地短语翻译记忆库（Safe Rust O(1) 索引 LRU + 人工校对锁定） ----

const OCR_CACHE_MAX: usize = 8000;
const OCR_CACHE_HEADER: &str =
    "# 快译NG 截屏翻译本地记忆库（UTF-8，Tab 分隔：原文<Tab>译文；开头加 *<Tab> 表示人工校对永久保留）\r\n";

struct CacheNode {
    key: String,
    raw_en: String,
    zh: String,
    locked: bool,
    prev: Option<usize>,
    next: Option<usize>,
}

struct OcrCache {
    inited: bool,
    dir: PathBuf,
    file_path: PathBuf,
    mtime: Option<SystemTime>,
    auto_count: usize,
    nodes: Vec<Option<CacheNode>>,
    free_slots: Vec<usize>,
    map: HashMap<String, usize>,
    head: Option<usize>,
    tail: Option<usize>,
}

static OCR_CACHE: LazyLock<Mutex<OcrCache>> = LazyLock::new(|| {
    Mutex::new(OcrCache {
        inited: false,
        dir: PathBuf::new(),
        file_path: PathBuf::new(),
        mtime: None,
        auto_count: 0,
        nodes: Vec::new(),
        free_slots: Vec::new(),
        map: HashMap::new(),
        head: None,
        tail: None,
    })
});

fn normalize_cache_key(src: &str) -> String {
    let mut out = String::with_capacity(src.len().min(128));
    let mut prev_sp = true;
    for ch in src.chars() {
        if matches!(ch, ' ' | '\t' | '\r' | '\n') {
            if !prev_sp && out.len() < 127 {
                out.push(' ');
                prev_sp = true;
            }
        } else {
            let lower = if ch.is_ascii_uppercase() {
                ch.to_ascii_lowercase()
            } else {
                ch
            };
            if out.len() + lower.len_utf8() >= 128 {
                break;
            }
            out.push(lower);
            prev_sp = false;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

fn can_cache_pair(en: &str, zh: &str) -> bool {
    if en.is_empty() || en.len() > 48 || zh.is_empty() || zh.len() > 120 || en == zh {
        return false;
    }
    let has_alpha = en.bytes().any(|b| b.is_ascii_alphabetic());
    if !has_alpha {
        return false;
    }
    !en.bytes().any(|b| matches!(b, b'\r' | b'\n' | b'\t'))
        && !zh.bytes().any(|b| matches!(b, b'\r' | b'\n' | b'\t'))
}

impl OcrCache {
    fn clear(&mut self) {
        self.nodes.clear();
        self.free_slots.clear();
        self.map.clear();
        self.head = None;
        self.tail = None;
        self.auto_count = 0;
    }

    fn unlink(&mut self, idx: usize) {
        let (prev, next) = match &self.nodes[idx] {
            Some(n) => (n.prev, n.next),
            None => return,
        };
        if let Some(p) = prev {
            if let Some(pn) = &mut self.nodes[p] {
                pn.next = next;
            }
        } else {
            self.head = next;
        }
        if let Some(nx) = next {
            if let Some(nn) = &mut self.nodes[nx] {
                nn.prev = prev;
            }
        } else {
            self.tail = prev;
        }
        if let Some(cur) = &mut self.nodes[idx] {
            cur.prev = None;
            cur.next = None;
        }
    }

    fn push_front(&mut self, idx: usize) {
        let old_head = self.head;
        if let Some(cur) = &mut self.nodes[idx] {
            cur.prev = None;
            cur.next = old_head;
        }
        if let Some(oh) = old_head {
            if let Some(ohn) = &mut self.nodes[oh] {
                ohn.prev = Some(idx);
            }
        }
        self.head = Some(idx);
        if self.tail.is_none() {
            self.tail = Some(idx);
        }
    }

    fn insert_mem(&mut self, raw_en: &str, zh: &str, locked: bool) {
        let key = normalize_cache_key(raw_en);
        if key.is_empty() {
            return;
        }
        if let Some(&idx) = self.map.get(&key) {
            if let Some(ex) = &mut self.nodes[idx] {
                if ex.locked && !locked {
                    return;
                }
                ex.zh = zh.to_string();
                if locked && !ex.locked {
                    ex.locked = true;
                    if self.auto_count > 0 {
                        self.auto_count -= 1;
                    }
                }
            }
            self.unlink(idx);
            self.push_front(idx);
            return;
        }

        let node = CacheNode {
            key: key.clone(),
            raw_en: raw_en.to_string(),
            zh: zh.to_string(),
            locked,
            prev: None,
            next: None,
        };
        let idx = if let Some(slot) = self.free_slots.pop() {
            self.nodes[slot] = Some(node);
            slot
        } else {
            self.nodes.push(Some(node));
            self.nodes.len() - 1
        };
        self.map.insert(key, idx);
        self.push_front(idx);
        if !locked {
            self.auto_count += 1;
        }
    }

    fn update_mtime(&mut self) {
        self.mtime = fs::metadata(&self.file_path)
            .ok()
            .and_then(|m| m.modified().ok());
    }

    fn rewrite_file(&mut self) {
        if self.file_path.as_os_str().is_empty() {
            return;
        }
        let mut out = String::from(OCR_CACHE_HEADER);
        let mut cur = self.head;
        while let Some(idx) = cur {
            let Some(node) = &self.nodes[idx] else {
                break;
            };
            if node.locked {
                out.push_str(&format!("*\t{}\t{}\r\n", node.raw_en, node.zh));
            } else {
                out.push_str(&format!("{}\t{}\r\n", node.raw_en, node.zh));
            }
            cur = node.next;
        }
        if fs::write(&self.file_path, out.as_bytes()).is_ok() {
            self.update_mtime();
        }
    }

    fn load_dir(&mut self, cache_dir: &Path) {
        if cache_dir.as_os_str().is_empty() {
            return;
        }
        let dir_changed = !self.inited || self.dir != cache_dir;
        if dir_changed {
            self.clear();
            self.dir = cache_dir.to_path_buf();
            self.file_path = cache_dir.join("ocr_cache.txt");
            self.inited = true;
            self.mtime = None;
        }

        let Ok(meta) = fs::metadata(&self.file_path) else {
            return;
        };
        let cur_mtime = meta.modified().ok();
        if !dir_changed && self.mtime.is_some() && cur_mtime == self.mtime {
            return;
        }

        self.clear();
        if let Ok(content) = fs::read_to_string(&self.file_path) {
            for raw_line in content.lines() {
                let mut line = raw_line.trim_end_matches('\r');
                if let Some(stripped) = line.strip_prefix('\u{FEFF}') {
                    line = stripped;
                }
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let (locked, rest) = if let Some(after) = line.strip_prefix("*\t") {
                    (true, after)
                } else {
                    (false, line)
                };
                let Some((en, zh)) = rest.split_once('\t') else {
                    continue;
                };
                if en.is_empty() || zh.is_empty() {
                    continue;
                }
                self.insert_mem(en, zh, locked);
            }
        }
        self.mtime = cur_mtime;
    }

    fn lookup(&mut self, en: &str) -> Option<String> {
        if !self.inited || en.is_empty() {
            return None;
        }
        let key = normalize_cache_key(en);
        if key.is_empty() || key.len() > 48 {
            return None;
        }
        let &idx = self.map.get(&key)?;
        let zh = self
            .nodes
            .get(idx)
            .and_then(|n| n.as_ref())
            .map(|n| n.zh.clone())
            .filter(|z| !z.is_empty())?;
        self.unlink(idx);
        self.push_front(idx);
        Some(zh)
    }

    fn put(&mut self, en: &str, zh: &str) {
        if !self.inited || !can_cache_pair(en, zh) {
            return;
        }
        let key = normalize_cache_key(en);
        if key.is_empty() {
            return;
        }
        if let Some(&idx) = self.map.get(&key) {
            let need_rewrite = if let Some(ex) = &mut self.nodes[idx] {
                if !ex.locked && ex.zh != zh {
                    ex.zh = zh.to_string();
                    true
                } else {
                    false
                }
            } else {
                false
            };
            if need_rewrite {
                self.unlink(idx);
                self.push_front(idx);
                self.rewrite_file();
            }
            return;
        }

        self.insert_mem(en, zh, false);
        if self.auto_count > OCR_CACHE_MAX {
            // 从尾部淘汰最久未用的非锁定条目
            let mut cur = self.tail;
            while let Some(v_idx) = cur {
                let (is_locked, prev, vkey) = match &self.nodes[v_idx] {
                    Some(n) => (n.locked, n.prev, n.key.clone()),
                    None => break,
                };
                if !is_locked {
                    self.map.remove(&vkey);
                    self.unlink(v_idx);
                    self.nodes[v_idx] = None;
                    self.free_slots.push(v_idx);
                    self.auto_count -= 1;
                    break;
                }
                cur = prev;
            }
            self.rewrite_file();
        } else if !self.file_path.as_os_str().is_empty() {
            let _ = fs::create_dir_all(&self.dir);
            let is_new = !self.file_path.exists();
            if let Ok(mut f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
            {
                if is_new {
                    let _ = f.write_all(OCR_CACHE_HEADER.as_bytes());
                }
                let _ = f.write_all(format!("{en}\t{zh}\r\n").as_bytes());
                drop(f);
                self.update_mtime();
            }
        }
    }
}

fn cache_load(cache_dir: &str) {
    if cache_dir.is_empty() {
        return;
    }
    let mut cache = OCR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.load_dir(Path::new(cache_dir));
}

fn cache_lookup(en: &str) -> Option<String> {
    let mut cache = OCR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.lookup(en)
}

fn cache_put(en: &str, zh: &str) {
    let mut cache = OCR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.put(en, zh);
}

#[allow(clippy::too_many_arguments)]
fn translate_blocks(
    blks: &mut [TextBlock],
    ai_url: &str,
    ai_key: &str,
    ai_model: &str,
    baidu_appid: &str,
    baidu_secret: &str,
    baidu_url: &str,
    cache_dir: &str,
) {
    if blks.is_empty() {
        return;
    }
    cache_load(cache_dir);

    let mut miss = Vec::new();
    let mut active = 0usize;
    let mut skipped = 0usize;
    for (i, b) in blks.iter_mut().enumerate() {
        if b.skip_translate {
            skipped += 1;
            continue;
        }
        active += 1;
        if let Some(cached) = cache_lookup(&b.en) {
            b.zh = cached;
        } else {
            miss.push(i);
        }
    }

    if active == 0 {
        log_write(
            "OCR",
            &format!("识别到 {skipped} 块均非英文（中文/数字/符号），已全部跳过"),
        );
        return;
    }
    if miss.is_empty() {
        log_write(
            "OCR",
            &format!("英文 {active} 块全部命中本地记忆库（跳过非英文 {skipped} 块），零网络请求"),
        );
        return;
    }
    if miss.len() < active || skipped > 0 {
        log_write(
            "OCR",
            &format!(
                "英文 {active} 块（记忆库命中 {}，待联网 {}），跳过非英文 {skipped} 块",
                active - miss.len(),
                miss.len()
            ),
        );
    }

    let flags = FY_CHAIN_NO_DICT | FY_CHAIN_PREFER_BAIDU;

    if miss.len() > 1 {
        let joined: String = miss
            .iter()
            .map(|&idx| blks[idx].en.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(res) = translate_chain_ex(
            &joined,
            FY_EN2ZH,
            ai_url,
            ai_key,
            ai_model,
            baidu_appid,
            baidu_secret,
            baidu_url,
            flags,
        ) {
            let mut body = res.as_str();
            if body.starts_with('（') {
                if let Some(nl) = body.find('\n') {
                    if body[..nl].contains("已用必应翻译") {
                        body = &body[nl + 1..];
                    }
                }
            }
            let body = body.trim_end_matches(['\r', '\n']);
            let lines: Vec<&str> = if body.is_empty() {
                Vec::new()
            } else {
                body.split('\n').map(|s| s.trim_end_matches('\r')).collect()
            };
            if lines.len() == miss.len() {
                for (&bi, &zh_line) in miss.iter().zip(lines.iter()) {
                    blks[bi].zh = zh_line.to_string();
                    cache_put(&blks[bi].en, &blks[bi].zh);
                }
                return;
            }
        }
    }

    // 单块或批量行数未对齐：最多 4 线程并发逐块请求
    let next_pos = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<String>>> = (0..miss.len()).map(|_| Mutex::new(None)).collect();
    let n_threads = miss.len().min(4);

    thread::scope(|s| {
        for _ in 0..n_threads {
            s.spawn(|| loop {
                let pos = next_pos.fetch_add(1, Ordering::Relaxed);
                if pos >= miss.len() {
                    break;
                }
                let bi = miss[pos];
                let en = &blks[bi].en;
                let single_line = blks[bi].n_lines <= 1;
                let translated = translate_chain_ex(
                    en,
                    FY_EN2ZH,
                    ai_url,
                    ai_key,
                    ai_model,
                    baidu_appid,
                    baidu_secret,
                    baidu_url,
                    flags,
                )
                .map(|r| clean_zh_for_overlay(&r, single_line))
                .unwrap_or_else(|| en.clone());
                *results[pos].lock().unwrap_or_else(|e| e.into_inner()) = Some(translated);
            });
        }
    });

    for (pos, &bi) in miss.iter().enumerate() {
        if let Some(zh) = results[pos]
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            blks[bi].zh = zh;
            cache_put(&blks[bi].en, &blks[bi].zh);
        }
    }
}

// ---- 颜色采样与 GDI 贴字渲染 ----

/// 采样矩形外围 1-2px 主背景色（4-bit 量化众数桶求均值，抗锯齿毛边）
fn sample_bg_color(bgra: &[u8], w: i32, h: i32, bx: i32, by: i32, bw: i32, bh: i32) -> COLORREF {
    let mut counts = [0i32; 4096];
    let mut sum_r = [0u64; 4096];
    let mut sum_g = [0u64; 4096];
    let mut sum_b = [0u64; 4096];

    let x0 = (bx - 2).max(0);
    let y0 = (by - 2).max(0);
    let x1 = (bx + bw + 1).min(w - 1);
    let y1 = (by + bh + 1).min(h - 1);

    let mut best_idx = 0usize;
    let mut best_cnt = 0i32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            if x > bx + 1 && x < bx + bw - 2 && y > by + 1 && y < by + bh - 2 {
                continue;
            }
            let off = ((y as usize) * (w as usize) + (x as usize)) * 4;
            let b = bgra[off] as usize;
            let g = bgra[off + 1] as usize;
            let r = bgra[off + 2] as usize;
            let idx = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
            counts[idx] += 1;
            sum_r[idx] += r as u64;
            sum_g[idx] += g as u64;
            sum_b[idx] += b as u64;
            if counts[idx] > best_cnt {
                best_cnt = counts[idx];
                best_idx = idx;
            }
        }
    }
    if best_cnt == 0 {
        return rgb(255, 255, 255);
    }
    let cnt = best_cnt as u64;
    rgb(
        (sum_r[best_idx] / cnt) as u8,
        (sum_g[best_idx] / cnt) as u8,
        (sum_b[best_idx] / cnt) as u8,
    )
}

/// 选择与背景高对比度的文字色（优先采样原文字主色）
fn sample_fg_color(
    bgra: &[u8],
    w: i32,
    h: i32,
    box_rc: (i32, i32, i32, i32),
    bg: COLORREF,
) -> COLORREF {
    let (bx, by, bw, bh) = box_rc;
    let bg_r = get_r_value(bg);
    let bg_g = get_g_value(bg);
    let bg_b = get_b_value(bg);
    let bg_y = (299 * bg_r + 587 * bg_g + 114 * bg_b) / 1000;

    let x0 = bx.max(0);
    let y0 = by.max(0);
    let x1 = (bx + bw).min(w);
    let y1 = (by + bh).min(h);

    let mut sr = 0u64;
    let mut sg = 0u64;
    let mut sb = 0u64;
    let mut cnt = 0u64;
    for y in y0..y1 {
        for x in x0..x1 {
            let off = ((y as usize) * (w as usize) + (x as usize)) * 4;
            let b = bgra[off] as i32;
            let g = bgra[off + 1] as i32;
            let r = bgra[off + 2] as i32;
            let diff = (r - bg_r).abs() + (g - bg_g).abs() + (b - bg_b).abs();
            if diff > 110 {
                sr += r as u64;
                sg += g as u64;
                sb += b as u64;
                cnt += 1;
            }
        }
    }
    if cnt >= 8 {
        let fr = (sr / cnt) as i32;
        let fg = (sg / cnt) as i32;
        let fb = (sb / cnt) as i32;
        let fy = (299 * fr + 587 * fg + 114 * fb) / 1000;
        if (fy - bg_y).abs() >= 75 {
            return rgb(fr as u8, fg as u8, fb as u8);
        }
    }
    if bg_y > 135 {
        rgb(22, 22, 22)
    } else {
        rgb(245, 245, 245)
    }
}

const OCR_MAX_WRAP_LINES: usize = 64;

#[derive(Clone, Copy)]
struct WrapLine {
    start: usize,
    len: usize,
    width: i32,
}

fn is_latin_token_w(c: u16) -> bool {
    (c >= b'A' as u16 && c <= b'Z' as u16)
        || (c >= b'a' as u16 && c <= b'z' as u16)
        || (c >= b'0' as u16 && c <= b'9' as u16)
        || c == b'_' as u16
        || c == b'.' as u16
        || c == b'-' as u16
}

fn measure_text_cx(hdc: HDC, slice: &[u16]) -> i32 {
    if slice.is_empty() {
        return 0;
    }
    let mut sz = SIZE::default();
    // SAFETY: hdc 为有效 DC，slice 指向 slice.len() 个 UTF-16 字符
    unsafe {
        let _ = GetTextExtentPoint32W(hdc, slice.as_ptr(), slice.len() as i32, &mut sz);
    }
    sz.cx
}

fn wrap_wide_text(
    hdc: HDC,
    wz: &[u16],
    max_w: i32,
    allow_wrap: bool,
) -> (Vec<WrapLine>, i32, bool) {
    if wz.is_empty() || max_w <= 0 {
        return (Vec::new(), 0, false);
    }
    if !allow_wrap {
        let cx = measure_text_cx(hdc, wz);
        return (
            vec![WrapLine {
                start: 0,
                len: wz.len(),
                width: cx,
            }],
            cx,
            false,
        );
    }

    let mut lines = Vec::new();
    let mut max_line_w = 0i32;
    let mut p = 0usize;
    let n = wz.len();

    while p < n {
        if wz[p] == b'\r' as u16 || wz[p] == b'\n' as u16 {
            p += 1;
            continue;
        }
        while p < n && (wz[p] == b' ' as u16 || wz[p] == b'\t' as u16) {
            p += 1;
        }
        if p >= n || wz[p] == b'\r' as u16 || wz[p] == b'\n' as u16 {
            continue;
        }
        if lines.len() >= OCR_MAX_WRAP_LINES {
            return (lines, max_line_w, true);
        }

        let line_start = p;
        let mut line_len = 0usize;
        while p < n && wz[p] != b'\r' as u16 && wz[p] != b'\n' as u16 {
            let tok_start = p;
            let mut tok_len = 1usize;
            if wz[p] == b' ' as u16 || wz[p] == b'\t' as u16 {
                while tok_start + tok_len < n
                    && (wz[tok_start + tok_len] == b' ' as u16
                        || wz[tok_start + tok_len] == b'\t' as u16)
                {
                    tok_len += 1;
                }
            } else if is_latin_token_w(wz[p]) {
                while tok_start + tok_len < n && is_latin_token_w(wz[tok_start + tok_len]) {
                    tok_len += 1;
                }
            }

            let cand_len = (tok_start + tok_len) - line_start;
            let cx = measure_text_cx(hdc, &wz[line_start..line_start + cand_len]);
            if cx <= max_w {
                line_len = cand_len;
                p = tok_start + tok_len;
            } else if line_len > 0 {
                break;
            } else {
                let mut fit = 1usize;
                for k in 2..=tok_len {
                    if measure_text_cx(hdc, &wz[line_start..line_start + k]) > max_w {
                        break;
                    }
                    fit = k;
                }
                line_len = fit;
                p = line_start + fit;
                break;
            }
        }

        while line_len > 0
            && (wz[line_start + line_len - 1] == b' ' as u16
                || wz[line_start + line_len - 1] == b'\t' as u16)
        {
            line_len -= 1;
        }
        if line_len > 0 {
            let line_w = measure_text_cx(hdc, &wz[line_start..line_start + line_len]);
            max_line_w = max_line_w.max(line_w);
            lines.push(WrapLine {
                start: line_start,
                len: line_len,
                width: line_w,
            });
        }
    }
    (lines, max_line_w, false)
}

fn create_overlay_font(fh: i32, n_width: i32) -> HFONT {
    let face = utf8_to_wide("Microsoft YaHei UI");
    // SAFETY: face 以 '\0' 结尾，参数符合 CreateFontW 要求
    unsafe {
        CreateFontW(
            -fh,
            n_width,
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
            face.as_ptr(),
        )
    }
}

fn calc_visual_total_h(fh: i32, n_lines: usize) -> (i32, i32, i32) {
    let vis_h = (((fh as f32) * 0.90).ceil() as i32).max(5);
    let step = (((fh as f32) * 1.15).ceil() as i32).max(vis_h + 1);
    let total = if n_lines <= 1 {
        vis_h
    } else {
        ((n_lines as i32) - 1) * step + vis_h
    };
    (total, vis_h, step)
}

fn test_font_fit(
    hdc: HDC,
    wz: &[u16],
    avail_w: i32,
    avail_h: i32,
    fh: i32,
    allow_wrap: bool,
) -> bool {
    let f = create_overlay_font(fh, 0);
    if f.is_null() {
        return false;
    }
    // SAFETY: f 为有效 HFONT，先选中测量再恢复并释放
    let (lines, max_lw, overflow) = unsafe {
        let old_f = SelectObject(hdc, f);
        let res = wrap_wide_text(hdc, wz, avail_w, allow_wrap);
        let _ = SelectObject(hdc, old_f);
        let _ = DeleteObject(f);
        res
    };
    if overflow || lines.is_empty() || max_lw > avail_w {
        return false;
    }
    let (total_h, _, _) = calc_visual_total_h(fh, lines.len());
    total_h <= avail_h
}

fn find_best_fh(
    hdc: HDC,
    wz: &[u16],
    avail_w: i32,
    avail_h: i32,
    min_fh: i32,
    max_fh: i32,
    allow_wrap: bool,
) -> i32 {
    if max_fh < min_fh {
        return 0;
    }
    let mut lo = min_fh;
    let mut hi = max_fh;
    let mut best = 0;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        if test_font_fit(hdc, wz, avail_w, avail_h, mid, allow_wrap) {
            best = mid;
            lo = mid + 1;
        } else {
            hi = mid - 1;
        }
    }
    best
}

fn compute_safe_vpad(blks: &[TextBlock], idx: usize, img_h: i32, orig_h: i32) -> (i32, i32) {
    let b = &blks[idx];
    let mut gap_top = b.y.max(0);
    let mut gap_bot = (img_h - (b.y + b.h)).max(0);

    for (j, o) in blks.iter().enumerate() {
        if j == idx {
            continue;
        }
        if o.x + o.w <= b.x - 2 || o.x >= b.x + b.w + 2 {
            continue;
        }
        if o.y + o.h <= b.y {
            gap_top = gap_top.min(b.y - (o.y + o.h));
        } else if o.y >= b.y + b.h {
            gap_bot = gap_bot.min(o.y - (b.y + b.h));
        }
    }

    let pad_cap = (((orig_h as f32) * 0.28).ceil() as i32).max(2);
    let pt = if gap_top > 2 {
        (gap_top - 1) / 2
    } else if gap_top > 0 {
        1
    } else {
        0
    }
    .min(pad_cap);
    let pb = if gap_bot > 2 {
        (gap_bot - 1) / 2
    } else if gap_bot > 0 {
        1
    } else {
        0
    }
    .min(pad_cap);
    (pt, pb)
}

fn render_blocks(bgra_in: &[u8], bgra_out: &mut [u8], w: i32, h: i32, blks: &[TextBlock]) {
    let img_bytes = (w as usize) * (h as usize) * 4;
    bgra_out[..img_bytes].copy_from_slice(&bgra_in[..img_bytes]);
    if blks.is_empty() {
        return;
    }

    // SAFETY: GetDC(NULL) / CreateCompatibleDC / CreateDIBSection 配对创建与释放，确保无 GDI 泄漏
    unsafe {
        let hdc_scr = GetDC(null_mut());
        let hdc = CreateCompatibleDC(hdc_scr);
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as DWORD,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };

        let mut bits: *mut c_void = null_mut();
        let hbm = CreateDIBSection(hdc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if hbm.is_null() || bits.is_null() {
            if !hbm.is_null() {
                let _ = DeleteObject(hbm);
            }
            if !hdc.is_null() {
                let _ = DeleteDC(hdc);
            }
            let _ = ReleaseDC(null_mut(), hdc_scr);
            return;
        }
        std::ptr::copy_nonoverlapping(bgra_in.as_ptr(), bits.cast(), img_bytes);
        let old_bm = SelectObject(hdc, hbm);
        let _ = SetBkMode(hdc, TRANSPARENT);

        for (i, b) in blks.iter().enumerate() {
            if b.skip_translate || b.zh.is_empty() {
                continue;
            }
            let wz_full = utf8_to_wide(&b.zh);
            let wz = &wz_full[..wz_full.len().saturating_sub(1)];
            if wz.is_empty() {
                continue;
            }

            let orig_h = (if b.n_lines > 1 { b.line_h } else { b.h }).max(8);
            let (pad_top, pad_bot) = compute_safe_vpad(blks, i, h, orig_h);

            let safe_rc = RECT {
                left: b.x.max(0),
                right: (b.x + b.w + 2).min(w),
                top: (b.y - pad_top).max(0),
                bottom: (b.y + b.h + pad_bot).min(h),
            };
            let avail_w = safe_rc.right - safe_rc.left;
            let avail_h = safe_rc.bottom - safe_rc.top;
            if avail_w <= 2 || avail_h <= 2 {
                continue;
            }

            let base_fh = ((orig_h as f32) * 1.05 + 0.5).floor() as i32;
            let max_fh = (((orig_h as f32) * 1.05 * 1.15 + 0.5).floor() as i32).clamp(10, 72);
            let min_fh = (if orig_h < 10 { 7 } else { 8 }).min(max_fh);

            let mut use_wrap = b.n_lines > 1;
            let mut best_fh = if !use_wrap {
                let fh_single = find_best_fh(hdc, wz, avail_w, avail_h, min_fh, max_fh, false);
                let fh_wrap =
                    if fh_single < (((base_fh as f32) * 0.85) as i32) && avail_h >= min_fh * 2 {
                        find_best_fh(hdc, wz, avail_w, avail_h, min_fh, max_fh, true)
                    } else {
                        0
                    };
                if fh_wrap > fh_single {
                    use_wrap = true;
                    fh_wrap
                } else {
                    fh_single
                }
            } else {
                find_best_fh(hdc, wz, avail_w, avail_h, min_fh, max_fh, true)
            };

            let mut n_width = 0;
            let mut need_ellipsis = false;
            if best_fh == 0 {
                best_fh = min_fh;
                let ftmp = create_overlay_font(best_fh, 0);
                if !ftmp.is_null() {
                    let old_f = SelectObject(hdc, ftmp);
                    let cx = measure_text_cx(hdc, wz);
                    let mut tm = TEXTMETRICW::default();
                    if cx > avail_w && GetTextMetricsW(hdc, &mut tm) != 0 && tm.tmAveCharWidth > 0 {
                        let ratio = ((avail_w as f32) / (cx as f32)).max(0.70);
                        let nw = ((tm.tmAveCharWidth as f32) * ratio) as i32;
                        if nw >= 2 && nw < tm.tmAveCharWidth {
                            n_width = nw;
                        }
                    }
                    let _ = SelectObject(hdc, old_f);
                    let _ = DeleteObject(ftmp);
                }
                need_ellipsis = true;
            }

            let hf = create_overlay_font(best_fh, n_width);
            if hf.is_null() {
                continue;
            }
            let old_f = SelectObject(hdc, hf);
            let (mut lines, _, _) = wrap_wide_text(hdc, wz, avail_w, use_wrap);
            if lines.len() > OCR_MAX_WRAP_LINES {
                lines.truncate(OCR_MAX_WRAP_LINES);
            }

            let (mut total_h, vis_h, line_step) = calc_visual_total_h(best_fh, lines.len().max(1));
            if lines.len() > 1 && total_h > avail_h {
                let max_fit_lines = (1 + if avail_h > vis_h {
                    (avail_h - vis_h) / line_step
                } else {
                    0
                }) as usize;
                if lines.len() > max_fit_lines {
                    lines.truncate(max_fit_lines);
                    need_ellipsis = true;
                    let res = calc_visual_total_h(best_fh, lines.len());
                    total_h = res.0;
                }
            }

            let bg = sample_bg_color(bgra_in, w, h, b.x, b.y, b.w, b.h);
            let fg = sample_fg_color(bgra_in, w, h, (b.x, b.y, b.w, b.h), bg);

            let mut fill_rc = RECT {
                left: (b.x - 2).max(0),
                right: (b.x + b.w + 2).min(w),
                top: (b.y - 1).max(0),
                bottom: (b.y + b.h + 1).min(h),
            };
            let mut fill_h = fill_rc.bottom - fill_rc.top;
            let need_h = total_h + 4;
            if need_h > fill_h {
                let center_y = (fill_rc.top + fill_rc.bottom) / 2;
                let mut nt = center_y - need_h / 2;
                let mut nb_y = nt + need_h;
                if nt < safe_rc.top {
                    nt = safe_rc.top;
                    nb_y = nt + need_h;
                }
                if nb_y > safe_rc.bottom {
                    nb_y = safe_rc.bottom;
                    nt = nb_y - need_h;
                }
                if nt < safe_rc.top {
                    nt = safe_rc.top;
                }
                if nt < fill_rc.top {
                    fill_rc.top = nt;
                }
                if nb_y > fill_rc.bottom {
                    fill_rc.bottom = nb_y;
                }
                fill_h = fill_rc.bottom - fill_rc.top;
            }

            let br: HBRUSH = CreateSolidBrush(bg);
            if !br.is_null() {
                let _ = FillRect(hdc, &fill_rc, br);
                let _ = DeleteObject(br);
            }

            let _ = SetTextColor(hdc, fg);
            let mut tm = TEXTMETRICW::default();
            let ink_offset = if GetTextMetricsW(hdc, &mut tm) != 0 {
                ((tm.tmInternalLeading * 2) / 3).max(0)
            } else {
                0
            };

            let ink_top = fill_rc.top + (fill_h - total_h) / 2;
            let clip_rc = RECT {
                left: safe_rc.left,
                top: fill_rc.top,
                right: safe_rc.right,
                bottom: fill_rc.bottom,
            };

            for (r, ln) in lines.iter().enumerate() {
                let draw_y = ink_top + (r as i32) * line_step - ink_offset;
                let is_last = r + 1 == lines.len();
                let line_slice = &wz[ln.start..ln.start + ln.len];
                if is_last && (need_ellipsis || ln.width > avail_w) {
                    let mut k = ln.len;
                    let mut ebuf = Vec::with_capacity(ln.len + 1);
                    while k > 0 {
                        ebuf.clear();
                        ebuf.extend_from_slice(&line_slice[..k]);
                        ebuf.push(0x2026); // '…'
                        if measure_text_cx(hdc, &ebuf) <= avail_w {
                            break;
                        }
                        k -= 1;
                    }
                    if k > 0 {
                        let _ = ExtTextOutW(
                            hdc,
                            safe_rc.left,
                            draw_y,
                            ETO_CLIPPED,
                            &clip_rc,
                            ebuf.as_ptr(),
                            ebuf.len() as UINT,
                            null(),
                        );
                    } else {
                        let _ = ExtTextOutW(
                            hdc,
                            safe_rc.left,
                            draw_y,
                            ETO_CLIPPED,
                            &clip_rc,
                            line_slice.as_ptr(),
                            line_slice.len() as UINT,
                            null(),
                        );
                    }
                } else {
                    let _ = ExtTextOutW(
                        hdc,
                        safe_rc.left,
                        draw_y,
                        ETO_CLIPPED,
                        &clip_rc,
                        line_slice.as_ptr(),
                        line_slice.len() as UINT,
                        null(),
                    );
                }
            }

            let _ = SelectObject(hdc, old_f);
            let _ = DeleteObject(hf);
        }

        let _ = GdiFlush();
        std::ptr::copy_nonoverlapping(bits.cast::<u8>(), bgra_out.as_mut_ptr(), img_bytes);
        let _ = SelectObject(hdc, old_bm);
        let _ = DeleteObject(hbm);
        let _ = DeleteDC(hdc);
        let _ = ReleaseDC(null_mut(), hdc_scr);
    }
}

fn join_block_texts(blks: &[TextBlock], is_zh: bool) -> String {
    let mut parts = Vec::new();
    for b in blks {
        if b.skip_translate {
            continue;
        }
        let s = if is_zh { &b.zh } else { &b.en };
        if !s.is_empty() {
            parts.push(s.as_str());
        }
    }
    parts.join("\n")
}

/// 截屏 OCR + 英文→中文原图替换翻译主入口
#[allow(clippy::too_many_arguments)]
pub fn snip_translate(
    bgra_in: &[u8],
    width: i32,
    height: i32,
    ai_url: &str,
    ai_key: &str,
    ai_model: &str,
    baidu_appid: &str,
    baidu_secret: &str,
    baidu_url: &str,
    cache_dir: &str,
) -> SnipResult {
    if width <= 0 || height <= 0 {
        return SnipResult::fail(width, height, Vec::new(), "无效图像尺寸");
    }
    let img_bytes = (width as usize) * (height as usize) * 4;
    if bgra_in.len() < img_bytes {
        return SnipResult::fail(width, height, Vec::new(), "无效图像尺寸");
    }
    let mut bgra_out = bgra_in[..img_bytes].to_vec();

    let lines = match run_winrt_ocr(&bgra_in[..img_bytes], width, height) {
        Ok(l) => l,
        Err(err) => return SnipResult::fail(width, height, bgra_out, &err),
    };
    if lines.is_empty() {
        return SnipResult::fail(width, height, bgra_out, "未识别到文本");
    }

    let mut blks = group_lines(&lines);
    let need_cnt = blks.iter().filter(|b| !b.skip_translate).count();
    if need_cnt == 0 {
        return SnipResult::fail(width, height, bgra_out, "未检测到需要翻译的英文文本");
    }

    translate_blocks(
        &mut blks,
        ai_url,
        ai_key,
        ai_model,
        baidu_appid,
        baidu_secret,
        baidu_url,
        cache_dir,
    );
    render_blocks(&bgra_in[..img_bytes], &mut bgra_out, width, height, &blks);

    SnipResult {
        ok: true,
        width,
        height,
        bgra_out,
        zh_text: join_block_texts(&blks, true),
        en_text: join_block_texts(&blks, false),
        err: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_translate_and_clean_overlay() {
        assert!(should_translate_block("File Settings"));
        assert!(!should_translate_block("X"));
        assert!(!should_translate_block("12345"));
        assert!(!should_translate_block("保存设置"));
        assert!(!should_translate_block("中文设置 UI")); // 中文比例高时跳过

        let raw = "（AI：错误。已用必应翻译）\n英[həˈləʊ]\n[həˈləʊ]\n你好世界\n拼音：nǐ hǎo";
        assert_eq!(clean_zh_for_overlay(raw, true), "你好世界");
        assert_eq!(
            clean_zh_for_overlay("第一行\n第二行", false),
            "第一行\n第二行"
        );
    }

    #[test]
    fn test_group_lines_and_safe_vpad() {
        let lines = vec![
            OcrLineBox {
                x: 20.0,
                y: 20.0,
                w: 100.0,
                h: 20.0,
                text: "Hello".into(),
                need_trans: true,
            },
            OcrLineBox {
                x: 22.0,
                y: 44.0,
                w: 90.0,
                h: 20.0,
                text: "World".into(),
                need_trans: true,
            },
            OcrLineBox {
                x: 20.0,
                y: 70.0,
                w: 80.0,
                h: 20.0,
                text: "中文标题".into(),
                need_trans: false,
            },
        ];
        let blks = group_lines(&lines);
        assert_eq!(blks.len(), 2);
        assert_eq!(blks[0].n_lines, 2);
        assert_eq!(blks[0].en, "Hello World");
        assert!(!blks[0].skip_translate);
        assert!(blks[1].skip_translate);

        // 相邻中文块在 y=70，blks[0] 底部在 64，下方空间仅 6px，安全借位应 <= 2px 不压到中文块
        let (pt, pb) = compute_safe_vpad(&blks, 0, 120, 20);
        assert!(pt > 0);
        assert!(pb <= 2);
    }

    #[test]
    fn test_ocr_cache_lru_and_locked_persistence() {
        let dir = std::env::temp_dir().join("ky_rust_ocr_cache_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create cache dir");

        let cache_file = dir.join("ocr_cache.txt");
        let init = format!("{OCR_CACHE_HEADER}*\tFile\t文件(校对)\r\nEdit\t编辑\r\n");
        fs::write(&cache_file, init.as_bytes()).expect("write initial cache");

        {
            let mut cache = OCR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            cache.load_dir(&dir);
            assert_eq!(cache.lookup("  FILE "), Some("文件(校对)".to_string()));
            assert_eq!(cache.lookup("edit"), Some("编辑".to_string()));

            // 锁定条目不允许被普通写入覆盖
            cache.put("File", "档案");
            assert_eq!(cache.lookup("File"), Some("文件(校对)".to_string()));

            // 非锁定条目允许追加写入并落盘
            cache.put("Settings", "设置");
            assert_eq!(cache.lookup("SETTINGS"), Some("设置".to_string()));
        }

        // 清空内存后从磁盘重新加载验证持久化
        {
            let mut cache = OCR_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            cache.clear();
            cache.inited = false;
            cache.load_dir(&dir);
            assert_eq!(cache.lookup("File"), Some("文件(校对)".to_string()));
            assert_eq!(cache.lookup("Settings"), Some("设置".to_string()));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_color_sampling_and_gdi_render_blocks() {
        let w = 160;
        let h = 60;
        let mut bgra_in = vec![0u8; (w as usize) * (h as usize) * 4];
        // 填充深蓝背景 (B=120, G=40, R=20, A=255)
        for px in bgra_in.as_chunks_mut::<4>().0 {
            px[0] = 120;
            px[1] = 40;
            px[2] = 20;
            px[3] = 255;
        }
        // 在文本块区域 (20..120, 15..35) 内画一些亮白模拟原英文笔画像素
        for y in 18..32 {
            for x in 25..100 {
                if (x + y) % 3 == 0 {
                    let off = (y * (w as usize) + x) * 4;
                    bgra_in[off] = 240;
                    bgra_in[off + 1] = 240;
                    bgra_in[off + 2] = 240;
                }
            }
        }

        let bg = sample_bg_color(&bgra_in, w, h, 20, 15, 100, 20);
        assert_eq!(
            (get_r_value(bg), get_g_value(bg), get_b_value(bg)),
            (20, 40, 120)
        );

        let fg = sample_fg_color(&bgra_in, w, h, (20, 15, 100, 20), bg);
        assert!(get_r_value(fg) > 200 && get_g_value(fg) > 200 && get_b_value(fg) > 200);

        let blks = vec![TextBlock {
            x: 20,
            y: 15,
            w: 100,
            h: 20,
            line_h: 20,
            n_lines: 1,
            skip_translate: false,
            en: "Settings".into(),
            zh: "系统设置".into(),
        }];
        let mut bgra_out = vec![0u8; bgra_in.len()];
        render_blocks(&bgra_in, &mut bgra_out, w, h, &blks);

        // 确认原区域像素已按背景色擦除并绘制了中文，且框外远端背景保持不变
        assert_ne!(bgra_out, bgra_in);
        let corner = &bgra_out[0..4];
        assert_eq!(corner[0..3], [120, 40, 20]);
    }
}
