# AGENTS.md — 快译NG 开发交接文档（给 AI agent）

> 给接手本项目的 AI：先通读 §4 红线，再动代码。这里每条教训都对应一次真实事故。

## 0. 项目一句话

快译NG：Windows 单文件翻译小工具（约 1.1MB、无 DLL 依赖）。
C 核心（WinINet + 必应网页接口 + 百度/AI API）+ Nim/wNim 原生界面 + 纯本地命名缩减。
仓库：https://github.com/TimKuitu/kuaiyi-ng（main 分支，MIT）。**项目目录即 git 仓库**。
当前版本 v1.4.0（已发布）。面向人的开发文档见 `说明.md`，本文件是给 agent 的。

## 1. 工具链与构建（路径必须精确）

- **Nim 1.6.20**：`D:\ProgramFiles\nim-1.6.20` —— 勿升级，Nim 2.x 与 wNim 不兼容
- gcc/windres：`C:\msys64\mingw64\bin`（gcc 16，"不兼容指针类型"当硬错误，参数里已豁免）
- wNim / winim / memlib：`D:\ProgramFiles\{wNim,winim,memlib}`（wNim 是本地补丁版，勿覆盖）
- NSIS：`D:\ProgramFiles\nsis-3.10\makensis.exe`

构建（双击 `build.bat` 即可；在 Git Bash 里**别调 bat**——中文注释会被乱码误判成命令，
直接跑等价命令）：

```bash
cd /d/Lenovo/Documents/001_Code/4_AIproject/快译NG
export PATH="/c/msys64/mingw64/bin:/d/ProgramFiles/nim-1.6.20/bin:$PATH"
nim c --hints:off --warnings:off --app:gui --opt:speed -d:release -d:mingw \
  --path:"D:/ProgramFiles/wNim" --path:"D:/ProgramFiles/winim" --path:"D:/ProgramFiles/memlib" \
  --passC:"-Wno-error=incompatible-pointer-types" \
  --passL:"res/app.res" --passL:"-lwininet" \
  -o:"D:/kuaityNG_build.exe" ui/main.nim
cp /d/kuaityNG_build.exe "快译NG.exe" && rm /d/kuaityNG_build.exe
```

- **`-d:release` 不可去掉**（§4 红线 1）
- 链接器**不能输出中文路径** → 先输出到 `D:\` 再拷回；链接前确认 `res/app.res` 存在，
  没有就 `windres app.rc -O coff -o res/app.res`
- 构建验证：`./快译NG.exe -testtranslate` 会自动翻译一段固定文本、写 `fastT\test.log` 后退出，
  exit=0 且 test.log 有 `TEST-RESULT: OK` 即通过（若用户开着程序会短路，见 §7）

## 2. 代码地图

```
core\   C 核心（UTF-8 源码；数据文件一律 GBK）
  bing.c    必应网页接口逆向（IG/IID/token 会话）+ 必应词典 + fy_translate_chain 回退链
  baidu.c   百度通用翻译 API（MD5 签名）
  ai.c      OpenAI 格式接口（DeepSeek/OpenAI/Moonshot/Ollama 预设）
  http.c    WinINet 封装（GET/POST/POST_JSON/download，含日志埋点）
  json.c    极简 JSON 路径取值器（fy_json_str("a[0].b")）
  cfg.c     GBK INI 配置（未知段/键原样保留——加新键不用迁移）
  hist.c    查询历史（去重、上限 200）；fmt.c 旧的单词格式化（原样/全小写还在用）
  log.c     运行日志（见 §5）；md5.c 签名用；update.c 检查更新
  test_*.c  C 层自测程序（见 §6）
ui\
  main.nim            全部界面与流程（主窗、设置窗、热键、托盘、worker 线程、布局）
  fastcore.nim        C 绑定：{.compile} 直接编译 core\*.c，importc 声明，改 C 函数签名要同步
  naming_reducer.nim  命名缩减模块（纯 std 库）：中文/英文短语 → 合法标识符；
                      自定义缩略/忽略表也从这里加载（设置页最后两页编辑）
  test_naming_reducer.nim  其单测（23 组，全过为绿）
