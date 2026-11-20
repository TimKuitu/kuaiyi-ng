//! 查询历史：缓存目录下 history.txt（UTF-8，每行一条，最新在最后，上限 200）

use std::fs;
use std::path::{Path, PathBuf};

pub const HIST_MAX: usize = 200;

/// 查询历史管理器
pub struct History {
    file_path: PathBuf,
    items: Vec<String>,
}

impl History {
    /// 打开指定缓存目录下的 `history.txt`（自动创建目录）
    pub fn open<P: AsRef<Path>>(dir: P) -> Option<Self> {
        let dir_path = dir.as_ref();
        if dir_path.as_os_str().is_empty() {
            return None;
        }
        fs::create_dir_all(dir_path).ok()?;
        let file_path = dir_path.join("history.txt");
        let mut items = Vec::new();
        if let Ok(content) = fs::read_to_string(&file_path) {
            for line in content.lines() {
                let clean = line.trim_end_matches('\r');
                if !clean.is_empty() {
                    items.push(clean.to_string());
                }
            }
        }
        Some(Self { file_path, items })
    }

    /// 当前历史条数
    pub fn count(&self) -> usize {
        self.items.len()
    }

    /// 按索引读取历史条目（`0` 为最旧，`count - 1` 为最新）
    pub fn get(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(String::as_str)
    }

    fn save(&self) {
        let mut out = String::new();
        for item in &self.items {
            out.push_str(item);
            out.push_str("\r\n");
        }
        let _ = fs::write(&self.file_path, out.as_bytes());
    }

    /// 追加历史记录：去重（旧条目删除后移到末尾），超过 `HIST_MAX` 淘汰最旧并立即存盘
    pub fn push(&mut self, text: &str) {
        let trimmed = text.trim_end_matches([' ', '\t', '\r', '\n']);
        if trimmed.is_empty() {
            return;
        }
        self.items.retain(|existing| existing != trimmed);
        self.items.push(trimmed.to_string());
        if self.items.len() > HIST_MAX {
            let excess = self.items.len() - HIST_MAX;
            self.items.drain(0..excess);
        }
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_dedup_limit_and_chinese_dir() {
        let dir = std::env::temp_dir().join("ky_rust_hist_中文目录");
        let _ = fs::remove_dir_all(&dir);

        {
            let mut h = History::open(&dir).expect("open history");
            h.push("hello");
            h.push("world");
            h.push("测试  \r\n");
            h.push("hello");
            assert_eq!(h.count(), 3);
            assert_eq!(h.get(0), Some("world"));
            assert_eq!(h.get(1), Some("测试"));
            assert_eq!(h.get(2), Some("hello"));

            for i in 0..210 {
                h.push(&format!("item_{i}"));
            }
            assert_eq!(h.count(), HIST_MAX);
            assert_eq!(h.get(0), Some("item_10"));
            assert_eq!(h.get(HIST_MAX - 1), Some("item_209"));
        }

        // 重开验证持久化
        let h2 = History::open(&dir).expect("reopen history");
        assert_eq!(h2.count(), HIST_MAX);
        assert_eq!(h2.get(0), Some("item_10"));
        assert_eq!(h2.get(HIST_MAX - 1), Some("item_209"));

        let _ = fs::remove_dir_all(&dir);
    }
}
