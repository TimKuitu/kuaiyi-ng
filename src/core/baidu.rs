//! 百度翻译开放平台 API（fanyi-api.baidu.com）
//!
//! - 签名：`MD5(appid + q原文 + salt + 密钥)`
//! - 传输：`q` 仅做一次 URL 编码随表单发送（多编一次必报 `54001 Invalid Sign`）

use crate::core::http::http_post;
use crate::core::json::json_str;
use crate::core::log::log_write;
use crate::core::md5::md5_hex;
use crate::core::str_util::{has_cjk, urlencode};
use crate::core::win32::GetTickCount;
use crate::core::{set_error, FY_EN2ZH, FY_ZH2EN};

const BAIDU_API: &str = "https://fanyi-api.baidu.com/api/trans/vip/translate";

/// 调用百度翻译开放平台接口
pub fn translate_baidu(
    utf8_text: &str,
    mode: i32,
    appid: &str,
    secret: &str,
    api_url: &str,
) -> Option<String> {
    // SAFETY: Win32 GetTickCount 无参数无副作用
    let tick = unsafe { GetTickCount() };
    let salt = tick.to_string();
    translate_baidu_with_salt(utf8_text, mode, appid, secret, api_url, &salt)
}

/// 指定 `salt` 调用百度翻译接口（便于确定性测试验证签名）
pub fn translate_baidu_with_salt(
    utf8_text: &str,
    mode: i32,
    appid: &str,
    secret: &str,
    api_url: &str,
    salt: &str,
) -> Option<String> {
    if utf8_text.is_empty() {
        set_error("请输入要翻译的内容");
        return None;
    }
    if appid.is_empty() {
        set_error("百度 AppID 为空");
        return None;
    }
    if secret.is_empty() {
        set_error("百度密钥为空");
        return None;
    }
    if utf8_text.len() > 6000 {
        set_error("文本超过百度接口 6000 字节上限");
        return None;
    }

    let url = if api_url.is_empty() {
        BAIDU_API
    } else {
        api_url
    };
    let from = "auto";
    let to = match mode {
        FY_ZH2EN => "en",
        FY_EN2ZH => "zh",
        _ => {
            if has_cjk(utf8_text) {
                "en"
            } else {
                "zh"
            }
        }
    };

    // 签名：MD5(appid + q原文 + salt + 密钥)
    let raw_sign = format!("{appid}{utf8_text}{salt}{secret}");
    let sign = md5_hex(raw_sign.as_bytes());

    // q 只做一次 URL 编码随表单发送
    let qenc = urlencode(utf8_text);
    let eappid = urlencode(appid);
    let esalt = urlencode(salt);
    let efrom = urlencode(from);
    let eto = urlencode(to);
    let esign = urlencode(&sign);

    log_write("百度", &format!("请求 {}字节 to={to}", utf8_text.len()));

    let form = format!("q={qenc}&from={efrom}&to={eto}&appid={eappid}&salt={esalt}&sign={esign}");
    let Some(resp) = http_post(url, Some("https://fanyi-api.baidu.com"), &form) else {
        set_error("百度接口连接失败（网络错误）");
        return None;
    };

    if let Some(errcode) = json_str(&resp, "error_code") {
        if !errcode.is_empty() {
            let msg = json_str(&resp, "error_msg").unwrap_or_default();
            let hint = if msg.is_empty() {
                errcode.as_str()
            } else {
                msg.as_str()
            };
            let formatted = match errcode.as_str() {
                "52003" => "百度接口：密钥或 AppID 无效".to_string(),
                "58003" => "百度接口：服务未开通（需在百度开放平台开通翻译）".to_string(),
                _ => format!("百度接口错误 {errcode}（{hint}）"),
            };
            set_error(&formatted);
            return None;
        }
    }

    let mut segments = Vec::new();
    for i in 0..32 {
        let path = format!("trans_result[{i}].dst");
        let Some(dst) = json_str(&resp, &path) else {
            break;
        };
        segments.push(dst);
    }
    if segments.is_empty() {
        set_error("百度返回内容解析失败");
        return None;
    }
    log_write("百度", &format!("解析出 {} 段", segments.len()));
    Some(segments.join("\n"))
}

