//! naming_reducer —— 翻译结果 → 合法标识符的本地缩减模块
//!
//! 用途：复制单词/AI 命名前，把中文或英文短语缩成符合预算的标识符。
//! 纯本地词表规则，不联网；仅依赖 Rust 标准库。
//!
//! 流水线：拆词 → 删泛词 → 缩写映射 → 预算裁剪 → 拼格式 → 校验
//! 常用词库（`PREFERRED_NOUNS` / `PREFERRED_VERBS` / `PREFERRED_ATTRS`）命中即原样保留：
//! 不删泛词、不做缩写。失败一律带原因返回（`ReduceResult.reason`），不静默截断字符串。

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::{LazyLock, RwLock};

/// 标识符命名风格
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamingStyle {
    /// camelCase（小驼峰）
    Camel,
    /// PascalCase（大驼峰）
    Pascal,
    /// snake_case（蛇形小写）
    Snake,
    /// SCREAMING_SNAKE（全大写下划线）
    ScreamingSnake,
    /// kebab-case（中划线，文件名/URL 场景，非严格标识符）
    Kebab,
}

/// 命名对象：决定前缀与大小写套路，枚举值对应主窗口「命名对象」下拉选项序号
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NameKind {
    /// 局部变量
    Local = 0,
    /// 全局变量（snake: `g_` 前缀，驼峰: `g` + 帕斯卡）
    Global = 1,
    /// 静态变量（`s_` / `s` + 帕斯卡）
    Static = 2,
    /// 指针变量（`p_` / `p` + 帕斯卡）
    Pointer = 3,
    /// 结构体/类型（帕斯卡；snake 风格时补 `_t` 后缀）
    Type = 4,
    /// 宏/常量（恒为 `SCREAMING_SNAKE`）
    Macro = 5,
    /// 函数
    Func = 6,
}

impl NameKind {
    /// 从配置或下拉框索引转换（越界回退为 `Local`）
    pub fn from_index(idx: i32) -> Self {
        match idx {
            0 => Self::Local,
            1 => Self::Global,
            2 => Self::Static,
            3 => Self::Pointer,
            4 => Self::Type,
            5 => Self::Macro,
            6 => Self::Func,
            _ => Self::Local,
        }
    }
}

impl fmt::Display for NameKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Local => "nkLocal",
            Self::Global => "nkGlobal",
            Self::Static => "nkStatic",
            Self::Pointer => "nkPointer",
            Self::Type => "nkType",
            Self::Macro => "nkMacro",
            Self::Func => "nkFunc",
        };
        f.write_str(s)
    }
}

/// 命名缩减结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReduceResult {
    /// 是否成功生成合法标识符
    pub ok: bool,
    /// 成功时的标识符
    pub name: String,
    /// 失败原因
    pub reason: String,
}

impl ReduceResult {
    fn success(name: String) -> Self {
        Self {
            ok: true,
            name,
            reason: String::new(),
        }
    }

    fn fail(reason: &str) -> Self {
        Self {
            ok: false,
            name: String::new(),
            reason: reason.to_string(),
        }
    }
}

// ---------------- 规则表 ----------------

/// 泛词删除表：虚词 + "X的Y"结构里的通用头名词（命名时冗余，直接删）
pub const STOP_WORDS: &[&str] = &[
    // 虚词
    "the",
    "a",
    "an",
    "of",
    "for",
    "to",
    "and",
    "or",
    "in",
    "on",
    "with",
    "by",
    "at",
    "from",
    "as",
    "into",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "being",
    "do",
    "does",
    "this",
    "that",
    "these",
    "those",
    "it",
    "its",
    "s",
    "please",
    "some",
    "any",
    "my",
    "your",
    "our",
    "their",
    "each",
    "every",
    "than",
    "then",
    "so",
    "such",
    "not",
    "no",
    "nor",
    "but",
    // 通用头名词（程序/管理器之类，命名时冗余）
    "program",
    "manager",
    "module",
    "system",
    "tool",
    "utility",
    "function",
    "method",
    "service",
    "handler",
    "wrapper",
    "helper",
    "implementation",
    "feature",
    "item",
    "detail",
];

/// 长词缩写映射（编程圈惯用缩写），复数自动带 s
pub const ABBREVIATIONS: &[(&str, &str)] = &[
    ("initialization", "init"),
    ("initialize", "init"),
    ("configuration", "config"),
    ("application", "app"),
    ("authentication", "auth"),
    ("synchronization", "sync"),
    ("temporary", "tmp"),
    ("message", "msg"),
    ("button", "btn"),
    ("window", "win"),
    ("database", "db"),
    ("repository", "repo"),
    ("parameter", "param"),
    ("argument", "arg"),
    ("attribute", "attr"),
    ("environment", "env"),
    ("document", "doc"),
    ("directory", "dir"),
    ("source", "src"),
    ("destination", "dst"),
    ("reference", "ref"),
    ("number", "num"),
    ("position", "pos"),
    ("previous", "prev"),
    ("current", "cur"),
    ("maximum", "max"),
    ("minimum", "min"),
    ("buffer", "buf"),
    ("context", "ctx"),
    ("command", "cmd"),
    ("execute", "exec"),
    ("validate", "valid"),
    ("generate", "gen"),
    ("calculate", "calc"),
    ("terminate", "term"),
    ("connection", "conn"),
    ("request", "req"),
    ("response", "resp"),
    ("session", "sess"),
    ("storage", "store"),
    // 补充常用
    ("address", "addr"),
    ("identifier", "id"),
    ("information", "info"),
    ("index", "idx"),
    ("length", "len"),
    ("string", "str"),
    ("image", "img"),
    ("count", "cnt"),
    ("specification", "spec"),
    ("object", "obj"),
    ("element", "elem"),
    ("collection", "coll"),
    ("history", "hist"),
    ("language", "lang"),
    ("library", "lib"),
    ("package", "pkg"),
    ("process", "proc"),
    ("statistics", "stats"),
    ("value", "val"),
    ("variable", "var"),
    // 常用词库的完整词形（temp/humi/freq 已是首选，英文输入也对齐）
    ("temperature", "temp"),
    ("humidity", "humi"),
    ("frequency", "freq"),
];

/// 保留字（C/C++/C#/Java/JS/Python/Go/Rust/Nim 常用保留字去重合集）
pub const RESERVED_WORDS: &[&str] = &[
    // C / C++
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "int",
    "long",
    "register",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "class",
    "public",
    "private",
    "protected",
    "virtual",
    "friend",
    "inline",
    "template",
    "typename",
    "namespace",
    "using",
    "new",
    "delete",
    "this",
    "try",
    "catch",
    "throw",
    "operator",
    "bool",
    "true",
    "false",
    "explicit",
    "export",
    "mutable",
    "nullptr",
    "constexpr",
    "decltype",
    "noexcept",
    "override",
    "final",
    "asm",
    // C# / Java
    "byte",
    "sbyte",
    "decimal",
    "delegate",
    "event",
    "fixed",
    "foreach",
    "in",
    "is",
    "lock",
    "object",
    "out",
    "params",
    "ref",
    "sealed",
    "typeof",
    "uint",
    "ulong",
    "unchecked",
    "unsafe",
    "ushort",
    "var",
    "abstract",
    "assert",
    "boolean",
    "extends",
    "implements",
    "instanceof",
    "interface",
    "native",
    "package",
    "super",
    "synchronized",
    "throws",
    "transient",
    // JS / Python
    "function",
    "let",
    "of",
    "async",
    "await",
    "yield",
    "typeof",
    "null",
    "undefined",
    "def",
    "lambda",
    "pass",
    "raise",
    "with",
    "global",
    "nonlocal",
    "del",
    "not",
    "and",
    "or",
    "import",
    "from",
    "as",
    "except",
    "finally",
    "none",
    "match",
    // Go / Rust
    "func",
    "chan",
    "go",
    "defer",
    "select",
    "range",
    "map",
    "type",
    "fallthrough",
    "nil",
    "fn",
    "mut",
    "impl",
    "trait",
    "use",
    "mod",
    "pub",
    "crate",
    "self",
    "move",
    "dyn",
    "where",
    "loop",
    "unsafe",
    // Nim
    "proc",
    "iterator",
    "converter",
    "template",
    "macro",
    "bind",
    "mixin",
    "discard",
    "elif",
    "when",
    "end",
    "addr",
    "ptr",
    "object",
    "tuple",
    "concept",
    "distinct",
    "array",
    "seq",
    "set",
    "string",
];

