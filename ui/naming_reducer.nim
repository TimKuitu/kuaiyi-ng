## naming_reducer —— 翻译结果 → 合法标识符 的本地缩减模块
## 用途：复制单词/AI 命名前，把中文或英文短语缩成符合预算的标识符
## 纯本地词表规则，不联网；依赖仅 std 库。
##
## 流水线：拆词 → 删泛词 → 缩写映射 → 预算裁剪 → 拼格式 → 校验
## 常用词库（preferredNouns/Verbs/Attrs，用户指定）命中即原样保留：不删泛词、不做缩写
## 失败一律带原因返回（ReduceResult.reason），不静默截断字符串。

import std/[strutils, tables, sets, unicode, os, algorithm]

type
  NamingStyle* = enum
    nsCamel          ## camelCase
    nsPascal         ## PascalCase
    nsSnake          ## snake_case
    nsScreamingSnake ## SCREAMING_SNAKE
    nsKebab          ## kebab-case（文件名/URL 场景，非严格标识符）

  ## 命名对象：决定前缀/大小写套路，序号即主窗口「命名对象」下拉的选项序
  NameKind* = enum
    nkLocal    ## 局部变量
    nkGlobal   ## 全局变量（snake: g_ 前缀，驼峰: g+帕斯卡）
    nkStatic   ## 静态变量（s_ / s+帕斯卡）
    nkPointer  ## 指针变量（p_ / p+帕斯卡）
    nkType     ## 结构体/类型（帕斯卡；snake 风格时带 _t 后缀）
    nkMacro    ## 宏/常量（恒为 SCREAMING_SNAKE）
    nkFunc     ## 函数

  ReduceResult* = object
    ok*: bool
    name*: string   ## 成功时的标识符
    reason*: string ## 失败原因

# ---------------- 规则表 ----------------

# 泛词删除表：虚词 + "X的Y"结构里的通用头名词（命名时冗余，直接删）
const stopWords* = [
  # 虚词
  "the", "a", "an", "of", "for", "to", "and", "or", "in", "on", "with",
  "by", "at", "from", "as", "into", "is", "are", "was", "were", "be",
  "been", "being", "do", "does", "this", "that", "these", "those", "it",
  "its", "s", "please", "some", "any", "my", "your", "our", "their",
  "each", "every", "than", "then", "so", "such", "not", "no", "nor", "but",
  # 通用头名词（程序/管理器之类，命名时冗余）
  "program", "manager", "module", "system", "tool", "utility",
  "function", "method", "service", "handler", "wrapper", "helper",
  "implementation", "feature", "item", "detail",
]

# 长词缩写映射（编程圈惯用缩写），复数自动带 s
const abbreviations* = [
  ("initialization", "init"), ("initialize", "init"),
  ("configuration", "config"), ("application", "app"),
  ("authentication", "auth"), ("synchronization", "sync"),
  ("temporary", "tmp"), ("message", "msg"), ("button", "btn"),
  ("window", "win"), ("database", "db"), ("repository", "repo"),
  ("parameter", "param"), ("argument", "arg"), ("attribute", "attr"),
  ("environment", "env"), ("document", "doc"), ("directory", "dir"),
  ("source", "src"), ("destination", "dst"), ("reference", "ref"),
  ("number", "num"), ("position", "pos"), ("previous", "prev"),
  ("current", "cur"), ("maximum", "max"), ("minimum", "min"),
  ("buffer", "buf"), ("context", "ctx"), ("command", "cmd"),
  ("execute", "exec"), ("validate", "valid"), ("generate", "gen"),
  ("calculate", "calc"), ("terminate", "term"), ("connection", "conn"),
  ("request", "req"), ("response", "resp"), ("session", "sess"),
  ("storage", "store"),
  # 补充常用
  ("address", "addr"), ("identifier", "id"), ("information", "info"),
  ("index", "idx"), ("length", "len"), ("string", "str"), ("image", "img"),
  ("count", "cnt"), ("specification", "spec"), ("object", "obj"),
  ("element", "elem"), ("collection", "coll"), ("history", "hist"),
  ("language", "lang"), ("library", "lib"), ("package", "pkg"),
  ("process", "proc"), ("statistics", "stats"), ("value", "val"),
  ("variable", "var"),
  # 常用词库的完整词形（temp/humi/freq 已是首选，英文输入也对齐）
  ("temperature", "temp"), ("humidity", "humi"), ("frequency", "freq"),
]

