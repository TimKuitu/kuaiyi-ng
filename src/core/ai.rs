//! OpenAI 格式 AI 翻译（DeepSeek / OpenAI / Moonshot / Ollama 等兼容接口）

use crate::core::http::http_post_json;
use crate::core::json::{json_escape, json_str};
use crate::core::log::log_write;
use crate::core::str_util::has_cjk;
use crate::core::{set_error, FY_EN2ZH, FY_ZH2EN};

/// 拼接接口完整地址：`base[/v1]/chat/completions`
pub fn build_ai_url(base: &str) -> String {
    let trimmed = base.trim_end_matches(['/', ' ']);
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/chat/completions")
    } else {
        format!("{trimmed}/v1/chat/completions")
    }
}

/// 调用 OpenAI 兼容 `/chat/completions` 接口执行翻译
pub fn translate_ai(
    utf8_text: &str,
    mode: i32,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Option<String> {
    if utf8_text.is_empty() {
        set_error("请输入要翻译的内容");
        return None;
    }
    if base_url.is_empty() {
        set_error("AI 接口地址为空");
        return None;
    }
    if api_key.is_empty() {
        set_error("API 密钥为空");
        return None;
    }
    let model = if model.is_empty() {
        "deepseek-chat"
    } else {
        model
    };

    let target = match mode {
        FY_ZH2EN => "English",
        FY_EN2ZH => "Chinese (Simplified)",
        _ => {
            if has_cjk(utf8_text) {
                "English"
            } else {
                "Chinese (Simplified)"
            }
        }
    };
    let sys = format!(
        "You are a translation engine. Translate the user's text into {target}. \
         Output ONLY the translation itself, with no explanations or quotes."
    );

    let e_sys = json_escape(&sys);
    let e_text = json_escape(utf8_text);
    let e_model = json_escape(model);

    log_write("AI", &format!("请求 {base_url} 模型={model}"));

    let url = build_ai_url(base_url);
    let body = format!(
        "{{\"model\":\"{e_model}\",\"temperature\":0,\
         \"messages\":[{{\"role\":\"system\",\"content\":\"{e_sys}\"}},\
         {{\"role\":\"user\",\"content\":\"{e_text}\"}}]}}"
    );

    let Some(resp) = http_post_json(&url, api_key, &body) else {
        set_error("AI 接口连接失败（网络错误或地址不对）");
        return None;
    };

    let Some(content) = json_str(&resp, "choices[0].message.content") else {
        let msg = json_str(&resp, "error.message").unwrap_or_default();
        let bad_key = resp.contains("invalid_api_key")
            || resp.contains("Authentication Fallback")
            || resp.contains("401");
        if bad_key {
            set_error("API 密钥无效（401）");
        } else if !msg.is_empty() {
            set_error(&msg);
        } else {
            set_error("AI 返回格式解析失败");
        }
        return None;
    };

    let mut s = content.trim_matches([' ', '\n', '\r']);
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s = &s[1..s.len() - 1];
    }
    s = s.trim_matches([' ', '\n', '\r']);
    if s.is_empty() {
        set_error("AI 返回了空内容");
        return None;
    }
    Some(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::baidu::test_mock::read_http_request;
    use crate::core::last_error;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_build_ai_url_variants() {
        assert_eq!(
            build_ai_url("https://api.deepseek.com"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            build_ai_url("https://api.openai.com/v1/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            build_ai_url("https://example.com/v1/chat/completions"),
            "https://example.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_ai_mock_auth_and_quote_strip() {
        let _guard = crate::core::TEST_GLOBAL_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ai mock");
        let port = listener.local_addr().expect("addr").port();
        let base = format!("http://127.0.0.1:{port}");

        let handle = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept");
                let (hdr, _body) = read_http_request(&mut stream);
                let ok_auth = hdr.lines().any(|l| {
                    l.to_ascii_lowercase()
                        .contains("authorization: bearer sk-good")
                });
                let (status, json) = if ok_auth {
                    (
                        "200 OK",
                        r#"{"choices":[{"message":{"content":" \"串口初始化\" \n"}}]}"#,
                    )
                } else {
                    (
                        "401 Unauthorized",
                        r#"{"error":{"message":"Incorrect API key (invalid_api_key)"}}"#,
                    )
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                    json.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });

        let ok = translate_ai("serial init", FY_EN2ZH, &base, "sk-good", "");
        assert_eq!(ok, Some("串口初始化".to_string()));

        let bad = translate_ai("serial init", FY_EN2ZH, &base, "sk-bad", "gpt-4o");
        assert_eq!(bad, None);
        assert!(last_error().contains("401"));

        handle.join().expect("join ai mock");
    }
}