// ---------------- 常用词库（嵌入式 C 场景） ----------------
// 三组词是命名的首选 vocabulary：中文映射直接指向这些词，
// 命中的词原样保留——不删泛词、不做缩写（否则 button 会被缩成 btn）。
// 动作动词组另用于预算裁剪时优先保留（word_priority）。

/// 常用词库：模块名词
pub const PREFERRED_NOUNS: &[&str] = &[
    "led",
    "key",
    "button",
    "buzzer",
    "beep",
    "relay",
    "motor",
    "servo",
    "stepper",
    "encoder",
    "segment",
    "lcd",
    "oled",
    "display",
    "keypad",
    "sensor",
    "temp",
    "humi",
    "light",
    "ir",
    "ultrasonic",
    "rfid",
    "uart",
    "usart",
    "i2c",
    "spi",
    "gpio",
    "timer",
    "pwm",
    "adc",
    "dac",
    "irq",
    "exti",
    "nvic",
    "dma",
    "rtc",
    "wdt",
    "eeprom",
    "flash",
    "bluetooth",
    "ble",
    "wifi",
    "usb",
    "can",
    "modbus",
    "fan",
    "pump",
    "valve",
    "solenoid",
    "switch",
];

/// 常用词库：动作动词
pub const PREFERRED_VERBS: &[&str] = &[
    "init",
    "deinit",
    "config",
    "set",
    "get",
    "read",
    "write",
    "send",
    "transmit",
    "receive",
    "recv",
    "open",
    "close",
    "start",
    "stop",
    "reset",
    "clear",
    "update",
    "scan",
    "display",
    "show",
    "toggle",
    "enable",
    "disable",
    "measure",
    "check",
    "is",
    "handle",
    "process",
    "callback",
    "feed",
    "beep",
    "blink",
    "forward",
    "backward",
    "brake",
    "calibrate",
];

/// 常用词库：属性参数
pub const PREFERRED_ATTRS: &[&str] = &[
    "state",
    "status",
    "value",
    "level",
    "mode",
    "speed",
    "angle",
    "duty",
    "period",
    "frequency",
    "freq",
    "channel",
    "pin",
    "port",
    "addr",
    "length",
    "len",
    "count",
    "flag",
    "timeout",
    "threshold",
    "high",
    "low",
    "on",
    "off",
    "rising",
    "falling",
];