# 保留字（C/C++/C#/Java/JS/Python/Go/Rust/Nim 常用保留字去重合集）
const reservedWords* = [
  # C / C++
  "auto", "break", "case", "char", "const", "continue", "default", "do",
  "double", "else", "enum", "extern", "float", "for", "goto", "if", "int",
  "long", "register", "return", "short", "signed", "sizeof", "static",
  "struct", "switch", "typedef", "union", "unsigned", "void", "volatile",
  "while", "class", "public", "private", "protected", "virtual", "friend",
  "inline", "template", "typename", "namespace", "using", "new", "delete",
  "this", "try", "catch", "throw", "operator", "bool", "true", "false",
  "explicit", "export", "mutable", "nullptr", "constexpr", "decltype",
  "noexcept", "override", "final", "asm",
  # C# / Java
  "byte", "sbyte", "decimal", "delegate", "event", "fixed", "foreach",
  "in", "is", "lock", "object", "out", "params", "ref", "sealed", "typeof",
  "uint", "ulong", "unchecked", "unsafe", "ushort", "var", "abstract",
  "assert", "boolean", "extends", "implements", "instanceof", "interface",
  "native", "package", "super", "synchronized", "throws", "transient",
  # JS / Python
  "function", "let", "of", "async", "await", "yield", "typeof", "null",
  "undefined", "def", "lambda", "pass", "raise", "with", "global",
  "nonlocal", "del", "not", "and", "or", "import", "from", "as", "except",
  "finally", "none", "match",
  # Go / Rust
  "func", "chan", "go", "defer", "select", "range", "map", "type",
  "fallthrough", "nil", "fn", "mut", "impl", "trait", "use", "mod", "pub",
  "crate", "self", "move", "dyn", "where", "loop", "unsafe",
  # Nim
  "proc", "iterator", "converter", "template", "macro", "bind", "mixin",
  "discard", "elif", "when", "end", "addr", "ptr", "object", "tuple",
  "concept", "distinct", "array", "seq", "set", "string",
]

# ---------------- 常用词库（2026-10-27 用户指定，嵌入式 C 场景） ----------------
# 三组词是命名的首选 vocabulary：中文映射直接指向这些词（见 zhTerms 末段），
# 命中的词原样保留——不删泛词、不做缩写（否则 button 会被缩成 btn，违背"优先使用"）。
# 动作动词组另用于预算裁剪时优先保留（wordPriority）。

const preferredNouns* = [
  "led", "key", "button", "buzzer", "beep", "relay", "motor", "servo",
  "stepper", "encoder", "segment", "lcd", "oled", "display", "keypad",
  "sensor", "temp", "humi", "light", "ir", "ultrasonic", "rfid",
  "uart", "usart", "i2c", "spi", "gpio", "timer", "pwm", "adc", "dac",
  "irq", "exti", "nvic", "dma", "rtc", "wdt", "eeprom", "flash",
  "bluetooth", "ble", "wifi", "usb", "can", "modbus",
  "fan", "pump", "valve", "solenoid", "switch",
]

const preferredVerbs* = [
  "init", "deinit", "config", "set", "get", "read", "write",
  "send", "transmit", "receive", "recv", "open", "close",
  "start", "stop", "reset", "clear", "update", "scan",
  "display", "show", "toggle", "enable", "disable",
  "measure", "check", "is", "handle", "process", "callback",
  "feed", "beep", "blink", "forward", "backward", "brake", "calibrate",
]

const preferredAttrs* = [
  "state", "status", "value", "level", "mode", "speed", "angle", "duty",
  "period", "frequency", "freq", "channel", "pin", "port", "addr",
  "length", "len", "count", "flag", "timeout", "threshold",
  "high", "low", "on", "off", "rising", "falling",
]

