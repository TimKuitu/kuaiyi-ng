# HANDOFF · 快译NG（纯 Rust 版）

> 交接对象：接手本项目的新工作区 / 新 Agent 会话  
> 编写时间：2026-11-22  
> 说明：本文聚焦**现行项目结构、模块职责、构建验证与避坑红线**，方便直接接手纯 Rust 主线开发。

---

## 0. 三十秒速览

| 项目 | 说明 |
| :--- | :--- |
| 现行主线路径 | `D:\Lenovo\Documents\01_Code\04_AIproject\快译\快译NG-rust\` |
| 项目定位 | Windows 纯 Rust 轻量单文件桌面翻译、截屏 OCR 贴图与编程命名工具 |
| 当前版本与状态 | `v3.2.0`（已落地「四槽位语义词序重排与主流嵌入式动词缩写」及设置页实时预览，50 组单测全绿） |
| 最新可运行产物 | `D:\Lenovo\Documents\01_Code\04_AIproject\快译\快译NG-rust\快译NG.exe` |
| 一句话接手动作 | 通读第 1、2 节目录划分 → `cargo test --lib` → `cargo build --release` → 拷出并跑 `-testtranslate` |

**三层纯 Rust 架构（零外部 DLL 运行时依赖）：**

```text
① 核心层       src/core/            WinINet HTTP、AI/百度/必应翻译链、GBK INI 配置、历史/日志/OCR/更新
② 命名引擎层   src/naming_reducer.rs 语义分词、四槽位词序重排、嵌入式/工程缩写表、预算裁剪与保留字校验
③ 原生界面层   src/ui/ + src/main.rs Win32/GDI+/DWM 无边框主窗、7 类 12 页设置窗、虚拟滚动词表、截屏贴图
```

---

## 1. 上级目录关系（先看清再动，别改错目录）

```text
D:\Lenovo\Documents\01_Code\04_AIproject\快译\
├─ 快译NG-rust\          ← ★ 现行主线（纯 Rust v3.1.3，所有新功能与修复都在这里）
├─ 快译NG\               ← 旧版 C + Nim/wNim 仓库（v2.6.1，已归档）
├─ 快译NG-modern-ui\     ← 早期 Nim modern-ui 试验目录（已停更）
└─ 快译_backup\          ← 历代安装包备份（KuaiYiNG_v1.1.1 ~ v3.1.3_Setup.exe）
```

**外部构建目录（不在仓库内）：**
- `C:\Users\Lenovo\AppData\Local\Temp\kuaiyi-ng-target\`：由 `.cargo/config.toml` 指定为 `target-dir`，存放编译缓存、`app.res` 图标资源及 release 输出文件 `kuaiyi-ng.exe`。

---

## 2. 现行项目结构地图（`快译NG-rust\`）

### A. 活跃纯 Rust 源码与配置（日常开发改这里）

```text
快译NG-rust\
├─ Cargo.toml / Cargo.lock        项目清单（kuaiyi-ng v3.2.0，纯 std + Win32 FFI）
├─ .cargo\config.toml             指定 target=x86_64-pc-windows-gnu 与纯 ASCII target-dir
├─ app.rc / res\                  Windows 应用图标资源（kuaiyi.ico + 生成脚本）
├─ installer.nsi                  NSIS 安装包脚本（!define VERSION "3.2.0"）
├─ rust\MAP.md                    C+Nim → 纯 Rust 迁移对照记录
├─ HANDOFF.md                     本文档（当前项目结构与开发交接）
├─ README.md / 说明.md            项目对外说明文档
├─ 快译NG.exe                     当前最新可运行单文件产物
├─ 快译NG-安装包.exe              当前安装包产物
│
├─ src\
│  ├─ lib.rs                      库根模块（导出 core、naming_reducer、ui）
│  ├─ main.rs                     主入口：单实例、托盘、7 路全局热键、翻译/命名/更新/截屏消息调度
│  ├─ naming_reducer.rs           本地编程命名缩减引擎：
│  │                              · WordOrder（保持原序 / 名词+动词 / 按对象智能区分）
│  │                              · VerbForm（精炼分词 / 动词原形 / 嵌入式极简 RX/TX）
│  │                              · 常用词库、过去分词缩写表、防误倒装护栏、自定义词表加载
│  │
│  ├─ core\                       核心能力层（15 个文件）
│  │  ├─ mod.rs                   核心导出、翻译方向常量（FY_AUTO/ZH2EN/EN2ZH）、错误/引擎状态
│  │  ├─ ai.rs                    OpenAI 格式 AI 翻译（DeepSeek/OpenAI/Moonshot/Ollama 等）
│  │  ├─ baidu.rs                 百度通用翻译 API（MD5 签名与单次 URL 编码）
│  │  ├─ bing.rs                  必应网页翻译/词典与 AI→百度→必应 自动回退链
│  │  ├─ cfg.rs                   GBK INI 配置读写（自动兼容 UTF-8 BOM，未知段/键原样保留）
│  │  ├─ fmt.rs                   旧版空格词格式化工具（兼容保留）
│  │  ├─ hist.rs                  查询历史管理（去重、上限 200）
│  │  ├─ http.rs                  WinINet HTTP 封装（GET / POST / POST_JSON / 下载）
│  │  ├─ json.rs                  轻量 JSON 路径取值与转义
│  │  ├─ log.rs                   UTF-8 运行日志、启动轮换及未处理异常崩溃捕获
│  │  ├─ md5.rs                   MD5 算法（百度签名用）
│  │  ├─ ocr.rs                   截屏 OCR 识别、译文贴字颜色采样与缓存
│  │  ├─ str_util.rs              UTF-8 / GBK / Wide 宽字符转码与 URL 编码
│  │  ├─ update.rs                GitHub Release 检查更新与下载
│  │  └─ win32.rs                 core 层 Win32/WinINet FFI 声明
│  │
│  └─ ui\                         纯原生 Win32/GDI+ 界面层（7 个文件）
│     ├─ mod.rs                   AppSettings 配置模型、预设常量、热键编码、深色模式探测
│     ├─ modern_main.rs           无边框圆角主窗口、GDI+ 双缓冲、风格/对象悬浮下拉
│     ├─ modern_settings.rs       7 大导航 / 12 子页设置窗口（含 Page 4「命名规则」实时预览）
│     ├─ dict_list.rs             虚拟滚动词表组件（缩略词 / 忽略词 / 常用词三页共用）
│     ├─ snip_ui.rs               多屏截屏选区、标注画笔（马赛克/模糊）、贴图与原位 OCR 选字
│     ├─ snip_toast.rs            贴图右上角胶囊轻提示（淡入淡出）
│     └─ win32_ui.rs              UI 层 Win32/GDI+/DWM FFI 与内存图标兜底
│
└─ fastT\                         运行时本地数据（不入 Git）
   ├─ config                      GBK 编码主配置文件
   ├─ custom_abbr.txt             用户自定义缩略词（UTF-8）
   ├─ custom_ignore.txt           用户自定义忽略词（UTF-8）
   ├─ custom_preferred.txt        用户自定义常用词（UTF-8）
   ├─ history.txt / ocr_cache.txt 查询历史与 OCR 缓存
   └─ run.log / run.last.log / test.log  运行日志与 -testtranslate 验证日志
