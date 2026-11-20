//! 快译NG 核心逻辑层（纯 Rust 重构版）
//!
//! 覆盖原 `core/*.c` 全部能力：
//! - Wave 1：`md5`, `str_util`, `json`, `fmt`
//! - Wave 2：`cfg`, `hist`, `log`
//! - Wave 3：`http`, `baidu`, `ai`, `bing`, `update`
//! - Wave 4：`ocr`

pub mod ai;
pub mod baidu;
pub mod bing;
pub mod cfg;
pub mod fmt;
pub mod hist;
pub mod http;
pub mod json;
pub mod log;
pub mod md5;
pub mod ocr;
pub mod str_util;
pub mod update;
pub mod win32;

use std::sync::{LazyLock, Mutex};

/// 翻译方向：按内容自动判断
pub const FY_AUTO: i32 = 0;
/// 翻译方向：中译英
pub const FY_ZH2EN: i32 = 1;
/// 翻译方向：英译中
pub const FY_EN2ZH: i32 = 2;

/// 单词命名格式（旧版兼容）
pub const FY_FMT_RAW: i32 = 0;
pub const FY_FMT_LOWER: i32 = 1;
pub const FY_FMT_UPPER: i32 = 2;
pub const FY_FMT_CAMEL: i32 = 3;
pub const FY_FMT_SNAKE: i32 = 4;
pub const FY_FMT_KEBAB: i32 = 5;

/// 回退链标志：跳过单词词典查询，仅做文本翻译
pub const FY_CHAIN_NO_DICT: i32 = 1;
/// 回退链标志：优先百度 → 必应 → AI（适合低延迟截屏 OCR）
pub const FY_CHAIN_PREFER_BAIDU: i32 = 2;

struct CoreStatus {
    last_error: String,
    last_engine: String,
}

static CORE_STATUS: LazyLock<Mutex<CoreStatus>> = LazyLock::new(|| {
    Mutex::new(CoreStatus {
        last_error: String::new(),
        last_engine: String::new(),
    })
});

/// 记录最近一次错误（同时写入运行日志）
pub fn set_error(msg: &str) {
    {
        let mut st = CORE_STATUS.lock().unwrap_or_else(|e| e.into_inner());
        st.last_error = msg.to_string();
    }
    log::log_write("错误", msg);
}

/// 获取最近一次错误提示（UTF-8）
pub fn last_error() -> String {
    CORE_STATUS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .last_error
        .clone()
}

/// 记录最近一次成功命中的翻译引擎名称（AI / 百度 / 必应）
pub fn set_engine(engine: &str) {
    let mut st = CORE_STATUS.lock().unwrap_or_else(|e| e.into_inner());
    st.last_engine = engine.to_string();
}

/// 获取最近一次成功命中的翻译引擎名称
pub fn last_engine() -> String {
    CORE_STATUS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .last_engine
        .clone()
}

/// 测试期间串行化读写全局日志与错误/引擎状态的用例，防止并发测试互相干扰
#[cfg(test)]
pub(crate) static TEST_GLOBAL_LOCK: Mutex<()> = Mutex::new(());
