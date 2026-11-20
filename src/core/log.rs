//! 运行日志：fastT\run.log（UTF-8）
//!
//! 每次启动自动把上次的 `run.log` 轮换为 `run.last.log`；
//! 日志写入受 `Mutex` 保护以保证多线程安全；未处理异常过滤器采用无锁直写，
//! 防止崩溃正好发生在持锁线程上时二次加锁导致死锁。

use crate::core::str_util::utf8_to_wide;
use crate::core::win32::{
    CloseHandle, CreateFileW, GetCurrentThreadId, GetLocalTime, SetUnhandledExceptionFilter,
    WriteFile, EXCEPTION_CONTINUE_SEARCH, EXCEPTION_POINTERS, FILE_APPEND_DATA,
    FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, INVALID_HANDLE_VALUE, OPEN_ALWAYS,
    SYSTEMTIME,
};
use std::cell::UnsafeCell;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::{fence, AtomicBool, AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

struct LogState {
    path: Option<PathBuf>,
}

static LOG_MUTEX: LazyLock<Mutex<LogState>> = LazyLock::new(|| Mutex::new(LogState { path: None }));

/// 供异常过滤器无锁读取的日志文件路径（UTF-16 含结尾 `\0`）
struct CrashPathSlot {
    /// 0 表示未开启或正在更新；>0 表示 buf 中有效长度（含结尾 `\0`）
    ready_len: AtomicUsize,
    buf: UnsafeCell<[u16; 1024]>,
}

// SAFETY: 写端仅在持有 LOG_MUTEX 时修改 buf，并在前后通过 ready_len(Release) 与 SeqCst 栅栏同步；
// 读端仅在异常处理时读取 ready_len(Acquire) 并只读访问已发布的切片。
unsafe impl Sync for CrashPathSlot {}

static CRASH_PATH: CrashPathSlot = CrashPathSlot {
    ready_len: AtomicUsize::new(0),
    buf: UnsafeCell::new([0; 1024]),
};

static IN_CRASH: AtomicBool = AtomicBool::new(false);

fn publish_crash_path(path: Option<&Path>) {
    CRASH_PATH.ready_len.store(0, Ordering::Release);
    fence(Ordering::SeqCst);
    let Some(p) = path else {
        return;
    };
    let s = p.to_string_lossy();
    let wide = utf8_to_wide(&s);
    if wide.is_empty() || wide.len() > 1024 {
        return;
    }
    // SAFETY: 调用方已持有 LOG_MUTEX，且上面已先将 ready_len 置 0 并加 SeqCst 栅栏，
    // 此时不存在其他写者；只写入 wide.len() 个 u16 后再 Release 发布长度。
    unsafe {
        let dst = &mut *CRASH_PATH.buf.get();
        dst[..wide.len()].copy_from_slice(&wide);
    }
    CRASH_PATH.ready_len.store(wide.len(), Ordering::Release);
}

fn format_timestamp_prefix() -> String {
    let mut st = SYSTEMTIME::default();
    // SAFETY: GetLocalTime 写入有效栈上 SYSTEMTIME 结构体
    unsafe { GetLocalTime(&mut st) };
    format!(
        "[{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}]",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond, st.wMilliseconds
    )
}

fn write_line_raw(path: &Path, line: &str) {
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let ts = format_timestamp_prefix();
    let _ = writeln!(file, "{ts}{line}");
}

fn compute_rotated_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stem) = s.strip_suffix(".log") {
        PathBuf::from(format!("{stem}.last.log"))
    } else {
        PathBuf::from(format!("{s}.last"))
    }
}

/// 打开日志文件（自动把已有日志轮换为 `.last.log`）；传入空路径则关闭日志
pub fn log_open<P: AsRef<Path>>(path: P) {
    let p = path.as_ref();
    let mut guard = LOG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if p.as_os_str().is_empty() {
        guard.path = None;
        publish_crash_path(None);
        return;
    }
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = fs::create_dir_all(parent);
        }
    }
    if p.exists() {
        let last = compute_rotated_path(p);
        let _ = fs::rename(p, &last);
    }
    let pb = p.to_path_buf();
    guard.path = Some(pb.clone());
    publish_crash_path(Some(&pb));
    write_line_raw(&pb, "==== 会话开始 ====");
}

/// 写入一条带标签的运行日志（线程安全）
pub fn log_write(tag: &str, msg: &str) {
    let guard = LOG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let Some(ref path) = guard.path else {
        return;
    };
    let tag_str = if tag.is_empty() { "log" } else { tag };
    write_line_raw(path, &format!("[{tag_str}] {msg}"));
}