# 中文技术词表：串口初始化程序 这类无空格描述靠它分词+翻译
# 正向最长匹配，一词可对应多个英文单词（空格分隔）
const zhTerms* = [
  ("串口", "uart"), ("初始化", "initialize"), ("程序", "program"),
  ("配置", "configuration"), ("文件", "file"), ("读取", "read"),
  ("写入", "write"), ("路径", "path"), ("目录", "directory"),
  ("文件夹", "folder"), ("用户", "user"), ("登录", "login"),
  ("注销", "logout"), ("密码", "password"), ("状态", "state"),
  ("缓存", "cache"), ("缓冲区", "buffer"), ("管理器", "manager"),
  ("数据", "data"), ("数据库", "database"), ("池", "pool"),
  ("连接", "connection"), ("网络", "network"), ("服务器", "server"),
  ("客户端", "client"), ("服务", "service"), ("接口", "interface"),
  ("协议", "protocol"), ("版本", "version"), ("更新", "update"),
  ("删除", "delete"), ("移除", "remove"), ("添加", "add"),
  ("创建", "create"), ("获取", "get"), ("设置", "set"),
  ("保存", "save"), ("打开", "open"), ("关闭", "close"),
  ("启动", "start"), ("停止", "stop"), ("暂停", "pause"),
  ("重启", "restart"), ("退出", "exit"), ("时间", "time"),
  ("日期", "date"), ("超时", "timeout"), ("重试", "retry"),
  ("大小", "size"), ("类型", "type"), ("名称", "name"), ("编号", "number"),
  ("序号", "index"), ("列表", "list"), ("标签", "tag"), ("按钮", "button"),
  ("窗口", "window"), ("菜单", "menu"), ("页面", "page"), ("图标", "icon"),
  ("图片", "image"), ("颜色", "color"), ("字体", "font"), ("文本", "text"),
  ("字符串", "string"), ("字符", "char"), ("数值", "value"),
  ("参数", "parameter"), ("变量", "variable"), ("常量", "constant"),
  ("函数", "function"), ("方法", "method"), ("类", "class"),
  ("对象", "object"), ("数组", "array"), ("集合", "collection"),
  ("队列", "queue"), ("栈", "stack"), ("节点", "node"), ("键", "key"),
  ("索引", "index"), ("长度", "len"), ("位置", "position"),
  ("方向", "direction"), ("通道", "channel"), ("线程", "thread"),
  ("进程", "process"), ("内存", "memory"), ("磁盘", "disk"),
  ("驱动", "driver"), ("设备", "device"), ("硬件", "hardware"),
  ("软件", "software"), ("命令", "command"), ("脚本", "script"),
  ("模块", "module"), ("组件", "component"), ("插件", "plugin"),
  ("库", "library"), ("框架", "framework"), ("工具", "tool"),
  ("模板", "template"), ("示例", "example"), ("测试", "test"),
  ("调试", "debug"), ("监控", "monitor"), ("统计", "statistics"),
  ("图表", "chart"), ("报告", "report"), ("屏幕", "screen"),
  ("键盘", "keypad"), ("剪贴板", "clipboard"), ("桌面", "desktop"),
  ("帮助", "help"), ("编码", "encode"), ("解码", "decode"),
  ("加密", "encrypt"), ("解密", "decrypt"), ("压缩", "compress"),
  ("解压", "decompress"), ("解析", "parse"), ("生成", "generate"),
  ("计算", "calculate"), ("转换", "convert"), ("校验", "validate"),
  ("验证", "validate"), ("检查", "check"), ("过滤", "filter"),
  ("排序", "sort"), ("比较", "compare"), ("合并", "merge"),
  ("拆分", "split"), ("格式化", "format"), ("复制", "copy"),
  ("粘贴", "paste"), ("搜索", "search"), ("查找", "find"),
  ("替换", "replace"), ("下载", "download"), ("上传", "upload"),
  ("发送", "send"), ("接收", "receive"), ("刷新", "refresh"),
  ("加载", "load"), ("清空", "clear"), ("重置", "reset"),
  ("同步", "sync"), ("异步", "async"), ("最大", "maximum"),
  ("最小", "minimum"), ("平均", "average"), ("当前", "current"),
  ("上一个", "previous"), ("下一个", "next"), ("默认", "default"),
  ("全局", "global"), ("本地", "local"), ("远程", "remote"),
  ("临时", "temporary"), ("内部", "internal"), ("外部", "external"),
  ("自动", "automatic"), ("手动", "manual"), ("高级", "advanced"),
  ("基本", "basic"), ("详细", "detail"), ("摘要", "summary"),
  ("信息", "information"), ("日志", "log"), ("记录", "record"),
  ("历史", "history"), ("通知", "notification"), ("消息", "message"),
  ("警告", "warning"), ("错误", "error"), ("异常", "exception"),
  ("成功", "success"), ("失败", "failure"), ("权限", "permission"),
  ("账号", "account"), ("邮箱", "email"), ("地址", "address"),
  ("端口", "port"), ("主机", "host"), ("会话", "session"),
  ("令牌", "token"), ("密钥", "secret"), ("安全", "security"),
  ("隐藏", "hide"), ("显示", "display"), ("语言", "language"),
  ("翻译", "translate"), ("输入", "input"), ("输出", "output"),
  ("源", "source"), ("目标", "target"), ("地址簿", "address book"),
  ("主题", "theme"), ("滚动", "scroll"), ("拖拽", "drag"),
  ("选择", "select"), ("管理", "manage"), ("处理", "handle"),
  ("控制", "control"),
  ("执行", "execute"), ("运行", "run"), ("调度", "schedule"),
  ("分配", "allocate"), ("释放", "release"), ("申请", "apply"),
  # —— 常用词库中文映射（2026-10-27 用户指定，见 preferredNouns/Verbs/Attrs）——
  # 模块名词：中文直接映射到词库用词（串口不再给 serial port，给 uart）
  ("指示灯", "led"), ("灯", "led"), ("蜂鸣器", "buzzer"), ("蜂鸣", "beep"),
  ("继电器", "relay"), ("电机", "motor"), ("马达", "motor"), ("舵机", "servo"),
  ("步进电机", "stepper motor"), ("步进", "stepper"), ("编码器", "encoder"),
  ("数码管", "segment"), ("液晶屏", "lcd"), ("液晶", "lcd"), ("矩阵键盘", "keypad"),
  ("传感器", "sensor"), ("温度", "temp"), ("湿度", "humi"), ("光照", "light"),
  ("灯光", "light"), ("光", "light"), ("红外", "ir"), ("超声波", "ultrasonic"),
  ("射频识别", "rfid"), ("定时器", "timer"), ("脉宽调制", "pwm"), ("脉宽", "pwm"),
  ("模数转换", "adc"), ("模数", "adc"), ("数模转换", "dac"), ("数模", "dac"),
  ("外部中断", "exti"), ("中断", "irq"), ("直接内存访问", "dma"),
  ("直接存储访问", "dma"), ("实时时钟", "rtc"), ("看门狗", "wdt"), ("闪存", "flash"),
  ("蓝牙", "bluetooth"), ("低功耗蓝牙", "ble"), ("无线网络", "wifi"),
  ("通用串行总线", "usb"), ("风扇", "fan"), ("水泵", "pump"), ("泵", "pump"),
  ("阀", "valve"), ("电磁阀", "solenoid"), ("开关", "switch"),
  # 动作动词
  ("反初始化", "deinit"), ("去初始化", "deinit"), ("发射", "transmit"),
  ("复位", "reset"), ("清除", "clear"), ("扫描", "scan"), ("展示", "show"),
  ("翻转", "toggle"), ("切换", "toggle"), ("使能", "enable"), ("启用", "enable"),
  ("禁用", "disable"), ("失能", "disable"), ("测量", "measure"), ("是否", "is"),
  ("回调", "callback"), ("喂狗", "feed dog"), ("喂", "feed"), ("闪烁", "blink"),
  ("正转", "forward"), ("前进", "forward"), ("反转", "backward"),
  ("后退", "backward"), ("刹车", "brake"), ("制动", "brake"),
  ("校准", "calibrate"), ("开始", "start"),
  # 属性参数
  ("电平", "level"), ("级别", "level"), ("模式", "mode"), ("速度", "speed"),
  ("角度", "angle"), ("占空比", "duty"), ("周期", "period"), ("频率", "freq"),
  ("引脚", "pin"), ("计数", "count"), ("数量", "count"), ("标志", "flag"),
  ("阈值", "threshold"), ("高", "high"), ("低", "low"), ("开", "on"), ("关", "off"),
  ("上升沿", "rising"), ("上升", "rising"), ("下降沿", "falling"),
  ("下降", "falling"),
  # 单字映射的防误伤保护（复合词更长，正向最长匹配下优先于单字命中）
  ("高级", "advanced"), ("最低", "minimum"), ("提高", "increase"),
  ("降低", "decrease"), ("开机", "start"), ("关机", "shutdown"),
  ("光标", "cursor"),
]