```

### B. 历史遗留参考目录（勿当作现行源码修改）

- `core\*.c`、`core\*.h`、`ui\*.nim`：迁移前 C + Nim 原始代码（仅作对照参考，不参与 Rust 编译）。
- `build.bat`、`AGENTS.md`：旧版 Nim/wNim 时代的构建脚本与交接文档（v1.4.0 口径）。
- `back\`：旧版构建/发版安装包副本。
- `docs\`：早期设置界面布局验证截图。

---

## 3. 核心改动入口对照表

| 想改什么 | 修改位置 | 验证方式 |
| :--- | :--- | :--- |
| 命名词序、动词缩写、中文/嵌入式词表 | `src/naming_reducer.rs` | `cargo test --lib naming_reducer` |
| 新增设置字段、预设常量、INI 读写 | `src/ui/mod.rs` | `cargo test --lib ui::tests` |
| 设置页 UI 布局、控件绑定、实时预览 | `src/ui/modern_settings.rs` | `cargo test --lib ui::modern_settings` + 实测打开设置窗 |
| 缩略词 / 忽略词 / 常用词列表逻辑 | `src/ui/dict_list.rs` | `cargo test --lib ui::dict_list` |
| 主窗口尺寸/按钮/下拉菜单/输入结果框 | `src/ui/modern_main.rs` | `cargo test --lib ui::modern_main` |
| 全局热键、托盘、翻译/命名复制调度 | `src/main.rs` | `cargo build --release` + 启动实测 |
| AI / 百度 / 必应翻译接口与回退链 | `src/core/{ai,baidu,bing}.rs` | `cargo test --lib core::` + `-testtranslate` |
| 截屏选区、贴图、模糊/马赛克、OCR | `src/ui/snip_ui.rs`、`src/core/ocr.rs` | `cargo test --lib snip_ui` + `F1`/`F2`/`F3` 实测 |

### 命名配置键与优先级速查
- `[复制单词]` 配置键：
  - `最大词数`（默认 `4`，范围 `1~16`）
  - `最大字符数`（默认 `40`，范围 `8~128`）
  - `词序模式`（默认 `1`：`0`=保持原序，`1`=名词+动词，`2`=按对象智能区分）
  - `动词词形`（默认 `0`：`0`=精炼分词如 `dataRcvd`，`1`=动词原形如 `dataRecv`，`2`=嵌入式极简如 `dataRx`）
- 规则优先级：`自定义忽略词 > 自定义缩略词 > 常用词库 > 内置规则`。

---

## 4. 构建、验证与发版流程

### 日常开发编译与验证（PowerShell）

```powershell
cd "D:\Lenovo\Documents\01_Code\04_AIproject\快译\快译NG-rust"