fastT\        配置与数据（已 gitignore）；config 是 GBK 编码 INI；
              custom_abbr.txt / custom_ignore.txt 是 UTF-8 纯文本（设置页维护，可手改）
res\          图标（genicon.ps1 + make_ico.py 可再生成）
installer.nsi NSIS 安装包脚本（!define VERSION 在第 9 行）
```

## 3. 核心数据流

1. 输入 → 翻译：`doTranslate` 组装 `TranslateJob`（C 堆字符串）→ `CreateThread` 裸线程跑
   `fy_translate_chain`（C 内完成 AI→百度→必应回退）→ `PostMessageW(g_hwndMain, WM_APP+0x200, res)`
   → UI 线程 `onTranslated`（显示 + 写历史）。
2. 复制：热键/Ctrl+双击 → `copyWord(true)` → 驼峰/下划线/大写/中划线走
   `naming_reducer.reduceName`（原样/全小写走旧 `fy_format_word`）→ 剪贴板；失败标题栏提示原因。
3. 配置：`fastT\config`（GBK）。段：`[通用设置]` `[AI翻译]` `[百度翻译]` `[快捷按键]` `[复制单词]`。
   复制模式下拉**选中即写配置**（不经过保存按钮）；预算键 `最大词数=4 / 最大字符数=40`。
   用户自定义缩略/忽略另有纯文本文件（UTF-8）：`fastT\custom_abbr.txt`（每行 `原词=缩写`，
   `原词=-` 表示禁用内置缩写）、`fastT\custom_ignore.txt`（每行一个词，`+词` 表示保留内置泛词）。
   启动时 `loadCustomAbbreviations/loadCustomIgnore` 读入。设置页「缩略词」「忽略词」两页显示的是
   **内置 + 自定义合并后的生效列表**（内置行也能改、能删），保存时只把与内置不同的条目写回文件。
   优先级：忽略 > 自定义缩写 > 内置泛词；保留表（+词）的内置泛词不删。
4. 日志：`fastT\run.log`（UTF-8），每次启动自动把上次内容轮换为 `run.last.log`；
   未处理异常过滤器把 `[CRASH] 异常码/地址/线程/读写` 追加进同一份日志。
   埋点：HTTP 层、翻译链每步、错误统一通道（bing.c 的 set_err）、UI 侧启动/翻译/退出。
   用户报问题时先要这两个文件。

## 4. 红线与血泪教训（逐条真实事故）

1. **`-d:release` 必须带**。缺了它 Nim 调试版开栈追踪，每个过程读写全局（非 TLS）
   `framePtr`，翻译裸线程与 UI 线程竞态写坏它 → 随机 0xC0000005，崩点漂移
   （wWndProc / messageLoop / newObj 都可能是第一现场）。v1.1 的闪退根因，修了整整一轮。
2. **裸线程里不碰 Nim GC**。`translateWorker` 只用 C 堆（fy_strdup/fy_free），
   窗口句柄用 `g_hwndMain` 值拷贝，不调 frame 的方法。新增后台线程必须照此办理。
3. **百度签名**：`sign = MD5(appid + q原文 + salt + 密钥)`，q 传输**只 URL 编码一次**。
   多编一次必报 54001（v1.1.2 修过）。q 上限 6000 字节。
   测百度接口的 mock 必须按服务端算法**验签**——只测"返回 200"测不出协议错。
4. **自定义消息 ID**：wNim 内部占用 WM_APP+0x10..0xE8，自定义必须避开（现用 +0x200/+0x201，
   曾因撞车随机崩溃）。
5. **ComboBox 可视高度**：setSize 设的是"含下拉列表的总高"，收起态由 item 高度决定，
   要发 `CB_SETITEMHEIGHT`（wParam=0xFFFFFFFF）+ SetWindowPos(SWP_FRAMECHANGED) 校准
   （见 `fitComboVisibleHeight`）。
6. **exe 放哪读哪的 config**：放 `D:\` 根目录会读到近乎空的 `D:\fastT\config`，
   实验结果会完全不同——测试一律在项目目录跑。
7. **wNim/winim API 差异**：strutils+unicode 同引时 `split/strip` 要限定模块名；
   这版 Nim 的 `seq.delete` 是单参数；`nim c … | grep -ciE error` 零匹配时退出码 1
   会断掉 `&&` 链（cp 没执行还以为构建成功）——用 `> /dev/null 2>&1; echo exit=$?`。

## 5. 运行日志（排障入口）

- `fastT\run.log` + `fastT\run.last.log`：闪退/异常时让用户把两个文件发来，时间线完整。
- 崩溃过滤器无法覆盖 fast-fail（栈溢出等），那种情况可能没有 [CRASH] 行——结合最后一条
  埋点推断现场。
- 用户报"闪退"先查这两份，再看 §4-1/2。

## 6. 测试与验证

```bash
# C 层（示例：test_log；test_core 需要参数；test_chain 压回退链；test_baidu 直调百度）
gcc -municode -O1 -Icore $(ls core/*.c | grep -v test_) core/test_log.c -o /d/test_log.exe -lwininet
# Nim 单测
nim c --hints:off --warnings:off -d:release -o:"D:/test_naming_reducer.exe" ui/test_naming_reducer.nim
```

- mock 百度：本地 HTTP 服务**按官方算法验签**（`md5(appid+q解码一次+salt+密钥)`，不符回 54001），
  客户端配 `[百度翻译] 地址=http://127.0.0.1:8766/...` 指过去；mock 脚本别放 /tmp（会丢），
  放 `C:\Users\<user>\AppData\Local\Temp` 用 Windows 路径。
- 布局/像素验证技巧：main() 里临时加 `fy_log_write("布局", "…getSize()…")`，跑一次
  -testtranslate 读 run.log，验证完删掉重编。
- 设置页「缩略词」「忽略词」是**按行动态生成**的（行控件懒创建、只隐藏不销毁，槽上限 240 行；
  内置约 60 缩写 + 65 泛词，其余留给自定义）：列表 = 内置 + 自定义合并，按词排序。
  `快译NG.exe -testtab=3` / `-testtab=4` 启动后自动打开设置并切到对应页，配合截图核对
  行距/列宽。这些控件的坐标一律用 `dpx()`（逻辑→物理像素）换算，别在 dpiAutoScale 块外写裸数值。
  两页列表支持**滚轮**（一格 3 行）：wNim 不派发滚轮事件、WM_MOUSEWHEEL 又会先落到光标下的
  Edit/Button 被吃掉，所以给页面本体、每个行控件、说明文字/状态栏/按钮/滚动条都挂了一层
  comctl32 `SetWindowSubclass`（`WheelSubId=0x4B59`，`dwRefData` 存 `WheelTagAbbr/WheelTagIgnore`）。
  **坑**：`dpiAutoScale` 宏会把块内**所有整数字面量**乘上 g_dpiScale，所以
  `hookWheel(pageAbbr, 1)` 在 150% 下 tag 变成 2（曾导致滚页面空白处串到另一页）；
  非布局参数一律传常量名，别传字面量。
  自动化点按钮/填框：`SendMessage(BM_CLICK)` + 给编辑框发 `WM_SETTEXT`（**ANSI 指针**，
  这套 wNim/winim 是 ANSI 编译，宽字符指针只会存进首字符）；跨进程 `GetWindowText` 读不到
  Edit 内容，用 UI Automation 的 `Name` 属性读。
- 用户报 UI 问题 = 他拿像素尺在 **150% DPI 屏**量**物理像素**。给 "NNpx" 指物理值。
- **界面缩放约定（方案 A）**：界面**不跟系统 DPI 走**，`g_uiScale` 只来自设置页「字体缩放」
  （75/100/110/125/150/175/200，默认 100%），整窗等比缩放。所有尺寸走 `dpx()`/`S()`，
  字体走 `makeUiFont()`（18px×scale 的 YaHei UI）+ `applyUiFont()`（递归 WM_SETFONT）；
  输入/结果框是 `RICHEDIT50W`，还得额外 `EM_SETCHARFORMAT`（`richFont()`，yHeight = 像素×15 twips），
  否则字号不跟着变。别再引入 `dpiAutoScale` 宏——它按窗口 DPI 乘字面量，会把"固定像素"方案搞坏。
  **坑**：`startTimer(seconds, id)` 第一个参数是**秒**不是毫秒（写 200 等于 200 秒）。
- **设置窗口固定尺寸**：建窗时 `style=wCaption or wSystemMenu or wMinimizeBox`（**不给
  `wResizeBorder`/`wMaximizeBox`**），并 `setFrame.minSize = setFrame.maxSize = (dw, dh)`——
  双保险，连 `SetWindowPos`/`SC_MAXIMIZE`/`SW_MAXIMIZE` 都会被 WM_GETMINMAXINFO 夹回原尺寸
  （实测三路都无效）。切档位时先更新 min/max 再设 size，否则会被旧限制夹住。
  内容不跟随用户（已无法拖拽），**只有「字体缩放」档位变化时**整套按新档位重排：`refreshSettings(g_uiScale)` →
  `relayoutSettings()` 用 `g_setDesign`（控件设计坐标表，`recordSettingsDesign()` 用
  EnumChildWindows + ScreenToClient 采集，页面窗口要跳过）按 `新档位/当前档位` 比例 SetWindowPos，
  并把 `g_setFont = makeFontAt(18×系数)` 套到整棵子树（旧字体 DeleteObject）；行控件再按同一系数
  重排（临时置 `g_uiScale = 系数` 后跑 `loadAbbrRows/loadIgnoreRows`）。
  **不要给 `wEvent_Size` 挂缩放**——用户明确要求"拖窗口时按钮大小不变"。
  两个窗口的 minSize 是固定像素（280×200 / 400×300），别写成 `×g_uiScale`，
  否则档位切换时被旧 minSize 夹住、缩放不准。
- **设置页留白网格（别随手改数字）**：`PadL = 24`（左右内边距）、`ContW = 496`（内容宽 = 544-48）、
  `PageH = 496`（页面内容高）；notebook 尺寸 = `544 × (PageH+38)`（38 = 页签条），窗口默认 =
  `544×档位+38` / `558×档位+72`（38/72 是不随缩放的边框，按 150% DPI 实测）。
  组标题用 `"▍" & 标题` 的 StaticText，组间用 `style=0x10`（SS_ETCHEDHORZ）的 2px 横线分隔；
  复选框行距 33、区块内其它行距 34、底部操作条固定在 `PageH-48` 基线。
- **缩略词/忽略词页的提示区**：两条提示各 `496×50`（y=10/60，能显示两行），列表行从 `AbbrTop = 116` 开始，10 行到 428，底部按钮在 448。改提示高度时 `AbbrTop` 必须跟着改。
  窗口大小行下面**不放提示文字**，「拖动调整」按钮整个模块已按用户要求删除
  （连同 g_sizeMode、定时器 91、标题提示），现在只能直接填宽高 + 恢复默认。
  去掉 `wResizeBorder` 后非客户区从 22/56 变成 **6/40**（150% DPI 实测），
  窗口尺寸公式随之是 `544×档位+22` / `534×档位+56`。
  改完必须跑一遍"控件矩形两两求交"的检查（脚本在 docs 里），确认无重叠且最低点 ≤ 页面高度。

- **切档位时两个窗口一起缩**：`saveSettings()` 里按 `ratio = 新档位/旧档位` 缩放；若当前尺寸就是
  默认尺寸（`nearSize` 判断）直接用新档位默认值，否则乘 ratio（避免"最小值夹住后再放大"跑飞）。
  设置窗口同样处理，然后 `resetSettingsBase()` + `refreshSettings()` 立刻重排，不用重启。
- 主窗口尺寸可保存：`窗口宽/窗口高`（像素，只在设置页「保存设置」时写），启动时直接 `frame.size`，
  但由 `computeDefWinSize()` 兜底（字体放大后装不下就自动撑高/加宽）；
  输入/结果框在 `layoutMain` 里平分多余高度。

## 7. 与用户协作的约定

- 用户经常**开着快译NG**：`快译NG.exe` 被锁时不要 taskkill 他的实例，
  挂后台轮询（每 2s `cp` 新 exe 过去，约 9 分钟上限），他一退出自动完成替换——两次实测都好使。
  替换后跑 -testtranslate 验证。
- 注释风格：像工程师开发日志，短句、要点式、少 AI 味；关键约束（线程安全、编码、上限）必须注释。
- **每完成一次改动就提交一次 git**（`git add` → commit → `git push` 到 main），不要攒着等；
  提交信息要说清这次改了什么。发版（改 `AppVersion` / `installer.nsi` 版本号、打 tag、
  建 Release、打安装包）**只在用户明确说「发布 release」时才做**，平时不要动版本号。
- 项目 `fastT\config` 的 `[AI翻译]启用=1` 是用户自己开的（密钥为空不影响运行），勿动勿提交。
- 用户实际使用的安装副本在 `D:\Lenovo\Desktop\ToolBox\快译\快译NG`，与仓库开发副本互不干扰。

## 8. 发布流程（版本 vX.Y.Z）

1. 改版本号：`ui/main.nim` 的 `AppVersion` + `installer.nsi` 的 `!define VERSION`（共两处）。
2. 构建 → `-testtranslate` 验证 → `makensis.exe installer.nsi` 重打安装包。
3. `git add -A && git commit && git push`（用户要求发版即含推送授权）。
4. 取 PAT：`printf "protocol=https\nhost=github.com\n" | git credential fill`（取 password 行，
   不要回显）。api.github.com 直连被墙：`curl --resolve api.github.com:443:140.82.112.5`；
   uploads.github.com：`--resolve uploads.github.com:443:20.205.243.161`
   （IP 是当时解析的，失效就重新查 A 记录）。
5. 建 Release：中文 JSON 必须写 UTF-8 **文件**再 `--data-binary @`（bash 内联会坏成 400）；
   curl 的文件路径用正斜杠（`@C:/x/$f`——反斜杠在双引号里会把 `\$f` 转成字面量）。
6. 资产名**固定格式**：`KuaiYiNG_vX.Y.Z_Setup.exe` / `KuaiYiNG_vX.Y.Z_Portable.exe`
   （update.c 按 tag 拼下载地址）。tag 形如 `vX.Y.Z`，`version_newer` 支持三段比较。
7. 验证：GET /releases/latest 看 tag 与资产 state=uploaded；本地跑 fy_check_update
   确认老版本能收到提示。

## 9. 当前状态与已知边界（2026-10-26）

- v1.4.0 已发布：设置页整体重排（三组 + 分割线 + 24px 留白、行距 33），新增「字体缩放」75~200% 整窗等比缩放、「窗口大小」数值框 + 恢复默认，设置窗口改固定尺寸（无 WS_SIZEBOX/WS_MAXIMIZEBOX，min=max=内容尺寸），删除「拖动调整」模块与窗口大小行提示文字、删除 25%/50% 档；界面从 `dpiAutoScale`（跟系统 DPI）改成像素固定 + 用户自选档位。
- v1.3.0：设置窗口最后两页「缩略词」「忽略词」把**内置表也列出来**，
  可改可删（缩略页删行写 `原词=-`，忽略页删行写 `+词` 保留），保存只写与内置不同的条目；
  两页列表支持滚轮（一格 3 行，页面空白处、行内输入框、按钮上都生效）。
  `ui/main.nim` 第 206-500 行是这两页（滚轮在 257-300、544-560 行），
  `ui/naming_reducer.nim` 增加禁用/保留语义与只读访问器，
  `ui/test_naming_reducer.nim` 增至 23 组用例。
- 已知限制 / 可选改进方向：
  - `copySource` 取结果区首行，带"（…已用必应翻译）"前缀时缩减模块会拿到前缀文本（可过滤）；
  - naming_reducer 的中文词表（约 200 条）可继续扩充；"读取/状态"这类核心词按设计保留，
    用户若要更激进的削减需调 `wordPriority`；
  - 百度单次请求上限 6000 字节，超长文本报错属预期；
  - 自定义词条只支持英文单词/数字（`isPlainWord`）：引擎按拆好的英文词查表，短语和中文
    永远匹配不到，所以在保存时直接拦下并提示；想映射中文词先用缩略页把中文对应英文写上。
  - 两个设置页的行控件是懒创建 + 隐藏复用（运行期销毁 wNim 控件不安全），行槽上限 240；
    合并后（内置+自定义）超过 240 条时只显示前 240 条并禁止保存（避免静默截断）。