let g_stop = toHashSet(stopWords)
let g_abbrev = toTable(abbreviations)
let g_zh = toTable(zhTerms)
# 常用词库合集：命中即原样保留（不删泛词、不缩写）；动词子集另用于裁剪优先级
let g_preferred* = toHashSet(preferredNouns) + toHashSet(preferredVerbs) +
                   toHashSet(preferredAttrs)
let g_prefVerbs* = toHashSet(preferredVerbs)

# 用户自定义缩写（设置页编辑，存 fastT\custom_abbr.txt），同名覆盖内置
var g_custom = initTable[string, string]()
var g_active = g_abbrev   # 内置 + 自定义合并后的生效表
let g_verbs = toHashSet([
  "get", "set", "read", "write", "load", "save", "init", "parse", "convert",
  "update", "delete", "remove", "create", "add", "check", "gen", "calc",
  "exec", "run", "open", "close", "start", "stop", "copy", "fetch", "build",
  "make", "find", "query", "send", "recv", "flush", "clear", "reset",
  "sync", "login", "logout", "download", "upload", "encode", "decode",
  "encrypt", "decrypt", "compress", "format", "print", "draw", "handle",
])

# 校验用的保留字集合（validateIdent 的 reserved 实参）
let reservedSet* = toHashSet(reservedWords)

# ---------------- 自定义缩写（fastT\custom_abbr.txt，每行 原词=缩写，# 注释） ----------------

proc parseAbbrText(text: string): tuple[table: Table[string, string], badLine: int] =
  ## 解析编辑框文本；badLine>0 = 该行缺 '='，整体拒绝
  result.table = initTable[string, string]()
  var i = 0
  for raw in strutils.splitLines(text):
    inc i
    let line = strutils.strip(raw)
    if line.len == 0 or line.startsWith("#"): continue
    let eq = line.find('=')
    if eq <= 0:
      result.badLine = i
      return
    let k = strutils.strip(line[0 ..< eq]).toLowerAscii
    let v = strutils.strip(line[eq + 1 ..^ 1]).toLowerAscii
    if k.len == 0 or v.len == 0:
      result.badLine = i
      return
    result.table[k] = v

proc rebuildActive() =
  g_active = g_abbrev
  for k, v in g_custom:
    g_active[k] = v

proc loadCustomAbbreviations*(path: string): int =
  ## 启动时加载；文件不存在/为空 = 清空自定义，只留内置。返回条数
  g_custom = initTable[string, string]()
  if fileExists(path):
    let r = parseAbbrText(readFile(path))
    if r.badLine == 0: g_custom = r.table
  rebuildActive()
  g_custom.len