/// 中文技术词表：正向最长匹配，一词可对应多个英文单词（空格分隔）
pub const ZH_TERMS: &[(&str, &str)] = &[
    ("串口", "uart"),
    ("初始化", "initialize"),
    ("程序", "program"),
    ("配置", "configuration"),
    ("文件", "file"),
    ("读取", "read"),
    ("写入", "write"),
    ("路径", "path"),
    ("目录", "directory"),
    ("文件夹", "folder"),
    ("用户", "user"),
    ("登录", "login"),
    ("注销", "logout"),
    ("密码", "password"),
    ("状态", "state"),
    ("缓存", "cache"),
    ("缓冲区", "buffer"),
    ("管理器", "manager"),
    ("数据", "data"),
    ("数据库", "database"),
    ("池", "pool"),
    ("连接", "connection"),
    ("网络", "network"),
    ("服务器", "server"),
    ("客户端", "client"),
    ("服务", "service"),
    ("接口", "interface"),
    ("协议", "protocol"),
    ("版本", "version"),
    ("更新", "update"),
    ("删除", "delete"),
    ("移除", "remove"),
    ("添加", "add"),
    ("创建", "create"),
    ("获取", "get"),
    ("设置", "set"),
    ("保存", "save"),
    ("打开", "open"),
    ("关闭", "close"),
    ("启动", "start"),
    ("停止", "stop"),
    ("暂停", "pause"),
    ("重启", "restart"),
    ("退出", "exit"),
    ("时间", "time"),
    ("日期", "date"),
    ("超时", "timeout"),
    ("重试", "retry"),
    ("大小", "size"),
    ("类型", "type"),
    ("名称", "name"),
    ("编号", "number"),
    ("序号", "index"),
    ("列表", "list"),
    ("标签", "tag"),
    ("按钮", "button"),
    ("窗口", "window"),
    ("菜单", "menu"),
    ("页面", "page"),
    ("图标", "icon"),
    ("图片", "image"),
    ("颜色", "color"),
    ("字体", "font"),
    ("文本", "text"),
    ("字符串", "string"),
    ("字符", "char"),
    ("数值", "value"),
    ("参数", "parameter"),
    ("变量", "variable"),
    ("常量", "constant"),
    ("函数", "function"),
    ("方法", "method"),
    ("类", "class"),
    ("对象", "object"),
    ("数组", "array"),
    ("集合", "collection"),
    ("队列", "queue"),
    ("栈", "stack"),
    ("节点", "node"),
    ("键", "key"),
    ("索引", "index"),
    ("长度", "len"),
    ("位置", "position"),
    ("方向", "direction"),
    ("通道", "channel"),
    ("线程", "thread"),
    ("进程", "process"),
    ("内存", "memory"),
    ("磁盘", "disk"),
    ("驱动", "driver"),
    ("设备", "device"),
    ("硬件", "hardware"),
    ("软件", "software"),
    ("命令", "command"),
    ("脚本", "script"),
    ("模块", "module"),
    ("组件", "component"),
    ("插件", "plugin"),
    ("库", "library"),
    ("框架", "framework"),
    ("工具", "tool"),
    ("模板", "template"),
    ("示例", "example"),
    ("测试", "test"),
    ("调试", "debug"),
    ("监控", "monitor"),
    ("统计", "statistics"),
    ("图表", "chart"),
    ("报告", "report"),
    ("屏幕", "screen"),
    ("键盘", "keypad"),
    ("剪贴板", "clipboard"),
    ("桌面", "desktop"),
    ("帮助", "help"),
    ("编码", "encode"),
    ("解码", "decode"),
    ("加密", "encrypt"),
    ("解密", "decrypt"),
    ("压缩", "compress"),
    ("解压", "decompress"),
    ("解析", "parse"),
    ("生成", "generate"),
    ("计算", "calculate"),
    ("转换", "convert"),
    ("校验", "validate"),
    ("验证", "validate"),
    ("检查", "check"),
    ("过滤", "filter"),
    ("排序", "sort"),
    ("比较", "compare"),
    ("合并", "merge"),
    ("拆分", "split"),
    ("格式化", "format"),
    ("复制", "copy"),
    ("粘贴", "paste"),
    ("搜索", "search"),
    ("查找", "find"),
    ("替换", "replace"),
    ("下载", "download"),
    ("上传", "upload"),
    ("发送", "send"),
    ("接收", "receive"),
    ("刷新", "refresh"),
    ("加载", "load"),
    ("清空", "clear"),
    ("重置", "reset"),
    ("同步", "sync"),
    ("异步", "async"),
    ("最大", "maximum"),
    ("最小", "minimum"),
    ("平均", "average"),
    ("当前", "current"),
    ("上一个", "previous"),
    ("下一个", "next"),
    ("默认", "default"),
    ("全局", "global"),
    ("本地", "local"),
    ("远程", "remote"),
    ("临时", "temporary"),
    ("内部", "internal"),
    ("外部", "external"),
    ("自动", "automatic"),
    ("手动", "manual"),
    ("高级", "advanced"),
    ("基本", "basic"),
    ("详细", "detail"),
    ("摘要", "summary"),
    ("信息", "information"),
    ("日志", "log"),
    ("记录", "record"),
    ("历史", "history"),
    ("通知", "notification"),
    ("消息", "message"),
    ("警告", "warning"),
    ("错误", "error"),
    ("异常", "exception"),
    ("成功", "success"),
    ("失败", "failure"),
    ("权限", "permission"),
    ("账号", "account"),
    ("邮箱", "email"),
    ("地址", "address"),
    ("端口", "port"),
    ("主机", "host"),
    ("会话", "session"),
    ("令牌", "token"),
    ("密钥", "secret"),
    ("安全", "security"),
    ("隐藏", "hide"),
    ("显示", "display"),
    ("语言", "language"),
    ("翻译", "translate"),
    ("输入", "input"),
    ("输出", "output"),
    ("源", "source"),
    ("目标", "target"),
    ("地址簿", "address book"),
    ("主题", "theme"),
    ("滚动", "scroll"),
    ("拖拽", "drag"),
    ("选择", "select"),
    ("管理", "manage"),
    ("处理", "handle"),
    ("控制", "control"),
    ("执行", "execute"),
    ("运行", "run"),
    ("调度", "schedule"),
    ("分配", "allocate"),
    ("释放", "release"),
    ("申请", "apply"),
    // —— 常用词库中文映射（见 PREFERRED_NOUNS / VERBS / ATTRS）——
    // 模块名词：中文直接映射到词库用词（串口不再给 serial port，给 uart）
    ("指示灯", "led"),
    ("灯", "led"),
    ("蜂鸣器", "buzzer"),
    ("蜂鸣", "beep"),
    ("继电器", "relay"),
    ("电机", "motor"),
    ("马达", "motor"),
    ("舵机", "servo"),
    ("步进电机", "stepper motor"),
    ("步进", "stepper"),
    ("编码器", "encoder"),
    ("数码管", "segment"),
    ("液晶屏", "lcd"),
    ("液晶", "lcd"),
    ("矩阵键盘", "keypad"),
    ("传感器", "sensor"),
    ("温度", "temp"),
    ("湿度", "humi"),
    ("光照", "light"),
    ("灯光", "light"),
    ("光", "light"),
    ("红外", "ir"),
    ("超声波", "ultrasonic"),
    ("射频识别", "rfid"),
    ("定时器", "timer"),
    ("脉宽调制", "pwm"),
    ("脉宽", "pwm"),
    ("模数转换", "adc"),
    ("模数", "adc"),
    ("数模转换", "dac"),
    ("数模", "dac"),
    ("外部中断", "exti"),
    ("中断", "irq"),
    ("直接内存访问", "dma"),
    ("直接存储访问", "dma"),
    ("实时时钟", "rtc"),
    ("看门狗", "wdt"),
    ("闪存", "flash"),
    ("蓝牙", "bluetooth"),
    ("低功耗蓝牙", "ble"),
    ("无线网络", "wifi"),
    ("通用串行总线", "usb"),
    ("风扇", "fan"),
    ("水泵", "pump"),
    ("泵", "pump"),
    ("阀", "valve"),
    ("电磁阀", "solenoid"),
    ("开关", "switch"),
    // 动作动词
    ("反初始化", "deinit"),
    ("去初始化", "deinit"),
    ("发射", "transmit"),
    ("复位", "reset"),
    ("清除", "clear"),
    ("扫描", "scan"),
    ("展示", "show"),
    ("翻转", "toggle"),
    ("切换", "toggle"),
    ("使能", "enable"),
    ("启用", "enable"),
    ("禁用", "disable"),
    ("失能", "disable"),
    ("测量", "measure"),
    ("是否", "is"),
    ("回调", "callback"),
    ("喂狗", "feed dog"),
    ("喂", "feed"),
    ("闪烁", "blink"),
    ("正转", "forward"),
    ("前进", "forward"),
    ("反转", "backward"),
    ("后退", "backward"),
    ("刹车", "brake"),
    ("制动", "brake"),
    ("校准", "calibrate"),
    ("开始", "start"),
    // 属性参数
    ("电平", "level"),
    ("级别", "level"),
    ("模式", "mode"),
    ("速度", "speed"),
    ("角度", "angle"),
    ("占空比", "duty"),
    ("周期", "period"),
    ("频率", "freq"),
    ("引脚", "pin"),
    ("计数", "count"),
    ("数量", "count"),
    ("标志", "flag"),
    ("阈值", "threshold"),
    ("高", "high"),
    ("低", "low"),
    ("开", "on"),
    ("关", "off"),
    ("上升沿", "rising"),
    ("上升", "rising"),
    ("下降沿", "falling"),
    ("下降", "falling"),
    // 单字映射的防误伤保护（复合词更长，正向最长匹配下优先于单字命中）
    ("高级", "advanced"),
    ("最低", "minimum"),
    ("提高", "increase"),
    ("降低", "decrease"),
    ("开机", "start"),
    ("关机", "shutdown"),
    ("光标", "cursor"),
];

const EXTRA_VERBS: &[&str] = &[
    "get", "set", "read", "write", "load", "save", "init", "parse", "convert", "update", "delete",
    "remove", "create", "add", "check", "gen", "calc", "exec", "run", "open", "close", "start",
    "stop", "copy", "fetch", "build", "make", "find", "query", "send", "recv", "flush", "clear",
    "reset", "sync", "login", "logout", "download", "upload", "encode", "decode", "encrypt",
    "decrypt", "compress", "format", "print", "draw", "handle",
];

struct BuiltinTables {
    stop: HashSet<&'static str>,
    abbrev: HashMap<&'static str, &'static str>,
    zh: HashMap<&'static str, &'static str>,
    preferred: HashSet<&'static str>,
    pref_verbs: HashSet<&'static str>,
    verbs: HashSet<&'static str>,
    reserved: HashSet<&'static str>,
}

static BUILTIN: LazyLock<BuiltinTables> = LazyLock::new(|| {
    let stop: HashSet<&'static str> = STOP_WORDS.iter().copied().collect();
    let abbrev: HashMap<&'static str, &'static str> = ABBREVIATIONS.iter().copied().collect();
    let zh: HashMap<&'static str, &'static str> = ZH_TERMS.iter().copied().collect();
    let mut preferred = HashSet::new();
    for &w in PREFERRED_NOUNS
        .iter()
        .chain(PREFERRED_VERBS.iter())
        .chain(PREFERRED_ATTRS.iter())
    {
        preferred.insert(w);
    }
    let pref_verbs: HashSet<&'static str> = PREFERRED_VERBS.iter().copied().collect();
    let verbs: HashSet<&'static str> = EXTRA_VERBS.iter().copied().collect();
    let reserved: HashSet<&'static str> = RESERVED_WORDS.iter().copied().collect();
    BuiltinTables {
        stop,
        abbrev,
        zh,
        preferred,
        pref_verbs,
        verbs,
        reserved,
    }
});

struct CustomState {
    /// 用户自定义缩写（存 `fastT\custom_abbr.txt`），同名覆盖内置
    custom_abbr: HashMap<String, String>,
    /// 内置 + 自定义合并后的生效缩写表
    active_abbr: HashMap<String, String>,
    /// 自定义忽略词（命中即删除，优先级最高）
    custom_ignore: HashSet<String>,
    /// 显式保留的内置泛词（`+word`）
    custom_keep: HashSet<String>,
    /// 自定义中文 → 英文映射（含对内置词库映射的覆盖）
    custom_zh: HashMap<String, String>,
    /// 被停用的内置词库映射（`中文=-`）
    zh_off: HashSet<String>,
    /// 自定义英文词集合（享受常用词不删不缩待遇）
    preferred_custom: HashSet<String>,
}

