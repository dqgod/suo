# Suo / 梭

Suo 是一个面向 Windows 与 macOS 的轻量快捷启动器。按下全局快捷键后，可以搜索应用和文件、计算表达式、翻译文本、打开自定义网络搜索，以及运行本地脚本命令。

项目当前处于技术验证阶段。稳定、可构建的基线保存在 `master`，日常开发在长期分支 `dev` 上进行。

## 产品目标范围

- Windows x64，macOS Apple Silicon / Intel；
- Windows 优先使用 Everything，失败时回退到 Suo 限定目录索引；
- macOS 使用 Spotlight；
- 计算器、Microsoft Translator、自定义 HTTP/HTTPS 搜索；
- Python、PowerShell、Bash 和可执行文件命令；
- 纯文本与 `suo-json-v1` 脚本输出；
- 内置主题与有限设计变量定制。

详细产品边界和验收标准见 [产品需求文档](./docs/PRODUCT_REQUIREMENTS.md)，可交互视觉稿见 [HTML UI 原型](./docs/ui-prototype.html)。

## `dev` 技术验证状态

当前 `dev` 已实现 Windows 首轮可运行闭环：

- `Alt+Space` 唤起/隐藏无边框窗口，`Esc` 关闭；macOS 源码默认使用 `Command+Space`；
- 搜索开始菜单应用和桌面、文档、下载目录文件；
- `f <关键词>` 通过官方 `ES.exe` 连接已有 Everything，IPC 不可用时自动回退限定目录索引；
- `11+1` 本地计算，`ts 1786082576069` 安全执行示例 Python 脚本；
- `google codex` 生成 HTTPS 查询地址并使用默认浏览器打开；
- `setting`、`settings` 或 `设置` 显示设置入口，按 Enter 打开设置窗口；
- 查询取消、陈旧结果保护、3 秒脚本超时、1 MB 流式输出上限和 Windows Job Object 进程树终止。

翻译 Provider、命令与网络搜索 CRUD、完整设置页、主题编辑和 macOS Spotlight 适配仍属于后续 MVP 工作，不应把当前技术验证版视为完整产品。

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

## 分支约定

- `master`：始终保持可构建，用于稳定基线和发布；
- `dev`：长期开发分支，Windows 技术验证和后续功能在此推进。

## 数据与隐私

本地应用和文件查询不会上传。翻译请求只发送给用户配置的服务商；API Key 写入系统凭据存储。Suo 默认不保存完整查询文本，也不自动上传遥测。

## License

[MIT](./LICENSE)
