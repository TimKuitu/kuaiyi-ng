//! 单词命名格式化：多词空格分隔 → 原样/小写/大写/驼峰/下划线/中划线

use crate::core::{FY_FMT_CAMEL, FY_FMT_KEBAB, FY_FMT_LOWER, FY_FMT_SNAKE, FY_FMT_UPPER};

const MAX_WORDS: usize = 32;

fn cap_first_ascii(w: &str) -> String {
    let mut bytes = w.as_bytes().to_vec();
    if let Some(first) = bytes.first_mut() {
        if first.is_ascii_lowercase() {
            *first -= 32;
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| w.to_string())
}

/// 将空格/空白分隔的短语按旧版 `FY_FMT_*` 格式常量转换为拼接字符串
pub fn format_word(utf8_text: &str, fmt: i32) -> Option<String> {
    let words: Vec<&str> = utf8_text
        .split([' ', '\t', '\r', '\n'])
        .filter(|s| !s.is_empty())
        .take(MAX_WORDS)
        .collect();
    if words.is_empty() {
        return None;
    }

    let transformed: Vec<String> = match fmt {
        FY_FMT_LOWER | FY_FMT_SNAKE | FY_FMT_KEBAB => {
            words.iter().map(|w| w.to_ascii_lowercase()).collect()
        }
        FY_FMT_UPPER => words.iter().map(|w| w.to_ascii_uppercase()).collect(),
        FY_FMT_CAMEL => {
            let mut v = Vec::with_capacity(words.len());
            v.push(words[0].to_ascii_lowercase());
            for w in &words[1..] {
                v.push(cap_first_ascii(w));
            }
            v
        }
        _ => words.iter().map(|w| (*w).to_string()).collect(),
    };

    let sep = match fmt {
        FY_FMT_SNAKE => "_",
        FY_FMT_KEBAB => "-",
        FY_FMT_CAMEL => "",
        _ => " ",
    };
    Some(transformed.join(sep))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::FY_FMT_RAW;

    #[test]
    fn test_format_word_all_modes() {
        let src = "  Hello   World_test  ";
        assert_eq!(
            format_word(src, FY_FMT_RAW),
            Some("Hello World_test".into())
        );
        assert_eq!(
            format_word(src, FY_FMT_LOWER),
            Some("hello world_test".into())
        );
        assert_eq!(
            format_word(src, FY_FMT_UPPER),
            Some("HELLO WORLD_TEST".into())
        );
        assert_eq!(
            format_word("hello world test", FY_FMT_CAMEL),
            Some("helloWorldTest".into())
        );
        assert_eq!(
            format_word("Hello World", FY_FMT_SNAKE),
            Some("hello_world".into())
        );
        assert_eq!(
            format_word("Hello World", FY_FMT_KEBAB),
            Some("hello-world".into())
        );
        assert_eq!(format_word("   \t\n ", FY_FMT_CAMEL), None);
    }
}
