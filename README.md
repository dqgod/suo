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
- 搜索界面与设置界面各自独立的三套内置皮肤，以及可导入、导出、实时预览的自定义皮肤。

详细产品边界和验收标准见 [产品需求文档](./docs/PRODUCT_REQUIREMENTS.md)，当前交互视觉稿见 [UI 方案索引](./docs/README.md)。

## `dev` 当前状态

当前 `dev` 已实现 Windows 首轮可运行闭环，并于 2026-08-08 完成 macOS Apple Silicon 首轮实机验证：

- `Alt+Space` 唤起/隐藏无边框窗口，`Esc` 关闭；macOS 源码默认使用 `Command+Space`；
- Windows 与 macOS 均启用单实例保护；正常运行时再次启动会唤醒并聚焦已有主窗口；
- 常驻系统托盘；左键唤醒主窗口，右键菜单可显示 Suo、打开设置或退出进程；macOS 使用随菜单栏明暗自动反色的中空模板图标，并可在通用设置中隐藏 Dock 图标；Windows 保留彩色托盘图标，快捷键搜索窗口不占任务栏，设置窗口仍显示任务栏图标；
- 搜索开始菜单应用和桌面、文档、下载目录文件；中文应用名称支持拼音全拼和首字母，例如 `weixin` / `wx` 可命中“微信”；
- `f <关键词>` 通过官方 `ES.exe` 连接已有 Everything，IPC 不可用时自动回退限定目录索引；
- macOS 的 `f <关键词>` 通过 Spotlight 文件名索引搜索，失败时同样回退限定目录索引；
- 通用设置可分别配置空输入和非空输入的尾沿防抖（均为 0–60000 ms，默认 0 / 50 ms），并取消或丢弃过期查询；即时脚本继续逐项配置自己的 20–60000 ms 执行延迟；
- 通用设置可在 560–1200 px 宽、320–720 px 高范围内调整启动器，并相对原始居中偏上位置微调水平/垂直偏移；多显示器上会自动夹紧到目标工作区；
- `11+1` 本地计算；脚本命令可在设置中增删改，支持 Python、PowerShell、Bash 和可执行文件、安全多 argv、即时或 Enter 执行；脚本路径旁可打开所在文件夹并选中文件；参数数量与含义由脚本决定；
- 网络搜索可在设置中增删改；`{query}` 表示无需引号的整段参数，`{query0}`、`{query1}`…表示位置参数；模板必须以规范的 `http://` 或 `https://` 开头，按 Enter 后才打开浏览器；
- `fy hello` 使用 Microsoft Translator，支持 `fy:ja hello`，API Key 存入系统凭据库而非 JSON；
- 搜索界面与设置界面拥有完全独立的午夜、纸张、森林主题库、自定义皮肤和强调色；分别使用严格校验的 `suo-launcher-theme-v1` / `suo-settings-theme-v1` 导入导出文件，支持实时预览、可读性提示、经受限解码的本地背景图和平台覆盖；
- 搜索皮肤可分别调整窗口和搜索框边框、输入文字、普通/选中结果颜色与字号、结果行高和图标尺寸，并可隐藏放大镜、Suo Logo 与来源标签；
- 应用搜索结果异步加载系统原生图标，并限制并发、缓存、像素大小与可访问路径；文件夹使用统一文件夹图标，其余文件使用统一文件图标；Windows 已通过微信开始菜单快捷方式的真实图标提取验证；
- 命令、网络搜索和翻译服务采用摘要卡片，点击后展开编辑；每项支持可选描述和独立启用开关，开启为绿色、关闭为灰色；设置页可选择最终统一“保存设置”或在合法修改后自动保存；
- 通用设置可开启“空输入时仅显示搜索框”；空查询收缩原生窗口，开始输入后恢复完整窗口；
- `setting`、`settings` 或 `设置` 显示设置入口，按 Enter 打开设置窗口；
- 设置保存到用户配置目录的 `config.json`，保存前校验命令关键字的全局唯一性；
- 查询取消、陈旧结果保护、可配置脚本超时、1 MB 流式输出上限和进程树终止。

2026-08-09 当前 v10 增量已在 Windows 10 x64 完成前端、69 项 Rust 测试和 MSVC no-bundle 构建，最终 EXE 为 x64；真实 `Alt+Space`、配置迁移、几何设置、多屏/DPI、彩色托盘图标、窗口级任务栏策略、应用图标、选中态和保存模式均通过。自动化截图对透明无边框窗口返回的 `0x80004002` 已确认仅为 Windows 10 捕获接口限制，不影响 Suo；详见 [`handoff/WINDOWS.md`](handoff/WINDOWS.md)。

### 命令参数约定

- 脚本命令后的内容按引号感知规则拆成 argv，Suo 不限定参数数量或含义。`ts 1786082576069 +8` 会传入 `["1786082576069", "+8"]`，由 `timestamp.py` 将第二项解释为时区偏移；
- 网络搜索 `{query}` 表示关键词后的完整文本，因此 `google test codex` 无需引号即可得到一个 `test codex` 值；
- `{query0}`、`{query1}`…表示位置参数。同一输入配合 `?q={query0}&v={query1}` 会分别填入 `test`、`codex`；只有单个位置参数本身包含空格时才需要引号。

