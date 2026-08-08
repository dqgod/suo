# Suo / 梭

Suo 是一个面向 Windows 与 macOS 的轻量快捷启动器。按下全局快捷键后，可以搜索应用和文件、计算表达式、翻译文本、打开自定义网络搜索，以及运行本地脚本命令。

项目当前处于 MVP 增量开发阶段。稳定、可构建的基线保存在 `master`，日常开发在长期分支 `dev` 上进行。

## 产品目标范围

- Windows x64，macOS Apple Silicon / Intel；
- Windows 优先使用 Everything，失败时回退到 Suo 限定目录索引；
- macOS 使用 Spotlight；
- 计算器、Microsoft Translator、自定义 HTTP/HTTPS 搜索；
- Python、PowerShell、Bash 和可执行文件命令；
- 纯文本与 `suo-json-v1` 脚本输出；
- 内置主题与有限设计变量定制。

详细产品边界和验收标准见 [产品需求文档](./docs/PRODUCT_REQUIREMENTS.md)，可交互视觉稿见 [HTML UI 原型](./docs/ui-prototype.html)。

## `dev` 当前状态

当前 `dev` 已实现 Windows 首轮可运行闭环：

- `Alt+Space` 唤起/隐藏无边框窗口，`Esc` 关闭；macOS 源码默认使用 `Command+Space`；
- Windows 与 macOS 均启用单实例保护；正常运行时再次启动会唤醒并聚焦已有主窗口；
- 常驻系统托盘；左键唤醒主窗口，右键菜单可显示 Suo、打开设置或退出进程；
- 搜索开始菜单应用和桌面、文档、下载目录文件；
- `f <关键词>` 通过官方 `ES.exe` 连接已有 Everything，IPC 不可用时自动回退限定目录索引；
- macOS 的 `f <关键词>` 通过 Spotlight 文件名索引搜索，失败时同样回退限定目录索引（源码已接入，仍需 Apple Silicon 实机验证）；
- 非空输入统一采用 50 ms 尾沿防抖，并取消/丢弃过期查询；
- `11+1` 本地计算；脚本命令可在设置中增删改，支持 Python、PowerShell、Bash 和可执行文件、安全 argv、即时或 Enter 执行；
- 网络搜索可在设置中增删改，校验 `{query}` 占位符及 HTTP/HTTPS URL，按 Enter 后才打开浏览器；
- `fy hello` 使用 Microsoft Translator，支持 `fy:ja hello`，API Key 存入系统凭据库而非 JSON；
- 提供午夜、纸张、森林三套主题和自定义强调色；
- 应用搜索结果异步加载系统原生图标，并限制并发、缓存与可访问路径；
- 命令、网络搜索和翻译服务采用摘要卡片，点击后展开编辑；每项支持可选描述；
- 通用设置可开启“空输入时仅显示搜索框”；空查询收缩原生窗口，开始输入后恢复完整窗口；
- `setting`、`settings` 或 `设置` 显示设置入口，按 Enter 打开设置窗口；
- 设置保存到用户配置目录的 `config.json`，保存前校验命令关键字的全局唯一性；
- 查询取消、陈旧结果保护、可配置脚本超时、1 MB 流式输出上限和进程树终止。

仍未完成的发布级工作包括：可录制快捷键、开机自启、应用来源与索引目录管理、脚本首次运行确认、`suo-json-v1` 多结果协议、日志与崩溃恢复、主题导入导出、安装升级，以及 macOS Apple Silicon 实机验证。

## 开发环境

- Node.js 22+
- pnpm 11+
- Rust stable（通过 rustup）
- Windows：Microsoft C++ Build Tools 与 WebView2
- macOS：Xcode Command Line Tools

## 常用命令

Windows 请从 Visual Studio 的 “Developer PowerShell for VS 2022” 运行原生构建命令，确保 MSVC `link.exe` 位于 Git for Windows 同名工具之前。

```powershell
pnpm install
pnpm build
pnpm tauri dev
pnpm tauri build
```

Rust 单元测试：

```powershell
Set-Location .\src-tauri
cargo test
```

### macOS Apple Silicon 首次验证

macOS 代码路径已经接入，但尚未完成 Apple Silicon 实机验收。请在 `dev` 最新版本上先建立未修改基线：

```bash
git switch dev
git pull --ff-only origin dev
uname -m                    # 应为 arm64
sw_vers
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --no-bundle
pnpm tauri build --bundles app
file src-tauri/target/release/suo
pnpm tauri dev
```

重点实测：

- `Command+Space` 默认会与 macOS Spotlight 冲突：先确认冲突提示，再临时关闭或改绑系统 Spotlight 快捷键，重启 Suo 后验证唤起；
- 菜单栏托盘菜单、Dock 图标、单实例、失焦/Esc、圆角透明窗口以及 Retina/多显示器定位；
- “空输入时仅显示搜索框”开启后，原生窗口应真实收缩，不能留下透明点击区域；
- `/Applications`、`/System/Applications`、`~/Applications` 中的 `.app` 搜索、原生图标与打开行为；
- `f <关键词>` 的 Spotlight 搜索、快速输入取消、隐私权限提示和限定目录索引回退；
- macOS Keychain 翻译凭据，以及 Python 3、Bash、可执行脚本的参数、超时与进程组终止。

可直接运行的应用包位于 `src-tauri/target/release/bundle/macos/Suo.app`。当前阶段不要求签名、公证或 DMG 发布。给 macOS Codex 的完整检查边界和故障记录格式见 [AGENTS.md](./AGENTS.md#macos-apple-silicon-handoff)。

## 分支约定

- `master`：始终保持可构建，用于稳定基线和发布；
- `dev`：长期开发分支，Windows 技术验证和后续功能在此推进。

## 数据与隐私

本地应用和文件查询不会上传。翻译请求只发送给用户配置的服务商；API Key 写入系统凭据存储。Suo 默认不保存完整查询文本，也不自动上传遥测。

## License

[MIT](./LICENSE)