impl CustomState {
    fn new() -> Self {
        let active_abbr = BUILTIN
            .abbrev
            .iter()
            .map(|(&k, &v)| (k.to_string(), v.to_string()))
            .collect();
        Self {
            custom_abbr: HashMap::new(),
            active_abbr,
            custom_ignore: HashSet::new(),
            custom_keep: HashSet::new(),
            custom_zh: HashMap::new(),
            zh_off: HashSet::new(),
            preferred_custom: HashSet::new(),
        }
    }

    fn rebuild_active(&mut self) {
        self.active_abbr.clear();
        for (&k, &v) in &BUILTIN.abbrev {
            self.active_abbr.insert(k.to_string(), v.to_string());
        }
        for (k, v) in &self.custom_abbr {
            self.active_abbr.insert(k.clone(), v.clone());
        }
    }
}

static STATE: LazyLock<RwLock<CustomState>> = LazyLock::new(|| RwLock::new(CustomState::new()));

/// 检查是否为内置或跨语言保留字（忽略大小写）
pub fn is_reserved_word(word: &str) -> bool {
    BUILTIN
        .reserved
        .contains(word.to_ascii_lowercase().as_str())
}

// ---------------- 自定义缩写（custom_abbr.txt，每行 原词=缩写，# 注释） ----------------

fn parse_abbr_text(text: &str) -> (HashMap<String, String>, usize) {
    let mut table = HashMap::new();
    for (idx, raw) in text.split('\n').enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else {
            return (table, line_no);
        };
        if eq == 0 {
            return (table, line_no);
        }
        let k = line[..eq].trim().to_ascii_lowercase();
        let v = line[eq + 1..].trim().to_ascii_lowercase();
        if k.is_empty() || v.is_empty() {
            return (table, line_no);
        }
        table.insert(k, v);
    }
    (table, 0)
}

/// 启动时加载自定义缩写；文件不存在/为空或有错行则清空自定义，只留内置。返回条数。
pub fn load_custom_abbreviations<P: AsRef<Path>>(path: P) -> usize {
    let custom = match fs::read_to_string(path) {
        Ok(content) => {
            let (table, bad_line) = parse_abbr_text(&content);
            if bad_line == 0 {
                table
            } else {
                HashMap::new()
            }
        }
        Err(_) => HashMap::new(),
    };
    let mut state = STATE.write().unwrap_or_else(|e| e.into_inner());
    state.custom_abbr = custom;
    state.rebuild_active();
    state.custom_abbr.len()
}

/// 校验 → 写文件 → 立即生效；`bad_line > 0` 时拒绝保存并返回 `(0, bad_line)`
pub fn save_custom_abbreviations<P: AsRef<Path>>(path: P, text: &str) -> (usize, usize) {
    let (table, bad_line) = parse_abbr_text(text);
    if bad_line > 0 {
        return (0, bad_line);
    }
    if fs::write(path, text).is_err() {
        return (0, 0);
    }
    let mut state = STATE.write().unwrap_or_else(|e| e.into_inner());
    state.custom_abbr = table;
    state.rebuild_active();
    (state.custom_abbr.len(), 0)
}

