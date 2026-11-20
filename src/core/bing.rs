//! Bing 免费翻译接口与多引擎回退链
//!
//! - 网页版会话：解析 `IG`、`IID`、`params_AbusePreventionHelper = [key,"token",ttl]`
//! - 单词词典：抓取 `<meta name="description">` 提取英/美音标、释义与网络释义
//! - 回退链：默认 `AI → 百度 → 必应`；`FY_CHAIN_PREFER_BAIDU` 时 `百度 → 必应 → AI`

use crate::core::ai::translate_ai;
use crate::core::baidu::translate_baidu;
use crate::core::http::{http_get, http_post};
use crate::core::json::json_str;
use crate::core::log::log_write;
use crate::core::str_util::{has_cjk, urlencode};
use crate::core::win32::{GetTickCount, GetTickCount64};
use crate::core::{
    last_error, set_engine, set_error, FY_CHAIN_NO_DICT, FY_CHAIN_PREFER_BAIDU, FY_EN2ZH, FY_ZH2EN,
};
use std::sync::{LazyLock, Mutex};

struct BingSession {
    ig: String,
    iid: String,
    token: String,
    key: String,
    tick: u64,
}

static BING_SESSION: LazyLock<Mutex<BingSession>> = LazyLock::new(|| {
    Mutex::new(BingSession {
        ig: String::new(),
        iid: String::new(),
        token: String::new(),
        key: String::new(),
        tick: 0,
    })
});

fn trim_cn(s: &str) -> &str {
    s.trim_matches(['，', '；', ',', ';', ' ', '\t'])
}