# 1. 全量单元测试与静态检查
cargo test --lib
cargo clippy --all-targets

# 2. 编译 Release 并覆盖本地可执行文件
cargo build --release
Copy-Item -Force "C:\Users\Lenovo\AppData\Local\Temp\kuaiyi-ng-target\x86_64-pc-windows-gnu\release\kuaiyi-ng.exe" ".\快译NG.exe"

# 3. 端到端翻译自测（exit=0 且 test.log 出现 TEST-RESULT: OK）
.\快译NG.exe -testtranslate
Get-Content ".\fastT\test.log" -Tail 5
```

- 若提示缺少 `app.res`，运行：
  `windres app.rc -O coff -o C:/Users/Lenovo/AppData/Local/Temp/kuaiyi-ng-target/app.res`

### 发版流程（仅在用户明确要求「发布 release」时执行）
1. 同步更新三处版本号：`Cargo.toml`（`version`）、`src/ui/mod.rs`（`APP_VERSION`）、`installer.nsi`（`!define VERSION`）。
2. 完成 Release 构建与 `-testtranslate` 验证后，用 NSIS 编译 `installer.nsi` 生成 `快译NG-安装包.exe`。
3. 按 `KuaiYiNG_vX.Y.Z_Setup.exe` 备份至 `..\快译_backup\`。

---

## 5. 踩坑清单与红线

| 类别 | 坑 / 红线 | 正确做法 |
| :--- | :--- | :--- |
| **Git worktree 路径** | `快译NG-rust\.git` 当前指向旧路径 `D:/Lenovo/Documents/01_Code/04_AIproject/快译NG/.git/worktrees/快译NG-rust` | 若需使用 Git，先到 `..\快译NG` 下执行 `git worktree repair ../快译NG-rust` 修复链接 |
| **误用旧构建脚本** | 根目录 `build.bat` 仍是旧版 Nim 脚本 | **绝对不要跑 `build.bat`**，统一用 `cargo build --release` |
| **中文路径链接失败** | MinGW `ld.exe` 无法解析含中文路径下的中间目标文件 | 保留 `.cargo/config.toml` 中纯 ASCII 的 `target-dir` |
| **UI `RefCell` 重入** | 在持有 `SET_UI` / `DICT_VIEWS` 借用时调用 `SetWindowText` / `MoveWindow` 会触发同步消息重入 panic | 先取出 `HWND` 释放 `RefCell` 借用，再调 Win32 API |
| **覆盖被占用的 exe** | 用户常开着 `快译NG.exe` 导致复制覆盖失败 | **禁止 `taskkill` 强杀用户实例**，提示用户关闭后再替换 |
| **敏感与运行数据** | `fastT\config` 含用户 API 密钥，日志与缓存体积大 | 不要把 `fastT\` 内容提交进仓库 |

---

## 6. 交接自检清单

```powershell
cd "D:\Lenovo\Documents\01_Code\04_AIproject\快译\快译NG-rust"
cargo test --lib          # 预期 50 passed, 0 failed
cargo clippy --all-targets
cargo build --release
```

- [ ] `快译NG.exe -testtranslate` 执行后 `fastT\test.log` 末尾为 `TEST-RESULT: OK`
- [ ] 打开主窗输入「接受的数据」双击命名，默认输出 `dataRcvd`
- [ ] 打开「设置 → 命名 → 命名规则」切换词序/动词风格，实时预览正常刷新且立即生效
- [ ] 截屏（`F1`）与贴图（`F3`）及原位翻译（`F2`）工作正常