proc saveCustomAbbreviations*(path, text: string): tuple[count: int, badLine: int] =
  ## 校验 → 写文件 → 立即生效；badLine>0 时拒绝保存（返回出错行号）
  let r = parseAbbrText(text)
  if r.badLine > 0: return (0, r.badLine)
  writeFile(path, text)
  g_custom = r.table
  rebuildActive()
  (g_custom.len, 0)

proc customAbbrText*(path: string): string =
  ## 给设置页编辑框回填当前文件内容
  if fileExists(path): readFile(path) else: ""

proc customAbbrEntries*(): seq[(string, string)] =
  ## 设置页回填用：全部自定义项（含 "-" = 禁用内置缩写），按原词排序
  result = @[]
  for k, v in g_custom: result.add((k, v))
  result.sort(proc (a, b: (string, string)): int = cmp(a[0], b[0]))

proc customAbbrPairs*(): seq[(string, string)] =
  ## 生效的自定义缩写（不含禁用项），按原词排序
  result = @[]
  for k, v in g_custom:
    if v != "-": result.add((k, v))
  result.sort(proc (a, b: (string, string)): int = cmp(a[0], b[0]))

proc builtinAbbreviations*(): seq[(string, string)] =
  ## 设置页「缩略词」的默认行：内置缩写表，按原词排序
  result = @[]
  for k, v in g_abbrev: result.add((k, v))
  result.sort(proc (a, b: (string, string)): int = cmp(a[0], b[0]))

proc builtinStopWords*(): seq[string] =
  ## 设置页「忽略词」的默认行：内置泛词表，按字母序
  result = @[]
  for w in g_stop: result.add(w)
  result.sort()

# ---------------- 自定义忽略词（fastT\custom_ignore.txt，每行一个词，# 注释） ----------------

# 命中即从命名结果里删掉。优先级最高：同一个词在缩略/忽略两边都填时按忽略处理
var g_customIgnore = initHashSet[string]()
# 保留表：内置泛词本来会被删掉，用户显式写 "+word" 表示要保留
var g_customKeep = initHashSet[string]()

proc parseIgnoreText(text: string): tuple[ignore, keep: HashSet[string]] =
  ## 每行一个词；"+word" = 保留（不删）；# 注释
  result.ignore = initHashSet[string]()
  result.keep = initHashSet[string]()
  for raw in strutils.splitLines(text):
    let line = strutils.strip(raw)
    if line.len == 0 or line.startsWith("#"): continue
    if line.startsWith("+"):
      let w = strutils.strip(line[1 .. ^1]).toLowerAscii
      if w.len > 0: result.keep.incl(w)
    else:
      result.ignore.incl(line.toLowerAscii)

proc loadCustomIgnore*(path: string): int =
  ## 启动时加载；文件不存在/为空 = 无自定义忽略。返回忽略词条数
  g_customIgnore = initHashSet[string]()
  g_customKeep = initHashSet[string]()
  if fileExists(path):
    let r = parseIgnoreText(readFile(path))
    g_customIgnore = r.ignore
    g_customKeep = r.keep
  g_customIgnore.len

proc saveCustomIgnore*(path, text: string): int =
  ## 写文件 → 立即生效；返回忽略词条数（空行/注释/+保留行不计）
  let r = parseIgnoreText(text)
  writeFile(path, text)
  g_customIgnore = r.ignore
  g_customKeep = r.keep
  r.ignore.len

proc customIgnoreText*(path: string): string =
  ## 给设置页编辑框回填当前文件内容
  if fileExists(path): readFile(path) else: ""

proc customIgnoreWords*(): seq[string] =
  ## 设置页回填用：自定义忽略词（按字母序）
  result = @[]
  for w in g_customIgnore: result.add(w)
  result.sort()

proc customKeepWords*(): seq[string] =
  ## 设置页回填用：内置泛词里被显式保留的词（按字母序）
  result = @[]
  for w in g_customKeep: result.add(w)
  result.sort()

# ---------------- 自定义常用词（fastT\custom_preferred.txt，每行 中文=英文，# 注释） ----------------
# 「常用词」设置页维护：改英文=覆盖内置映射（串口=serial port），中文=- 停用内置映射，
# 也可以加全新词条（气泵=pump）。自定义英文词自动获得常用词待遇（不删不缩写）。
# 键限定：内置「词库」映射的中文，或全新中文；内置普通映射（配置→configuration）不归这页管。

var g_customZh = initTable[string, string]()  # 中文 → 英文（含对内置词库映射的覆盖）
var g_zhOff = initHashSet[string]()           # 被停用的内置词库映射（中文）
var g_preferredCustom = initHashSet[string]() # 自定义英文词（享受常用词待遇）

proc isPreferredWord(w: string): bool =
  ## 自定义词库的英文词：纯字母/数字，1..20 字符
  if w.len == 0 or w.len > 20: return false
  for c in w:
    if not (c in {'a'..'z', 'A'..'Z', '0'..'9'}): return false
  true

