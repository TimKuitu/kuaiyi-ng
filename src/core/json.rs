//! 极简 JSON 路径取值器与字符串转义工具
//!
//! 用法：
//! - `json_str(json, "[0].translations[0].text")`
//! - `json_str(json, "detectedLanguage.language")`
//! - `json_num(json, "usage.total_tokens")`

struct Jctx<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Jctx<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            bytes: s.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }
}

fn hex4(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 4 {
        return None;
    }
    let mut v = 0u32;
    for &b in &bytes[..4] {
        v <<= 4;
        v |= match b {
            b'0'..=b'9' => (b - b'0') as u32,
            b'a'..=b'f' => (b - b'a' + 10) as u32,
            b'A'..=b'F' => (b - b'A' + 10) as u32,
            _ => return None,
        };
    }
    Some(v)
}

fn parse_jstring(j: &mut Jctx<'_>) -> Option<String> {
    if j.peek() != Some(b'"') {
        return None;
    }
    j.pos += 1;
    let mut raw_bytes: Vec<u8> = Vec::new();
    while j.pos < j.bytes.len() {
        let b = j.bytes[j.pos];
        if b == b'"' {
            j.pos += 1;
            return Some(String::from_utf8_lossy(&raw_bytes).into_owned());
        }
        if b == b'\\' {
            j.pos += 1;
            let esc = j.peek()?;
            match esc {
                b'"' => {
                    raw_bytes.push(b'"');
                    j.pos += 1;
                }
                b'\\' => {
                    raw_bytes.push(b'\\');
                    j.pos += 1;
                }
                b'/' => {
                    raw_bytes.push(b'/');
                    j.pos += 1;
                }
                b'b' => {
                    raw_bytes.push(0x08);
                    j.pos += 1;
                }
                b'f' => {
                    raw_bytes.push(0x0c);
                    j.pos += 1;
                }
                b'n' => {
                    raw_bytes.push(b'\n');
                    j.pos += 1;
                }
                b'r' => {
                    raw_bytes.push(b'\r');
                    j.pos += 1;
                }
                b't' => {
                    raw_bytes.push(b'\t');
                    j.pos += 1;
                }
                b'u' => {
                    let mut cp = hex4(&j.bytes[j.pos + 1..])?;
                    j.pos += 5;
                    if (0xD800..=0xDBFF).contains(&cp)
                        && j.bytes.get(j.pos) == Some(&b'\\')
                        && j.bytes.get(j.pos + 1) == Some(&b'u')
                    {
                        if let Some(lo) = hex4(&j.bytes[j.pos + 2..]) {
                            if (0xDC00..=0xDFFF).contains(&lo) {
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                j.pos += 6;
                            }
                        }
                    }
                    if let Some(ch) = char::from_u32(cp) {
                        let mut tmp = [0u8; 4];
                        let s = ch.encode_utf8(&mut tmp);
                        raw_bytes.extend_from_slice(s.as_bytes());
                    }
                }
                _ => return None,
            }
        } else {
            raw_bytes.push(b);
            j.pos += 1;
        }
    }
    None
}

fn skip_quoted_raw(j: &mut Jctx<'_>) {
    j.pos += 1;
    while j.pos < j.bytes.len() {
        let b = j.bytes[j.pos];
        if b == b'"' {
            j.pos += 1;
            return;
        }
        if b == b'\\' {
            j.pos += 1;
            if j.pos < j.bytes.len() {
                j.pos += 1;
            }
        } else {
            j.pos += 1;
        }
    }
}

fn skip_value(j: &mut Jctx<'_>) -> bool {
    j.skip_ws();
    match j.peek() {
        Some(b'"') => {
            skip_quoted_raw(j);
            true
        }
        Some(b'{') => {
            let mut depth = 0i32;
            while j.pos < j.bytes.len() {
                match j.bytes[j.pos] {
                    b'"' => skip_quoted_raw(j),
                    b'{' => {
                        depth += 1;
                        j.pos += 1;
                    }
                    b'}' => {
                        depth -= 1;
                        j.pos += 1;
                        if depth == 0 {
                            return true;
                        }
                    }
                    _ => j.pos += 1,
                }
            }
            false
        }
        Some(b'[') => {
            let mut depth = 0i32;
            while j.pos < j.bytes.len() {
                match j.bytes[j.pos] {
                    b'"' => skip_quoted_raw(j),
                    b'[' => {
                        depth += 1;
                        j.pos += 1;
                    }
                    b']' => {
                        depth -= 1;
                        j.pos += 1;
                        if depth == 0 {
                            return true;
                        }
                    }
                    _ => j.pos += 1,
                }
            }
            false
        }
        Some(_) => {
            while let Some(b) = j.peek() {
                if matches!(b, b',' | b'}' | b']') {
                    break;
                }
                j.pos += 1;
            }
            true
        }
        None => false,
    }
}