fn refresh_session() -> Option<(String, String, String, String)> {
    {
        let sess = BING_SESSION.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: Win32 GetTickCount64 无副作用
        let now = unsafe { GetTickCount64() };
        if !sess.ig.is_empty()
            && !sess.token.is_empty()
            && now.saturating_sub(sess.tick) < 45 * 60 * 1000
        {
            return Some((
                sess.ig.clone(),
                sess.iid.clone(),
                sess.token.clone(),
                sess.key.clone(),
            ));
        }
    }

    let Some(page) = http_get("https://cn.bing.com/translator", None) else {
        set_error("无法连接必应翻译（网络不通或被防火墙拦截）");
        return None;
    };

    let Some(ig_pos) = page.find("IG:\"") else {
        set_error("翻译页面解析失败：找不到 IG");
        return None;
    };
    let ig: String = page[ig_pos + 4..]
        .chars()
        .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        .take(32)
        .collect();
    if ig.is_empty() {
        set_error("翻译页面解析失败：IG 格式异常");
        return None;
    }

    let iid = if let Some(iid_pos) = page.find("data-iid=\"translator.") {
        let digits: String = page[iid_pos + "data-iid=\"translator.".len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(n) = digits.parse::<u32>() {
            if n > 0 {
                format!("translator.{n}")
            } else {
                "translator.5023".to_string()
            }
        } else {
            "translator.5023".to_string()
        }
    } else {
        "translator.5023".to_string()
    };

    let Some(tok_pos) = page.find("params_AbusePreventionHelper = [") else {
        set_error("翻译页面解析失败：找不到 token");
        return None;
    };
    let rest = &page[tok_pos + "params_AbusePreventionHelper = [".len()..];
    let Some(comma) = rest.find(",\"") else {
        set_error("翻译页面解析失败：token 格式异常");
        return None;
    };
    let key: String = rest[..comma]
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if key.is_empty() {
        set_error("翻译页面解析失败：token 格式异常");
        return None;
    }
    let tok_after_quote = &rest[comma + 2..];
    let token: String = tok_after_quote
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(95)
        .collect();
    if token.is_empty() {
        set_error("翻译页面解析失败：token 格式异常");
        return None;
    }

    // SAFETY: Win32 GetTickCount64 无副作用
    let now = unsafe { GetTickCount64() };
    let mut sess = BING_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    sess.ig = ig.clone();
    sess.iid = iid.clone();
    sess.token = token.clone();
    sess.key = key.clone();
    sess.tick = now;

    Some((ig, iid, token, key))
}

fn clear_session_cache() {
    let mut sess = BING_SESSION.lock().unwrap_or_else(|e| e.into_inner());
    sess.ig.clear();
    sess.token.clear();
    sess.tick = 0;
}

fn ttranslate(text: &str, from: &str, to: &str) -> Option<String> {
    let enc = urlencode(text);
    for attempt in 0..2 {
        let (ig, iid, token, key) = refresh_session()?;
        let efrom = urlencode(from);
        let eto = urlencode(to);
        let etok = urlencode(&token);
        let ekey = urlencode(&key);

        // SAFETY: GetTickCount 无副作用
        let sfx = (unsafe { GetTickCount() }) & 0xFFFF;
        let url =
            format!("https://cn.bing.com/ttranslatev3?isVertical=1&&IG={ig}&IID={iid}&SFX={sfx}");
        let form =
            format!("fromLang={efrom}&to={eto}&tone=Casual&text={enc}&token={etok}&key={ekey}");

        let Some(resp) = http_post(&url, Some("https://cn.bing.com/translator"), &form) else {
            set_error("翻译请求发送失败（网络错误）");
            return None;
        };

        if let Some(trans) = json_str(&resp, "[0].translations[0].text") {
            let pin =
                json_str(&resp, "[0].translations[0].transliteration.text").unwrap_or_default();
            if !pin.is_empty() && to == "zh-Hans" {
                return Some(format!("{trans}\n拼音：{pin}"));
            }
            return Some(trans);
        }

        let msg = json_str(&resp, "errorMessage").unwrap_or_default();
        if resp.contains("ShowCaptcha") {
            set_error("必应要求人机验证，请稍后重试");
            return None;
        }
        if attempt == 0 {
            log_write("必应", "会话疑失效，刷新 token 重试");
            clear_session_cache();
            continue;
        }
        if !msg.is_empty() {
            set_error(&msg);
        } else {
            set_error("翻译结果解析失败");
        }
        return None;
    }
    None
}

/// HTML 实体反编码（支持十进制 `&#NNN;` 及 `&amp;` / `&quot;` / `&lt;` / `&gt;`）
pub fn html_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'&' {
            let rest = &s[i..];
            if let Some(after_hash) = rest.strip_prefix("&#") {
                if let Some(semi) = after_hash.find(';') {
                    if semi < 6 {
                        if let Ok(cp) = after_hash[..semi].parse::<u32>() {
                            if cp > 0 && cp < 0x11_0000 {
                                if let Some(ch) = char::from_u32(cp) {
                                    out.push(ch);
                                    i += 2 + semi + 1;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            if rest.starts_with("&amp;") {
                out.push('&');
                i += 5;
                continue;
            }
            if rest.starts_with("&quot;") {
                out.push('"');
                i += 6;
                continue;
            }
            if rest.starts_with("&lt;") {
                out.push('<');
                i += 4;
                continue;
            }
            if rest.starts_with("&gt;") {
                out.push('>');
                i += 4;
                continue;
            }
        }
        if let Some(ch) = s[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

/// 从必应词典 `<meta name="description" content="...">` 内容中提取词典释义
pub fn parse_dict_meta_desc(raw_desc: &str) -> Option<String> {
    let desc = html_unescape(raw_desc);
    let head = desc.find("为您提供")?;
    let after_head = head + "为您提供".len();
    let de = desc[after_head..].find("的释义").map(|p| after_head + p)?;
    if de <= after_head {
        return None;
    }
    let word = &desc[after_head..de];
    if word.len() >= 128 {
        return None;
    }

    let pus = desc[de..].find("，美").map(|p| de + p);
    let puk = desc[de..].find("，英").map(|p| de + p);
    let mut us = "";
    let mut uk = "";
    let mut uk_end = None;

    if let (Some(us_pos), Some(uk_pos)) = (pus, puk) {
        if uk_pos > us_pos {
            let us_start = us_pos + "，美".len();
            let us_slice = &desc[us_start..uk_pos];
            if us_slice.len() < 160 {
                us = us_slice;
            }
            let uk_start = uk_pos + "，英".len();
            let uk_rest = &desc[uk_start..];
            let end_rel = uk_rest
                .find(['，', '；', ',', ';'])
                .unwrap_or(uk_rest.len());
            let uk_slice = &uk_rest[..end_rel];
            if uk_slice.len() < 160 {
                uk = uk_slice;
            }
            uk_end = Some(uk_start + end_rel);
        }
    }

    let pweb = desc.find("网络释义：");
    let mut pdefs_idx = uk_end.unwrap_or(de);
    while pdefs_idx < desc.len() {
        let ch = desc[pdefs_idx..].chars().next()?;
        if matches!(ch, '，' | '；' | ',' | ';') {
            break;
        }
        pdefs_idx += ch.len_utf8();
    }
    while pdefs_idx < desc.len() {
        let ch = desc[pdefs_idx..].chars().next()?;
        if !matches!(ch, '，' | '；' | ',' | ';') {
            break;
        }
        pdefs_idx += ch.len_utf8();
    }

    let defs_raw = match pweb {
        Some(web_pos) if web_pos > pdefs_idx => &desc[pdefs_idx..web_pos],
        _ => "",
    };
    let web_raw = match pweb {
        Some(web_pos) => {
            let start = web_pos + "网络释义：".len();
            let end_rel = desc[start..]
                .find(['；', ';'])
                .unwrap_or(desc.len() - start);
            &desc[start..start + end_rel]
        }
        None => "",
    };

    let mut buf = String::from(word);
    if !uk.is_empty() || !us.is_empty() {
        buf.push_str("  ");
        if !uk.is_empty() {
            buf.push('英');
            buf.push_str(uk);
        }
        if !uk.is_empty() && !us.is_empty() {
            buf.push(' ');
        }
        if !us.is_empty() {
            buf.push('美');
            buf.push_str(us);
        }
    }
    let tdefs = trim_cn(defs_raw);
    let tweb = trim_cn(web_raw);
    if !tdefs.is_empty() {
        buf.push('\n');
        buf.push_str(tdefs);
    }
    if !tweb.is_empty() {
        buf.push_str("\n网络释义：");
        buf.push_str(tweb);
    }
    Some(buf)
}

fn dict_lookup(word: &str) -> Option<String> {
    let encw = urlencode(word);
    let url = format!("https://cn.bing.com/dict/search?q={encw}");
    let page = http_get(&url, Some("Referer: https://cn.bing.com/dict\r\n"))?;
    let marker = "<meta name=\"description\" content=\"";
    let start = page.find(marker)? + marker.len();
    let end = page[start..].find('"')?;
    if end >= 6000 {
        return None;
    }
    parse_dict_meta_desc(&page[start..start + end])
}

fn is_plain_word(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|b| b.is_ascii_alphabetic() || b == b'-' || b == b'\'')
}

/// 必应翻译（可选跳过词典查询）
pub fn translate_bing_ex(utf8_text: &str, mode: i32, no_dict: bool) -> Option<String> {
    if utf8_text.is_empty() {
        set_error("请输入要翻译的内容");
        return None;
    }
    let trimmed = utf8_text.trim_matches([' ', '\t', '\r', '\n']);
    let (from, to) = match mode {
        FY_ZH2EN => ("zh-Hans", "en"),
        FY_EN2ZH => ("en", "zh-Hans"),
        _ => {
            if has_cjk(trimmed) {
                ("zh-Hans", "en")
            } else {
                ("en", "zh-Hans")
            }
        }
    };

    if !no_dict && is_plain_word(trimmed) && to == "zh-Hans" {
        if let Some(mut dict_out) = dict_lookup(trimmed) {
            if let Some(tr) = ttranslate(trimmed, from, to) {
                dict_out.push_str("\n译：");
                dict_out.push_str(&tr);
            }
            return Some(dict_out);
        }
    }
    ttranslate(trimmed, from, to)
}

/// 必应翻译默认入口（含英文单词词典增强）
pub fn translate_bing(utf8_text: &str, mode: i32) -> Option<String> {
    translate_bing_ex(utf8_text, mode, false)
}

#[allow(clippy::too_many_arguments)]
fn translate_chain_impl<F>(
    utf8_text: &str,
    mode: i32,
    ai_url: &str,
    ai_key: &str,
    ai_model: &str,
    baidu_appid: &str,
    baidu_secret: &str,
    baidu_url: &str,
    flags: i32,
    mut bing_fn: F,
) -> Option<String>
where
    F: FnMut(&str, i32, bool) -> Option<String>,
{
    if utf8_text.is_empty() {
        set_error("请输入要翻译的内容");
        return None;
    }

    let has_ai = !ai_url.is_empty() && !ai_key.is_empty();
    let has_bd = !baidu_appid.is_empty() && !baidu_secret.is_empty();
    let prefer_bd = (flags & FY_CHAIN_PREFER_BAIDU) != 0;
    let no_dict = (flags & FY_CHAIN_NO_DICT) != 0;

    log_write(
        "链路",
        &format!(
            "开始 文本{}字节 mode={mode} AI={} 百度={} flags={flags}",
            utf8_text.len(),
            usize::from(has_ai),
            usize::from(has_bd)
        ),
    );

    let mut notes = String::new();
    set_engine("");

    let order = if prefer_bd { [1, 2, 0] } else { [0, 1, 2] };
    for kind in order {
        if kind == 0 && has_ai {
            log_write("链路", "AI 请求中");
            if let Some(res) = translate_ai(utf8_text, mode, ai_url, ai_key, ai_model) {
                set_engine("AI");
                log_write("链路", &format!("AI 成功 {}字节", res.len()));
                return Some(res);
            }
            notes.push_str(&format!("AI：{}；", last_error()));
        } else if kind == 1 && has_bd {
            log_write("链路", "百度 请求中");
            if let Some(res) =
                translate_baidu(utf8_text, mode, baidu_appid, baidu_secret, baidu_url)
            {
                set_engine("百度");
                log_write("链路", &format!("百度 成功 {}字节", res.len()));
                return Some(res);
            }
            notes.push_str(&format!("百度：{}；", last_error()));
        } else if kind == 2 {
            log_write(
                "链路",
                if prefer_bd {
                    "必应 请求中"
                } else {
                    "必应兜底 请求中"
                },
            );
            if let Some(res) = bing_fn(utf8_text, mode, no_dict) {
                set_engine("必应");
                if !prefer_bd && !notes.is_empty() {
                    let clean_notes = notes.strip_suffix('；').unwrap_or(&notes);
                    return Some(format!("（{clean_notes}。已用必应翻译）\n{res}"));
                }
                return Some(res);
            }
            notes.push_str(&format!("必应：{}；", last_error()));
        }
    }

    log_write("链路", "全部失败");
    None
}

/// 多引擎回退链（支持控制 `FY_CHAIN_NO_DICT` 与 `FY_CHAIN_PREFER_BAIDU`）
#[allow(clippy::too_many_arguments)]
pub fn translate_chain_ex(
    utf8_text: &str,
    mode: i32,
    ai_url: &str,
    ai_key: &str,
    ai_model: &str,
    baidu_appid: &str,
    baidu_secret: &str,
    baidu_url: &str,
    flags: i32,
) -> Option<String> {
    translate_chain_impl(
        utf8_text,
        mode,
        ai_url,
        ai_key,
        ai_model,
        baidu_appid,
        baidu_secret,
        baidu_url,
        flags,
        translate_bing_ex,
    )
}

/// 多引擎回退链默认入口：`AI → 百度 → 必应`
#[allow(clippy::too_many_arguments)]
pub fn translate_chain(
    utf8_text: &str,
    mode: i32,
    ai_url: &str,
    ai_key: &str,
    ai_model: &str,
    baidu_appid: &str,
    baidu_secret: &str,
    baidu_url: &str,
) -> Option<String> {
    translate_chain_ex(
        utf8_text,
        mode,
        ai_url,
        ai_key,
        ai_model,
        baidu_appid,
        baidu_secret,
        baidu_url,
        0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::baidu::test_mock::{read_http_request, spawn_baidu_mock};
    use crate::core::last_engine;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn test_html_unescape_and_dict_meta_parser() {
        assert_eq!(
            html_unescape("a &amp; b &quot;c&quot; &#39;d&#39; &lt;e&gt;"),
            "a & b \"c\" 'd' <e>"
        );
        let meta = "必应词典为您提供hello的释义，美[həˈloʊ]，英[həˈləʊ]，int. 你好；喂；； 网络释义：哈喽；你好吗；";
        let parsed = parse_dict_meta_desc(meta).expect("parse dict meta");
        assert!(parsed.starts_with("hello  英[həˈləʊ] 美[həˈloʊ]"));
        assert!(parsed.contains("\nint. 你好；喂"));
        assert!(parsed.contains("\n网络释义：哈喽"));
    }

    #[test]
    fn test_chain_fallback_and_baidu_short_circuit() {
        let _guard = crate::core::TEST_GLOBAL_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        // 1. Mock 百度成功时在 PREFER_BAIDU 模式下短路，不触及必应或 AI
        let (bd_url, bd_handle) = spawn_baidu_mock("ok_id", "ok_sec", 1);
        let res1 = translate_chain_impl(
            "hello",
            FY_EN2ZH,
            "",
            "",
            "",
            "ok_id",
            "ok_sec",
            &bd_url,
            FY_CHAIN_PREFER_BAIDU | FY_CHAIN_NO_DICT,
            |_, _, _| unreachable!("should not fallback to bing"),
        );
        assert_eq!(res1, Some("译文1\n译文2".to_string()));
        assert_eq!(last_engine(), "百度");
        bd_handle.join().expect("join baidu");

        // 2. Mock AI(401) + Mock 百度(52003) 均失败，回退至 stub 必应并收集错误前缀
        let ai_listener = TcpListener::bind("127.0.0.1:0").expect("bind ai");
        let ai_port = ai_listener.local_addr().expect("ai addr").port();
        let ai_handle = thread::spawn(move || {
            let (mut stream, _) = ai_listener.accept().expect("accept");
            let _ = read_http_request(&mut stream);
            let body = r#"{"error":{"message":"invalid_api_key"}}"#;
            let resp = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        });
        let (bd_fail_url, bd_fail_handle) = spawn_baidu_mock("good_id", "good_sec", 1);

        let res2 = translate_chain_impl(
            "hello",
            FY_EN2ZH,
            &format!("http://127.0.0.1:{ai_port}"),
            "bad-key",
            "deepseek-chat",
            "wrong_id",
            "wrong_sec",
            &bd_fail_url,
            0,
            |_, _, no_dict| {
                assert!(!no_dict);
                Some("你好".to_string())
            },
        )
        .expect("fallback result");

        assert_eq!(last_engine(), "必应");
        assert!(res2.starts_with(
            "（AI：API 密钥无效（401）；百度：百度接口：密钥或 AppID 无效。已用必应翻译）\n你好"
        ));

        ai_handle.join().expect("join ai");
        bd_fail_handle.join().expect("join bd fail");
    }
}
