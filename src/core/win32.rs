//! Windows 原生 API FFI 绑定（分波次按需声明，保持零第三方依赖）

#![allow(non_camel_case_types, non_snake_case)]

use std::ffi::c_void;

pub type BOOL = i32;
pub type BYTE = u8;
pub type WORD = u16;
pub type DWORD = u32;
pub type LONG = i32;
pub type UINT = u32;
pub type WCHAR = u16;
pub type HANDLE = *mut c_void;
pub type HWND = *mut c_void;
pub type HDC = *mut c_void;
pub type HBITMAP = *mut c_void;
pub type HFONT = *mut c_void;
pub type HBRUSH = *mut c_void;
pub type HGDIOBJ = *mut c_void;
pub type HMODULE = *mut c_void;
pub type HINTERNET = *mut c_void;
pub type INTERNET_PORT = WORD;
pub type INTERNET_SCHEME = i32;
pub type COLORREF = DWORD;
pub type HRESULT = i32;

pub const CP_UTF8: UINT = 65001;
pub const CP_GBK: UINT = 936;
pub const EXCEPTION_CONTINUE_SEARCH: LONG = 0;
pub const FILE_APPEND_DATA: DWORD = 0x0004;
pub const FILE_SHARE_READ: DWORD = 0x0001;
pub const FILE_SHARE_WRITE: DWORD = 0x0002;
pub const OPEN_ALWAYS: DWORD = 4;
pub const FILE_ATTRIBUTE_NORMAL: DWORD = 0x0000_0080;
pub const INVALID_HANDLE_VALUE: HANDLE = (-1isize) as HANDLE;

pub const DIB_RGB_COLORS: UINT = 0;
pub const BI_RGB: DWORD = 0;
pub const HALFTONE: i32 = 4;
pub const SRCCOPY: DWORD = 0x00CC_0020;
pub const TRANSPARENT: i32 = 1;
pub const DEFAULT_CHARSET: DWORD = 1;
pub const FW_NORMAL: i32 = 400;
pub const CLEARTYPE_QUALITY: DWORD = 5;
pub const ETO_CLIPPED: UINT = 4;

#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    (r as DWORD) | ((g as DWORD) << 8) | ((b as DWORD) << 16)
}

#[inline]
pub const fn get_r_value(c: COLORREF) -> i32 {
    (c & 0xFF) as i32
}

#[inline]
pub const fn get_g_value(c: COLORREF) -> i32 {
    ((c >> 8) & 0xFF) as i32
}

#[inline]
pub const fn get_b_value(c: COLORREF) -> i32 {
    ((c >> 16) & 0xFF) as i32
}

pub const INTERNET_OPEN_TYPE_PRECONFIG: DWORD = 0;
pub const INTERNET_SERVICE_HTTP: DWORD = 3;
pub const INTERNET_SCHEME_HTTPS: INTERNET_SCHEME = 4;
pub const INTERNET_OPTION_CONNECT_TIMEOUT: DWORD = 2;
pub const INTERNET_OPTION_SEND_TIMEOUT: DWORD = 5;
pub const INTERNET_OPTION_RECEIVE_TIMEOUT: DWORD = 6;
pub const INTERNET_FLAG_RELOAD: DWORD = 0x8000_0000;
pub const INTERNET_FLAG_NO_CACHE_WRITE: DWORD = 0x0400_0000;
pub const INTERNET_FLAG_KEEP_CONNECTION: DWORD = 0x0040_0000;
pub const INTERNET_FLAG_NO_AUTO_REDIRECT: DWORD = 0x0020_0000;
pub const INTERNET_FLAG_SECURE: DWORD = 0x0080_0000;

pub const HTTP_QUERY_CONTENT_LENGTH: DWORD = 5;
pub const HTTP_QUERY_STATUS_CODE: DWORD = 19;
pub const HTTP_QUERY_LOCATION: DWORD = 33;
pub const HTTP_QUERY_FLAG_NUMBER: DWORD = 0x2000_0000;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SYSTEMTIME {
    pub wYear: WORD,
    pub wMonth: WORD,
    pub wDayOfWeek: WORD,
    pub wDay: WORD,
    pub wHour: WORD,
    pub wMinute: WORD,
    pub wSecond: WORD,
    pub wMilliseconds: WORD,
}

