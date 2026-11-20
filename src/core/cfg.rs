//! INI 配置读写：GBK 文件格式，未知段/未知键原样保留

use crate::core::str_util::{gbk_to_utf8, utf8_to_gbk};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// INI 配置对象（内存全量以 UTF-8 行保存，存盘转码为 GBK）
pub struct Config {
    lines: Vec<String>,
    path: PathBuf,
    dirty: bool,
}

fn split_lines_preserve_c_behavior(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && bytes[i] != b'\r' && bytes[i] != b'\n' {
            i += 1;
        }
        lines.push(text[start..i].to_string());
        while i < bytes.len() && (bytes[i] == b'\r' || bytes[i] == b'\n') {
            i += 1;
        }
    }
    lines
}

impl Config {
    /// 打开或初始化配置文件（自动识别 UTF-8 BOM 或 GBK 编码）
    pub fn open<P: AsRef<Path>>(path: P) -> Self {
        let pb = path.as_ref().to_path_buf();
        let mut lines = Vec::new();
        if let Ok(raw) = fs::read(&pb) {
            if raw.len() <= 4 * 1024 * 1024 {
                let utf8_opt = if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    Some(String::from_utf8_lossy(&raw[3..]).into_owned())
                } else {
                    gbk_to_utf8(&raw)
                };
                if let Some(u) = utf8_opt {
                    lines = split_lines_preserve_c_behavior(&u);
                }
            }
        }
        Self {
            lines,
            path: pb,
            dirty: false,
        }
    }

    fn find_key(&self, sec: &str, key: &str) -> (Option<usize>, usize) {
        let mut in_sec = false;
        let mut sec_end = self.lines.len();
        for (i, line) in self.lines.iter().enumerate() {
            let trimmed = line.trim_end_matches([' ', '\t']);
            if let Some(rest) = trimmed.strip_prefix('[') {
                if let Some(cb) = rest.find(']') {
                    if &rest[..cb] == sec {
                        in_sec = true;
                        sec_end = self.lines.len();
                        continue;
                    }
                }
                if in_sec {
                    sec_end = i;
                    return (None, sec_end);
                }
                continue;
            }
            if !in_sec {
                continue;
            }
            sec_end = self.lines.len();
            if let Some(after_key) = line.strip_prefix(key) {
                if after_key.starts_with('=') {
                    return (Some(i), sec_end);
                }
            }
        }
        (None, sec_end)
    }

    /// 读取指定段/键；未命中返回 `(default, false)`，命中返回 `(value, true)`
    pub fn get(&self, sec: &str, key: &str, default: &str) -> (String, bool) {
        if sec.is_empty() || key.is_empty() {
            return (default.to_string(), false);
        }
        let (idx, _) = self.find_key(sec, key);
        match idx {
            Some(i) => {
                let val = &self.lines[i][key.len() + 1..];
                (val.to_string(), true)
            }
            None => (default.to_string(), false),
        }
    }

    /// 读取字符串配置（便捷方法）
    pub fn get_str(&self, sec: &str, key: &str, default: &str) -> String {
        self.get(sec, key, default).0
    }

    /// 写入或更新配置键值（未知段/键原样保留）
    pub fn set(&mut self, sec: &str, key: &str, val: &str) {
        if sec.is_empty() || key.is_empty() {
            return;
        }
        let (idx, sec_end) = self.find_key(sec, key);
        let new_line = format!("{key}={val}");
        if let Some(i) = idx {
            self.lines[i] = new_line;
        } else if sec_end < self.lines.len() {
            self.lines.insert(sec_end, new_line);
        } else {
            let hdr = format!("[{sec}]");
            let has_sec = self.lines.iter().any(|l| l.starts_with(&hdr));
            if !has_sec {
                if !self.lines.is_empty() {
                    self.lines.push(String::new());
                }
                self.lines.push(hdr);
            }
            self.lines.push(new_line);
        }
        self.dirty = true;
    }

    /// 是否有未落盘修改
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// 以 GBK 编码 + `\r\n` 换行符保存到文件
    pub fn save(&mut self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let mut bytes = Vec::new();
        for line in &self.lines {
            if let Some(gbk) = utf8_to_gbk(line) {
                bytes.extend_from_slice(&gbk);
            } else {
                bytes.extend_from_slice(line.as_bytes());
            }
            bytes.extend_from_slice(b"\r\n");
        }
        fs::write(&self.path, bytes)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_gbk_roundtrip_and_preserve_unknown() {
        let path = std::env::temp_dir().join("ky_rust_cfg_test.ini");
        let _ = fs::remove_file(&path);

        // 构造带未知段和注释的 GBK 配置文件
        let initial_utf8 =
            "# 注释保留\r\n[通用设置]\r\n窗口宽=550\r\n未知键=abc\r\n\r\n[未知段落]\r\nfoo=bar\r\n";
        let gbk = utf8_to_gbk(initial_utf8).expect("encode gbk");
        fs::write(&path, gbk).expect("write initial gbk");

        let mut cfg = Config::open(&path);
        assert_eq!(cfg.get("通用设置", "窗口宽", "?"), ("550".into(), true));
        assert_eq!(
            cfg.get("通用设置", "不存在的键", "默认值"),
            ("默认值".into(), false)
        );

        cfg.set("通用设置", "窗口置顶", "1");
        cfg.set("通用设置", "缓存目录", "D:\\某目录");
        cfg.set("新段落", "测试键", "中文值");
        assert!(cfg.is_dirty());
        cfg.save().expect("save cfg");
        assert!(!cfg.is_dirty());

        // 确认写盘确为 GBK 编码且保留了未知段落与未知键
        let raw_saved = fs::read(&path).expect("read saved");
        assert!(!raw_saved.starts_with(&[0xEF, 0xBB, 0xBF]));
        let cfg2 = Config::open(&path);
        assert_eq!(cfg2.get_str("通用设置", "未知键", ""), "abc");
        assert_eq!(cfg2.get_str("通用设置", "缓存目录", ""), "D:\\某目录");
        assert_eq!(cfg2.get_str("未知段落", "foo", ""), "bar");
        assert_eq!(cfg2.get_str("新段落", "测试键", ""), "中文值");

        // UTF-8 BOM 兼容读取测试
        let bom_path = std::env::temp_dir().join("ky_rust_cfg_bom.ini");
        let mut bom_bytes = vec![0xEF, 0xBB, 0xBF];
        bom_bytes.extend_from_slice("[通用设置]\r\n字体缩放=150\r\n".as_bytes());
        fs::write(&bom_path, bom_bytes).expect("write bom");
        let cfg_bom = Config::open(&bom_path);
        assert_eq!(cfg_bom.get_str("通用设置", "字体缩放", ""), "150");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&bom_path);
    }
}
