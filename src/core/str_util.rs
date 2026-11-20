//! 字符串与编码工具：UTF-8 / GBK / UTF-16 / URL 编码 / CJK 检测

use crate::core::win32::{MultiByteToWideChar, WideCharToMultiByte, CP_GBK, CP_UTF8};
use std::ptr::{null, null_mut};

/// 将 UTF-8 字符串转换为以 `\0` 结尾的 UTF-16 序列
pub fn utf8_to_wide(s: &str) -> Vec<u16> {
    let mut out: Vec<u16> = s.encode_utf16().collect();
    out.push(0);
    out
}

/// 将 UTF-16 切片（遇到首个 0 截断）解码为 UTF-8 字符串
pub fn wide_to_utf8(w: &[u16]) -> String {
    let len = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..len])
}

fn mb_to_wide(cp: u32, bytes: &[u8]) -> Option<Vec<u16>> {
    if bytes.is_empty() {
        return Some(vec![0]);
    }
    let Ok(in_len) = i32::try_from(bytes.len()) else {
        return None;
    };
    // SAFETY: MultiByteToWideChar 仅读取长度为 in_len 的输入字节切片，首次调用传入 null 查询所需宽字符长度
    let needed = unsafe { MultiByteToWideChar(cp, 0, bytes.as_ptr(), in_len, null_mut(), 0) };
    if needed <= 0 {
        return None;
    }
    let mut wide = vec![0u16; needed as usize + 1];
    // SAFETY: wide 容量为 needed + 1，足以容纳转换后的 UTF-16 字符
    let written =
        unsafe { MultiByteToWideChar(cp, 0, bytes.as_ptr(), in_len, wide.as_mut_ptr(), needed) };
    if written <= 0 {
        return None;
    }
    wide.truncate(written as usize);
    wide.push(0);
    Some(wide)
}

/// GBK（代码页 936）字节序列转 UTF-8 字符串
pub fn gbk_to_utf8(gbk_bytes: &[u8]) -> Option<String> {
    // 截断至首个 NUL 字节，保持与 C 字符串语义一致
    let end = gbk_bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(gbk_bytes.len());
    let slice = &gbk_bytes[..end];
    if slice.is_empty() {
        return Some(String::new());
    }
    let wide = mb_to_wide(CP_GBK, slice)?;
    let wlen = wide.len().saturating_sub(1);
    Some(String::from_utf16_lossy(&wide[..wlen]))
}

/// UTF-8 字符串转 GBK（代码页 936）字节序列（不含末尾 `\0`）
pub fn utf8_to_gbk(s: &str) -> Option<Vec<u8>> {
    if s.is_empty() {
        return Some(Vec::new());
    }
    let wide = utf8_to_wide(s);
    let wlen = i32::try_from(wide.len().saturating_sub(1)).ok()?;
    if wlen == 0 {
        return Some(Vec::new());
    }
    // SAFETY: wide 为有效 UTF-16 切片，首次调用传入 null 查询所需 GBK 字节数
    let needed = unsafe {
        WideCharToMultiByte(
            CP_GBK,
            0,
            wide.as_ptr(),
            wlen,
            null_mut(),
            0,
            null(),
            null_mut(),
        )
    };
    if needed <= 0 {
        return None;
    }
    let mut out = vec![0u8; needed as usize];
    // SAFETY: out 已按 Win32 返回大小分配
    let written = unsafe {
        WideCharToMultiByte(
            CP_GBK,
            0,
            wide.as_ptr(),
            wlen,
            out.as_mut_ptr(),
            needed,
            null(),
            null_mut(),
        )
    };
    if written <= 0 {
        return None;
    }
    out.truncate(written as usize);
    Some(out)
}

/// 系统 ANSI 代码页或 UTF-8 解码辅助（优先保留原 UTF-8，与 `CP_UTF8` 对齐）
pub fn decode_utf8_c_bytes(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// URL 编码（表单体）：空格编码为 `%20`，非安全字符逐字节百分号编码（大写十六进制）
pub fn urlencode(s: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else if b == b' ' {
            out.push_str("%20");
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
    }
    out
}

/// 判断 UTF-8 字符串是否包含 CJK 字符（简繁中文/日文假名汉字）
pub fn has_cjk(utf8: &str) -> bool {
    utf8.chars().any(|ch| {
        let cp = ch as u32;
        (0x4E00..=0x9FFF).contains(&cp)
            || (0x3400..=0x4DBF).contains(&cp)
            || (0xF900..=0xFAFF).contains(&cp)
            || (0x3040..=0x30FF).contains(&cp)
    })
}

/// 确保字符串供 C Win32 `*A` 函数使用（含末尾 `\0`），内部截断或保留 UTF-8
pub fn to_c_bytes(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend_from_slice(s.as_bytes());
    v.push(0);
    v
}

/// 保证常量引用有效时将含 `CP_UTF8` 编解码供内部自测
pub fn roundtrip_cp_utf8(s: &str) -> Option<String> {
    let wide = mb_to_wide(CP_UTF8, s.as_bytes())?;
    Some(wide_to_utf8(&wide))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gbk_utf8_roundtrip() {
        let src = "快译NG 测试目录\\配置_123";
        let gbk = utf8_to_gbk(src).expect("utf8_to_gbk failed");
        assert_ne!(gbk, src.as_bytes());
        let decoded = gbk_to_utf8(&gbk).expect("gbk_to_utf8 failed");
        assert_eq!(decoded, src);
        assert_eq!(gbk_to_utf8(b""), Some(String::new()));
        assert_eq!(utf8_to_gbk(""), Some(Vec::new()));
        assert_eq!(
            roundtrip_cp_utf8("你好 Rust"),
            Some("你好 Rust".to_string())
        );
    }

    #[test]
    fn test_urlencode_and_has_cjk() {
        assert_eq!(urlencode("hello world!"), "hello%20world%21");
        assert_eq!(urlencode("a-b_c.d~1"), "a-b_c.d~1");
        assert_eq!(urlencode("中文"), "%E4%B8%AD%E6%96%87");

        assert!(has_cjk("hello 世界"));
        assert!(has_cjk("テスト"));
        assert!(!has_cjk("Hello, World! 123"));
        assert!(!has_cjk(""));
    }
}