仍未完成的发布级工作包括：可录制快捷键、开机自启、应用来源与索引目录管理、脚本首次运行确认、`suo-json-v1` 多结果协议、日志与崩溃恢复、安装升级、签名、公证和 DMG 发布。

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

### macOS Apple Silicon 验证

2026-08-08 已在 Apple Silicon（`arm64`）、macOS 15.6.1（24G90）完成首轮硬件验证。前端构建、全部 Rust 测试、无 bundle 构建、`.app` 构建和开发模式启动均通过，产物为 arm64。

2026-08-09 在同一台机器上对 `dev` 的 `eccc848` 增量再次完成验证：v7 配置迁移到 v8 后保留原有命令、网络搜索、主题和通用偏好，新增空查询/非空查询延迟按 0/50 ms 落默认值，保存后生成 v8 且 `.bak` 与迁移前配置一致；统一保存与自动保存模式、普通查询的 trailing-edge 延迟、即时脚本自己的 50 ms 延迟、启动器/设置两套独立主题库及保存/应用状态均通过。应用目录的原生名称、全拼和拼音首字母检索均通过，文件检索不会使用拼音；`/Applications` 与 `~/Applications` 的原生图标、缓存复用和打开行为、目录/普通文件固定图标、符号链接 `.app` 拒绝也已实测。Retina 尺寸调整、背景图与透明度、搜索图标/Logo 显隐、Spotlight 快速输入取消、计算器、Google 搜索以及未配置 Keychain 时的翻译提示均正常。跨作用域和旧版主题包由前端解析器正确拒绝；自动保存使用修订号队列，保存响应仅在草稿仍与该请求一致时回写，避免覆盖保存期间的新输入。

本轮 `pnpm build`、64 项 Rust 测试、无 bundle 构建和 `.app` 构建再次通过，二进制与 `Suo.app` 内可执行文件均为 arm64。验证机同时安装了 Intel 与 Apple Silicon 工具链，因此执行时显式选择 arm64 Node 和 Rust；在类似环境中复验前应额外确认 `node -p process.arch` 为 `arm64`、`rustc -vV` 的 host 为 `aarch64-apple-darwin`，避免 Rollup 载入错误架构的原生依赖或产出 x86_64 二进制。

同日后续增量已将配置升级到 v9：v8 实机配置保存为 v9 后，原有字段逐项保持一致，新增宽度为“跟随搜索皮肤”、高度 520 px、水平/垂直偏移 0/0；迁移前 v8 文件继续保留为 `.bak`。通用设置的四组滑杆与数值输入、越界值夹紧、800×600 与 1200×320 实际窗口、紧凑空输入、工作区位置算法和纯背景选中行均已验证。macOS 托盘使用独立 44×44 透明模板资产并启用系统 template 模式，Windows 图标链路未改动；辅助功能接口无法截取 SystemUIServer 菜单栏，最终明暗菜单栏观感仍需肉眼确认。本轮前端构建、68 项 Rust 测试、无 bundle 构建和 `.app` 构建通过。

同日再次升级到配置 v10，并在通用设置加入 macOS 专属“在 Dock 中显示图标”开关。v9 实机配置迁移后原有字段逐项一致且默认继续显示 Dock 图标；关闭开关时进程由 LaunchServices `Foreground` 即时切换为 `UIElement`，设置窗口和后台进程保持运行，重新开启后恢复 `Foreground`，隐藏状态重启后仍生效。验证完成后原始配置与 `.bak` 已按 SHA-256 原样恢复。本轮前端构建、69 项 Rust 测试、无 bundle 构建和 `.app` 构建通过，产物为 arm64。

实机已通过应用发现/原生图标/打开、Spotlight 文件名搜索与快速输入取消、计算器、Google 搜索、Python/Bash/Executable argv、脚本超时进程组终止、圆角透明窗口、紧凑空查询窗口、Finder 再次打开唤醒和单实例进程检查。Dock 图标已使用 macOS 专用透明边距资源，未改动 Windows 图标资产。

本机 `Command+Space` 可直接注册并显示“已就绪”，因此未复现系统 Spotlight 占用时的冲突提示；该分支仍需在保留系统默认快捷键的机器上补测。Microsoft Translator 的缺少凭据提示已通过，按本次验证选择未向 Keychain 写入真实密钥，也未发送真实翻译请求。菜单栏托盘右键菜单、真实快捷键按键唤起和多显示器定位仍需人工操作补充确认。

复验命令：

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
- 搜索皮肤与设置皮肤分别切换、编辑和导入，确认两侧主题库与运行时样式互不污染；跨作用域及旧版主题文件必须被拒绝；带透明度或背景图的主题需结合真实桌面复核可读性；
- `/Applications`、`/System/Applications`、`~/Applications` 中的 `.app` 搜索、原生图标与打开行为，并确认目录/普通文件显示各自固定图标；
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
