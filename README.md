# 快译NG (KuaiYiNG)

轻量级 Windows 中英互译工具——按一下热键，窗口出现在鼠标旁，查词、翻句、按驼峰命名复制，一气呵成。

**纯 C 核心 + Nim/wNim 原生控件界面**，单文件 2.4MB，**私有内存约 4MB**，
无浏览器内核、无运行时依赖，开箱即用。

## 功能

- **必应免费翻译**（无需 API Key）：句子走网页翻译接口，英文单词自动查必应词典（英/美音标 + 释义 + 网络释义 + 拼音）
- **AI 接口翻译（可选）**：支持一切 OpenAI 格式接口（DeepSeek / OpenAI / Moonshot / 本地 Ollama 等），
  设置页填地址、密钥、模型即可
- **百度翻译 API（可选）**：填开放平台的 AppID + 密钥即可（免费标准版）；支持自定义接口地址
- **三级回退链**：AI → 百度 → 必应免费，前面的未启用或失败自动切换到后面的（结果前会注明）
- **检查更新**：设置页一键检查 GitHub Releases 新版本，可直接下载并拉起安装包
- 三个按钮 + 回车即译：`中 > 英`、`英 > 中`、`翻译`（按内容自动判断方向）
- **全局热键**（可自定义）：
  - `Ctrl+Alt+F` 显示/隐藏窗口（可勾选"召唤时出现在鼠标旁"）
  - `Ctrl+Alt+↑/↓` 翻历史记录　`Ctrl+Alt+C` 复制单词
- **面向写代码的复制**：中文译英文后，热键/Ctrl+双击复制的是**英文译文**并按命名格式化——
  驼峰/下划线/中划线/大小写，格式化后自动去空格（`串口初始化程序` → `serialPortInitializationProgram`）
- 查询历史（去重、200 条）、托盘常驻、窗口置顶、开机自启动（可选）
- 设置界面：热键可视化录制、缓存目录可改、关闭行为（隐藏/退出）等
- 单实例保护 + 自愈：重复启动自动唤醒窗口；残留进程自动清理
- 每次启动恢复默认窗口尺寸，运行期间调整的大小在隐藏/召唤之间保留

## 下载安装

到 [Releases](../../releases) 下载：

| 文件                       | 说明                                                            |
| -------------------------- | --------------------------------------------------------------- |
| `KuaiYiNG_v*_Setup.exe`    | 安装包：可选目录、桌面/开始菜单快捷方式、开机自启动、自带卸载器 |
| `KuaiYiNG_v*_Portable.exe` | 便携版：单文件直接运行，数据存在同目录 `fastT\`                 |

## 从源码构建

依赖：

- [MSYS2](https://www.msys2.org/) MinGW-w64（gcc ≥ 13、windres）
- [Nim 1.6.20](https://nim-lang.org/install_windows.html)（**必须是 1.6.x**，2.x 与 wNim 不兼容）
- [wNim](https://github.com/khchen/wNim)、[winim](https://github.com/khchen/winim)、[memlib](https://github.com/khchen/memlib)
- [NSIS 3.x](https://nsis.sourceforge.io/)（仅打安装包需要）

```bat
git clone https://github.com/TimKuitu/kuaiyi-ng.git
cd kuaiyi-ng
:: 编译主程序（按 build.bat 内注释调整依赖路径后）
build.bat
:: 打安装包
makensis installer.nsi
```

## 架构

```
core\   纯 C 核心（gcc -O2 静态链入，无 DLL 依赖）
        bing.c   必应免费翻译 + 必应词典（IG/IID/token 会话管理）
        ai.c     OpenAI 格式 AI 翻译（DeepSeek/OpenAI/Moonshot/Ollama 等）
        baidu.c  百度翻译开放平台（MD5 签名）
        md5.c    MD5（RFC 1321）
        update.c 检查更新（GitHub Releases 查询 + 安装包下载）
        http.c   WinINet 会话（自动 Cookie）
        json.c   极简 JSON 路径取值器
        cfg.c    INI 配置（GBK 编码）
        hist.c   查询历史
        str.c/fmt.c  编码转换 / 单词命名格式化
ui\     Nim + wNim 原生控件界面
        main.nim        主窗口 + 设置 + 热键 + 托盘 + 后台翻译线程
        fastcore.nim    C 核心绑定（{.compile.} 直接编译 C 源码）
```

## 必应免费接口原理

1. `GET cn.bing.com/translator` 提取会话参数（IG / IID / token / key，约 1 小时有效，自动刷新）
2. `POST /ttranslatev3` 表单携带 token/key 获取译文与拼音
3. 单词走 `cn.bing.com/dict/search` 解析音标与释义

无需任何密钥；若必应临时要求人机验证，程序会提示稍后重试。

## 说明

- 发音（TTS）功能未包含
- 配置文件 `fastT\config` 为 GBK 编码的 INI（段/键名见程序设置）
- 详细开发记录见 [说明.md](说明.md)

## License

[MIT](LICENSE)