/// 读取自定义缩写文件全文，用于设置页回填
pub fn custom_abbr_text<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// 设置页回填用：全部自定义项（含 `"-"` = 禁用内置缩写），按原词排序
pub fn custom_abbr_entries() -> Vec<(String, String)> {
    let state = STATE.read().unwrap_or_else(|e| e.into_inner());
    let mut out: Vec<(String, String)> = state
        .custom_abbr
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// 生效的自定义缩写（不含禁用项），按原词排序
pub fn custom_abbr_pairs() -> Vec<(String, String)> {
    let state = STATE.read().unwrap_or_else(|e| e.into_inner());
    let mut out: Vec<(String, String)> = state
        .custom_abbr
        .iter()
        .filter(|(_, v)| v.as_str() != "-")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// 设置页「缩略词」默认行：内置缩写表，按原词排序
pub fn builtin_abbreviations() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = BUILTIN
        .abbrev
        .iter()
        .map(|(&k, &v)| (k.to_string(), v.to_string()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// 设置页「忽略词」默认行：内置泛词表，按字母序排序
pub fn builtin_stop_words() -> Vec<String> {
    let mut out: Vec<String> = BUILTIN.stop.iter().map(|&s| s.to_string()).collect();
    out.sort();
    out
}

// ---------------- 自定义忽略词（custom_ignore.txt，每行一个词，# 注释） ----------------

fn parse_ignore_text(text: &str) -> (HashSet<String>, HashSet<String>) {
    let mut ignore = HashSet::new();
    let mut keep = HashSet::new();
    for raw in text.split('\n') {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('+') {
            let w = rest.trim().to_ascii_lowercase();
            if !w.is_empty() {
                keep.insert(w);
            }
        } else {
            ignore.insert(line.to_ascii_lowercase());
        }
    }
    (ignore, keep)
}

/// 启动时加载自定义忽略词；文件不存在/为空 = 无自定义忽略。返回忽略词条数。
pub fn load_custom_ignore<P: AsRef<Path>>(path: P) -> usize {
    let (ignore, keep) = match fs::read_to_string(path) {
        Ok(content) => parse_ignore_text(&content),
        Err(_) => (HashSet::new(), HashSet::new()),
    };
    let mut state = STATE.write().unwrap_or_else(|e| e.into_inner());
    state.custom_ignore = ignore;
    state.custom_keep = keep;
    state.custom_ignore.len()
}

/// 写文件 → 立即生效；返回忽略词条数（空行/注释/`+`保留行不计）
pub fn save_custom_ignore<P: AsRef<Path>>(path: P, text: &str) -> usize {
    let (ignore, keep) = parse_ignore_text(text);
    if fs::write(path, text).is_err() {
        return 0;
    }
    let mut state = STATE.write().unwrap_or_else(|e| e.into_inner());
    state.custom_ignore = ignore;
    state.custom_keep = keep;
    state.custom_ignore.len()
}

/// 读取自定义忽略词文件全文，用于设置页回填
pub fn custom_ignore_text<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// 设置页回填用：自定义忽略词（按字母序）
pub fn custom_ignore_words() -> Vec<String> {
    let state = STATE.read().unwrap_or_else(|e| e.into_inner());
    let mut out: Vec<String> = state.custom_ignore.iter().cloned().collect();
    out.sort();
    out
}

/// 设置页回填用：内置泛词里被显式保留的词（按字母序）
pub fn custom_keep_words() -> Vec<String> {
    let state = STATE.read().unwrap_or_else(|e| e.into_inner());
    let mut out: Vec<String> = state.custom_keep.iter().cloned().collect();
    out.sort();
    out
}

// ---------------- 自定义常用词（custom_preferred.txt，每行 中文=英文，# 注释） ----------------

fn is_preferred_word(w: &str) -> bool {
    !w.is_empty() && w.len() <= 20 && w.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn parse_preferred_text(text: &str) -> (HashMap<String, String>, usize) {
    let mut table = HashMap::new();
    for (idx, raw) in text.split('\n').enumerate() {
        let line_no = idx + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else {
            return (table, line_no);
        };
        if eq == 0 {
            return (table, line_no);
        }
        let zh = line[..eq].trim();
        let en_raw = line[eq + 1..].trim();
        if zh.is_empty() || en_raw.is_empty() {
            return (table, line_no);
        }
        // 键必须含中文（>= 0x2E80）且 <= 8 字
        let has_cjk = zh.chars().any(|c| (c as u32) >= 0x2E80);
        if !has_cjk || zh.chars().count() > 8 {
            return (table, line_no);
        }
        // 内置词条只允许动「词库」映射；普通映射（配置→configuration）不在本页范围
        if let Some(&builtin_en) = BUILTIN.zh.get(zh) {
            let first = builtin_en.split(' ').next().unwrap_or("");
            if !BUILTIN.preferred.contains(first) {
                return (table, line_no);
            }
        }
        let en = if en_raw != "-" {
            let words: Vec<&str> = en_raw.split(' ').filter(|w| !w.is_empty()).collect();
            if words.is_empty() || words.len() > 4 || !words.iter().all(|w| is_preferred_word(w)) {
                return (table, line_no);
            }
            words.join(" ")
        } else {
            "-".to_string()
        };
        table.insert(zh.to_string(), en);
    }
    (table, 0)
}

fn apply_custom_preferred(state: &mut CustomState, table: HashMap<String, String>) {
    state.custom_zh.clear();
    state.zh_off.clear();
    state.preferred_custom.clear();
    for (zh, en) in table {
        if en == "-" {
            state.zh_off.insert(zh);
        } else {
            for w in en.split(' ') {
                if !w.is_empty() {
                    state.preferred_custom.insert(w.to_string());
                }
            }
            state.custom_zh.insert(zh, en);
        }
    }
}

/// 启动时加载自定义常用词；文件不存在/为空 = 无自定义。返回自定义条数（含停用）。
pub fn load_custom_preferred<P: AsRef<Path>>(path: P) -> usize {
    let table = match fs::read_to_string(path) {
        Ok(content) => {
            let (t, bad_line) = parse_preferred_text(&content);
            if bad_line == 0 {
                t
            } else {
                HashMap::new()
            }
        }
        Err(_) => HashMap::new(),
    };
    let mut state = STATE.write().unwrap_or_else(|e| e.into_inner());
    apply_custom_preferred(&mut state, table);
    state.custom_zh.len() + state.zh_off.len()
}

/// 校验 → 写文件 → 立即生效；`bad_line > 0` 时拒绝保存并返回 `(0, bad_line)`
pub fn save_custom_preferred<P: AsRef<Path>>(path: P, text: &str) -> (usize, usize) {
    let (table, bad_line) = parse_preferred_text(text);
    if bad_line > 0 {
        return (0, bad_line);
    }
    if fs::write(path, text).is_err() {
        return (0, 0);
    }
    let mut state = STATE.write().unwrap_or_else(|e| e.into_inner());
    apply_custom_preferred(&mut state, table);
    (state.custom_zh.len() + state.zh_off.len(), 0)
}

/// 读取自定义常用词文件全文，用于设置页回填
pub fn custom_preferred_text<P: AsRef<Path>>(path: P) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

/// 全部自定义常用词条目（含 `"-"` = 停用），按中文排序
pub fn custom_preferred_entries() -> Vec<(String, String)> {
    let state = STATE.read().unwrap_or_else(|e| e.into_inner());
    let mut out: Vec<(String, String)> = state
        .custom_zh
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for k in &state.zh_off {
        out.push((k.clone(), "-".to_string()));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// 生效的自定义映射（不含停用项），按中文排序
pub fn custom_preferred_pairs() -> Vec<(String, String)> {
    custom_preferred_entries()
        .into_iter()
        .filter(|(_, en)| en != "-")
        .collect()
}

/// 内置词库映射（值的第一个词属于常用词库），按中文排序；「常用词」页的内置行
pub fn builtin_preferred_pairs() -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = BUILTIN
        .zh
        .iter()
        .filter(|(_, &en)| {
            let first = en.split(' ').next().unwrap_or("");
            BUILTIN.preferred.contains(first)
        })
        .map(|(&zh, &en)| (zh.to_string(), en.to_string()))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// 判断中文词在内置词表中的类别：
/// `0` = 不在内置词表，`1` = 内置词库映射（本页可改可删），`2` = 内置普通映射（本页不管）
pub fn builtin_zh_kind(zh: &str) -> u8 {
    let Some(&en) = BUILTIN.zh.get(zh) else {
        return 0;
    };
    let first = en.split(' ').next().unwrap_or("");
    if BUILTIN.preferred.contains(first) {
        1
    } else {
        2
    }
}

/// 内置 + 自定义常用词合并视图：自定义覆盖内置，`"-"` = 停用（不出现），按中文排序
pub fn preferred_merge() -> Vec<(String, String)> {
    let mut map: HashMap<String, String> = builtin_preferred_pairs().into_iter().collect();
    for (zh, en) in custom_preferred_entries() {
        if en == "-" {
            map.remove(&zh);
        } else {
            map.insert(zh, en);
        }
    }
    let mut out: Vec<(String, String)> = map.into_iter().collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ---------------- 拆词 ----------------

fn split_camel(tok: &str) -> Vec<String> {
    let bytes = tok.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    for i in 1..bytes.len() {
        let c = bytes[i];
        let prev = bytes[i - 1];
        let boundary = (c.is_ascii_uppercase() && prev.is_ascii_lowercase())
            || (c.is_ascii_uppercase()
                && prev.is_ascii_uppercase()
                && i + 1 < bytes.len()
                && bytes[i + 1].is_ascii_lowercase())
            || (c.is_ascii_lowercase() && prev.is_ascii_digit());
        if boundary {
            out.push(tok[start..i].to_ascii_lowercase());
            start = i;
        }
    }
    if start < bytes.len() {
        out.push(tok[start..].to_ascii_lowercase());
    }
    out
}

fn split_ascii_words(s: &str) -> Vec<String> {
    let mut cleaned = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            cleaned.push(c);
        } else if c == '\'' {
            // 撇号直接并入前词：user's -> users
        } else {
            cleaned.push(' ');
        }
    }
    let mut out = Vec::new();
    for tok in cleaned.split_whitespace() {
        out.extend(split_camel(tok));
    }
    out
}

/// 拆词入口：中文按词表正向最长匹配（内置最长 6 字，自定义可到 8 字），英文按空格/标点/驼峰。
/// 未收录的中文字以单字混进结果，由 `apply_reduction` 丢弃。
pub fn split_words(phrase: &str) -> Vec<String> {
    let s = phrase.trim();
    if s.is_empty() {
        return Vec::new();
    }
    if s.is_ascii() {
        return split_ascii_words(s);
    }

    let state = STATE.read().unwrap_or_else(|e| e.into_inner());
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut ascii_buf = String::new();
    let mut term = String::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.len_utf8() >= 3 {
            // 3 字节及以上 UTF-8 ≈ 中日韩字符与全角标点
            if !ascii_buf.is_empty() {
                out.extend(split_ascii_words(&ascii_buf));
                ascii_buf.clear();
            }
            let mut matched = false;
            for len in (1..=8).rev() {
                if i + len <= chars.len() {
                    term.clear();
                    for &ch in &chars[i..i + len] {
                        term.push(ch);
                    }
                    // 自定义词库映射优先；被停用（中文=-）的内置映射视作无此词条，继续尝试更短词
                    let en: Option<&str> = if let Some(custom_en) = state.custom_zh.get(&term) {
                        Some(custom_en.as_str())
                    } else if let Some(&builtin_en) = BUILTIN.zh.get(term.as_str()) {
                        if !state.zh_off.contains(&term) {
                            Some(builtin_en)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(mapped) = en {
                        if !mapped.is_empty() {
                            for w in mapped.split(' ') {
                                if !w.is_empty() {
                                    out.push(w.to_string());
                                }
                            }
                            i += len;
                            matched = true;
                            break;
                        }
                    }
                }
            }
            if !matched {
                out.push(c.to_string());
                i += 1;
            }
        } else {
            if c.is_ascii() {
                ascii_buf.push(c);
            } else {
                // 非 ASCII 双字节字符等同标点视作分隔空格
                ascii_buf.push(' ');
            }
            i += 1;
        }
    }

    if !ascii_buf.is_empty() {
        out.extend(split_ascii_words(&ascii_buf));
    }
    out
}

// ---------------- 缩减 ----------------

fn lookup_abbrev_with_state(w: &str, state: &CustomState) -> String {
    if let Some(a) = state.active_abbr.get(w) {
        return if a == "-" { w.to_string() } else { a.clone() };
    }
    if w.len() > 3 && w.ends_with('s') {
        let base = &w[..w.len() - 1];
        if BUILTIN.preferred.contains(base) || state.preferred_custom.contains(base) {
            // 常用词（含自定义）的复数不缩写：buttons -> buttons
            return w.to_string();
        }
        if let Some(a) = state.active_abbr.get(base) {
            return if a == "-" {
                w.to_string()
            } else {
                format!("{a}s")
            };
        }
        if let Some(stem) = base.strip_suffix("es") {
            if let Some(a) = state.active_abbr.get(stem) {
                return if a == "-" {
                    w.to_string()
                } else {
                    format!("{a}s")
                };
            }
        }
    }
    w.to_string()
}

fn push_dedup(out: &mut Vec<String>, word: String) {
    if out.last().map(String::as_str) != Some(word.as_str()) {
        out.push(word);
    }
}

/// 删泛词/忽略词 + 缩写映射 + 相邻去重；未收录中文字在此丢弃。
///
/// 优先级：自定义忽略 > 自定义缩写 > 常用词库（原样保留）> 内置泛词/内置缩写。
pub fn apply_reduction(words: &[String]) -> Vec<String> {
    let state = STATE.read().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::new();

    for w in words {
        if w.is_empty() || !w.is_ascii() {
            continue;
        }
        if state.custom_ignore.contains(w) {
            continue;
        }
        if state.custom_abbr.contains_key(w) {
            // 用户显式定义的缩写优先于常用词库
            let a = lookup_abbrev_with_state(w, &state);
            push_dedup(&mut out, a);
            continue;
        }
        if BUILTIN.preferred.contains(w.as_str()) || state.preferred_custom.contains(w) {
            push_dedup(&mut out, w.clone());
            continue;
        }
        if BUILTIN.stop.contains(w.as_str()) && !state.custom_keep.contains(w) {
            continue;
        }
        let a = lookup_abbrev_with_state(w, &state);
        push_dedup(&mut out, a);
    }
    out
}

fn word_priority(w: &str) -> u8 {
    if BUILTIN.verbs.contains(w) || BUILTIN.pref_verbs.contains(w) {
        return 2;
    }
    if w.len() > 3
        && (w.ends_with("ize") || w.ends_with("ise") || w.ends_with("ate") || w.ends_with("fy"))
    {
        return 2;
    }
    1
}

fn capitalize_first_ascii(w: &str) -> String {
    let mut chars = w.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut s = String::with_capacity(w.len());
    s.push(first.to_ascii_uppercase());
    s.push_str(chars.as_str());
    s
}

/// 按命名风格拼接词序列；输入单词应为全小写
pub fn format_ident(words: &[String], style: NamingStyle) -> String {
    if words.is_empty() {
        return String::new();
    }
    match style {
        NamingStyle::Camel => {
            let mut res = words[0].clone();
            for w in &words[1..] {
                if !w.is_empty() {
                    res.push_str(&capitalize_first_ascii(w));
                }
            }
            res
        }
        NamingStyle::Pascal => {
            let mut res = String::new();
            for w in words {
                if !w.is_empty() {
                    res.push_str(&capitalize_first_ascii(w));
                }
            }
            res
        }
        NamingStyle::Snake => words.join("_").to_ascii_lowercase(),
        NamingStyle::ScreamingSnake => words.join("_").to_ascii_uppercase(),
        NamingStyle::Kebab => words.join("-").to_ascii_lowercase(),
    }
}

/// 预算裁剪：从尾部丢弃优先级最低的词，同级丢最靠后的。
/// 只剩一个词仍超长时保留原词，失败判定交给 `reduce_name` 系列函数，不做字符截断。
pub fn trim_to_budget(
    words: &[String],
    max_words: usize,
    max_chars: isize,
    style: NamingStyle,
) -> Vec<String> {
    let mut res = words.to_vec();
    let mw = max_words.max(1);
    while res.len() > 1
        && (res.len() > mw || (format_ident(&res, style).len() as isize) > max_chars)
    {
        let min_pri = res.iter().map(|w| word_priority(w)).min().unwrap_or(2);
        if let Some(drop_at) = res.iter().rposition(|w| word_priority(w) == min_pri) {
            res.remove(drop_at);
        } else {
            break;
        }
    }
    res
}

/// 使用自定义保留字集合校验标识符
pub fn validate_ident_with_reserved(name: &str, reserved: &HashSet<&str>) -> bool {
    if name.is_empty() {
        return false;
    }
    let bytes = name.as_bytes();
    if name.contains('-') {
        if !bytes[0].is_ascii_lowercase() {
            return false;
        }
        if !bytes
            .iter()
            .all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return false;
        }
    } else {
        let b0 = bytes[0];
        if !(b0.is_ascii_alphabetic() || b0 == b'_') {
            return false;
        }
        if !bytes
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return false;
        }
    }
    !reserved.contains(name.to_ascii_lowercase().as_str())
}

/// 校验标识符：非空、首字符合法、字符集合法、不撞保留字（忽略大小写）
pub fn validate_ident(name: &str) -> bool {
    validate_ident_with_reserved(name, &BUILTIN.reserved)
}

// ---------------- 主入口 ----------------

/// 短语 → 合法标识符（默认 `NameKind::Local`）
pub fn reduce_name(
    phrase: &str,
    style: NamingStyle,
    max_words: usize,
    max_chars: usize,
) -> ReduceResult {
    reduce_name_with_kind(phrase, style, NameKind::Local, max_words, max_chars)
}

/// 短语 → 合法标识符。`kind` 按命名对象套用 C 惯例：
/// 全局/静态/指针加前缀（`g_`/`s_`/`p_` 或 `g`/`s`/`p` + 帕斯卡），宏恒全大写，
/// 类型帕斯卡（snake 风格时补 `_t`）。任何失败都带原因返回，绝不静默截断。
pub fn reduce_name_with_kind(
    phrase: &str,
    style: NamingStyle,
    kind: NameKind,
    max_words: usize,
    max_chars: usize,
) -> ReduceResult {
    let mw = max_words.max(1);
    let mc = max_chars.max(2);

    let mut eff = style;
    let mut prefix = "";
    let mut suffix = "";

    match kind {
        NameKind::Local | NameKind::Func => {}
        NameKind::Macro => {
            eff = NamingStyle::ScreamingSnake;
        }
        NameKind::Type => {
            eff = NamingStyle::Pascal;
            if style == NamingStyle::Snake {
                suffix = "_t";
            }
        }
        NameKind::Global | NameKind::Static | NameKind::Pointer => {
            let p = match kind {
                NameKind::Global => "g",
                NameKind::Static => "s",
                _ => "p",
            };
            if style == NamingStyle::Snake {
                prefix = match p {
                    "g" => "g_",
                    "s" => "s_",
                    _ => "p_",
                };
            } else {
                eff = NamingStyle::Pascal;
                prefix = p;
            }
        }
    }

    // 预算扣掉前后缀，防止前缀把名字挤超长；用有符号整数避免下溢
    let inner_max = (mc as isize) - (prefix.len() as isize) - (suffix.len() as isize);

    let words = split_words(phrase);
    if words.is_empty() {
        return ReduceResult::fail("空输入");
    }
    let reduced = apply_reduction(&words);
    if reduced.is_empty() {
        return ReduceResult::fail("全部为泛词或未收录词");
    }
    let trimmed = trim_to_budget(&reduced, mw, inner_max, eff);
    if trimmed.is_empty() {
        return ReduceResult::fail("无法在约束内缩减");
    }

    let name = format!("{prefix}{}{suffix}", format_ident(&trimmed, eff));
    if name.len() > mc {
        return ReduceResult::fail("超出预算（最短仍超 maxChars）");
    }
    if validate_ident(&name) {
        return ReduceResult::success(name);
    }

    // 保留字冲突：加下划线后缀再验（class -> class_），仍不行才报失败
    let fallback = format!("{name}_");
    if validate_ident(&fallback) {
        return ReduceResult::success(fallback);
    }
    ReduceResult::fail("保留字冲突且无法加后缀")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// 串行化会修改全局自定义词表状态的测试，防止 cargo test 并发互相干扰
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_all_custom_state(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir();
        let abbr = dir.join(format!("ky_rust_abbr_{tag}.txt"));
        let ign = dir.join(format!("ky_rust_ign_{tag}.txt"));
        let pref = dir.join(format!("ky_rust_pref_{tag}.txt"));
        let _ = fs::remove_file(&abbr);
        let _ = fs::remove_file(&ign);
        let _ = fs::remove_file(&pref);
        load_custom_abbreviations(&abbr);
        load_custom_ignore(&ign);
        load_custom_preferred(&pref);
        (abbr, ign, pref)
    }

    fn check(phrase: &str, style: NamingStyle, mw: usize, mc: usize, expect: &str) {
        let r = reduce_name(phrase, style, mw, mc);
        assert!(
            r.ok && r.name == expect,
            "phrase={phrase:?} -> got ok={} name={:?} reason={:?}, want {expect:?}",
            r.ok,
            r.name,
            r.reason
        );
    }

    fn check_fail(phrase: &str, style: NamingStyle, mw: usize, mc: usize, reason_part: &str) {
        let r = reduce_name(phrase, style, mw, mc);
        assert!(
            !r.ok && r.reason.contains(reason_part),
            "phrase={phrase:?} -> got ok={} name={:?} reason={:?}, want fail({reason_part:?})",
            r.ok,
            r.name,
            r.reason
        );
    }

    fn check_k(
        phrase: &str,
        style: NamingStyle,
        kind: NameKind,
        mw: usize,
        mc: usize,
        expect: &str,
    ) {
        let r = reduce_name_with_kind(phrase, style, kind, mw, mc);
        assert!(
            r.ok && r.name == expect,
            "[{kind}] phrase={phrase:?} -> got ok={} name={:?} reason={:?}, want {expect:?}",
            r.ok,
            r.name,
            r.reason
        );
    }

    fn check_fail_k(
        phrase: &str,
        style: NamingStyle,
        kind: NameKind,
        mw: usize,
        mc: usize,
        reason_part: &str,
    ) {
        let r = reduce_name_with_kind(phrase, style, kind, mw, mc);
        assert!(
            !r.ok && r.reason.contains(reason_part),
            "[{kind}] phrase={phrase:?} -> got ok={} name={:?} reason={:?}, want fail({reason_part:?})",
            r.ok,
            r.name,
            r.reason
        );
    }

    #[test]
    fn test_basic_chinese_english_and_mixed() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (a, i, p) = reset_all_custom_state("basic");

        // —— 中文场景 ——
        check("串口初始化程序", NamingStyle::Camel, 4, 40, "uartInit");
        check(
            "用户登录状态缓存管理器",
            NamingStyle::Camel,
            4,
            40,
            "userLoginStateCache",
        );
        check(
            "配置文件读取路径",
            NamingStyle::Camel,
            4,
            40,
            "configFileReadPath",
        );

        // —— 英文：泛词 + 缩写 + 预算裁剪 ——
        check(
            "The quick brown fox jumps over the lazy dog",
            NamingStyle::Snake,
            4,
            40,
            "quick_brown_fox_jumps",
        );
        check(
            "Serial Port Initialization Program",
            NamingStyle::Camel,
            4,
            40,
            "serialPortInit",
        );
        check(
            "get the maximum value of the buffer",
            NamingStyle::Camel,
            4,
            40,
            "getMaxValueBuf",
        );
        check(
            " temporary configuration file reader ",
            NamingStyle::Camel,
            4,
            40,
            "tmpConfigFileReader",
        );

        // —— 混排与驼峰拆词 ——
        check(
            "TCP/IP 网络连接管理器",
            NamingStyle::Pascal,
            4,
            40,
            "TcpIpNetworkConn",
        );
        check(
            "parseXMLDocument configuration",
            NamingStyle::Camel,
            4,
            40,
            "parseXmlDocConfig",
        );
        check(
            "HTTP 服务器状态",
            NamingStyle::Camel,
            4,
            40,
            "httpServerState",
        );

        // —— 其它风格 ——
        check("数据库连接池", NamingStyle::Camel, 3, 40, "dbConnPool");
        check(
            "配置文件读取路径",
            NamingStyle::Kebab,
            4,
            40,
            "config-file-read-path",
        );

        // —— 命名对象：C 惯例前缀/大小写 ——
        check_k(
            "led 初始化",
            NamingStyle::Snake,
            NameKind::Func,
            4,
            40,
            "led_init",
        );
        check_k(
            "led 初始化",
            NamingStyle::Camel,
            NameKind::Func,
            4,
            40,
            "ledInit",
        );
        check_k(
            "led 状态",
            NamingStyle::Snake,
            NameKind::Local,
            4,
            40,
            "led_state",
        );
        check_k(
            "led 状态",
            NamingStyle::Camel,
            NameKind::Local,
            4,
            40,
            "ledState",
        );
        check_k(
            "用户登录状态",
            NamingStyle::Snake,
            NameKind::Global,
            4,
            40,
            "g_user_login_state",
        );
        check_k(
            "用户登录状态",
            NamingStyle::Camel,
            NameKind::Global,
            4,
            40,
            "gUserLoginState",
        );
        check_k(
            "led 状态",
            NamingStyle::Snake,
            NameKind::Static,
            4,
            40,
            "s_led_state",
        );
        check_k(
            "led 状态",
            NamingStyle::Camel,
            NameKind::Static,
            4,
            40,
            "sLedState",
        );
        check_k(
            "数据缓冲区",
            NamingStyle::Snake,
            NameKind::Pointer,
            4,
            40,
            "p_data_buf",
        );
        check_k(
            "数据缓冲区",
            NamingStyle::Camel,
            NameKind::Pointer,
            4,
            40,
            "pDataBuf",
        );
        check_k(
            "最大长度",
            NamingStyle::Snake,
            NameKind::Macro,
            4,
            40,
            "MAX_LEN",
        );
        check_k(
            "最大长度",
            NamingStyle::Camel,
            NameKind::Macro,
            4,
            40,
            "MAX_LEN",
        );
        check_k(
            "串口控制",
            NamingStyle::Snake,
            NameKind::Type,
            4,
            40,
            "UartControl_t",
        );
        check_k(
            "串口控制",
            NamingStyle::Camel,
            NameKind::Type,
            4,
            40,
            "UartControl",
        );
        check_fail_k(
            "配置文件读取路径",
            NamingStyle::Snake,
            NameKind::Pointer,
            4,
            5,
            "超出预算",
        );

        // —— 常用词库 ——
        check("串口发送数据", NamingStyle::Snake, 4, 40, "uart_send_data");
        check("温度传感器", NamingStyle::Camel, 4, 40, "tempSensor");
        check("led 灯闪烁", NamingStyle::Snake, 4, 40, "led_blink");
        check("按键扫描", NamingStyle::Camel, 4, 40, "keyScan");
        check("看门狗喂狗", NamingStyle::Snake, 4, 40, "wdt_feed_dog");
        check("按钮", NamingStyle::Camel, 4, 40, "button");
        check("buttons", NamingStyle::Camel, 4, 40, "buttons");
        check(
            "数值是否超时",
            NamingStyle::Snake,
            4,
            40,
            "value_is_timeout",
        );
        check("电机正转", NamingStyle::Snake, 4, 40, "motor_forward");
        check("舵机角度", NamingStyle::Camel, 4, 40, "servoAngle");
        check("占空比", NamingStyle::Snake, 4, 40, "duty");
        check("高电平", NamingStyle::Snake, 4, 40, "high_level");
        check("外部中断", NamingStyle::Snake, 4, 40, "exti");
        check("定时器中断", NamingStyle::Camel, 4, 40, "timerIrq");
        check("频率周期", NamingStyle::Snake, 4, 40, "freq_period");
        check("高级设置", NamingStyle::Camel, 4, 40, "advancedSet");

        // —— 保留字冲突与失败原因 ——
        check("class", NamingStyle::Snake, 4, 40, "class_");
        check_fail(
            "the of and",
            NamingStyle::Camel,
            4,
            40,
            "全部为泛词或未收录词",
        );
        check_fail("", NamingStyle::Camel, 4, 40, "空输入");
        check_fail(
            "internationalization configuration",
            NamingStyle::Camel,
            4,
            5,
            "超出预算",
        );

        // —— validate_ident 直测 ——
        assert!(validate_ident("userLogin_2"));
        assert!(!validate_ident("2fast"));
        assert!(!validate_ident("If"));
        assert!(validate_ident("config-file-path"));

        let _ = fs::remove_file(a);
        let _ = fs::remove_file(i);
        let _ = fs::remove_file(p);
    }

    #[test]
    fn test_custom_abbreviations() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (tmp, ign, pref) = reset_all_custom_state("abbr");

        let n0 = load_custom_abbreviations(&tmp);
        assert_eq!(n0, 0);
        assert_eq!(
            reduce_name("configuration reader", NamingStyle::Camel, 4, 40).name,
            "configReader"
        );

        let (cnt1, bad1) = save_custom_abbreviations(&tmp, "configuration = cfg\nservice = svc\n");
        assert_eq!((cnt1, bad1), (2, 0));
        assert_eq!(
            reduce_name("configuration reader", NamingStyle::Camel, 4, 40).name,
            "cfgReader"
        );
        assert_eq!(
            reduce_name("service status", NamingStyle::Camel, 4, 40).name,
            "svcStatus"
        );

        let (_, bad2) = save_custom_abbreviations(&tmp, "novalue\nservice = svc\n");
        assert_eq!(bad2, 1);
        assert_eq!(
            reduce_name("configuration reader", NamingStyle::Camel, 4, 40).name,
            "cfgReader"
        );

        let (cnt3, bad3) = save_custom_abbreviations(&tmp, "# 全删\n\n");
        assert_eq!((cnt3, bad3), (0, 0));
        assert_eq!(
            reduce_name("configuration reader", NamingStyle::Camel, 4, 40).name,
            "configReader"
        );

        let _ = fs::remove_file(tmp);
        let _ = fs::remove_file(ign);
        let _ = fs::remove_file(pref);
    }

    #[test]
    fn test_custom_ignore() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (tmp_abbr, tmp_ign, pref) = reset_all_custom_state("ign");

        assert_eq!(
            reduce_name("user login status cache", NamingStyle::Camel, 4, 40).name,
            "userLoginStatusCache"
        );

        let n1 = save_custom_ignore(&tmp_ign, "# 测试\nstatus\n\n");
        assert_eq!(n1, 1);
        assert_eq!(
            reduce_name("user login status cache", NamingStyle::Camel, 4, 40).name,
            "userLoginCache"
        );

        save_custom_abbreviations(&tmp_abbr, "service = svc\n");
        save_custom_ignore(&tmp_ign, "zeta\nalpha\nservice\n");
        let r2 = reduce_name("service login", NamingStyle::Camel, 4, 40);
        assert!(r2.ok);
        assert_eq!(r2.name, "login");
        assert_eq!(
            custom_ignore_words(),
            vec![
                "alpha".to_string(),
                "service".to_string(),
                "zeta".to_string()
            ]
        );

        assert_eq!(save_custom_ignore(&tmp_ign, ""), 0);
        assert_eq!(
            reduce_name("user login status cache", NamingStyle::Camel, 4, 40).name,
            "userLoginStatusCache"
        );

        let _ = fs::remove_file(tmp_abbr);
        let _ = fs::remove_file(tmp_ign);
        let _ = fs::remove_file(pref);
    }

    #[test]
    fn test_builtin_disable_and_keep() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (tmp_abbr, tmp_ign, pref) = reset_all_custom_state("builtin");

        let b0 = reduce_name("initialization detail", NamingStyle::Camel, 3, 40);
        assert!(b0.ok && b0.name == "init");

        save_custom_abbreviations(&tmp_abbr, "initialization=-\n");
        let d1 = reduce_name("initialization", NamingStyle::Camel, 3, 40);
        let d2 = reduce_name("initializations", NamingStyle::Camel, 3, 40);
        assert!(d1.ok && d1.name == "initialization");
        assert!(d2.ok && d2.name == "initializations");

        save_custom_ignore(&tmp_ign, "+detail\n");
        let k1 = reduce_name("initialization detail", NamingStyle::Camel, 3, 40);
        assert!(k1.ok && k1.name == "initializationDetail");

        let entries = custom_abbr_entries();
        assert!(entries
            .iter()
            .any(|(w, a)| w == "initialization" && a == "-"));
        let pairs = custom_abbr_pairs();
        assert!(!pairs.iter().any(|(w, _)| w == "initialization"));

        let stops = builtin_stop_words();
        assert!(stops.len() > 50);
        assert!(stops.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(custom_keep_words(), vec!["detail".to_string()]);
        assert!(builtin_abbreviations().len() > 30);

        save_custom_abbreviations(&tmp_abbr, "");
        save_custom_ignore(&tmp_ign, "");
        let z = reduce_name("initialization detail", NamingStyle::Camel, 3, 40);
        assert!(z.ok && z.name == "init");
        assert!(custom_keep_words().is_empty());

        let _ = fs::remove_file(tmp_abbr);
        let _ = fs::remove_file(tmp_ign);
        let _ = fs::remove_file(pref);
    }

    #[test]
    fn test_custom_preferred() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (abbr, ign, tmp) = reset_all_custom_state("pref");

        let n0 = load_custom_preferred(&tmp);
        assert_eq!(n0, 0);
        assert_eq!(
            reduce_name("串口初始化", NamingStyle::Snake, 4, 40).name,
            "uart_init"
        );
        assert!(builtin_preferred_pairs().len() > 80);

        let (cnt1, bad1) = save_custom_preferred(&tmp, "串口=serial port\n");
        assert_eq!((cnt1, bad1), (1, 0));
        assert_eq!(
            reduce_name("串口初始化", NamingStyle::Snake, 4, 40).name,
            "serial_port_init"
        );

        save_custom_preferred(&tmp, "气泵=pump\n蜂鸣器=-\n");
        let r3a = reduce_name("气泵启动", NamingStyle::Snake, 4, 40).name;
        let r3b = reduce_name("蜂鸣器响", NamingStyle::Snake, 4, 40).name;
        assert_eq!(r3a, "pump_start");
        assert_eq!(r3b, "beep");

        let (_, bad4) = save_custom_preferred(&tmp, "badline\n");
        let (_, bad5) = save_custom_preferred(&tmp, "usb=usb\n");
        assert_eq!(bad4, 1);
        assert_eq!(bad5, 1);
        assert_eq!(
            reduce_name("气泵启动", NamingStyle::Snake, 4, 40).name,
            "pump_start"
        );
        // 内置普通映射（配置→configuration）不允许通过常用词页修改
        assert_eq!(builtin_zh_kind("配置"), 2);
        assert_eq!(builtin_zh_kind("串口"), 1);
        assert_eq!(builtin_zh_kind("不存在的词"), 0);
        assert_eq!(save_custom_preferred(&tmp, "配置=cfg\n").1, 1);

        let merged = preferred_merge();
        assert!(merged.iter().any(|(zh, en)| zh == "气泵" && en == "pump"));
        assert!(!merged.iter().any(|(zh, _)| zh == "蜂鸣器"));

        let (cnt6, bad6) = save_custom_preferred(&tmp, "# 全删\n\n");
        assert_eq!((cnt6, bad6), (0, 0));
        assert_eq!(
            reduce_name("串口初始化", NamingStyle::Snake, 4, 40).name,
            "uart_init"
        );

        let _ = fs::remove_file(abbr);
        let _ = fs::remove_file(ign);
        let _ = fs::remove_file(tmp);
    }
}
