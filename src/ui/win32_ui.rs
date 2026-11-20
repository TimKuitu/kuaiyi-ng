//! UI 层 Windows API、DWM、UxTheme 与 GDI+ Flat API FFI 绑定

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use crate::core::str_util::{utf8_to_wide, wide_to_utf8};
pub use crate::core::win32::{
    get_b_value, get_g_value, get_r_value, rgb, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BOOL, BYTE,
    CLEARTYPE_QUALITY, COLORREF, DEFAULT_CHARSET, DIB_RGB_COLORS, DWORD, ETO_CLIPPED, FW_NORMAL,
    GUID, HALFTONE, HANDLE, HBITMAP, HBRUSH, HDC, HFONT, HGDIOBJ, HMODULE, HRESULT,
    INVALID_HANDLE_VALUE, LONG, POINT, RECT, RGBQUAD, SIZE, SRCCOPY, TEXTMETRICW, TRANSPARENT,
    UINT, WCHAR, WORD,
};
pub use crate::core::win32::{
    CloseHandle, CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DeleteDC,
    DeleteObject, ExtTextOutW, FillRect, GdiFlush, GetDC, GetLastError, GetProcAddress,
    GetTextExtentPoint32W, GetTextMetricsW, GetTickCount64, LoadLibraryA, ReleaseDC, SelectObject,
    SetBkMode, SetBrushOrgEx, SetStretchBltMode, SetTextColor, StretchBlt,
};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::path::Path;
use std::ptr::{copy_nonoverlapping, null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub type WPARAM = usize;
pub type LPARAM = isize;
pub type LRESULT = isize;
pub type ATOM = u16;
pub type HINSTANCE = *mut c_void;
pub type HICON = *mut c_void;
pub type HCURSOR = *mut c_void;
pub type HPEN = *mut c_void;
pub type HRGN = *mut c_void;
pub type HMENU = *mut c_void;
pub type HMONITOR = *mut c_void;
pub type LPCWSTR = *const u16;
pub type LPWSTR = *mut u16;
pub type LPCSTR = *const u8;
pub type UINT_PTR = usize;
pub type DWORD_PTR = usize;
pub type HKEY = *mut c_void;

pub type WNDPROC = unsafe extern "system" fn(HWND_TYPE, UINT, WPARAM, LPARAM) -> LRESULT;
pub type SUBCLASSPROC =
    unsafe extern "system" fn(HWND_TYPE, UINT, WPARAM, LPARAM, UINT_PTR, DWORD_PTR) -> LRESULT;
pub type WNDENUMPROC = unsafe extern "system" fn(HWND_TYPE, LPARAM) -> BOOL;
pub type MONITORENUMPROC = unsafe extern "system" fn(HMONITOR, HDC, *mut RECT, LPARAM) -> BOOL;
pub type BFFCALLBACK = unsafe extern "system" fn(HWND_TYPE, UINT, LPARAM, LPARAM) -> i32;

pub type HWND = crate::core::win32::HWND;
type HWND_TYPE = HWND;

// ---- 窗口类、样式与常用消息常量 ----

pub const CS_VREDRAW: UINT = 0x0001;
pub const CS_HREDRAW: UINT = 0x0002;
pub const CS_DBLCLKS: UINT = 0x0008;

pub const WS_OVERLAPPED: DWORD = 0x0000_0000;
pub const WS_POPUP: DWORD = 0x8000_0000;
pub const WS_CHILD: DWORD = 0x4000_0000;
pub const WS_MINIMIZE: DWORD = 0x2000_0000;
pub const WS_VISIBLE: DWORD = 0x1000_0000;
pub const WS_DISABLED: DWORD = 0x0800_0000;
pub const WS_CLIPSIBLINGS: DWORD = 0x0400_0000;
pub const WS_CLIPCHILDREN: DWORD = 0x0200_0000;
pub const WS_CAPTION: DWORD = 0x00C0_0000;
pub const WS_BORDER: DWORD = 0x0080_0000;
pub const WS_SYSMENU: DWORD = 0x0008_0000;
pub const WS_THICKFRAME: DWORD = 0x0004_0000;
pub const WS_TABSTOP: DWORD = 0x0001_0000;
pub const WS_MINIMIZEBOX: DWORD = 0x0002_0000;
pub const WS_MAXIMIZEBOX: DWORD = 0x0001_0000;
pub const WS_VSCROLL: DWORD = 0x0020_0000;

pub const WS_EX_TOPMOST: DWORD = 0x0000_0008;
pub const WS_EX_TRANSPARENT: DWORD = 0x0000_0020;
pub const WS_EX_TOOLWINDOW: DWORD = 0x0000_0080;
pub const WS_EX_CONTROLPARENT: DWORD = 0x0001_0000;
pub const WS_EX_APPWINDOW: DWORD = 0x0004_0000;
pub const WS_EX_LAYERED: DWORD = 0x0008_0000;
pub const WS_EX_NOACTIVATE: DWORD = 0x0800_0000;

pub const ES_MULTILINE: DWORD = 0x0004;
pub const ES_AUTOVSCROLL: DWORD = 0x0040;
pub const ES_AUTOHSCROLL: DWORD = 0x0080;
pub const ES_READONLY: DWORD = 0x0800;
pub const ES_WANTRETURN: DWORD = 0x1000;
pub const ES_PASSWORD: DWORD = 0x0020;
pub const ES_NUMBER: DWORD = 0x2000;
pub const EN_CHANGE: WORD = 0x0300;
pub const EN_SETFOCUS: WORD = 0x0100;
pub const EN_KILLFOCUS: WORD = 0x0200;

pub const CBS_DROPDOWNLIST: DWORD = 0x0003;
pub const CBS_HASSTRINGS: DWORD = 0x0200;
pub const CBN_SELCHANGE: WORD = 1;
pub const CB_ADDSTRING: UINT = 0x0143;
pub const CB_GETCURSEL: UINT = 0x0147;
pub const CB_SETCURSEL: UINT = 0x014E;
pub const CB_RESETCONTENT: UINT = 0x014B;
pub const CB_SETITEMHEIGHT: UINT = 0x0153;

pub const SS_LEFT: DWORD = 0x0000;
pub const SS_NOTIFY: DWORD = 0x0100;
pub const SS_ETCHEDHORZ: DWORD = 0x0010;
pub const SBS_VERT: DWORD = 0x0001;

pub const CW_USEDEFAULT: i32 = i32::MIN;

// 窗口消息
pub const WM_NULL: UINT = 0x0000;
pub const WM_CREATE: UINT = 0x0001;
pub const WM_DESTROY: UINT = 0x0002;
pub const WM_MOVE: UINT = 0x0003;
pub const WM_SIZE: UINT = 0x0005;
pub const WM_ACTIVATE: UINT = 0x0006;
pub const WM_SETFOCUS: UINT = 0x0007;
pub const WM_KILLFOCUS: UINT = 0x0008;
pub const WM_ENABLE: UINT = 0x000A;
pub const WM_SETREDRAW: UINT = 0x000B;
pub const WM_SETTEXT: UINT = 0x000C;
pub const WM_GETTEXT: UINT = 0x000D;
pub const WM_GETTEXTLENGTH: UINT = 0x000E;
pub const WM_PAINT: UINT = 0x000F;
pub const WM_CLOSE: UINT = 0x0010;
pub const WM_QUIT: UINT = 0x0012;
pub const WM_ERASEBKGND: UINT = 0x0014;
pub const WM_SHOWWINDOW: UINT = 0x0018;
pub const WM_SETTINGCHANGE: UINT = 0x001A;
pub const WM_ACTIVATEAPP: UINT = 0x001C;
pub const WM_SETCURSOR: UINT = 0x0020;
pub const WM_MOUSEACTIVATE: UINT = 0x0021;
pub const WM_GETMINMAXINFO: UINT = 0x0024;
pub const WM_SETFONT: UINT = 0x0030;
pub const WM_GETICON: UINT = 0x007F;
pub const WM_SETICON: UINT = 0x0080;
pub const WM_NCCREATE: UINT = 0x0081;
pub const WM_NCDESTROY: UINT = 0x0082;
pub const WM_NCCALCSIZE: UINT = 0x0083;
pub const WM_NCHITTEST: UINT = 0x0084;
pub const WM_NCPAINT: UINT = 0x0085;
pub const WM_NCACTIVATE: UINT = 0x0086;
pub const WM_NCLBUTTONDBLCLK: UINT = 0x00A3;
pub const WM_KEYDOWN: UINT = 0x0100;
pub const WM_KEYUP: UINT = 0x0101;
pub const WM_CHAR: UINT = 0x0102;
pub const WM_SYSKEYDOWN: UINT = 0x0104;
pub const WM_SYSKEYUP: UINT = 0x0105;
pub const WM_COMMAND: UINT = 0x0111;
pub const WM_SYSCOMMAND: UINT = 0x0112;
pub const WM_TIMER: UINT = 0x0113;
pub const WM_VSCROLL: UINT = 0x0115;
pub const WM_CTLCOLOREDIT: UINT = 0x0133;
pub const WM_CTLCOLORLISTBOX: UINT = 0x0134;
pub const WM_CTLCOLORSTATIC: UINT = 0x0138;
pub const WM_MOUSEMOVE: UINT = 0x0200;
pub const WM_LBUTTONDOWN: UINT = 0x0201;
pub const WM_LBUTTONUP: UINT = 0x0202;
pub const WM_LBUTTONDBLCLK: UINT = 0x0203;
pub const WM_RBUTTONDOWN: UINT = 0x0204;
pub const WM_RBUTTONUP: UINT = 0x0205;
pub const WM_RBUTTONDBLCLK: UINT = 0x0206;
pub const WM_MBUTTONDOWN: UINT = 0x0207;
pub const WM_MOUSEWHEEL: UINT = 0x020A;
pub const WM_CAPTURECHANGED: UINT = 0x0215;
pub const WM_DPICHANGED: UINT = 0x02E0;
pub const WM_MOUSELEAVE: UINT = 0x02A3;
pub const WM_HOTKEY: UINT = 0x0312;
pub const WM_PRINT: UINT = 0x0317;
pub const WM_THEMECHANGED: UINT = 0x031A;
pub const WM_USER: UINT = 0x0400;
pub const WM_APP: UINT = 0x8000;

pub const EM_SETSEL: UINT = 0x00B1;
pub const EM_SETPASSWORDCHAR: UINT = 0x00CC;
pub const EM_SETMARGINS: UINT = 0x00D3;
pub const EC_LEFTMARGIN: WPARAM = 0x0001;
pub const EC_RIGHTMARGIN: WPARAM = 0x0002;
pub const EM_SETBKGNDCOLOR: UINT = WM_USER + 67;
pub const EM_SETCHARFORMAT: UINT = WM_USER + 68;
pub const SCF_ALL: WPARAM = 0x0004;
pub const CFM_COLOR: DWORD = 0x4000_0000;
pub const CFM_FACE: DWORD = 0x2000_0000;
pub const CFM_SIZE: DWORD = 0x8000_0000;
pub const CFE_AUTOCOLOR: DWORD = 0x4000_0000;

// 命中测试
pub const HTNOWHERE: LRESULT = 0;
pub const HTCLIENT: LRESULT = 1;
pub const HTCAPTION: LRESULT = 2;
pub const HTLEFT: LRESULT = 10;
pub const HTRIGHT: LRESULT = 11;
pub const HTTOP: LRESULT = 12;
pub const HTTOPLEFT: LRESULT = 13;
pub const HTTOPRIGHT: LRESULT = 14;
pub const HTBOTTOM: LRESULT = 15;
pub const HTBOTTOMLEFT: LRESULT = 16;
pub const HTBOTTOMRIGHT: LRESULT = 17;

pub const MA_NOACTIVATE: LRESULT = 3;
pub const SIZE_MINIMIZED: WPARAM = 1;
pub const SC_MAXIMIZE: WPARAM = 0xF030;
pub const SC_RESTORE: WPARAM = 0xF120;

// ShowWindow / SetWindowPos
pub const SW_HIDE: i32 = 0;
pub const SW_SHOWNORMAL: i32 = 1;
pub const SW_SHOWMINIMIZED: i32 = 2;
pub const SW_SHOW: i32 = 5;
pub const SW_MINIMIZE: i32 = 6;
pub const SW_SHOWNA: i32 = 8;
pub const SW_RESTORE: i32 = 9;

pub const HWND_TOP: HWND = 0isize as HWND;
pub const HWND_TOPMOST: HWND = (-1isize) as HWND;
pub const HWND_NOTOPMOST: HWND = (-2isize) as HWND;

pub const SWP_NOSIZE: UINT = 0x0001;
pub const SWP_NOMOVE: UINT = 0x0002;
pub const SWP_NOZORDER: UINT = 0x0004;
pub const SWP_NOACTIVATE: UINT = 0x0010;
pub const SWP_FRAMECHANGED: UINT = 0x0020;
pub const SWP_SHOWWINDOW: UINT = 0x0040;
pub const SWP_HIDEWINDOW: UINT = 0x0080;

pub const GWL_STYLE: i32 = -16;
pub const GWL_EXSTYLE: i32 = -20;
pub const GWLP_USERDATA: i32 = -21;

pub const RDW_INVALIDATE: UINT = 0x0001;
pub const RDW_ERASE: UINT = 0x0004;
pub const RDW_UPDATENOW: UINT = 0x0100;
pub const RDW_FRAME: UINT = 0x0400;
pub const RDW_ALLCHILDREN: UINT = 0x0080;

pub const GA_ROOT: UINT = 2;
pub const LWA_ALPHA: DWORD = 0x0000_0002;
pub const ULW_ALPHA: DWORD = 0x0000_0002;
pub const CAPTUREBLT: DWORD = 0x4000_0000;
pub const AC_SRC_OVER: BYTE = 0x00;
pub const AC_SRC_ALPHA: BYTE = 0x01;

// 光标、图标与系统指标
pub const IDC_ARROW: LPCWSTR = 32512usize as LPCWSTR;
pub const IDC_IBEAM: LPCWSTR = 32513usize as LPCWSTR;
pub const IDC_HAND: LPCWSTR = 32649usize as LPCWSTR;
pub const IDC_CROSS: LPCWSTR = 32515usize as LPCWSTR;
pub const IDC_SIZEALL: LPCWSTR = 32646usize as LPCWSTR;
pub const IDC_SIZENWSE: LPCWSTR = 32642usize as LPCWSTR;
pub const IDC_SIZENESW: LPCWSTR = 32643usize as LPCWSTR;
pub const IDC_SIZEWE: LPCWSTR = 32644usize as LPCWSTR;
pub const IDC_SIZENS: LPCWSTR = 32645usize as LPCWSTR;

pub const IMAGE_ICON: UINT = 1;
pub const LR_DEFAULTSIZE: UINT = 0x0040;
pub const LR_SHARED: UINT = 0x8000;
pub const ICON_SMALL: WPARAM = 0;
pub const ICON_BIG: WPARAM = 1;

pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;
pub const SM_CXVSCROLL: i32 = 2;
pub const SM_XVIRTUALSCREEN: i32 = 76;
pub const SM_YVIRTUALSCREEN: i32 = 77;
pub const SM_CXVIRTUALSCREEN: i32 = 78;
pub const SM_CYVIRTUALSCREEN: i32 = 79;

pub const MONITOR_DEFAULTTONEAREST: DWORD = 2;
pub const TME_LEAVE: DWORD = 0x0000_0002;

// 键盘与虚拟键码
pub const VK_BACK: i32 = 0x08;
pub const VK_TAB: i32 = 0x09;
pub const VK_RETURN: i32 = 0x0D;
pub const VK_SHIFT: i32 = 0x10;
pub const VK_CONTROL: i32 = 0x11;
pub const VK_MENU: i32 = 0x12;
pub const VK_ESCAPE: i32 = 0x1B;
pub const VK_SPACE: i32 = 0x20;
pub const VK_PRIOR: i32 = 0x21;
pub const VK_NEXT: i32 = 0x22;
pub const VK_END: i32 = 0x23;
pub const VK_HOME: i32 = 0x24;
pub const VK_LEFT: i32 = 0x25;
pub const VK_UP: i32 = 0x26;
pub const VK_RIGHT: i32 = 0x27;
pub const VK_DOWN: i32 = 0x28;
pub const VK_DELETE: i32 = 0x2E;
pub const VK_LWIN: i32 = 0x5B;
pub const VK_RWIN: i32 = 0x5C;

pub const MOD_ALT: UINT = 0x0001;
pub const MOD_CONTROL: UINT = 0x0002;
pub const MOD_SHIFT: UINT = 0x0004;
pub const MOD_WIN: UINT = 0x0008;
pub const MOD_NOREPEAT: UINT = 0x4000;

// GDI 绘制
pub const FW_MEDIUM: i32 = 500;
pub const FW_SEMIBOLD: i32 = 600;
pub const FW_BOLD: i32 = 700;
pub const PS_SOLID: i32 = 0;
pub const PS_DASH: i32 = 1;
pub const NULL_BRUSH: i32 = 5;
pub const COLORONCOLOR: i32 = 3;
pub const BLACKNESS: DWORD = 0x0000_0042;

pub const DT_TOP: UINT = 0x0000;
pub const DT_LEFT: UINT = 0x0000;
pub const DT_CENTER: UINT = 0x0001;
pub const DT_RIGHT: UINT = 0x0002;
pub const DT_VCENTER: UINT = 0x0004;
pub const DT_WORDBREAK: UINT = 0x0010;
pub const DT_SINGLELINE: UINT = 0x0020;
pub const DT_CALCRECT: UINT = 0x0400;
pub const DT_NOPREFIX: UINT = 0x0800;
pub const DT_END_ELLIPSIS: UINT = 0x8000;

// 滚动条
pub const SB_CTL: i32 = 2;
pub const SB_LINEUP: WORD = 0;
pub const SB_LINEDOWN: WORD = 1;
pub const SB_PAGEUP: WORD = 2;
pub const SB_PAGEDOWN: WORD = 3;
pub const SB_THUMBPOSITION: WORD = 4;
pub const SB_THUMBTRACK: WORD = 5;
pub const SIF_RANGE: UINT = 0x0001;
pub const SIF_PAGE: UINT = 0x0002;
pub const SIF_POS: UINT = 0x0004;
pub const SIF_ALL: UINT = SIF_RANGE | SIF_PAGE | SIF_POS | 0x0010;

// 菜单与托盘
pub const MF_STRING: UINT = 0x0000;
pub const MF_SEPARATOR: UINT = 0x0800;
pub const MF_CHECKED: UINT = 0x0008;
pub const MF_GRAYED: UINT = 0x0001;
pub const TPM_RIGHTBUTTON: UINT = 0x0002;
pub const TPM_RETURNCMD: UINT = 0x0100;
pub const TPM_NONOTIFY: UINT = 0x0080;

pub const NIM_ADD: DWORD = 0x0000_0000;
pub const NIM_MODIFY: DWORD = 0x0000_0001;
pub const NIM_DELETE: DWORD = 0x0000_0002;
pub const NIF_MESSAGE: UINT = 0x0000_0001;
pub const NIF_ICON: UINT = 0x0000_0002;
pub const NIF_TIP: UINT = 0x0000_0004;
pub const NIF_INFO: UINT = 0x0000_0010;
pub const NIIF_INFO: DWORD = 0x0000_0001;

// DWM 属性
pub const DWMWA_EXTENDED_FRAME_BOUNDS: DWORD = 9;
pub const DWMWA_CLOAKED: DWORD = 14;
pub const DWMWA_USE_IMMERSIVE_DARK_MODE: DWORD = 20;
pub const DWMWA_WINDOW_CORNER_PREFERENCE: DWORD = 33;
pub const DWMWCP_ROUND: i32 = 2;

// MessageBox & Clipboard & FileDialog & Common Controls
pub const MB_OK: UINT = 0x0000_0000;
pub const MB_YESNO: UINT = 0x0000_0004;
pub const MB_ICONINFORMATION: UINT = 0x0000_0040;
pub const MB_ICONWARNING: UINT = 0x0000_0030;
pub const MB_ICONERROR: UINT = 0x0000_0010;
pub const IDYES: i32 = 6;

pub const CF_DIB: UINT = 8;
pub const CF_UNICODETEXT: UINT = 13;
pub const GMEM_MOVEABLE: UINT = 0x0002;

pub const ICC_STANDARD_CLASSES: DWORD = 0x0000_4000;
pub const ICC_WIN95_CLASSES: DWORD = 0x0000_00FF;
pub const ICC_HOTKEY_CLASS: DWORD = 0x0000_0040;

pub const TOOLTIPS_CLASSW_STR: &str = "tooltips_class32";
pub const TTM_ACTIVATE: UINT = WM_USER + 1;
pub const TTM_ADDTOOLW: UINT = WM_USER + 50;
pub const TTM_SETMAXTIPWIDTH: UINT = WM_USER + 24;
pub const TTF_IDISHWND: UINT = 0x0001;
pub const TTF_SUBCLASS: UINT = 0x0010;

pub const OFN_OVERWRITEPROMPT: DWORD = 0x0000_0002;
pub const OFN_PATHMUSTEXIST: DWORD = 0x0000_0800;
pub const OFN_NOCHANGEDIR: DWORD = 0x0000_0008;

pub const BIF_RETURNONLYFSDIRS: UINT = 0x0001;
pub const BIF_NEWDIALOGSTYLE: UINT = 0x0040;
pub const BFFM_INITIALIZED: UINT = 1;
pub const BFFM_SETSELECTIONW: UINT = WM_USER + 103;

// 注册表与进程快照
pub const HKEY_CURRENT_USER: HKEY = 0x8000_0001usize as HKEY;
pub const KEY_READ: DWORD = 0x0002_0019;
pub const KEY_WRITE: DWORD = 0x0002_0006;
pub const REG_SZ: DWORD = 1;
pub const REG_DWORD: DWORD = 4;
pub const ERROR_SUCCESS: LONG = 0;

pub const TH32CS_SNAPPROCESS: DWORD = 0x0000_0002;
pub const PROCESS_TERMINATE: DWORD = 0x0001;
pub const PROCESS_SYNCHRONIZE: DWORD = 0x0010_0000;
pub const ERROR_ALREADY_EXISTS: DWORD = 183;
pub const WAIT_TIMEOUT: DWORD = 0x0000_0102;

// GDI+ 枚举
pub const GP_OK: i32 = 0;
pub const GP_UNIT_PIXEL: i32 = 2;
pub const GP_SMOOTHING_ANTIALIAS: i32 = 4;
pub const GP_PIXEL_OFFSET_HALF: i32 = 4;
pub const GP_TEXT_RENDERING_CLEARTYPE: i32 = 5;
pub const GP_LINE_CAP_ROUND: i32 = 2;
pub const GP_LINE_JOIN_ROUND: i32 = 2;
pub const GP_FONT_STYLE_REGULAR: i32 = 0;
pub const GP_FONT_STYLE_BOLD: i32 = 1;
pub const GP_STRING_ALIGN_NEAR: i32 = 0;
pub const GP_STRING_ALIGN_CENTER: i32 = 1;
pub const GP_STRING_ALIGN_FAR: i32 = 2;
pub const GP_PIXEL_FORMAT_32BPP_ARGB: i32 = 0x0026_200A;

pub const PNG_ENCODER_CLSID: GUID = GUID {
    Data1: 0x557C_F406,
    Data2: 0x1A04,
    Data3: 0x11D3,
    Data4: [0x9A, 0x73, 0x00, 0x00, 0xF8, 0x1E, 0xF3, 0x2E],
};

// ---- 结构体定义 ----

#[repr(C)]
#[derive(Clone, Copy)]
pub struct WNDCLASSEXW {
    pub cbSize: UINT,
    pub style: UINT,
    pub lpfnWndProc: Option<WNDPROC>,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: HINSTANCE,
    pub hIcon: HICON,
    pub hCursor: HCURSOR,
    pub hbrBackground: HBRUSH,
    pub lpszMenuName: LPCWSTR,
    pub lpszClassName: LPCWSTR,
    pub hIconSm: HICON,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: UINT,
    pub wParam: WPARAM,
    pub lParam: LPARAM,
    pub time: DWORD,
    pub pt: POINT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PAINTSTRUCT {
    pub hdc: HDC,
    pub fErase: BOOL,
    pub rcPaint: RECT,
    pub fRestore: BOOL,
    pub fIncUpdate: BOOL,
    pub rgbReserved: [BYTE; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MONITORINFO {
    pub cbSize: DWORD,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MINMAXINFO {
    pub ptReserved: POINT,
    pub ptMaxSize: POINT,
    pub ptMaxPosition: POINT,
    pub ptMinTrackSize: POINT,
    pub ptMaxTrackSize: POINT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TRACKMOUSEEVENT {
    pub cbSize: DWORD,
    pub dwFlags: DWORD,
    pub hwndTrack: HWND,
    pub dwHoverTime: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct INITCOMMONCONTROLSEX {
    pub dwSize: DWORD,
    pub dwICC: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NOTIFYICONDATAW {
    pub cbSize: DWORD,
    pub hWnd: HWND,
    pub uID: UINT,
    pub uFlags: UINT,
    pub uCallbackMessage: UINT,
    pub hIcon: HICON,
    pub szTip: [WCHAR; 128],
    pub dwState: DWORD,
    pub dwStateMask: DWORD,
    pub szInfo: [WCHAR; 256],
    pub uTimeout_or_uVersion: UINT,
    pub szInfoTitle: [WCHAR; 64],
    pub dwInfoFlags: DWORD,
    pub guidItem: GUID,
    pub hBalloonIcon: HICON,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OPENFILENAMEW {
    pub lStructSize: DWORD,
    pub hwndOwner: HWND,
    pub hInstance: HINSTANCE,
    pub lpstrFilter: LPCWSTR,
    pub lpstrCustomFilter: LPWSTR,
    pub nMaxCustFilter: DWORD,
    pub nFilterIndex: DWORD,
    pub lpstrFile: LPWSTR,
    pub nMaxFile: DWORD,
    pub lpstrFileTitle: LPWSTR,
    pub nMaxFileTitle: DWORD,
    pub lpstrInitialDir: LPCWSTR,
    pub lpstrTitle: LPCWSTR,
    pub Flags: DWORD,
    pub nFileOffset: u16,
    pub nFileExtension: u16,
    pub lpstrDefExt: LPCWSTR,
    pub lCustData: LPARAM,
    pub lpfnHook: *const c_void,
    pub lpTemplateName: LPCWSTR,
    pub pvReserved: *mut c_void,
    pub dwReserved: DWORD,
    pub FlagsEx: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TOOLINFOW {
    pub cbSize: UINT,
    pub uFlags: UINT,
    pub hwnd: HWND,
    pub uId: UINT_PTR,
    pub rect: RECT,
    pub hinst: HINSTANCE,
    pub lpszText: LPWSTR,
    pub lParam: LPARAM,
    pub lpReserved: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CHARFORMATW {
    pub cbSize: UINT,
    pub dwMask: DWORD,
    pub dwEffects: DWORD,
    pub yHeight: LONG,
    pub yOffset: LONG,
    pub crTextColor: COLORREF,
    pub bCharSet: BYTE,
    pub bPitchAndFamily: BYTE,
    pub szFaceName: [WCHAR; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BLENDFUNCTION {
    pub BlendOp: BYTE,
    pub BlendFlags: BYTE,
    pub SourceConstantAlpha: BYTE,
    pub AlphaFormat: BYTE,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SCROLLINFO {
    pub cbSize: UINT,
    pub fMask: UINT,
    pub nMin: i32,
    pub nMax: i32,
    pub nPage: UINT,
    pub nPos: i32,
    pub nTrackPos: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BROWSEINFOW {
    pub hwndOwner: HWND,
    pub pidlRoot: *const c_void,
    pub pszDisplayName: LPWSTR,
    pub lpszTitle: LPCWSTR,
    pub ulFlags: UINT,
    pub lpfn: Option<BFFCALLBACK>,
    pub lParam: LPARAM,
    pub iImage: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PROCESSENTRY32W {
    pub dwSize: DWORD,
    pub cntUsage: DWORD,
    pub th32ProcessID: DWORD,
    pub th32DefaultHeapID: usize,
    pub th32ModuleID: DWORD,
    pub cntThreads: DWORD,
    pub th32ParentProcessID: DWORD,
    pub pcPriClassBase: LONG,
    pub dwFlags: DWORD,
    pub szExeFile: [WCHAR; 260],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GdiplusStartupInput {
    pub GdiplusVersion: u32,
    pub DebugEventCallback: *const c_void,
    pub SuppressBackgroundThread: BOOL,
    pub SuppressExternalCodecs: BOOL,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpPointF {
    pub x: f32,
    pub y: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GpRectF {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BITMAPINFO_V5_HEADER {
    pub bV5Size: DWORD,
    pub bV5Width: LONG,
    pub bV5Height: LONG,
    pub bV5Planes: u16,
    pub bV5BitCount: u16,
    pub bV5Compression: DWORD,
    pub bV5SizeImage: DWORD,
    pub bV5XPelsPerMeter: LONG,
    pub bV5YPelsPerMeter: LONG,
    pub bV5ClrUsed: DWORD,
    pub bV5ClrImportant: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SECURITY_ATTRIBUTES {
    pub nLength: DWORD,
    pub lpSecurityDescriptor: *mut c_void,
    pub bInheritHandle: BOOL,
}

// ---- 系统 DLL 导入 ----

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(lpModuleName: LPCWSTR) -> HMODULE;
    pub fn GetModuleFileNameW(hModule: HMODULE, lpFilename: LPWSTR, nSize: DWORD) -> DWORD;
    pub fn GetCurrentProcessId() -> DWORD;
    pub fn CreateMutexW(
        lpMutexAttributes: *const SECURITY_ATTRIBUTES,
        bInitialOwner: BOOL,
        lpName: LPCWSTR,
    ) -> HANDLE;
    pub fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
    pub fn TerminateProcess(hProcess: HANDLE, uExitCode: UINT) -> BOOL;
    pub fn CreateToolhelp32Snapshot(dwFlags: DWORD, th32ProcessID: DWORD) -> HANDLE;
    pub fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    pub fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    pub fn GlobalAlloc(uFlags: UINT, dwBytes: usize) -> HANDLE;
    pub fn GlobalLock(hMem: HANDLE) -> *mut c_void;
    pub fn GlobalUnlock(hMem: HANDLE) -> BOOL;
    pub fn GlobalFree(hMem: HANDLE) -> HANDLE;
    pub fn GlobalSize(hMem: HANDLE) -> usize;
    pub fn MulDiv(nNumber: i32, nNumerator: i32, nDenominator: i32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    pub fn SetProcessDPIAware() -> BOOL;
    pub fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> ATOM;
    pub fn CreateWindowExW(
        dwExStyle: DWORD,
        lpClassName: LPCWSTR,
        lpWindowName: LPCWSTR,
        dwStyle: DWORD,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    pub fn DestroyWindow(hWnd: HWND) -> BOOL;
    pub fn DefWindowProcW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn CallWindowProcW(
        lpPrevWndFunc: *const c_void,
        hWnd: HWND,
        Msg: UINT,
        wParam: WPARAM,
        lParam: LPARAM,
    ) -> LRESULT;
    pub fn GetWindowLongPtrW(hWnd: HWND, nIndex: i32) -> isize;
    pub fn SetWindowLongPtrW(hWnd: HWND, nIndex: i32, dwNewLong: isize) -> isize;
    pub fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    pub fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> BOOL;
    pub fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: BOOL) -> BOOL;
    pub fn UpdateWindow(hWnd: HWND) -> BOOL;
    pub fn RedrawWindow(hWnd: HWND, lprcUpdate: *const RECT, hrgnUpdate: HRGN, flags: UINT)
        -> BOOL;
    pub fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> BOOL;
    pub fn SetWindowPos(
        hWnd: HWND,
        hWndInsertAfter: HWND,
        X: i32,
        Y: i32,
        cx: i32,
        cy: i32,
        uFlags: UINT,
    ) -> BOOL;
    pub fn MoveWindow(
        hWnd: HWND,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        bRepaint: BOOL,
    ) -> BOOL;
    pub fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn GetWindowRect(hWnd: HWND, lpRect: *mut RECT) -> BOOL;
    pub fn ScreenToClient(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    pub fn ClientToScreen(hWnd: HWND, lpPoint: *mut POINT) -> BOOL;
    pub fn EnableWindow(hWnd: HWND, bEnable: BOOL) -> BOOL;
    pub fn IsWindow(hWnd: HWND) -> BOOL;
    pub fn IsWindowVisible(hWnd: HWND) -> BOOL;
    pub fn IsIconic(hWnd: HWND) -> BOOL;
    pub fn SetFocus(hWnd: HWND) -> HWND;
    pub fn GetFocus() -> HWND;
    pub fn SetCapture(hWnd: HWND) -> HWND;
    pub fn ReleaseCapture() -> BOOL;
    pub fn SetCursor(hCursor: HCURSOR) -> HCURSOR;
    pub fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: LPCWSTR) -> HCURSOR;
    pub fn LoadImageW(
        hInst: HINSTANCE,
        name: LPCWSTR,
        type_: UINT,
        cx: i32,
        cy: i32,
        fuLoad: UINT,
    ) -> HANDLE;
    pub fn LoadIconW(hInstance: HINSTANCE, lpIconName: LPCWSTR) -> HICON;
    pub fn CreateIconFromResourceEx(
        presbits: *const u8,
        dwResSize: DWORD,
        fIcon: BOOL,
        dwVer: DWORD,
        cxDesired: i32,
        cyDesired: i32,
        Flags: UINT,
    ) -> HICON;
    pub fn LookupIconIdFromDirectoryEx(
        presbits: *const u8,
        fIcon: BOOL,
        cxDesired: i32,
        cyDesired: i32,
        Flags: UINT,
    ) -> i32;
    pub fn DestroyIcon(hIcon: HICON) -> BOOL;
    pub fn GetCursorPos(lpPoint: *mut POINT) -> BOOL;
    pub fn TrackMouseEvent(lpEventTrack: *mut TRACKMOUSEEVENT) -> BOOL;
    pub fn SetTimer(
        hWnd: HWND,
        nIDEvent: UINT_PTR,
        uElapse: UINT,
        lpTimerFunc: *const c_void,
    ) -> UINT_PTR;
    pub fn KillTimer(hWnd: HWND, uIDEvent: UINT_PTR) -> BOOL;
    pub fn SetLayeredWindowAttributes(
        hwnd: HWND,
        crKey: COLORREF,
        bAlpha: BYTE,
        dwFlags: DWORD,
    ) -> BOOL;
    pub fn EnumWindows(lpEnumFunc: WNDENUMPROC, lParam: LPARAM) -> BOOL;
    pub fn EnumChildWindows(hWndParent: HWND, lpEnumFunc: WNDENUMPROC, lParam: LPARAM) -> BOOL;
    pub fn GetWindowTextW(hWnd: HWND, lpString: LPWSTR, nMaxCount: i32) -> i32;
    pub fn GetWindowTextLengthW(hWnd: HWND) -> i32;
    pub fn SetWindowTextW(hWnd: HWND, lpString: LPCWSTR) -> BOOL;
    pub fn GetClassNameW(hWnd: HWND, lpClassName: LPWSTR, nMaxCount: i32) -> i32;
    pub fn GetWindowThreadProcessId(hWnd: HWND, lpdwProcessId: *mut DWORD) -> DWORD;
    pub fn FindWindowW(lpClassName: LPCWSTR, lpWindowName: LPCWSTR) -> HWND;
    pub fn GetAncestor(hwnd: HWND, gaFlags: UINT) -> HWND;
    pub fn GetParent(hWnd: HWND) -> HWND;
    pub fn AttachThreadInput(idAttach: DWORD, idAttachTo: DWORD, fAttach: BOOL) -> BOOL;
    pub fn SetForegroundWindow(hWnd: HWND) -> BOOL;
    pub fn BringWindowToTop(hWnd: HWND) -> BOOL;
    pub fn GetForegroundWindow() -> HWND;
    pub fn OpenClipboard(hWndNewOwner: HWND) -> BOOL;
    pub fn CloseClipboard() -> BOOL;
    pub fn EmptyClipboard() -> BOOL;
    pub fn GetClipboardData(uFormat: UINT) -> HANDLE;
    pub fn SetClipboardData(uFormat: UINT, hMem: HANDLE) -> HANDLE;
    pub fn IsClipboardFormatAvailable(format: UINT) -> BOOL;
    pub fn RegisterHotKey(hWnd: HWND, id: i32, fsModifiers: UINT, vk: UINT) -> BOOL;
    pub fn UnregisterHotKey(hWnd: HWND, id: i32) -> BOOL;
    pub fn GetKeyState(nVirtKey: i32) -> i16;
    pub fn GetAsyncKeyState(vKey: i32) -> i16;
    pub fn MapVirtualKeyW(uCode: UINT, uMapType: UINT) -> UINT;
    pub fn GetKeyNameTextW(lParam: LONG, lpString: LPWSTR, cchSize: i32) -> i32;
    pub fn CreatePopupMenu() -> HMENU;
    pub fn DestroyMenu(hMenu: HMENU) -> BOOL;
    pub fn AppendMenuW(
        hMenu: HMENU,
        uFlags: UINT,
        uIDNewItem: UINT_PTR,
        lpNewItem: LPCWSTR,
    ) -> BOOL;
    pub fn TrackPopupMenu(
        hMenu: HMENU,
        uFlags: UINT,
        x: i32,
        y: i32,
        nReserved: i32,
        hWnd: HWND,
        prcRect: *const RECT,
    ) -> BOOL;
    pub fn PostMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> BOOL;
    pub fn SendMessageW(hWnd: HWND, Msg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    pub fn PostQuitMessage(nExitCode: i32);
    pub fn GetMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
    ) -> BOOL;
    pub fn PeekMessageW(
        lpMsg: *mut MSG,
        hWnd: HWND,
        wMsgFilterMin: UINT,
        wMsgFilterMax: UINT,
        wRemoveMsg: UINT,
    ) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    pub fn MessageBoxW(hWnd: HWND, lpText: LPCWSTR, lpCaption: LPCWSTR, uType: UINT) -> i32;
    pub fn GetSystemMetrics(nIndex: i32) -> i32;
    pub fn MonitorFromPoint(pt: POINT, dwFlags: DWORD) -> HMONITOR;
    pub fn MonitorFromWindow(hwnd: HWND, dwFlags: DWORD) -> HMONITOR;
    pub fn GetMonitorInfoW(hMonitor: HMONITOR, lpmi: *mut MONITORINFO) -> BOOL;
    pub fn EnumDisplayMonitors(
        hdc: HDC,
        lprcClip: *const RECT,
        lpfnEnum: MONITORENUMPROC,
        dwData: LPARAM,
    ) -> BOOL;
    pub fn SetScrollInfo(hwnd: HWND, nBar: i32, lpsi: *const SCROLLINFO, redraw: BOOL) -> i32;
    pub fn GetScrollInfo(hwnd: HWND, nBar: i32, lpsi: *mut SCROLLINFO) -> BOOL;
    pub fn ShowScrollBar(hWnd: HWND, wBar: i32, bShow: BOOL) -> BOOL;
    pub fn DrawTextW(
        hdc: HDC,
        lpchText: LPCWSTR,
        cchText: i32,
        lprc: *mut RECT,
        format: UINT,
    ) -> i32;
    pub fn FrameRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    pub fn WindowFromPoint(Point: POINT) -> HWND;
    pub fn GetDlgCtrlID(hwnd: HWND) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateCompatibleBitmap(hdc: HDC, cx: i32, cy: i32) -> HBITMAP;
    pub fn CreatePen(iStyle: i32, cWidth: i32, color: COLORREF) -> HPEN;
    pub fn GetStockObject(i: i32) -> HGDIOBJ;
    pub fn SetBkColor(hdc: HDC, color: COLORREF) -> COLORREF;
    pub fn BitBlt(
        hdc: HDC,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        hdcSrc: HDC,
        x1: i32,
        y1: i32,
        rop: DWORD,
    ) -> BOOL;
    pub fn Rectangle(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
    pub fn Ellipse(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) -> BOOL;
    pub fn RoundRect(
        hdc: HDC,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> BOOL;
    pub fn MoveToEx(hdc: HDC, x: i32, y: i32, lppt: *mut POINT) -> BOOL;
    pub fn LineTo(hdc: HDC, x: i32, y: i32) -> BOOL;
    pub fn CreateRectRgn(x1: i32, y1: i32, x2: i32, y2: i32) -> HRGN;
    pub fn SelectClipRgn(hdc: HDC, hrgn: HRGN) -> i32;
    pub fn GetDeviceCaps(hdc: HDC, index: i32) -> i32;
}

#[link(name = "msimg32")]
extern "system" {
    pub fn AlphaBlend(
        hdcDest: HDC,
        xoriginDest: i32,
        yoriginDest: i32,
        wDest: i32,
        hDest: i32,
        hdcSrc: HDC,
        xoriginSrc: i32,
        yoriginSrc: i32,
        wSrc: i32,
        hSrc: i32,
        ftn: BLENDFUNCTION,
    ) -> BOOL;
}

#[link(name = "dwmapi")]
extern "system" {
    pub fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwAttribute: DWORD,
        pvAttribute: *const c_void,
        cbAttribute: DWORD,
    ) -> HRESULT;
    pub fn DwmGetWindowAttribute(
        hwnd: HWND,
        dwAttribute: DWORD,
        pvAttribute: *mut c_void,
        cbAttribute: DWORD,
    ) -> HRESULT;
}

#[link(name = "comctl32")]
extern "system" {
    pub fn InitCommonControlsEx(picce: *const INITCOMMONCONTROLSEX) -> BOOL;
    pub fn SetWindowSubclass(
        hWnd: HWND,
        pfnSubclass: SUBCLASSPROC,
        uIdSubclass: UINT_PTR,
        dwRefData: DWORD_PTR,
    ) -> BOOL;
    pub fn RemoveWindowSubclass(
        hWnd: HWND,
        pfnSubclass: SUBCLASSPROC,
        uIdSubclass: UINT_PTR,
    ) -> BOOL;
    pub fn DefSubclassProc(hWnd: HWND, uMsg: UINT, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
}

#[link(name = "winmm")]
extern "system" {
    pub fn timeBeginPeriod(uPeriod: UINT) -> UINT;
    pub fn timeEndPeriod(uPeriod: UINT) -> UINT;
}

#[link(name = "shell32")]
extern "system" {
    pub fn Shell_NotifyIconW(dwMessage: DWORD, lpData: *const NOTIFYICONDATAW) -> BOOL;
    pub fn ShellExecuteW(
        hwnd: HWND,
        lpOperation: LPCWSTR,
        lpFile: LPCWSTR,
        lpParameters: LPCWSTR,
        lpDirectory: LPCWSTR,
        nShowCmd: i32,
    ) -> HINSTANCE;
    pub fn SHBrowseForFolderW(lpbi: *mut BROWSEINFOW) -> *mut c_void;
    pub fn SHGetPathFromIDListW(pidl: *const c_void, pszPath: LPWSTR) -> BOOL;
}

#[link(name = "ole32")]
extern "system" {
    pub fn CoTaskMemFree(pv: *mut c_void);
}

#[link(name = "comdlg32")]
extern "system" {
    pub fn GetSaveFileNameW(lpofn: *mut OPENFILENAMEW) -> BOOL;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegOpenKeyExW(
        hKey: HKEY,
        lpSubKey: LPCWSTR,
        ulOptions: DWORD,
        samDesired: DWORD,
        phkResult: *mut HKEY,
    ) -> LONG;
    pub fn RegCloseKey(hKey: HKEY) -> LONG;
    pub fn RegQueryValueExW(
        hKey: HKEY,
        lpValueName: LPCWSTR,
        lpReserved: *mut DWORD,
        lpType: *mut DWORD,
        lpData: *mut BYTE,
        lpcbData: *mut DWORD,
    ) -> LONG;
    pub fn RegSetValueExW(
        hKey: HKEY,
        lpValueName: LPCWSTR,
        Reserved: DWORD,
        dwType: DWORD,
        lpData: *const BYTE,
        cbData: DWORD,
    ) -> LONG;
    pub fn RegDeleteValueW(hKey: HKEY, lpValueName: LPCWSTR) -> LONG;
}

#[link(name = "gdiplus")]
extern "system" {
    pub fn GdiplusStartup(
        token: *mut usize,
        input: *const GdiplusStartupInput,
        output: *mut c_void,
    ) -> i32;
    pub fn GdiplusShutdown(token: usize);
    pub fn GdipCreateFromHDC(hdc: HDC, graphics: *mut *mut c_void) -> i32;
    pub fn GdipDeleteGraphics(graphics: *mut c_void) -> i32;
    pub fn GdipSetSmoothingMode(graphics: *mut c_void, smoothingMode: i32) -> i32;
    pub fn GdipSetPixelOffsetMode(graphics: *mut c_void, pixelOffsetMode: i32) -> i32;
    pub fn GdipSetTextRenderingHint(graphics: *mut c_void, mode: i32) -> i32;
    pub fn GdipCreateSolidFill(color: u32, brush: *mut *mut c_void) -> i32;
    pub fn GdipDeleteBrush(brush: *mut c_void) -> i32;
    pub fn GdipCreatePen1(color: u32, width: f32, unit: i32, pen: *mut *mut c_void) -> i32;
    pub fn GdipSetPenStartCap(pen: *mut c_void, startCap: i32) -> i32;
    pub fn GdipSetPenEndCap(pen: *mut c_void, endCap: i32) -> i32;
    pub fn GdipSetPenLineJoin(pen: *mut c_void, lineJoin: i32) -> i32;
    pub fn GdipDeletePen(pen: *mut c_void) -> i32;
    pub fn GdipCreatePath(brushMode: i32, path: *mut *mut c_void) -> i32;
    pub fn GdipDeletePath(path: *mut c_void) -> i32;
    pub fn GdipAddPathArc(
        path: *mut c_void,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        startAngle: f32,
        sweepAngle: f32,
    ) -> i32;
    pub fn GdipClosePathFigure(path: *mut c_void) -> i32;
    pub fn GdipFillPath(graphics: *mut c_void, brush: *mut c_void, path: *mut c_void) -> i32;
    pub fn GdipDrawPath(graphics: *mut c_void, pen: *mut c_void, path: *mut c_void) -> i32;
    pub fn GdipFillRectangle(
        graphics: *mut c_void,
        brush: *mut c_void,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> i32;
    pub fn GdipDrawRectangle(
        graphics: *mut c_void,
        pen: *mut c_void,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> i32;
    pub fn GdipFillEllipse(
        graphics: *mut c_void,
        brush: *mut c_void,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> i32;
    pub fn GdipDrawEllipse(
        graphics: *mut c_void,
        pen: *mut c_void,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> i32;
    pub fn GdipDrawLine(
        graphics: *mut c_void,
        pen: *mut c_void,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    ) -> i32;
    pub fn GdipDrawLines(
        graphics: *mut c_void,
        pen: *mut c_void,
        points: *const GpPointF,
        count: i32,
    ) -> i32;
    pub fn GdipCreateFontFamilyFromName(
        name: LPCWSTR,
        fontCollection: *mut c_void,
        fontFamily: *mut *mut c_void,
    ) -> i32;
    pub fn GdipDeleteFontFamily(fontFamily: *mut c_void) -> i32;
    pub fn GdipCreateFont(
        fontFamily: *mut c_void,
        emSize: f32,
        style: i32,
        unit: i32,
        font: *mut *mut c_void,
    ) -> i32;
    pub fn GdipDeleteFont(font: *mut c_void) -> i32;
    pub fn GdipCreateStringFormat(
        formatAttributes: i32,
        language: u16,
        format: *mut *mut c_void,
    ) -> i32;
    pub fn GdipSetStringFormatAlign(format: *mut c_void, align: i32) -> i32;
    pub fn GdipSetStringFormatLineAlign(format: *mut c_void, align: i32) -> i32;
    pub fn GdipDeleteStringFormat(format: *mut c_void) -> i32;
    pub fn GdipDrawString(
        graphics: *mut c_void,
        string: LPCWSTR,
        length: i32,
        font: *const c_void,
        layoutRect: *const GpRectF,
        stringFormat: *const c_void,
        brush: *const c_void,
    ) -> i32;
    pub fn GdipCreateBitmapFromScan0(
        width: i32,
        height: i32,
        stride: i32,
        format: i32,
        scan0: *mut u8,
        bitmap: *mut *mut c_void,
    ) -> i32;
    pub fn GdipDisposeImage(image: *mut c_void) -> i32;
    pub fn GdipSaveImageToFile(
        image: *mut c_void,
        filename: LPCWSTR,
        clsidEncoder: *const GUID,
        encoderParams: *const c_void,
    ) -> i32;
}

// ---- 全局 GDI+ 会话与安全封装函数 ----

static GP_READY: AtomicBool = AtomicBool::new(false);
static GP_TOKEN: AtomicUsize = AtomicUsize::new(0);

/// 合成 GDI+ ARGB 颜色值
#[inline]
pub const fn argb(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// 拆分 `lParam` 中的有符号 X/Y 坐标
#[inline]
pub const fn lparam_xy(lp: LPARAM) -> (i32, i32) {
    let x = ((lp as usize) & 0xFFFF) as i16 as i32;
    let y = (((lp as usize) >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

/// 提取 `wParam` 中的有符号滚轮增量（通常每格 ±120）
#[inline]
pub const fn wheel_delta(wp: WPARAM) -> i32 {
    ((wp >> 16) & 0xFFFF) as i16 as i32
}

/// 按需初始化 GDI+
pub fn ensure_gdiplus() -> bool {
    if GP_READY.load(Ordering::Acquire) {
        return true;
    }
    let input = GdiplusStartupInput {
        GdiplusVersion: 1,
        DebugEventCallback: null(),
        SuppressBackgroundThread: 0,
        SuppressExternalCodecs: 0,
    };
    let mut token = 0usize;
    // SAFETY: input 为有效栈结构体，token 用于接收 GDI+ 实例句柄
    let st = unsafe { GdiplusStartup(&mut token, &input, null_mut()) };
    if st == GP_OK {
        GP_TOKEN.store(token, Ordering::Release);
        GP_READY.store(true, Ordering::Release);
        true
    } else {
        false
    }
}

/// 释放 GDI+ 运行时
pub fn shutdown_gdiplus() {
    if GP_READY.swap(false, Ordering::AcqRel) {
        let tok = GP_TOKEN.swap(0, Ordering::AcqRel);
        if tok != 0 {
            // SAFETY: tok 来自成功的 GdiplusStartup 调用
            unsafe { GdiplusShutdown(tok) };
        }
    }
}

/// 优先启用 Per-Monitor V2 DPI 感知，确保混合 DPI 多屏下返回真实物理坐标
unsafe fn enable_per_monitor_dpi_awareness() {
    type PfnSetProcessDpiAwarenessContext = unsafe extern "system" fn(*mut c_void) -> BOOL;
    type PfnSetProcessDpiAwareness = unsafe extern "system" fn(i32) -> i32;

    let h_u32 = LoadLibraryA(c"user32.dll".as_ptr().cast());
    if !h_u32.is_null() {
        let p = GetProcAddress(h_u32, c"SetProcessDpiAwarenessContext".as_ptr().cast());
        if !p.is_null() {
            let f: PfnSetProcessDpiAwarenessContext = std::mem::transmute(p);
            if f((-4isize) as *mut c_void) != 0 || f((-3isize) as *mut c_void) != 0 {
                return;
            }
        }
    }

    let h_shcore = LoadLibraryA(c"shcore.dll".as_ptr().cast());
    if !h_shcore.is_null() {
        let p = GetProcAddress(h_shcore, c"SetProcessDpiAwareness".as_ptr().cast());
        if !p.is_null() {
            let f: PfnSetProcessDpiAwareness = std::mem::transmute(p);
            let hr = f(2); // PROCESS_PER_MONITOR_DPI_AWARE
            if hr == 0 || hr == (0x8007_0005u32 as i32) {
                return;
            }
        }
    }

    SetProcessDPIAware();
}

/// 初始化通用控件库（标准控件与热键控件）及 RichEdit 控件库
pub fn init_ui_libraries() {
    let icc = INITCOMMONCONTROLSEX {
        dwSize: size_of::<INITCOMMONCONTROLSEX>() as DWORD,
        dwICC: ICC_WIN95_CLASSES | ICC_STANDARD_CLASSES | ICC_HOTKEY_CLASS,
    };
    // SAFETY: 进程启动初期先设置 Per-Monitor V2 DPI 感知，再初始化公共控件库与 RichEdit
    unsafe {
        enable_per_monitor_dpi_awareness();
        InitCommonControlsEx(&icc);
        LoadLibraryA(c"Msftedit.dll".as_ptr().cast());
    }
}

/// 构造 GDI+ 圆角矩形路径
///
/// # Safety
/// 调用方负责在绘图完成后使用 `GdipDeletePath` 释放返回的非空路径句柄。
pub unsafe fn make_round_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> *mut c_void {
    let mut path: *mut c_void = null_mut();
    if GdipCreatePath(0, &mut path) != GP_OK || path.is_null() {
        return null_mut();
    }
    let rr = r.min(w * 0.5).min(h * 0.5);
    if rr <= 0.5 {
        let pts = [
            GpPointF { x, y },
            GpPointF { x: x + w, y },
            GpPointF { x: x + w, y: y + h },
            GpPointF { x, y: y + h },
        ];
        let _ = pts.len();
        GdipAddPathArc(path, x, y, 0.1, 0.1, 180.0, 90.0);
        GdipAddPathArc(path, x + w - 0.1, y, 0.1, 0.1, 270.0, 90.0);
        GdipAddPathArc(path, x + w - 0.1, y + h - 0.1, 0.1, 0.1, 0.0, 90.0);
        GdipAddPathArc(path, x, y + h - 0.1, 0.1, 0.1, 90.0, 90.0);
    } else {
        let d = rr * 2.0;
        GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
        GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
        GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
        GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    }
    GdipClosePathFigure(path);
    path
}

/// 使用 GDI+ 填充平滑圆角矩形
///
/// # Safety
/// `g` 必须为有效的 GDI+ `GpGraphics` 句柄指针。
pub unsafe fn gp_fill_round_rect(g: *mut c_void, x: f32, y: f32, w: f32, h: f32, r: f32, col: u32) {
    if g.is_null() || w <= 0.0 || h <= 0.0 {
        return;
    }
    let path = make_round_rect_path(x, y, w, h, r);
    if path.is_null() {
        return;
    }
    let mut br: *mut c_void = null_mut();
    if GdipCreateSolidFill(col, &mut br) == GP_OK && !br.is_null() {
        GdipFillPath(g, br, path);
        GdipDeleteBrush(br);
    }
    GdipDeletePath(path);
}

/// 使用 GDI+ 绘制平滑圆角矩形边框
///
/// # Safety
/// `g` 必须为有效的 GDI+ `GpGraphics` 句柄指针。
#[allow(clippy::too_many_arguments)]
pub unsafe fn gp_draw_round_rect(
    g: *mut c_void,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    col: u32,
    width: f32,
) {
    if g.is_null() || w <= 0.0 || h <= 0.0 {
        return;
    }
    let path = make_round_rect_path(x, y, w, h, r);
    if path.is_null() {
        return;
    }
    let mut pen: *mut c_void = null_mut();
    if GdipCreatePen1(col, width, GP_UNIT_PIXEL, &mut pen) == GP_OK && !pen.is_null() {
        GdipDrawPath(g, pen, path);
        GdipDeletePen(pen);
    }
    GdipDeletePath(path);
}

/// 使用 GDI+ 绘制圆角矩形并可选描边
///
/// # Safety
/// `g` 必须为有效的 GDI+ `GpGraphics` 句柄指针。
#[allow(clippy::too_many_arguments)]
pub unsafe fn gp_round_card(
    g: *mut c_void,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    fill_col: u32,
    border_col: u32,
    border_w: f32,
) {
    gp_fill_round_rect(g, x, y, w, h, r, fill_col);
    if (border_col >> 24) > 0 && border_w > 0.0 {
        gp_draw_round_rect(g, x, y, w, h, r, border_col, border_w);
    }
}

/// 使用 GDI+ 绘制居中或对齐文本
///
/// # Safety
/// `g` 必须为有效的 GDI+ `GpGraphics` 句柄指针。
#[allow(clippy::too_many_arguments)]
pub unsafe fn gp_draw_text(
    g: *mut c_void,
    text: &str,
    rc: &GpRectF,
    font_name: &str,
    px: f32,
    bold: bool,
    col: u32,
    h_align: i32,
    v_align: i32,
) {
    if g.is_null() || text.is_empty() {
        return;
    }
    let wtext = utf8_to_wide(text);
    let wfont = utf8_to_wide(font_name);
    let mut fam: *mut c_void = null_mut();
    if GdipCreateFontFamilyFromName(wfont.as_ptr(), null_mut(), &mut fam) != GP_OK || fam.is_null()
    {
        let fallback = utf8_to_wide("Segoe UI");
        let _ = GdipCreateFontFamilyFromName(fallback.as_ptr(), null_mut(), &mut fam);
    }
    if fam.is_null() {
        return;
    }
    let mut font: *mut c_void = null_mut();
    let style = if bold {
        GP_FONT_STYLE_BOLD
    } else {
        GP_FONT_STYLE_REGULAR
    };
    if GdipCreateFont(fam, px.max(8.0), style, GP_UNIT_PIXEL, &mut font) == GP_OK && !font.is_null()
    {
        let mut fmt: *mut c_void = null_mut();
        if GdipCreateStringFormat(0, 0, &mut fmt) == GP_OK && !fmt.is_null() {
            GdipSetStringFormatAlign(fmt, h_align);
            GdipSetStringFormatLineAlign(fmt, v_align);
            let mut br: *mut c_void = null_mut();
            if GdipCreateSolidFill(col, &mut br) == GP_OK && !br.is_null() {
                GdipDrawString(g, wtext.as_ptr(), -1, font, rc, fmt, br);
                GdipDeleteBrush(br);
            }
            GdipDeleteStringFormat(fmt);
        }
        GdipDeleteFont(font);
    }
    GdipDeleteFontFamily(fam);
}

/// 使用 GDI+ 从 UTF-16 单字符绘制图标字形（如 Segoe MDL2 Assets）
///
/// # Safety
/// `g` 必须为有效的 GDI+ `GpGraphics` 句柄指针。
#[allow(clippy::too_many_arguments)]
pub unsafe fn gp_draw_glyph(
    g: *mut c_void,
    codepoint: u16,
    rc: &GpRectF,
    font_name: &str,
    px: f32,
    col: u32,
) {
    if g.is_null() {
        return;
    }
    let buf = [codepoint, 0u16];
    let wfont = utf8_to_wide(font_name);
    let mut fam: *mut c_void = null_mut();
    if GdipCreateFontFamilyFromName(wfont.as_ptr(), null_mut(), &mut fam) != GP_OK || fam.is_null()
    {
        return;
    }
    let mut font: *mut c_void = null_mut();
    if GdipCreateFont(fam, px, GP_FONT_STYLE_REGULAR, GP_UNIT_PIXEL, &mut font) == GP_OK
        && !font.is_null()
    {
        let mut fmt: *mut c_void = null_mut();
        if GdipCreateStringFormat(0, 0, &mut fmt) == GP_OK && !fmt.is_null() {
            GdipSetStringFormatAlign(fmt, GP_STRING_ALIGN_CENTER);
            GdipSetStringFormatLineAlign(fmt, GP_STRING_ALIGN_CENTER);
            let mut br: *mut c_void = null_mut();
            if GdipCreateSolidFill(col, &mut br) == GP_OK && !br.is_null() {
                GdipDrawString(g, buf.as_ptr(), 1, font, rc, fmt, br);
                GdipDeleteBrush(br);
            }
            GdipDeleteStringFormat(fmt);
        }
        GdipDeleteFont(font);
    }
    GdipDeleteFontFamily(fam);
}

/// 动态调用 `dwmapi::DwmSetWindowAttribute` 启用 Win10/11 标题栏暗色与圆角
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn apply_dwm_window_style(hwnd: HWND, dark: bool, round: bool) {
    if hwnd.is_null() {
        return;
    }
    let dark_val: BOOL = if dark { 1 } else { 0 };
    // SAFETY: hwnd 有效，dark_val 长度为 4 字节 BOOL
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark_val as *const BOOL).cast(),
            size_of::<BOOL>() as DWORD,
        );
        if round {
            let pref: i32 = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&pref as *const i32).cast(),
                size_of::<i32>() as DWORD,
            );
        }
    }
}

/// 动态启用/更新 `uxtheme` 深色控件主题（用于原生 ComboBox / ScrollBar）
pub fn apply_uxtheme_dark_app_mode(dark: bool) {
    // SAFETY: uxtheme.dll 为 Windows 系统 DLL；序号 135/136 仅当地址非空时调用
    unsafe {
        let h_ux = LoadLibraryA(c"uxtheme.dll".as_ptr().cast());
        if h_ux.is_null() {
            return;
        }
        let ord_mode = 135usize as *const u8;
        let p_mode = GetProcAddress(h_ux, ord_mode);
        if !p_mode.is_null() {
            type FnSetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
            let f: FnSetPreferredAppMode = std::mem::transmute(p_mode);
            // 1 = AllowDark, 3 = ForceLight
            let _ = f(if dark { 1 } else { 3 });
        }
        let ord_flush = 136usize as *const u8;
        let p_flush = GetProcAddress(h_ux, ord_flush);
        if !p_flush.is_null() {
            type FnFlushMenuThemes = unsafe extern "system" fn();
            let f: FnFlushMenuThemes = std::mem::transmute(p_flush);
            f();
        }
    }
}

/// 对子控件调用 `uxtheme::SetWindowTheme`
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn set_control_theme(hwnd: HWND, sub_app_name: &str) {
    if hwnd.is_null() {
        return;
    }
    // SAFETY: 动态解析 SetWindowTheme，不存在或系统较旧时安全跳过
    unsafe {
        let h_ux = LoadLibraryA(c"uxtheme.dll".as_ptr().cast());
        if h_ux.is_null() {
            return;
        }
        let p_fn = GetProcAddress(h_ux, c"SetWindowTheme".as_ptr().cast());
        if !p_fn.is_null() {
            type FnSetWindowTheme = unsafe extern "system" fn(HWND, LPCWSTR, LPCWSTR) -> HRESULT;
            let f: FnSetWindowTheme = std::mem::transmute(p_fn);
            let wname = utf8_to_wide(sub_app_name);
            let _ = f(hwnd, wname.as_ptr(), null());
        }
    }
}

/// 读取窗口文本（UTF-8）
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn get_window_text(hwnd: HWND) -> String {
    if hwnd.is_null() {
        return String::new();
    }
    // SAFETY: GetWindowTextLengthW 与 GetWindowTextW 缓冲区按 len + 1 分配
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len as usize) + 2];
        let got = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if got <= 0 {
            return String::new();
        }
        wide_to_utf8(&buf[..got as usize])
    }
}

/// 设置窗口/控件文本（UTF-8 -> UTF-16）
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn set_window_text(hwnd: HWND, text: &str) {
    if hwnd.is_null() {
        return;
    }
    let w = utf8_to_wide(text);
    // SAFETY: w 以 NUL 结尾
    unsafe {
        SetWindowTextW(hwnd, w.as_ptr());
    }
}

/// 为 `RICHEDIT50W` 统一配置字体、颜色、边距与背景色
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn apply_richedit_style(hwnd: HWND, face: &str, px: f32, fg_col: COLORREF, bg_col: COLORREF) {
    if hwnd.is_null() {
        return;
    }
    let mut cf: CHARFORMATW = unsafe { zeroed() };
    cf.cbSize = size_of::<CHARFORMATW>() as UINT;
    cf.dwMask = CFM_FACE | CFM_SIZE | CFM_COLOR;
    cf.dwEffects = 0;
    cf.yHeight = (px * 15.0).round().max(120.0) as LONG;
    cf.crTextColor = fg_col;
    let wface = utf8_to_wide(face);
    let copy_len = wface.len().min(31);
    cf.szFaceName[..copy_len].copy_from_slice(&wface[..copy_len]);
    // SAFETY: CHARFORMATW 符合 Win32 EM_SETCHARFORMAT ABI，hwnd 为 RichEdit 句柄
    unsafe {
        SendMessageW(hwnd, EM_SETBKGNDCOLOR, 0, bg_col as LPARAM);
        SendMessageW(
            hwnd,
            EM_SETCHARFORMAT,
            SCF_ALL,
            (&cf as *const CHARFORMATW) as LPARAM,
        );
    }
}

/// 递归通知子控件刷新字体（保留字体生命周期由调用方管理）
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn apply_font_to_children(parent: HWND, hfont: HFONT) {
    if parent.is_null() || hfont.is_null() {
        return;
    }
    unsafe extern "system" fn cb(child: HWND, lp: LPARAM) -> BOOL {
        SendMessageW(child, WM_SETFONT, lp as WPARAM, 1);
        1
    }
    // SAFETY: hfont 指针值透传至同线程回调发送 WM_SETFONT
    unsafe {
        EnumChildWindows(parent, cb, hfont as LPARAM);
    }
}

static APP_ICO_BYTES: &[u8] = include_bytes!("../../res/kuaiyi.ico");

/// 从内存中的 `.ico` 数据匹配最接近尺寸的帧并创建 `HICON`
fn create_icon_from_ico_bytes(ico: &[u8], cx: i32, cy: i32) -> HICON {
    if ico.len() < 22 || u16::from_le_bytes([ico[2], ico[3]]) != 1 {
        return null_mut();
    }
    let count = u16::from_le_bytes([ico[4], ico[5]]) as usize;
    if count == 0 || ico.len() < 6 + count * 16 {
        return null_mut();
    }
    let want = cx.max(16);
    let mut best_idx = 0usize;
    let mut best_score = i64::MAX;
    for i in 0..count {
        let off = 6 + i * 16;
        let w = if ico[off] == 0 {
            256i32
        } else {
            ico[off] as i32
        };
        let bpp = u16::from_le_bytes([ico[off + 6], ico[off + 7]]) as i64;
        let diff = ((w - want).abs() as i64) * 1000 - bpp;
        if diff < best_score {
            best_score = diff;
            best_idx = i;
        }
    }
    let entry = 6 + best_idx * 16;
    let bytes_in_res = u32::from_le_bytes([
        ico[entry + 8],
        ico[entry + 9],
        ico[entry + 10],
        ico[entry + 11],
    ]) as usize;
    let img_off = u32::from_le_bytes([
        ico[entry + 12],
        ico[entry + 13],
        ico[entry + 14],
        ico[entry + 15],
    ]) as usize;
    if bytes_in_res == 0 || img_off.saturating_add(bytes_in_res) > ico.len() {
        return null_mut();
    }
    let slice = &ico[img_off..img_off + bytes_in_res];
    // SAFETY: slice 为有效 ICONDIRENTRY 指向的图像资源块（DIB 或 PNG 帧），版本传 0x00030000
    unsafe {
        CreateIconFromResourceEx(
            slice.as_ptr(),
            slice.len() as DWORD,
            1,
            0x0003_0000,
            cx,
            cy,
            0,
        )
    }
}

/// 加载应用图标（优先 PE 资源 `1 ICON`，若缺失则回退至内存内嵌的 `kuaiyi.ico`）
pub fn load_app_icon(cx: i32, cy: i32) -> HICON {
    // SAFETY: resource id 1 对应链接期嵌入 PE 资源的 app.res；未链接资源时使用内存兜底
    let h = unsafe {
        let h_inst = GetModuleHandleW(null());
        LoadImageW(h_inst, 1usize as LPCWSTR, IMAGE_ICON, cx, cy, LR_SHARED)
    };
    if !h.is_null() {
        return h;
    }
    create_icon_from_ico_bytes(APP_ICO_BYTES, cx, cy)
}

/// 获取当前可执行文件完整路径
pub fn current_exe_path() -> String {
    let mut buf = vec![0u16; 1024];
    // SAFETY: buf 长度充足
    let n = unsafe { GetModuleFileNameW(null_mut(), buf.as_mut_ptr(), buf.len() as DWORD) };
    if n == 0 {
        return String::new();
    }
    wide_to_utf8(&buf[..n as usize])
}

/// 获取主显示器工作区位置与尺寸，避免窗口落在屏幕外
pub fn primary_work_area() -> RECT {
    let mut mi = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as DWORD,
        ..Default::default()
    };
    // SAFETY: mi.cbSize 已设定，查询原点所在显示器
    unsafe {
        let hm = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTONEAREST);
        if !hm.is_null() && GetMonitorInfoW(hm, &mut mi) != 0 {
            return mi.rcWork;
        }
        RECT {
            left: 0,
            top: 0,
            right: GetSystemMetrics(SM_CXSCREEN).max(1024),
            bottom: GetSystemMetrics(SM_CYSCREEN).max(768),
        }
    }
}

/// 将坐标限制在鼠标或当前窗口所在显示器的工作区中
pub fn work_area_from_point(pt: POINT) -> RECT {
    let mut mi = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as DWORD,
        ..Default::default()
    };
    // SAFETY: MonitorFromPoint 与 GetMonitorInfoW 调用安全
    unsafe {
        let hm = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
        if !hm.is_null() && GetMonitorInfoW(hm, &mut mi) != 0 {
            return mi.rcWork;
        }
    }
    primary_work_area()
}

/// 单块在线显示器的完整物理边界与排除任务栏后的工作区边界
#[derive(Clone, Copy, Debug, Default)]
pub struct MonitorBounds {
    pub monitor: RECT,
    pub work: RECT,
}

/// 枚举所有在线物理显示器的完整矩形与工作区矩形（`rcMonitor` 与 `rcWork`）
pub fn enum_display_monitors_info() -> Vec<MonitorBounds> {
    unsafe extern "system" fn cb(
        hmon: HMONITOR,
        _hdc: HDC,
        _lprc: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let list = &mut *(data as *mut Vec<MonitorBounds>);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as DWORD,
            ..Default::default()
        };
        if GetMonitorInfoW(hmon, &mut mi) != 0
            && mi.rcMonitor.right > mi.rcMonitor.left
            && mi.rcMonitor.bottom > mi.rcMonitor.top
        {
            let work = if mi.rcWork.right > mi.rcWork.left && mi.rcWork.bottom > mi.rcWork.top {
                mi.rcWork
            } else {
                mi.rcMonitor
            };
            list.push(MonitorBounds {
                monitor: mi.rcMonitor,
                work,
            });
        }
        1
    }
    let mut list = Vec::new();
    // SAFETY: cb 写入栈上的 list，不越界不逃逸
    unsafe {
        EnumDisplayMonitors(
            null_mut(),
            null(),
            cb,
            (&mut list as *mut Vec<MonitorBounds>) as LPARAM,
        );
    }
    list
}

/// 枚举所有在线物理显示器的桌面坐标矩形
pub fn enum_display_monitor_rects() -> Vec<RECT> {
    enum_display_monitors_info()
        .into_iter()
        .map(|m| m.monitor)
        .collect()
}

/// 写入 UTF-8 文本到系统剪贴板
pub fn set_clipboard_text(text: &str) -> bool {
    let w = utf8_to_wide(text);
    let byte_len = w.len() * 2;
    // SAFETY: Win32 剪贴板标准协议，SetClipboardData 成功后所有权转交给系统
    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if h_mem.is_null() {
            CloseClipboard();
            return false;
        }
        let dst = GlobalLock(h_mem) as *mut u16;
        if dst.is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        copy_nonoverlapping(w.as_ptr(), dst, w.len());
        GlobalUnlock(h_mem);
        if SetClipboardData(CF_UNICODETEXT, h_mem).is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        CloseClipboard();
        true
    }
}

/// 从系统剪贴板读取文本
pub fn get_clipboard_text() -> Option<String> {
    // SAFETY: CF_UNICODETEXT 内存由剪贴板持有，解锁后立即关闭剪贴板
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return None;
        }
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let h = GetClipboardData(CF_UNICODETEXT);
        if h.is_null() {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(h) as *const u16;
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let cap_w = GlobalSize(h) / 2;
        let mut end = 0usize;
        while end < cap_w && *ptr.add(end) != 0 {
            end += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, end);
        let text = wide_to_utf8(slice);
        GlobalUnlock(h);
        CloseClipboard();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

/// 将 32-bit top-down BGRA 图像写入剪贴板 (`CF_DIB`)
pub fn set_clipboard_bgra(bgra: &[u8], w: i32, h: i32) -> bool {
    if w <= 0 || h <= 0 || bgra.len() < (w as usize) * (h as usize) * 4 {
        return false;
    }
    let hdr_sz = size_of::<BITMAPINFOHEADER>();
    let row_bytes = (w as usize) * 4;
    let total = hdr_sz + row_bytes * (h as usize);
    // SAFETY: 写入 bottom-up 32bpp BI_RGB DIB 以确保所有 Windows 应用兼容识别
    unsafe {
        if OpenClipboard(null_mut()) == 0 {
            return false;
        }
        EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, total);
        if h_mem.is_null() {
            CloseClipboard();
            return false;
        }
        let ptr = GlobalLock(h_mem) as *mut u8;
        if ptr.is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        let hdr = BITMAPINFOHEADER {
            biSize: hdr_sz as DWORD,
            biWidth: w,
            biHeight: h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: (row_bytes * (h as usize)) as DWORD,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        copy_nonoverlapping((&hdr as *const BITMAPINFOHEADER).cast::<u8>(), ptr, hdr_sz);
        let dst_pixels = ptr.add(hdr_sz);
        for y in 0..h as usize {
            let src_row = &bgra[y * row_bytes..(y + 1) * row_bytes];
            let dst_y = (h as usize) - 1 - y;
            copy_nonoverlapping(
                src_row.as_ptr(),
                dst_pixels.add(dst_y * row_bytes),
                row_bytes,
            );
        }
        GlobalUnlock(h_mem);
        if SetClipboardData(CF_DIB, h_mem).is_null() {
            GlobalFree(h_mem);
            CloseClipboard();
            return false;
        }
        CloseClipboard();
        true
    }
}

/// 从剪贴板读取 `CF_DIB` 并解码为 top-down 32-bit BGRA
pub fn get_clipboard_bgra() -> Option<(i32, i32, Vec<u8>)> {
    // SAFETY: 按 BITMAPINFOHEADER 解码 24bpp/32bpp（支持 top-down 与 bottom-up）
    unsafe {
        if IsClipboardFormatAvailable(CF_DIB) == 0 {
            return None;
        }
        if OpenClipboard(null_mut()) == 0 {
            return None;
        }
        let h = GetClipboardData(CF_DIB);
        if h.is_null() {
            CloseClipboard();
            return None;
        }
        let sz = GlobalSize(h);
        let ptr = GlobalLock(h) as *const u8;
        if ptr.is_null() || sz < size_of::<BITMAPINFOHEADER>() {
            if !ptr.is_null() {
                GlobalUnlock(h);
            }
            CloseClipboard();
            return None;
        }
        let raw = std::slice::from_raw_parts(ptr, sz);
        let decoded = decode_dib_bytes(raw);
        GlobalUnlock(h);
        CloseClipboard();
        decoded
    }
}

/// 将 `CF_DIB` 字节解析为 `(width, height, top_down_bgra)`
pub fn decode_dib_bytes(raw: &[u8]) -> Option<(i32, i32, Vec<u8>)> {
    if raw.len() < size_of::<BITMAPINFOHEADER>() {
        return None;
    }
    let bi_size = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
    if bi_size < 40 || bi_size > raw.len() {
        return None;
    }
    let w = i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]);
    let h_raw = i32::from_le_bytes([raw[8], raw[9], raw[10], raw[11]]);
    let bpp = u16::from_le_bytes([raw[14], raw[15]]);
    let compression = u32::from_le_bytes([raw[16], raw[17], raw[18], raw[19]]);
    let clr_used = u32::from_le_bytes([raw[32], raw[33], raw[34], raw[35]]) as usize;

    let top_down = h_raw < 0;
    let h = h_raw.abs();
    if w <= 0 || h <= 0 || w > 16384 || h > 16384 {
        return None;
    }

    let mask_bytes = if bi_size == 40 && compression == 3 {
        12
    } else {
        0
    };
    let palette_bytes = if bpp <= 8 {
        let entries = if clr_used > 0 {
            clr_used
        } else {
            1usize << bpp
        };
        entries * 4
    } else {
        clr_used * 4
    };
    let pixel_off = bi_size + mask_bytes + palette_bytes;
    if pixel_off > raw.len() {
        return None;
    }
    let pixels = &raw[pixel_off..];
    let stride = ((w as usize) * (bpp as usize)).div_ceil(32) * 4;
    if pixels.len() < stride * (h as usize) {
        return None;
    }

    let mut out = vec![0u8; (w as usize) * (h as usize) * 4];
    for y in 0..h as usize {
        let src_y = if top_down { y } else { (h as usize) - 1 - y };
        let src_row = &pixels[src_y * stride..src_y * stride + stride];
        let dst_row = &mut out[y * (w as usize) * 4..(y + 1) * (w as usize) * 4];
        match bpp {
            32 => {
                dst_row.copy_from_slice(&src_row[..(w as usize) * 4]);
                // 若 32bpp 全部 Alpha=0（常见于非透明截屏 DIB），补为不透明 255
                for px in dst_row.as_chunks_mut::<4>().0 {
                    if px[3] == 0 {
                        px[3] = 255;
                    }
                }
            }
            24 => {
                for x in 0..w as usize {
                    let s = &src_row[x * 3..x * 3 + 3];
                    let d = &mut dst_row[x * 4..x * 4 + 4];
                    d[0] = s[0];
                    d[1] = s[1];
                    d[2] = s[2];
                    d[3] = 255;
                }
            }
            _ => return None,
        }
    }
    Some((w, h, out))
}

/// 将 32-bit top-down BGRA 图像通过 GDI+ 保存为 PNG 文件
pub fn save_bgra_as_png<P: AsRef<Path>>(path: P, bgra: &[u8], w: i32, h: i32) -> bool {
    if w <= 0 || h <= 0 || bgra.len() < (w as usize) * (h as usize) * 4 {
        return false;
    }
    if !ensure_gdiplus() {
        return false;
    }
    if let Some(parent) = path.as_ref().parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let mut buf = bgra[..(w as usize) * (h as usize) * 4].to_vec();
    for px in buf.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    let wpath = utf8_to_wide(&path.as_ref().to_string_lossy());
    // SAFETY: buf 与 stride 按 width*4*height 严格对应，在 GdipDisposeImage 之前有效
    unsafe {
        let mut gp_bmp: *mut c_void = null_mut();
        let st = GdipCreateBitmapFromScan0(
            w,
            h,
            w * 4,
            GP_PIXEL_FORMAT_32BPP_ARGB,
            buf.as_mut_ptr(),
            &mut gp_bmp,
        );
        if st != GP_OK || gp_bmp.is_null() {
            return false;
        }
        let ok = GdipSaveImageToFile(gp_bmp, wpath.as_ptr(), &PNG_ENCODER_CLSID, null()) == GP_OK;
        GdipDisposeImage(gp_bmp);
        ok
    }
}

/// 打开 PNG 保存对话框
pub fn save_file_dialog_png(owner: HWND, default_dir: &str, default_name: &str) -> Option<String> {
    let mut file_buf = vec![0u16; 520];
    let wname = utf8_to_wide(default_name);
    let cp = wname.len().min(file_buf.len() - 1);
    file_buf[..cp].copy_from_slice(&wname[..cp]);

    let wfilter = utf8_to_wide("PNG 图像 (*.png)\0*.png\0所有文件 (*.*)\0*.*\0");
    let wdir = utf8_to_wide(default_dir);
    let wtitle = utf8_to_wide("保存截图");
    let wext = utf8_to_wide("png");

    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as DWORD,
        hwndOwner: owner,
        hInstance: null_mut(),
        lpstrFilter: wfilter.as_ptr(),
        lpstrCustomFilter: null_mut(),
        nMaxCustFilter: 0,
        nFilterIndex: 1,
        lpstrFile: file_buf.as_mut_ptr(),
        nMaxFile: file_buf.len() as DWORD,
        lpstrFileTitle: null_mut(),
        nMaxFileTitle: 0,
        lpstrInitialDir: if default_dir.is_empty() {
            null()
        } else {
            wdir.as_ptr()
        },
        lpstrTitle: wtitle.as_ptr(),
        Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        nFileOffset: 0,
        nFileExtension: 0,
        lpstrDefExt: wext.as_ptr(),
        lCustData: 0,
        lpfnHook: null(),
        lpTemplateName: null(),
        pvReserved: null_mut(),
        dwReserved: 0,
        FlagsEx: 0,
    };

    // SAFETY: 所有指针均在本函数栈帧期间有效
    unsafe {
        if GetSaveFileNameW(&mut ofn) != 0 {
            let end = file_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(file_buf.len());
            let path = wide_to_utf8(&file_buf[..end]);
            if !path.is_empty() {
                return Some(path);
            }
        }
    }
    None
}

/// 打开目录选择对话框
pub fn browse_for_folder(owner: HWND, title: &str, init_dir: &str) -> Option<String> {
    unsafe extern "system" fn browse_cb(hwnd: HWND, msg: UINT, _lp: LPARAM, data: LPARAM) -> i32 {
        if msg == BFFM_INITIALIZED && data != 0 {
            SendMessageW(hwnd, BFFM_SETSELECTIONW, 1, data);
        }
        0
    }

    let mut display = [0u16; 260];
    let wtitle = utf8_to_wide(title);
    let winit = utf8_to_wide(init_dir);
    let mut bi = BROWSEINFOW {
        hwndOwner: owner,
        pidlRoot: null(),
        pszDisplayName: display.as_mut_ptr(),
        lpszTitle: wtitle.as_ptr(),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        lpfn: Some(browse_cb),
        lParam: if init_dir.is_empty() {
            0
        } else {
            winit.as_ptr() as LPARAM
        },
        iImage: 0,
    };
    // SAFETY: BROWSEINFOW 各指针有效期覆盖整个模态调用
    unsafe {
        let pidl = SHBrowseForFolderW(&mut bi);
        if pidl.is_null() {
            return None;
        }
        let mut path_buf = [0u16; 520];
        let ok = SHGetPathFromIDListW(pidl, path_buf.as_mut_ptr()) != 0;
        CoTaskMemFree(pidl);
        if ok {
            let end = path_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(path_buf.len());
            let s = wide_to_utf8(&path_buf[..end]);
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

/// 用系统默认程序或浏览器打开文件/链接
pub fn shell_open(target: &str) {
    let op = utf8_to_wide("open");
    let wt = utf8_to_wide(target);
    // SAFETY: 以空父窗口调用 ShellExecuteW
    unsafe {
        ShellExecuteW(
            null_mut(),
            op.as_ptr(),
            wt.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
    }
}

/// 为控件附加悬停提示（Win32 `tooltips_class32`）
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn attach_tooltip(hwnd_tip: HWND, target: HWND, text: &str) {
    if hwnd_tip.is_null() || target.is_null() {
        return;
    }
    let mut wtext = utf8_to_wide(text);
    let ti = TOOLINFOW {
        // 使用 V2 尺寸（64 字节，不含 lpReserved）以兼容未带 ComCtl v6 清单的 EXE
        cbSize: (size_of::<TOOLINFOW>() - size_of::<*mut c_void>()) as UINT,
        uFlags: TTF_IDISHWND | TTF_SUBCLASS,
        hwnd: unsafe { GetParent(target) },
        uId: target as UINT_PTR,
        rect: RECT::default(),
        hinst: null_mut(),
        lpszText: wtext.as_mut_ptr(),
        lParam: 0,
        lpReserved: null_mut(),
    };
    // SAFETY: SendMessageW TTM_ADDTOOLW 内部同步复制文本
    unsafe {
        SendMessageW(
            hwnd_tip,
            TTM_ADDTOOLW,
            0,
            (&ti as *const TOOLINFOW) as LPARAM,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn test_win32_ui_struct_layouts() {
        assert_eq!(size_of::<WNDCLASSEXW>(), 80);
        assert_eq!(offset_of!(WNDCLASSEXW, lpfnWndProc), 8);
        assert_eq!(offset_of!(WNDCLASSEXW, lpszClassName), 64);

        assert_eq!(size_of::<MSG>(), 48);
        assert_eq!(offset_of!(MSG, message), 8);
        assert_eq!(offset_of!(MSG, pt), 36);

        assert_eq!(size_of::<PAINTSTRUCT>(), 72);
        assert_eq!(size_of::<MONITORINFO>(), 40);
        assert_eq!(size_of::<MINMAXINFO>(), 40);
        assert_eq!(size_of::<TRACKMOUSEEVENT>(), 24);
        assert_eq!(size_of::<BLENDFUNCTION>(), 4);
        assert_eq!(size_of::<SCROLLINFO>(), 28);
        assert_eq!(size_of::<CHARFORMATW>(), 92);
        assert_eq!(size_of::<BROWSEINFOW>(), 64);

        assert_eq!(size_of::<NOTIFYICONDATAW>(), 976);
        assert_eq!(offset_of!(NOTIFYICONDATAW, szTip), 40);
        assert_eq!(offset_of!(NOTIFYICONDATAW, szInfo), 304);
        assert_eq!(offset_of!(NOTIFYICONDATAW, guidItem), 952);

        assert_eq!(size_of::<OPENFILENAMEW>(), 152);
        assert_eq!(offset_of!(OPENFILENAMEW, lpstrFile), 48);
        assert_eq!(offset_of!(OPENFILENAMEW, Flags), 96);

        assert_eq!(size_of::<TOOLINFOW>(), 72);
        assert_eq!(offset_of!(TOOLINFOW, lpReserved), 64);

        assert_eq!(size_of::<PROCESSENTRY32W>(), 568);
        assert_eq!(offset_of!(PROCESSENTRY32W, szExeFile), 44);
    }

    #[test]
    fn test_decode_dib_24bpp_and_32bpp() {
        // 构造 2x1 的 24-bit DIB（每行 6 字节，补齐到 8 字节）
        let mut raw = vec![0u8; 40 + 8];
        raw[0..4].copy_from_slice(&40u32.to_le_bytes());
        raw[4..8].copy_from_slice(&2i32.to_le_bytes());
        raw[8..12].copy_from_slice(&1i32.to_le_bytes());
        raw[12..14].copy_from_slice(&1u16.to_le_bytes());
        raw[14..16].copy_from_slice(&24u16.to_le_bytes());
        // 像素 (B,G,R) = (10,20,30), (40,50,60)
        raw[40..46].copy_from_slice(&[10, 20, 30, 40, 50, 60]);
        let (w, h, bgra) = decode_dib_bytes(&raw).expect("decode 24bpp dib");
        assert_eq!((w, h), (2, 1));
        assert_eq!(bgra, vec![10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn test_load_app_icon_fallback() {
        let h16 = load_app_icon(16, 16);
        let h32 = load_app_icon(32, 32);
        assert!(!h16.is_null());
        assert!(!h32.is_null());
    }
}
