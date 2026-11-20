//! HTTP 客户端：基于系统 WinINet（自动保持 Cookie），供翻译引擎与检查更新使用

use crate::core::log::log_write;
use crate::core::str_util::{decode_utf8_c_bytes, to_c_bytes};
use crate::core::win32::{
    GetLastError, HttpOpenRequestA, HttpQueryInfoA, HttpSendRequestA, InternetCloseHandle,
    InternetConnectA, InternetCrackUrlA, InternetOpenA, InternetOpenUrlA, InternetReadFile,
    InternetSetOptionA, DWORD, HINTERNET, HTTP_QUERY_CONTENT_LENGTH, HTTP_QUERY_FLAG_NUMBER,
    HTTP_QUERY_LOCATION, HTTP_QUERY_STATUS_CODE, INTERNET_FLAG_KEEP_CONNECTION,
    INTERNET_FLAG_NO_AUTO_REDIRECT, INTERNET_FLAG_NO_CACHE_WRITE, INTERNET_FLAG_RELOAD,
    INTERNET_FLAG_SECURE, INTERNET_OPEN_TYPE_PRECONFIG, INTERNET_OPTION_CONNECT_TIMEOUT,
    INTERNET_OPTION_RECEIVE_TIMEOUT, INTERNET_OPTION_SEND_TIMEOUT, INTERNET_SCHEME_HTTPS,
    INTERNET_SERVICE_HTTP, URL_COMPONENTSA,
};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::{LazyLock, Mutex};

/// 单次 HTTP 响应最大读取上限（6MB）
pub const FY_HTTP_MAX: usize = 6 * 1024 * 1024;

const USER_AGENT: &[u8] = b"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36\0";

#[derive(Clone, Copy)]
struct SessionHandle(HINTERNET);

// SAFETY: WinINet HINTERNET 会话句柄支持跨线程并发创建请求句柄，且仅在进程级单次初始化后只读复用
unsafe impl Send for SessionHandle {}

static HTTP_SESSION: LazyLock<Mutex<Option<SessionHandle>>> = LazyLock::new(|| Mutex::new(None));

fn get_session() -> Option<HINTERNET> {
    let mut guard = HTTP_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(s) = *guard {
        return Some(s.0);
    }
    // SAFETY: USER_AGENT 以 '\0' 结尾，参数符合 InternetOpenA 规范
    let h = unsafe {
        InternetOpenA(
            USER_AGENT.as_ptr(),
            INTERNET_OPEN_TYPE_PRECONFIG,
            null(),
            null(),
            0,
        )
    };
    if h.is_null() {
        return None;
    }
    // SAFETY: h 为有效的 WinINet 会话句柄，传入栈上 u32 超时值
    unsafe {
        let mut t: DWORD = 8000;
        let _ = InternetSetOptionA(
            h,
            INTERNET_OPTION_CONNECT_TIMEOUT,
            (&raw const t).cast(),
            size_of::<DWORD>() as DWORD,
        );
        t = 15000;
        let _ = InternetSetOptionA(
            h,
            INTERNET_OPTION_RECEIVE_TIMEOUT,
            (&raw const t).cast(),
            size_of::<DWORD>() as DWORD,
        );
        t = 15000;
        let _ = InternetSetOptionA(
            h,
            INTERNET_OPTION_SEND_TIMEOUT,
            (&raw const t).cast(),
            size_of::<DWORD>() as DWORD,
        );
    }
    *guard = Some(SessionHandle(h));
    Some(h)
}

fn read_all_handle(h_file: HINTERNET) -> Option<Vec<u8>> {
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut chunk = [0u8; 16 * 1024];
    loop {
        if buf.len() >= FY_HTTP_MAX {
            break;
        }
        let max_read = (FY_HTTP_MAX - buf.len()).min(chunk.len()) as DWORD;
        let mut got: DWORD = 0;
        // SAFETY: h_file 为有效请求句柄，chunk 在作用域内可写且长度至少 max_read
        let ok = unsafe { InternetReadFile(h_file, chunk.as_mut_ptr().cast(), max_read, &mut got) };
        if ok == 0 || got == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..got as usize]);
    }
    Some(buf)
}