fn exc_name(code: u32) -> &'static str {
    match code {
        0xC000_0005 => "访问违例",
        0xC000_00FD => "栈溢出",
        0xC000_0409 => "栈保护/快速失败",
        0xC000_0094 => "除零",
        0xE06D_7363 => "C++异常",
        _ => "未知异常",
    }
}

/// 在栈上格式化数字到字节数组（零堆分配，适用于异常过滤器）
fn push_bytes(buf: &mut [u8], pos: &mut usize, s: &[u8]) {
    let avail = buf.len().saturating_sub(*pos);
    let n = s.len().min(avail);
    buf[*pos..*pos + n].copy_from_slice(&s[..n]);
    *pos += n;
}

fn push_dec_padded(buf: &mut [u8], pos: &mut usize, mut val: u64, width: usize) {
    let mut tmp = [b'0'; 24];
    let mut len = 0usize;
    if val == 0 {
        tmp[0] = b'0';
        len = 1;
    } else {
        while val > 0 && len < tmp.len() {
            tmp[len] = b'0' + (val % 10) as u8;
            val /= 10;
            len += 1;
        }
    }
    let total = len.max(width);
    for _ in len..total {
        push_bytes(buf, pos, b"0");
    }
    for i in (0..len).rev() {
        push_bytes(buf, pos, &[tmp[i]]);
    }
}

fn push_hex(buf: &mut [u8], pos: &mut usize, val: usize, min_width: usize) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut tmp = [b'0'; 16];
    let mut v = val;
    let mut len = 0usize;
    if v == 0 {
        tmp[0] = b'0';
        len = 1;
    } else {
        while v > 0 && len < tmp.len() {
            tmp[len] = HEX[v & 0x0f];
            v >>= 4;
            len += 1;
        }
    }
    let total = len.max(min_width);
    for _ in len..total {
        push_bytes(buf, pos, b"0");
    }
    for i in (0..len).rev() {
        push_bytes(buf, pos, &[tmp[i]]);
    }
}

/// 格式化未处理异常记录行（无锁无堆分配，方便单元测试直接校验格式）
fn format_crash_line(
    st: &SYSTEMTIME,
    code: u32,
    addr: usize,
    tid: u32,
    access_info: Option<(usize, usize)>,
    out: &mut [u8; 512],
) -> usize {
    let mut pos = 0usize;
    push_bytes(out, &mut pos, b"[");
    push_dec_padded(out, &mut pos, st.wYear as u64, 4);
    push_bytes(out, &mut pos, b"-");
    push_dec_padded(out, &mut pos, st.wMonth as u64, 2);
    push_bytes(out, &mut pos, b"-");
    push_dec_padded(out, &mut pos, st.wDay as u64, 2);
    push_bytes(out, &mut pos, b" ");
    push_dec_padded(out, &mut pos, st.wHour as u64, 2);
    push_bytes(out, &mut pos, b":");
    push_dec_padded(out, &mut pos, st.wMinute as u64, 2);
    push_bytes(out, &mut pos, b":");
    push_dec_padded(out, &mut pos, st.wSecond as u64, 2);
    push_bytes(out, &mut pos, b".");
    push_dec_padded(out, &mut pos, st.wMilliseconds as u64, 3);
    push_bytes(out, &mut pos, "][CRASH] 未处理异常 0x".as_bytes());
    push_hex(out, &mut pos, code as usize, 8);
    push_bytes(out, &mut pos, "（".as_bytes());
    push_bytes(out, &mut pos, exc_name(code).as_bytes());
    push_bytes(out, &mut pos, "）地址=0x".as_bytes());
    push_hex(out, &mut pos, addr, 1);
    push_bytes(out, &mut pos, " 线程=".as_bytes());
    push_dec_padded(out, &mut pos, tid as u64, 1);

    if let Some((op, target)) = access_info {
        let op_str = match op {
            0 => " 读 0x",
            1 => " 写 0x",
            _ => " 执行 0x",
        };
        push_bytes(out, &mut pos, op_str.as_bytes());
        push_hex(out, &mut pos, target, 1);
    }
    push_bytes(out, &mut pos, b"\n");
    pos
}