#[cfg(test)]
pub(crate) mod test_mock {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    pub fn url_decode_once(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'+' {
                out.push(b' ');
                i += 1;
            } else if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let (Ok(s_hex), true) = (
                    std::str::from_utf8(&bytes[i + 1..i + 3]),
                    bytes[i + 1].is_ascii_hexdigit() && bytes[i + 2].is_ascii_hexdigit(),
                ) {
                    if let Ok(v) = u8::from_str_radix(s_hex, 16) {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                }
                out.push(bytes[i]);
                i += 1;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    pub fn read_http_request(stream: &mut std::net::TcpStream) -> (String, String) {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end = None;
        let mut content_len = 0usize;

        while let Ok(n) = stream.read(&mut tmp) {
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if header_end.is_none() {
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                    let hdr_str = String::from_utf8_lossy(&buf[..pos]);
                    for line in hdr_str.lines() {
                        if let Some((k, v)) = line.split_once(':') {
                            if k.trim().eq_ignore_ascii_case("Content-Length") {
                                content_len = v.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                }
            }
            if let Some(hend) = header_end {
                if buf.len() >= hend + content_len {
                    let hdr = String::from_utf8_lossy(&buf[..hend]).into_owned();
                    let body = String::from_utf8_lossy(&buf[hend..hend + content_len]).into_owned();
                    return (hdr, body);
                }
            }
        }
        (String::new(), String::new())
    }

    pub fn spawn_baidu_mock(
        expected_appid: &'static str,
        expected_secret: &'static str,
        max_requests: usize,
    ) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind baidu mock");
        let port = listener.local_addr().expect("local addr").port();
        let url = format!("http://127.0.0.1:{port}/api/trans/vip/translate");

        let handle = thread::spawn(move || {
            for _ in 0..max_requests {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                let (_, body) = read_http_request(&mut stream);
                let mut params = HashMap::new();
                for pair in body.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        params.insert(k.to_string(), url_decode_once(v));
                    }
                }
                let q = params.get("q").map(String::as_str).unwrap_or("");
                let appid = params.get("appid").map(String::as_str).unwrap_or("");
                let salt = params.get("salt").map(String::as_str).unwrap_or("");
                let sign = params.get("sign").map(String::as_str).unwrap_or("");

                // 按百度官方规则严格验签：MD5(appid + url解码一次的q + salt + secret)
                let expected = crate::core::md5::md5_hex(
                    format!("{expected_appid}{q}{salt}{expected_secret}").as_bytes(),
                );
                let resp_json = if appid != expected_appid {
                    r#"{"error_code":"52003","error_msg":"UNAUTHORIZED USER"}"#.to_string()
                } else if sign != expected {
                    r#"{"error_code":"54001","error_msg":"Invalid Sign"}"#.to_string()
                } else {
                    r#"{"from":"en","to":"zh","trans_result":[{"src":"line1","dst":"译文1"},{"src":"line2","dst":"译文2"}]}"#.to_string()
                };
                let http_resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    resp_json.len(),
                    resp_json
                );
                let _ = stream.write_all(http_resp.as_bytes());
                let _ = stream.flush();
            }
        });
        (url, handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::last_error;

    #[test]
    fn test_baidu_sign_verification_and_errors() {
        let _guard = crate::core::TEST_GLOBAL_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (url, handle) = test_mock::spawn_baidu_mock("my_appid", "my_secret", 2);

        // 1. 含空格、+、中文与特殊符号，验证只做一次 URL 编码且服务端验签通过
        let ok = translate_baidu_with_salt(
            "hello world + 测试 & 100%",
            FY_EN2ZH,
            "my_appid",
            "my_secret",
            &url,
            "123456",
        );
        assert_eq!(ok, Some("译文1\n译文2".to_string()));

        // 2. 密钥错误触发服务端 54001 验签失败
        let bad = translate_baidu_with_salt(
            "hello",
            FY_EN2ZH,
            "my_appid",
            "wrong_secret",
            &url,
            "123456",
        );
        assert_eq!(bad, None);
        assert!(last_error().contains("54001"));

        // 3. 超过 6000 字节上限直接本地拦截
        let too_long = "a".repeat(6001);
        assert_eq!(
            translate_baidu_with_salt(&too_long, FY_EN2ZH, "my_appid", "my_secret", &url, "1"),
            None
        );
        assert!(last_error().contains("6000"));

        handle.join().expect("join baidu mock");
    }
}