proc parsePreferredText(text: string): tuple[table: Table[string, string],
                                             badLine: int] =
  ## 解析编辑框文本；badLine>0 = 该行有问题，整体拒绝（和缩略词文件同一策略）
  result.table = initTable[string, string]()
  var i = 0
  for raw in strutils.splitLines(text):
    inc i
    let line = strutils.strip(raw)
    if line.len == 0 or line.startsWith("#"): continue
    let eq = line.find('=')
    if eq <= 0:
      result.badLine = i
      return
    let zh = strutils.strip(line[0 ..< eq])
    var en = strutils.strip(line[eq + 1 ..^ 1])
    if zh.len == 0 or en.len == 0:
      result.badLine = i
      return
    # 键必须含中文且 ≤8 字（纯英文键在拆词时走不到中文词表，永远匹配不上）
    var hasCjk = false
    for r in toRunes(zh):
      if r.int >= 0x2E80: hasCjk = true
    if not hasCjk or toRunes(zh).len > 8:
      result.badLine = i
      return
    # 内置词条只允许动「词库」映射；普通映射（配置→configuration）不在本页范围
    if g_zh.hasKey(zh):
      let first = strutils.split(g_zh[zh], ' ')[0]
      if first notin g_preferred:
        result.badLine = i
        return
    if en != "-":
      var words: seq[string] = @[]
      for w in strutils.split(en, ' '):
        if w.len > 0: words.add(w)
      if words.len == 0 or words.len > 4:
        result.badLine = i
        return
      for w in words:
        if not isPreferredWord(w):
          result.badLine = i
          return
      en = words.join(" ")
    result.table[zh] = en

proc applyCustomPreferred(t: Table[string, string]) =
  ## 把解析结果套进运行时表（加载/保存共用）
  g_customZh = initTable[string, string]()
  g_zhOff = initHashSet[string]()
  g_preferredCustom = initHashSet[string]()
  for zh, en in t:
    if en == "-":
      g_zhOff.incl(zh)
    else:
      g_customZh[zh] = en
      for w in strutils.split(en, ' '):
        if w.len > 0: g_preferredCustom.incl(w)

proc loadCustomPreferred*(path: string): int =
  ## 启动时加载；文件不存在/为空 = 无自定义。返回自定义条数（含停用）
  g_customZh = initTable[string, string]()
  g_zhOff = initHashSet[string]()
  g_preferredCustom = initHashSet[string]()
  if fileExists(path):
    let r = parsePreferredText(readFile(path))
    if r.badLine == 0: applyCustomPreferred(r.table)
  g_customZh.len + g_zhOff.len

proc saveCustomPreferred*(path, text: string): tuple[count, badLine: int] =
  ## 校验 → 写文件 → 立即生效；badLine>0 时拒绝保存（返回出错行号）
  let r = parsePreferredText(text)
  if r.badLine > 0: return (0, r.badLine)
  writeFile(path, text)
  applyCustomPreferred(r.table)
  (g_customZh.len + g_zhOff.len, 0)

proc customPreferredText*(path: string): string =
  ## 给设置页编辑框回填当前文件内容
  if fileExists(path): readFile(path) else: ""

proc customPreferredEntries*(): seq[(string, string)] =
  ## 全部自定义条目（含 "-" = 停用），按中文排序
  result = @[]
  for k, v in g_customZh: result.add((k, v))
  for k in g_zhOff: result.add((k, "-"))
  result.sort(proc (a, b: (string, string)): int = cmp(a[0], b[0]))

proc customPreferredPairs*(): seq[(string, string)] =
  ## 生效的自定义映射（不含停用项），按中文排序
  for e in customPreferredEntries():
    if e[1] != "-": result.add(e)

proc builtinPreferredPairs*(): seq[(string, string)] =
  ## 内置词库映射（值的第一个词属于常用词库），按中文排序；「常用词」页的内置行
  result = @[]
  for zh, en in g_zh:
    let first = strutils.split(en, ' ')[0]
    if first in g_preferred: result.add((zh, en))
  result.sort(proc (a, b: (string, string)): int = cmp(a[0], b[0]))

proc builtinZhKind*(zh: string): int =
  ## 0=不在内置词表 1=内置词库映射（本页可改可删）2=内置普通映射（本页不管）
  if not g_zh.hasKey(zh): return 0
  let first = strutils.split(g_zh[zh], ' ')[0]
  if first in g_preferred: return 1
  2

proc preferredMerge*(): seq[(string, string)] =
  ## 内置 + 自定义合并：自定义覆盖内置，"-" = 停用（不出行），按中文排序
  var t = initTable[string, string]()
  for (zh, en) in builtinPreferredPairs(): t[zh] = en
  for (zh, en) in customPreferredEntries():
    if en == "-": t.del(zh)
    else: t[zh] = en
  result = @[]
  for k, v in t: result.add((k, v))
  result.sort(proc (x, y: (string, string)): int = cmp(x[0], y[0]))

# ---------------- 拆词 ----------------

proc hasNonAscii(s: string): bool =
  for c in s:
    if ord(c) >= 0x80: return true
  false