fn last_err() -> DWORD {
    // SAFETY: Win32 GetLastError 无前提条件
    unsafe { GetLastError() }
}

/// 发起 HTTP(S) GET 请求并返回 UTF-8 响应文本
pub fn http_get(url: &str, extra_headers: Option<&str>) -> Option<String> {
    let session = get_session()?;
    let headers = match extra_headers {
        Some(eh) => format!(
            "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n{eh}"
        ),
        None => "Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8\r\n"
            .to_string(),
    };
    let c_url = to_c_bytes(url);
    let c_hdr = to_c_bytes(&headers);
    let flags = INTERNET_FLAG_RELOAD | INTERNET_FLAG_NO_CACHE_WRITE | INTERNET_FLAG_KEEP_CONNECTION;

    // SAFETY: session 有效，c_url 和 c_hdr 以 '\0' 结尾，dwHeadersLength = u32::MAX 表示由 API 按 NUL 计长
    let h_file =
        unsafe { InternetOpenUrlA(session, c_url.as_ptr(), c_hdr.as_ptr(), u32::MAX, flags, 0) };
    if h_file.is_null() {
        log_write("HTTP", &format!("GET {url} 失败 err={}", last_err()));
        return None;
    }
    let body = read_all_handle(h_file);
    // SAFETY: h_file 非空，读取完毕立即关闭
    unsafe {
        let _ = InternetCloseHandle(h_file);
    }
    let bytes = body?;
    log_write("HTTP", &format!("GET {url} 返回 {}字节", bytes.len()));
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

struct ParsedUrl {
    host: String,
    path: String,
    port: u16,
    secure: bool,
}

fn crack_url(url: &str) -> Option<ParsedUrl> {
    let c_url = to_c_bytes(url);
    let mut host_buf = [0u8; 256];
    let mut path_buf = [0u8; 2048];
    let mut uc = URL_COMPONENTSA {
        dwStructSize: size_of::<URL_COMPONENTSA>() as DWORD,
        lpszScheme: null_mut(),
        dwSchemeLength: 0,
        nScheme: 0,
        lpszHostName: host_buf.as_mut_ptr(),
        dwHostNameLength: host_buf.len() as DWORD,
        nPort: 0,
        lpszUserName: null_mut(),
        dwUserNameLength: 0,
        lpszPassword: null_mut(),
        dwPasswordLength: 0,
        lpszUrlPath: path_buf.as_mut_ptr(),
        dwUrlPathLength: path_buf.len() as DWORD,
        lpszExtraInfo: null_mut(),
        dwExtraInfoLength: 0,
    };
    // SAFETY: uc 已设置正确 dwStructSize 与有效接收缓冲区，c_url 以 '\0' 结尾
    let ok = unsafe { InternetCrackUrlA(c_url.as_ptr(), 0, 0, &mut uc) };
    if ok == 0 {
        return None;
    }
    let secure = uc.nScheme == INTERNET_SCHEME_HTTPS;
    let port = if uc.nPort != 0 {
        uc.nPort
    } else if secure {
        443
    } else {
        80
    };
    let host = decode_utf8_c_bytes(&host_buf);
    let mut path = decode_utf8_c_bytes(&path_buf);
    if path.is_empty() {
        path.push('/');
    }
    Some(ParsedUrl {
        host,
        path,
        port,
        secure,
    })
}

fn http_post_raw<F>(url: &str, body_bytes: &[u8], tag: &str, build_headers: F) -> Option<String>
where
    F: FnOnce(&str) -> String,
{
    let session = get_session()?;
    let Some(parsed) = crack_url(url) else {
        log_write(tag, &format!("{tag} 地址解析失败 err={} {url}", last_err()));
        return None;
    };

    let c_host = to_c_bytes(&parsed.host);
    // SAFETY: session 有效，c_host 以 '\0' 结尾
    let h_conn = unsafe {
        InternetConnectA(
            session,
            c_host.as_ptr(),
            parsed.port,
            null(),
            null(),
            INTERNET_SERVICE_HTTP,
            0,
            0,
        )
    };
    if h_conn.is_null() {
        log_write(
            tag,
            &format!("{tag} 连接 {} 失败 err={}", parsed.host, last_err()),
        );
        return None;
    }

    let c_verb = b"POST\0";
    let c_path = to_c_bytes(&parsed.path);
    let c_ver = b"HTTP/1.1\0";
    let mut flags = INTERNET_FLAG_KEEP_CONNECTION | INTERNET_FLAG_NO_CACHE_WRITE;
    if parsed.secure {
        flags |= INTERNET_FLAG_SECURE;
    }

    // SAFETY: h_conn 有效，字符串参数均以 '\0' 结尾
    let h_req = unsafe {
        HttpOpenRequestA(
            h_conn,
            c_verb.as_ptr(),
            c_path.as_ptr(),
            c_ver.as_ptr(),
            null(),
            null(),
            flags,
            0,
        )
    };
    if h_req.is_null() {
        log_write(tag, &format!("{tag} 打开请求失败 err={}", last_err()));
        // SAFETY: h_conn 非空
        unsafe {
            let _ = InternetCloseHandle(h_conn);
        }
        return None;
    }

    let hdr_str = build_headers(&parsed.host);
    let c_hdr = to_c_bytes(&hdr_str);
    let opt_ptr = if body_bytes.is_empty() {
        null()
    } else {
        body_bytes.as_ptr().cast()
    };

    // SAFETY: h_req 有效，c_hdr 以 '\0' 结尾，opt_ptr 指向 body_bytes.len() 字节可读内存
    let sent = unsafe {
        HttpSendRequestA(
            h_req,
            c_hdr.as_ptr(),
            u32::MAX,
            opt_ptr,
            body_bytes.len() as DWORD,
        )
    };
    let body = if sent != 0 {
        read_all_handle(h_req)
    } else {
        let send_err = last_err();
        log_write(tag, &format!("{tag} 发送失败 err={send_err}"));
        None
    };

    // SAFETY: 关闭请求与连接句柄
    unsafe {
        let _ = InternetCloseHandle(h_req);
        let _ = InternetCloseHandle(h_conn);
    }

    let bytes = match body {
        Some(b) => b,
        None => {
            if sent != 0 {
                log_write(tag, &format!("{tag} 响应读取失败"));
            }
            return None;
        }
    };
    log_write(tag, &format!("{tag} 返回 {}字节", bytes.len()));
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// 发送 `application/x-www-form-urlencoded` POST 请求
pub fn http_post(url: &str, referer: Option<&str>, form: &str) -> Option<String> {
    log_write("HTTP", &format!("POST {url} form={}字节", form.len()));
    let ref_str = referer.unwrap_or(url);
    http_post_raw(url, form.as_bytes(), "POST", |host| {
        format!(
            "Content-Type: application/x-www-form-urlencoded\r\n\
             Accept: */*\r\n\
             Origin: https://{host}\r\n\
             Referer: {ref_str}\r\n"
        )
    })
}

/// 发送 `application/json` POST 请求（携带 Bearer 认证头）
pub fn http_post_json(url: &str, bearer: &str, json: &str) -> Option<String> {
    http_post_raw(url, json.as_bytes(), "POST_JSON", |_| {
        format!(
            "Content-Type: application/json\r\n\
             Accept: application/json\r\n\
             Authorization: Bearer {bearer}\r\n"
        )
    })
}

/// 二进制安全下载：直接写文件，支持可选进度回调 `progress(done, total)`；失败返回 `-1`
pub fn http_download<P: AsRef<Path>>(url: &str, dest: P, progress: Option<fn(i64, i64)>) -> i64 {
    let Some(session) = get_session() else {
        return -1;
    };
    let dest_path = dest.as_ref();
    let Ok(mut file) = File::create(dest_path) else {
        return -1;
    };

    let c_url = to_c_bytes(url);
    let flags = INTERNET_FLAG_RELOAD | INTERNET_FLAG_NO_CACHE_WRITE | INTERNET_FLAG_KEEP_CONNECTION;
    // SAFETY: session 有效，c_url 以 '\0' 结尾
    let h_file = unsafe { InternetOpenUrlA(session, c_url.as_ptr(), null(), 0, flags, 0) };
    if h_file.is_null() {
        log_write("HTTP", &format!("下载打开失败 err={} {url}", last_err()));
        drop(file);
        let _ = fs::remove_file(dest_path);
        return -1;
    }

    let mut status: DWORD = 0;
    let mut cb_status: DWORD = size_of::<DWORD>() as DWORD;
    // SAFETY: 查询 HTTP 状态码写入栈上 DWORD
    let has_status = unsafe {
        HttpQueryInfoA(
            h_file,
            HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
            (&raw mut status).cast(),
            &mut cb_status,
            null_mut(),
        )
    };
    if has_status != 0 && status >= 400 {
        log_write("HTTP", &format!("下载状态异常 HTTP {status} {url}"));
        // SAFETY: 关闭句柄后删除无效输出文件
        unsafe {
            let _ = InternetCloseHandle(h_file);
        }
        drop(file);
        let _ = fs::remove_file(dest_path);
        return -1;
    }

    let mut content_len: DWORD = 0;
    let mut cb_len: DWORD = size_of::<DWORD>() as DWORD;
    // SAFETY: 查询 Content-Length 写入栈上 DWORD
    let has_len = unsafe {
        HttpQueryInfoA(
            h_file,
            HTTP_QUERY_CONTENT_LENGTH | HTTP_QUERY_FLAG_NUMBER,
            (&raw mut content_len).cast(),
            &mut cb_len,
            null_mut(),
        )
    };
    let total: i64 = if has_len != 0 && content_len > 0 {
        content_len as i64
    } else {
        -1
    };

    let mut buf = vec![0u8; 65536];
    let mut done: i64 = 0;
    loop {
        let mut got: DWORD = 0;
        // SAFETY: h_file 有效，buf 长度为 65536 字节
        let ok = unsafe {
            InternetReadFile(
                h_file,
                buf.as_mut_ptr().cast(),
                buf.len() as DWORD,
                &mut got,
            )
        };
        if ok == 0 || got == 0 {
            break;
        }
        if file.write_all(&buf[..got as usize]).is_err() {
            // SAFETY: 关闭句柄
            unsafe {
                let _ = InternetCloseHandle(h_file);
            }
            drop(file);
            let _ = fs::remove_file(dest_path);
            return -1;
        }
        done += got as i64;
        if let Some(cb) = progress {
            cb(done, total);
        }
    }

    // SAFETY: 关闭句柄
    unsafe {
        let _ = InternetCloseHandle(h_file);
    }
    done
}

/// 发起 GET 请求（禁用自动重定向），返回 `Location` 响应头；无重定向返回 `None`
pub fn http_get_location(url: &str) -> Option<String> {
    let session = get_session()?;
    let c_url = to_c_bytes(url);
    let flags = INTERNET_FLAG_RELOAD
        | INTERNET_FLAG_NO_CACHE_WRITE
        | INTERNET_FLAG_KEEP_CONNECTION
        | INTERNET_FLAG_NO_AUTO_REDIRECT;
    // SAFETY: session 有效，c_url 以 '\0' 结尾
    let h_file = unsafe { InternetOpenUrlA(session, c_url.as_ptr(), null(), 0, flags, 0) };
    if h_file.is_null() {
        return None;
    }
    let mut loc = [0u8; 1024];
    let mut cb: DWORD = (loc.len() - 1) as DWORD;
    // SAFETY: 查询 HTTP_QUERY_LOCATION 写入 loc 缓冲区
    let ok = unsafe {
        HttpQueryInfoA(
            h_file,
            HTTP_QUERY_LOCATION,
            loc.as_mut_ptr().cast(),
            &mut cb,
            null_mut(),
        )
    };
    // SAFETY: 关闭句柄
    unsafe {
        let _ = InternetCloseHandle(h_file);
    }
    if ok == 0 || cb == 0 {
        return None;
    }
    let n = (cb as usize).min(loc.len());
    let s = decode_utf8_c_bytes(&loc[..n]);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