fn walk_object_key(j: &mut Jctx<'_>, target_key: &str) -> bool {
    if j.peek() != Some(b'{') {
        return false;
    }
    j.pos += 1;
    loop {
        j.skip_ws();
        if j.peek() == Some(b'}') {
            return false;
        }
        let Some(cur_key) = parse_jstring(j) else {
            return false;
        };
        j.skip_ws();
        if j.peek() != Some(b':') {
            return false;
        }
        j.pos += 1;
        j.skip_ws();
        if cur_key == target_key {
            return true;
        }
        if !skip_value(j) {
            return false;
        }
        j.skip_ws();
        match j.peek() {
            Some(b',') => j.pos += 1,
            _ => return false,
        }
    }
}

fn walk_path(j: &mut Jctx<'_>, path: &str) -> bool {
    j.skip_ws();
    let p = path.as_bytes();
    let mut pi = 0usize;

    while pi < p.len() {
        j.skip_ws();
        if p[pi] == b'[' {
            if j.peek() != Some(b'[') {
                return false;
            }
            pi += 1;
            let num_start = pi;
            while pi < p.len() && p[pi].is_ascii_digit() {
                pi += 1;
            }
            let Ok(idx) = std::str::from_utf8(&p[num_start..pi])
                .unwrap_or("")
                .parse::<usize>()
            else {
                return false;
            };
            if pi < p.len() && p[pi] == b']' {
                pi += 1;
            }
            j.pos += 1;
            j.skip_ws();
            if j.peek() == Some(b']') {
                return false;
            }
            for _ in 0..idx {
                if !skip_value(j) {
                    return false;
                }
                j.skip_ws();
                if j.peek() != Some(b',') {
                    return false;
                }
                j.pos += 1;
                j.skip_ws();
            }
        } else if p[pi] == b'.' {
            pi += 1;
            if pi < p.len() && p[pi] == b'[' {
                // ".[0]" 归一化为 "[0]"，下轮走 '[' 分支
                continue;
            }
            let key_start = pi;
            while pi < p.len() && p[pi] != b'.' && p[pi] != b'[' {
                pi += 1;
            }
            let key = std::str::from_utf8(&p[key_start..pi]).unwrap_or("");
            if !walk_object_key(j, key) {
                return false;
            }
        } else if matches!(j.peek(), Some(b'{') | Some(b'[')) {
            let key_start = pi;
            while pi < p.len() && p[pi] != b'.' && p[pi] != b'[' {
                pi += 1;
            }
            let key = std::str::from_utf8(&p[key_start..pi]).unwrap_or("");
            if !walk_object_key(j, key) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

/// 按路径从 JSON 中读取字符串值
pub fn json_str(json: &str, path: &str) -> Option<String> {
    if json.is_empty() || path.is_empty() {
        return None;
    }
    let mut j = Jctx::new(json);
    if !walk_path(&mut j, path) {
        return None;
    }
    j.skip_ws();
    if j.peek() != Some(b'"') {
        return None;
    }
    parse_jstring(&mut j)
}

/// 按路径从 JSON 中读取数值
pub fn json_num(json: &str, path: &str) -> Option<f64> {
    if json.is_empty() || path.is_empty() {
        return None;
    }
    let mut j = Jctx::new(json);
    if !walk_path(&mut j, path) {
        return None;
    }
    j.skip_ws();
    let start = j.pos;
    while let Some(b) = j.peek() {
        if b.is_ascii_digit() || matches!(b, b'-' | b'+' | b'.' | b'e' | b'E') {
            j.pos += 1;
        } else {
            break;
        }
    }
    if j.pos == start {
        return None;
    }
    std::str::from_utf8(&j.bytes[start..j.pos])
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
}

/// 将字符串转义为可直接嵌入 JSON 双引号内的安全文本
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_str_paths_and_escapes() {
        let bing_resp = r#"[{"detectedLanguage":{"language":"en","score":1.0},"translations":[{"text":"你好\n世界","to":"zh-Hans","transliteration":{"text":"nǐ hǎo"}}]}]"#;
        assert_eq!(
            json_str(bing_resp, "[0].translations[0].text"),
            Some("你好\n世界".to_string())
        );
        assert_eq!(json_str(bing_resp, "[0].[0].text"), None);
        assert_eq!(
            json_str(bing_resp, "[0].translations.[0].transliteration.text"),
            Some("nǐ hǎo".to_string())
        );
        assert_eq!(json_num(bing_resp, "[0].detectedLanguage.score"), Some(1.0));

        let baidu_resp = r#"{"from":"en","to":"zh","trans_result":[{"src":"hello","dst":"\u4f60\u597d\uD83D\uDE00"}]}"#;
        assert_eq!(
            json_str(baidu_resp, "trans_result[0].dst"),
            Some("你好😀".to_string())
        );

        let openai_resp = r#"{"id":"1","choices":[{"index":0,"message":{"role":"assistant","content":"\"quoted\"\\path"}}]}"#;
        assert_eq!(
            json_str(openai_resp, "choices[0].message.content"),
            Some("\"quoted\"\\path".to_string())
        );
    }

    #[test]
    fn test_json_escape() {
        assert_eq!(
            json_escape("a\"b\\c\nd\r\t\x01"),
            "a\\\"b\\\\c\\nd\\r\\t\\u0001"
        );
    }
}
