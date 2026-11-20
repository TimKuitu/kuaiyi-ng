# 快译NG 纯 Rust 重构迁移地图（方案 B）

- **目标**：将快译NG（C 核心 + Nim/wNim 界面）完整迁移为纯 Rust 原生 Windows 应用。
- **分支**：`rust-rewrite`
- **工作区**：`D:\Lenovo\Documents\01_Code\04_AIproject\快译NG-rust`

## 阶段规划

### Phase 1：`naming_reducer`（中英文短语缩减与命名引擎）
| 单元 | 源文件参考 | Rust 目标文件 | 状态 | 验证方式 |
| --- | --- | --- | --- | --- |
| `naming_reducer` | `ui/naming_reducer.nim` | `src/naming_reducer.rs` | `done` | `cargo test`（54 组用例对照 `ui/test_naming_reducer.nim`）+ `cargo clippy` |

### Phase 2：`core` 核心层（4 波次逐步迁移）
| 波次 | 单元 | 源文件参考 | Rust 目标文件 | 状态 |
| --- | --- | --- | --- | --- |
| Wave 1 | 纯算法与文本工具 | `core/md5.c`, `core/str.c`, `core/json.c`, `core/fmt.c` | 待定 | `open` |
| Wave 2 | 本地配置、历史与日志 | `core/cfg.c`, `core/hist.c`, `core/log.c` | 待定 | `open` |
| Wave 3 | HTTP 与翻译回退链/更新 | `core/http.c`, `core/baidu.c`, `core/ai.c`, `core/bing.c`, `core/update.c` | 待定 | `open` |
| Wave 4 | 截屏 OCR 与原图贴字 | `core/ocr.c` | 待定 | `open` |

### Phase 3：`ui` 界面层（纯 Rust 原生 Windows GUI）
| 单元 | 源文件参考 | Rust 目标文件 | 状态 |
| --- | --- | --- | --- |
| `ui` | `ui/main.nim` | 待定（主窗、设置窗口、全局热键、托盘、工作线程） | `open` |

## 工具链与构建说明

- **`.cargo/config.toml`**：通过 `[build]` 指定 `target = "x86_64-pc-windows-gnu"`（沿用仓库原有的 MinGW-w64 链接器与已安装的 `rustfmt`/`clippy`），并将 `target-dir` 指向纯 ASCII 路径 `C:/Users/Lenovo/AppData/Local/Temp/kuaiyi-ng-target`，规避 MinGW `ld.exe` 无法解析含中文路径（`快译NG-rust`）下目标文件的问题，使 `cargo test`、`cargo clippy` 与 `cargo build` 直接开箱即用。