pub const EXCEPTION_MAXIMUM_PARAMETERS: usize = 15;

#[repr(C)]
pub struct EXCEPTION_RECORD {
    pub ExceptionCode: DWORD,
    pub ExceptionFlags: DWORD,
    pub ExceptionRecord: *const EXCEPTION_RECORD,
    pub ExceptionAddress: *const c_void,
    pub NumberParameters: DWORD,
    pub ExceptionInformation: [usize; EXCEPTION_MAXIMUM_PARAMETERS],
}

#[repr(C)]
pub struct EXCEPTION_POINTERS {
    pub ExceptionRecord: *const EXCEPTION_RECORD,
    pub ContextRecord: *const c_void,
}

pub type LPTOP_LEVEL_EXCEPTION_FILTER =
    Option<unsafe extern "system" fn(exception_info: *const EXCEPTION_POINTERS) -> LONG>;

#[repr(C)]
pub struct URL_COMPONENTSA {
    pub dwStructSize: DWORD,
    pub lpszScheme: *mut u8,
    pub dwSchemeLength: DWORD,
    pub nScheme: INTERNET_SCHEME,
    pub lpszHostName: *mut u8,
    pub dwHostNameLength: DWORD,
    pub nPort: INTERNET_PORT,
    pub lpszUserName: *mut u8,
    pub dwUserNameLength: DWORD,
    pub lpszPassword: *mut u8,
    pub dwPasswordLength: DWORD,
    pub lpszUrlPath: *mut u8,
    pub dwUrlPathLength: DWORD,
    pub lpszExtraInfo: *mut u8,
    pub dwExtraInfoLength: DWORD,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct POINT {
    pub x: LONG,
    pub y: LONG,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct SIZE {
    pub cx: LONG,
    pub cy: LONG,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RECT {
    pub left: LONG,
    pub top: LONG,
    pub right: LONG,
    pub bottom: LONG,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BITMAPINFOHEADER {
    pub biSize: DWORD,
    pub biWidth: LONG,
    pub biHeight: LONG,
    pub biPlanes: WORD,
    pub biBitCount: WORD,
    pub biCompression: DWORD,
    pub biSizeImage: DWORD,
    pub biXPelsPerMeter: LONG,
    pub biYPelsPerMeter: LONG,
    pub biClrUsed: DWORD,
    pub biClrImportant: DWORD,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RGBQUAD {
    pub rgbBlue: BYTE,
    pub rgbGreen: BYTE,
    pub rgbRed: BYTE,
    pub rgbReserved: BYTE,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct BITMAPINFO {
    pub bmiHeader: BITMAPINFOHEADER,
    pub bmiColors: [RGBQUAD; 1],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TEXTMETRICW {
    pub tmHeight: LONG,
    pub tmAscent: LONG,
    pub tmDescent: LONG,
    pub tmInternalLeading: LONG,
    pub tmExternalLeading: LONG,
    pub tmAveCharWidth: LONG,
    pub tmMaxCharWidth: LONG,
    pub tmWeight: LONG,
    pub tmOverhang: LONG,
    pub tmDigitizedAspectX: LONG,
    pub tmDigitizedAspectY: LONG,
    pub tmFirstChar: WCHAR,
    pub tmLastChar: WCHAR,
    pub tmDefaultChar: WCHAR,
    pub tmBreakChar: WCHAR,
    pub tmItalic: BYTE,
    pub tmUnderlined: BYTE,
    pub tmStruckOut: BYTE,
    pub tmPitchAndFamily: BYTE,
    pub tmCharSet: BYTE,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GUID {
    pub Data1: DWORD,
    pub Data2: WORD,
    pub Data3: WORD,
    pub Data4: [BYTE; 8],
}

#[link(name = "kernel32")]
extern "system" {
    pub fn MultiByteToWideChar(
        code_page: UINT,
        dw_flags: DWORD,
        lp_multi_byte_str: *const u8,
        cb_multi_byte: i32,
        lp_wide_char_str: *mut WCHAR,
        cch_wide_char: i32,
    ) -> i32;

    pub fn WideCharToMultiByte(
        code_page: UINT,
        dw_flags: DWORD,
        lp_wide_char_str: *const WCHAR,
        cch_wide_char: i32,
        lp_multi_byte_str: *mut u8,
        cb_multi_byte: i32,
        lp_default_char: *const u8,
        lp_used_default_char: *mut BOOL,
    ) -> i32;

    pub fn GetLocalTime(lp_system_time: *mut SYSTEMTIME);
    pub fn GetCurrentThreadId() -> DWORD;
    pub fn GetTickCount() -> DWORD;
    pub fn GetTickCount64() -> u64;
    pub fn GetLastError() -> DWORD;
    pub fn LoadLibraryA(lp_lib_file_name: *const u8) -> HMODULE;
    pub fn GetProcAddress(h_module: HMODULE, lp_proc_name: *const u8) -> *const c_void;
    pub fn SetUnhandledExceptionFilter(
        lp_top_level_exception_filter: LPTOP_LEVEL_EXCEPTION_FILTER,
    ) -> LPTOP_LEVEL_EXCEPTION_FILTER;

    pub fn CreateFileW(
        lp_file_name: *const WCHAR,
        dw_desired_access: DWORD,
        dw_share_mode: DWORD,
        lp_security_attributes: *mut c_void,
        dw_creation_disposition: DWORD,
        dw_flags_and_attributes: DWORD,
        h_template_file: HANDLE,
    ) -> HANDLE;

    pub fn WriteFile(
        h_file: HANDLE,
        lp_buffer: *const c_void,
        n_number_of_bytes_to_write: DWORD,
        lp_number_of_bytes_written: *mut DWORD,
        lp_overlapped: *mut c_void,
    ) -> BOOL;

    pub fn CloseHandle(h_object: HANDLE) -> BOOL;
}

#[link(name = "user32")]
extern "system" {
    pub fn GetDC(h_wnd: HWND) -> HDC;
    pub fn ReleaseDC(h_wnd: HWND, h_dc: HDC) -> i32;
    pub fn FillRect(h_dc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn CreateDIBSection(
        hdc: HDC,
        pbmi: *const BITMAPINFO,
        usage: UINT,
        ppv_bits: *mut *mut c_void,
        h_section: HANDLE,
        offset: DWORD,
    ) -> HBITMAP;
    pub fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
    pub fn DeleteObject(ho: HGDIOBJ) -> BOOL;
    pub fn SetStretchBltMode(hdc: HDC, mode: i32) -> i32;
    pub fn SetBrushOrgEx(hdc: HDC, x: i32, y: i32, lppt: *mut POINT) -> BOOL;
    pub fn StretchBlt(
        hdc_dest: HDC,
        x_dest: i32,
        y_dest: i32,
        w_dest: i32,
        h_dest: i32,
        hdc_src: HDC,
        x_src: i32,
        y_src: i32,
        w_src: i32,
        h_src: i32,
        rop: DWORD,
    ) -> BOOL;
    pub fn GdiFlush() -> BOOL;
    pub fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    pub fn SetTextColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn CreateFontW(
        c_height: i32,
        c_width: i32,
        c_escapement: i32,
        c_orientation: i32,
        c_weight: i32,
        b_italic: DWORD,
        b_underline: DWORD,
        b_strike_out: DWORD,
        i_char_set: DWORD,
        i_out_precision: DWORD,
        i_clip_precision: DWORD,
        i_quality: DWORD,
        i_pitch_and_family: DWORD,
        psz_face_name: *const WCHAR,
    ) -> HFONT;
    pub fn GetTextExtentPoint32W(
        hdc: HDC,
        lp_string: *const WCHAR,
        c: i32,
        psizl: *mut SIZE,
    ) -> BOOL;
    pub fn GetTextMetricsW(hdc: HDC, lptm: *mut TEXTMETRICW) -> BOOL;
    pub fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
    pub fn ExtTextOutW(
        hdc: HDC,
        x: i32,
        y: i32,
        options: UINT,
        lprect: *const RECT,
        lp_string: *const WCHAR,
        c: UINT,
        lp_dx: *const i32,
    ) -> BOOL;
}

#[link(name = "wininet")]
extern "system" {
    pub fn InternetOpenA(
        lpsz_agent: *const u8,
        dw_access_type: DWORD,
        lpsz_proxy: *const u8,
        lpsz_proxy_bypass: *const u8,
        dw_flags: DWORD,
    ) -> HINTERNET;

    pub fn InternetSetOptionA(
        h_internet: HINTERNET,
        dw_option: DWORD,
        lp_buffer: *const c_void,
        dw_buffer_length: DWORD,
    ) -> BOOL;

    pub fn InternetOpenUrlA(
        h_internet: HINTERNET,
        lpsz_url: *const u8,
        lpsz_headers: *const u8,
        dw_headers_length: DWORD,
        dw_flags: DWORD,
        dw_context: usize,
    ) -> HINTERNET;

    pub fn InternetReadFile(
        h_file: HINTERNET,
        lp_buffer: *mut c_void,
        dw_number_of_bytes_to_read: DWORD,
        lpdw_number_of_bytes_read: *mut DWORD,
    ) -> BOOL;

    pub fn InternetCloseHandle(h_internet: HINTERNET) -> BOOL;

    pub fn InternetCrackUrlA(
        lpsz_url: *const u8,
        dw_url_length: DWORD,
        dw_flags: DWORD,
        lp_url_components: *mut URL_COMPONENTSA,
    ) -> BOOL;

    pub fn InternetConnectA(
        h_internet: HINTERNET,
        lpsz_server_name: *const u8,
        n_server_port: INTERNET_PORT,
        lpsz_user_name: *const u8,
        lpsz_password: *const u8,
        dw_service: DWORD,
        dw_flags: DWORD,
        dw_context: usize,
    ) -> HINTERNET;

    pub fn HttpOpenRequestA(
        h_connect: HINTERNET,
        lpsz_verb: *const u8,
        lpsz_object_name: *const u8,
        lpsz_version: *const u8,
        lpsz_referrer: *const u8,
        lplpsz_accept_types: *const *const u8,
        dw_flags: DWORD,
        dw_context: usize,
    ) -> HINTERNET;

    pub fn HttpSendRequestA(
        h_request: HINTERNET,
        lpsz_headers: *const u8,
        dw_headers_length: DWORD,
        lp_optional: *const c_void,
        dw_optional_length: DWORD,
    ) -> BOOL;

    pub fn HttpQueryInfoA(
        h_request: HINTERNET,
        dw_info_level: DWORD,
        lp_buffer: *mut c_void,
        lpdw_buffer_length: *mut DWORD,
        lpdw_index: *mut DWORD,
    ) -> BOOL;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn test_win32_basic_layouts() {
        assert_eq!(size_of::<SYSTEMTIME>(), 16);
        assert_eq!(size_of::<EXCEPTION_RECORD>(), 152);
        assert_eq!(offset_of!(EXCEPTION_RECORD, NumberParameters), 24);
        assert_eq!(offset_of!(EXCEPTION_RECORD, ExceptionInformation), 32);
        assert_eq!(size_of::<URL_COMPONENTSA>(), 104);
        assert_eq!(offset_of!(URL_COMPONENTSA, nScheme), 20);
        assert_eq!(offset_of!(URL_COMPONENTSA, lpszHostName), 24);
        assert_eq!(offset_of!(URL_COMPONENTSA, nPort), 36);
        assert_eq!(offset_of!(URL_COMPONENTSA, lpszUrlPath), 72);
        assert_eq!(size_of::<BITMAPINFOHEADER>(), 40);
        assert_eq!(size_of::<BITMAPINFO>(), 44);
        assert_eq!(size_of::<TEXTMETRICW>(), 60);
        assert_eq!(offset_of!(TEXTMETRICW, tmInternalLeading), 12);
        assert_eq!(offset_of!(TEXTMETRICW, tmAveCharWidth), 20);
        assert_eq!(offset_of!(TEXTMETRICW, tmCharSet), 56);
    }
}