unsafe extern "system" fn crash_filter(ep: *const EXCEPTION_POINTERS) -> i32 {
    if ep.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    let len = CRASH_PATH.ready_len.load(Ordering::Acquire);
    if len == 0 || IN_CRASH.swap(true, Ordering::AcqRel) {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    // SAFETY: 经上面 Acquire 与 len > 0 检查后，buf 中前 len 个 u16 包含以 0 结尾的有效日志路径
    let er = unsafe { (*ep).ExceptionRecord };
    if er.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    // SAFETY: er 非空，读取异常记录字段
    let (code, addr, access_info) = unsafe {
        let rec = &*er;
        let info = if rec.ExceptionCode == 0xC000_0005 && rec.NumberParameters >= 2 {
            Some((rec.ExceptionInformation[0], rec.ExceptionInformation[1]))
        } else {
            None
        };
        (rec.ExceptionCode, rec.ExceptionAddress as usize, info)
    };

    let mut st = SYSTEMTIME::default();
    // SAFETY: 栈上局部变量 SYSTEMTIME 有效
    unsafe { GetLocalTime(&mut st) };
    // SAFETY: GetCurrentThreadId 为无副作用 Win32 查询
    let tid = unsafe { GetCurrentThreadId() };

    let mut line = [0u8; 512];
    let nbytes = format_crash_line(&st, code, addr, tid, access_info, &mut line);

    // SAFETY: CRASH_PATH.buf 在 ready_len > 0 时有效，调用 CreateFileW / WriteFile / CloseHandle 无锁追加
    unsafe {
        let wpath = &*CRASH_PATH.buf.get();
        let h = CreateFileW(
            wpath.as_ptr(),
            FILE_APPEND_DATA,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null_mut(),
            OPEN_ALWAYS,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        );
        if h != INVALID_HANDLE_VALUE {
            let mut written = 0u32;
            let _ = WriteFile(
                h,
                line.as_ptr().cast(),
                nbytes as u32,
                &mut written,
                null_mut(),
            );
            let _ = CloseHandle(h);
        }
    }
    EXCEPTION_CONTINUE_SEARCH
}

/// 安装未处理异常过滤器，在闪退时把异常信息无锁追加进当前日志文件
pub fn log_install_crash_filter() {
    // SAFETY: 传入静态函数指针 crash_filter 供 Windows SEH 异常分发调用
    unsafe {
        SetUnhandledExceptionFilter(Some(crash_filter));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// 日志模块共享全局日志文件路径，加互斥锁串行执行日志测试
    static LOG_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_log_rotation_and_multithreaded_writes() {
        let _global_guard = crate::core::TEST_GLOBAL_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let _guard = LOG_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join("ky_rust_log_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp log dir");
        let log_path = dir.join("run.log");
        let last_path = dir.join("run.last.log");

        // 会话一：主线程 10 行 + 两个线程各 200 行 = 411 行（含会话头）
        log_open(&log_path);
        for i in 0..10 {
            log_write("主", &format!("第{i}行"));
        }
        let t1 = thread::spawn(|| {
            for i in 0..200 {
                log_write("线程", &format!("t1 i={i} 中文行"));
            }
        });
        let t2 = thread::spawn(|| {
            for i in 0..200 {
                log_write("线程", &format!("t2 i={i} 中文行"));
            }
        });
        t1.join().expect("join t1");
        t2.join().expect("join t2");

        let c0 = fs::read_to_string(&log_path)
            .expect("read log1")
            .lines()
            .count();
        assert_eq!(c0, 411);

        // 会话二：自动轮换上次内容到 run.last.log
        log_open(&log_path);
        log_write("主", "第二次会话");
        let c_last = fs::read_to_string(&last_path)
            .expect("read last")
            .lines()
            .count();
        let c_new = fs::read_to_string(&log_path)
            .expect("read new")
            .lines()
            .count();
        assert_eq!(c_last, 411);
        assert_eq!(c_new, 2);

        log_open("");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_crash_line_formatting() {
        let st = SYSTEMTIME {
            wYear: 2026,
            wMonth: 11,
            wDay: 20,
            wDayOfWeek: 5,
            wHour: 15,
            wMinute: 8,
            wSecond: 9,
            wMilliseconds: 42,
        };
        let mut buf = [0u8; 512];
        let n = format_crash_line(
            &st,
            0xC000_0005,
            0x7FF6_1234_5678,
            4321,
            Some((1, 0x0000_0000)),
            &mut buf,
        );
        let text = std::str::from_utf8(&buf[..n]).expect("utf8 crash line");
        assert!(
            text.starts_with("[2026-11-20 15:08:09.042][CRASH] 未处理异常 0xC0000005（访问违例）")
        );
        assert!(text.contains("地址=0x7FF612345678 线程=4321 写 0x0"));
    }
}