proc splitCamel(tok: string): seq[string] =
  ## 驼峰/数字边界切词：parseXMLDocument -> parse xml document
  result = @[]
  var start = 0
  for i in 1 ..< tok.len:
    let c = tok[i]
    let prev = tok[i - 1]
    let boundary =
      (c in 'A'..'Z' and prev in 'a'..'z') or
      (c in 'A'..'Z' and prev in 'A'..'Z' and i + 1 < tok.len and
        tok[i + 1] in 'a'..'z') or
      (c in 'a'..'z' and prev in '0'..'9')
    if boundary:
      result.add(tok[start ..< i].toLowerAscii)
      start = i
  if start < tok.len:
    result.add(tok[start ..^ 1].toLowerAscii)

proc splitAsciiWords(s: string): seq[string] =
  ## 标点→空格（撇号并入：user's -> users），再按驼峰边界拆
  var cleaned = ""
  for c in s:
    if c in 'a'..'z' or c in 'A'..'Z' or c in '0'..'9':
      cleaned.add(c)
    elif c == '\'':
      discard
    else:
      cleaned.add(' ')
  result = @[]
  for tok in strutils.split(cleaned):
    result.add(splitCamel(tok))

proc splitWords*(phrase: string): seq[string] =
  ## 拆词入口：中文按词表正向最长匹配（最长 6 字），英文按空格/标点/驼峰
  ## 未收录的中文字以单字混进结果，由 applyReduction 丢弃
  result = @[]
  let s = strutils.strip(phrase)
  if s.len == 0: return
  if not hasNonAscii(s):
    return splitAsciiWords(s)

  let runes = toRunes(s)
  var asciiBuf = ""
  var i = 0
  while i < runes.len:
    let r = runes[i]
    if r.toUTF8().len >= 3:   # 3 字节 UTF-8 ≈ 中日韩字符与全角标点
      if asciiBuf.len > 0:
        result.add(splitAsciiWords(asciiBuf))
        asciiBuf = ""
      var matched = false
      for l in countdown(8, 1):   # 内置最长 6 字，自定义词条可到 8 字
        if i + l <= runes.len:
          var term = ""
          for j in i ..< i + l:
            term.add(runes[j].toUTF8())
          # 自定义词库映射优先；被停用（中文=-）的内置映射当作没这条，继续试更短的
          var en = ""
          if g_customZh.hasKey(term):
            en = g_customZh[term]
          elif g_zh.hasKey(term) and term notin g_zhOff:
            en = g_zh[term]
          if en.len > 0:
            for w in strutils.split(en, ' '):
              if w.len > 0: result.add(w)
            i += l
            matched = true
            break
      if not matched:
        result.add(r.toUTF8())   # 未收录单字，后面按泛词丢
        inc i
    else:
      asciiBuf.add(r.toUTF8()[0])
      inc i
  if asciiBuf.len > 0:
    result.add(splitAsciiWords(asciiBuf))

# ---------------- 缩减 ----------------

proc lookupAbbrev(w: string): string =
  ## 缩写映射（自定义优先于内置），复数自动带 s：messages -> msgs
  ## 自定义值 "-" = 明确不缩写，原词返回；常用词库的词不参与缩写（调用方保证）
  if g_active.hasKey(w):
    result = g_active[w]
    if result == "-": result = w
    return
  if w.len > 3 and w.endsWith("s"):
    let base = w[0 ..< ^1]
    if base in g_preferred or base in g_preferredCustom:
      return w   # 常用词（含自定义）的复数不缩写：buttons -> buttons
    if g_active.hasKey(base):
      result = g_active[base]
      if result == "-": return w
      return result & "s"
    if base.endsWith("es") and g_active.hasKey(base[0 ..< ^2]):
      result = g_active[base[0 ..< ^2]]
      if result == "-": return w
      return result & "s"
  return w

proc applyReduction*(words: seq[string]): seq[string] =
  ## 删泛词/忽略词 + 缩写映射 + 相邻去重；未收录中文字在此丢弃。
  ## 优先级：自定义忽略 > 自定义缩写 > 常用词库（原样保留）> 内置泛词/内置缩写。
  ## 常用词库的词不删不缩——用户指定的命名首选词（button 不缩成 btn）；
  ## 想缩写某个库内词，在缩略页给该词显式加一行自定义缩写即可。
  result = @[]
  for w in words:
    if w.len == 0: continue
    if hasNonAscii(w): continue
    if w in g_customIgnore: continue
    if g_custom.hasKey(w):
      # 用户显式定义的缩写优先于常用词库
      let a = lookupAbbrev(w)
      if result.len > 0 and result[^1] == a: continue
      result.add(a)
      continue
    if w in g_preferred or w in g_preferredCustom:
      if result.len > 0 and result[^1] == w: continue
      result.add(w)
      continue
    if w in g_stop and w notin g_customKeep: continue
    let a = lookupAbbrev(w)
    if result.len > 0 and result[^1] == a: continue
    result.add(a)

