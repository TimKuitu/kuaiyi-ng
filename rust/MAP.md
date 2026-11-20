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
| Wave 1 | 纯算法与文本工具 | `core/md5.c`, `core/str.c`, `core/json.c`, `core/fmt.c` | `src/core/{md5,str_util,json,fmt}.rs` | `done` |
| Wave 2 | 本地配置、历史与日志 | `core/cfg.c`, `core/hist.c`, `core/log.c` | `src/core/{cfg,hist,log}.rs` | `done` |
| Wave 3 | HTTP 与翻译回退链/更新 | `core/http.c`, `core/baidu.c`, `core/ai.c`, `core/bing.c`, `core/update.c` | `src/core/{http,baidu,ai,bing,update}.rs` | `done` |
| Wave 4 | 截屏 OCR 与原图贴字 | `core/ocr.c` | `src/core/ocr.rs` | `done` |

### Phase 3：`ui` 界面层（纯 Rust 原生 Windows GUI + modern-ui）
| 单元 | 源文件参考 | Rust 目标文件 | 状态 |
| --- | --- | --- | --- |
| 资源编译 | `app.rc`, `res/kuaiyi.ico` | `.cargo/config.toml` (`app.res`) + `win32_ui.rs` 内存图标兜底 | `done` |
| Win32/GDI+/DWM FFI | — | `src/ui/win32_ui.rs` | `done` |
| UI 配置与热键 | `ui/main.nim` | `src/ui/mod.rs` | `done` |
| 现代无边框主窗口 | `ui/modern_main.nim` | `src/ui/modern_main.rs` | `done` |
| 虚拟滚动词表组件 | `ui/dict_list.nim` | `src/ui/dict_list.rs` | `done` |
| 现代设置窗口（12 页） | `ui/modern_settings.nim`, `ui/main.nim` | `src/ui/modern_settings.rs` | `done` |
| Snipaste 式截屏与贴图 | `ui/snip_ui.nim` | `src/ui/snip_ui.rs` | `done` |
| 主入口、托盘与调度 | `ui/main.nim` | `src/main.rs` | `done` |

## 工具链与构建说明

- **`.cargo/config.toml`**：通过 `[build]` 指定 `target = "x86_64-pc-windows-gnu"`（沿用仓库原有的 MinGW-w64 链接器与已安装的 `rustfmt`/`clippy`），并将 `target-dir` 指向纯 ASCII 路径 `C:/Users/Lenovo/AppData/Local/Temp/kuaiyi-ng-target`，规避 MinGW `ld.exe` 无法解析含中文路径（`快译NG-rust`）下目标文件的问题，使 `cargo test`、`cargo clippy` 与 `cargo build` 直接开箱即用。
- **PE 图标资源（`app.res`）**：不引入 `build.rs`（避免在无 MSVC Build Tools 的环境中编译构建脚本触发 `link.exe` 失败）。先使用 `windres` 将 `res/kuaiyi.ico` 编译生成 `C:/Users/Lenovo/AppData/Local/Temp/kuaiyi-ng-target/app.res`（`1 ICON "kuaiyi.ico"`），再由 `[target.x86_64-pc-windows-gnu]` 的 `rustflags` 链接进最终 EXE；同时 `src/ui/win32_ui.rs` 通过 `include_bytes!("../../res/kuaiyi.ico")` 保留运行时内存创建 `HICON` 兜底。
- **端到端验证**：支持 `./快译NG.exe -testtranslate`（自动翻译固定英文并写入 `fastT/test.log` 与 `fastT/run.log` 后以 0 退出码退出）。