proc wordPriority(w: string): int =
  ## 裁剪优先级：动词 2 > 普通词 1；超预算时优先保动词（含用户常用动词表）
  if w in g_verbs or w in g_prefVerbs: return 2
  if w.len > 3 and (w.endsWith("ize") or w.endsWith("ise") or
      w.endsWith("ate") or w.endsWith("fy")): return 2
  1

proc formatIdent*(words: seq[string], style: NamingStyle): string =
  ## 按风格拼接；words 应为全小写
  if words.len == 0: return ""
  case style
  of nsCamel:
    result = words[0]
    for i in 1 ..< words.len:
      let w = words[i]
      if w.len > 0:
        result.add(w[0].toUpperAscii & w.substr(1))
  of nsPascal:
    for w in words:
      if w.len > 0:
        result.add(w[0].toUpperAscii & w.substr(1))
  of nsSnake:
    result = words.join("_").toLowerAscii
  of nsScreamingSnake:
    result = words.join("_").toUpperAscii
  of nsKebab:
    result = words.join("-").toLowerAscii

proc trimToBudget*(words: seq[string], maxWords, maxChars: int,
                   style: NamingStyle): seq[string] =
  ## 预算裁剪：从尾部丢优先级最低的词，同级丢最靠后的
  ## （前面的词是主语尽量不动，动词优先级高天然靠后保留）
  ## 只剩一个词仍超长时保留原词，失败判定交给 reduceName，不做字符截断
  result = words
  let mw = max(maxWords, 1)
  while result.len > 1 and
        (result.len > mw or formatIdent(result, style).len > maxChars):
    var minPri = 2
    for w in result:
      let p = wordPriority(w)
      if p < minPri: minPri = p
    var dropAt = -1
    for i in countdown(result.len - 1, 0):
      if wordPriority(result[i]) == minPri:
        dropAt = i
        break
    if dropAt < 0: break
    result.delete(dropAt)

proc validateIdent*(name: string, reserved: HashSet[string]): bool =
  ## 校验：非空、首字符、字符集、不撞保留字（忽略大小写）
  ## kebab 风格（含 -）单独按小写字母数字连字符验
  if name.len == 0: return false
  if name.find('-') >= 0:
    if name[0] notin 'a'..'z': return false
    for c in name:
      if c notin 'a'..'z' and c notin '0'..'9' and c != '-': return false
  else:
    let c0 = name[0]
    if not (c0 in 'a'..'z' or c0 in 'A'..'Z' or c0 == '_'): return false
    for c in name:
      if not (c in 'a'..'z' or c in 'A'..'Z' or c in '0'..'9' or c == '_'):
        return false
  if name.toLowerAscii in reserved: return false
  true

# ---------------- 主入口 ----------------

proc reduceName*(phrase: string, style: NamingStyle, kind = nkLocal,
                 maxWords = 4, maxChars = 40): ReduceResult =
  ## 短语 → 合法标识符。kind 按对象套 C 命名惯例：
  ## 全局/静态/指针加前缀（g_/s_/p_ 或 g+s帕斯卡），宏恒 SCREAMING，
  ## 类型帕斯卡（snake 风格时补 _t）。任何失败都带原因返回，绝不静默截断
  let mw = max(maxWords, 1)
  let mc = max(maxChars, 2)
  # 对象 → 实际风格 + 前后缀；例：系统滴答 snake+全局 = g_system_tick，驼峰 = gSystemTick
  var eff = style
  var prefix = ""
  var suffix = ""
  case kind
  of nkLocal, nkFunc:
    discard
  of nkMacro:
    eff = nsScreamingSnake
  of nkType:
    eff = nsPascal
    if style == nsSnake: suffix = "_t"
  of nkGlobal, nkStatic, nkPointer:
    let p = case kind
            of nkGlobal: "g"
            of nkStatic: "s"
            else: "p"
    if style == nsSnake:
      prefix = p & "_"
    else:
      eff = nsPascal
      prefix = p
  # 预算扣掉前后缀，别让前缀把名字挤超长；预算过小时 trim 到底后由长度检查报失败
  let innerMax = mc - prefix.len - suffix.len
  let words = splitWords(phrase)
  if words.len == 0:
    return ReduceResult(ok: false, reason: "空输入")
  let reduced = applyReduction(words)
  if reduced.len == 0:
    return ReduceResult(ok: false, reason: "全部为泛词或未收录词")
  let trimmed = trimToBudget(reduced, mw, innerMax, eff)
  if trimmed.len == 0:
    return ReduceResult(ok: false, reason: "无法在约束内缩减")
  let name = prefix & formatIdent(trimmed, eff) & suffix
  if name.len > mc:
    return ReduceResult(ok: false, reason: "超出预算（最短仍超 maxChars）")
  if validateIdent(name, reservedSet):
    return ReduceResult(ok: true, name: name)
  # 保留字冲突：加下划线后缀再验（class -> class_），仍不行才报失败
  let fallback = name & "_"
  if validateIdent(fallback, reservedSet):
    return ReduceResult(ok: true, name: fallback)
  ReduceResult(ok: false, reason: "保留字冲突且无法加后缀")
