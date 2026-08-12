# Suo / 梭

Suo 是一个面向 Windows 与 macOS 的轻量快捷启动器。按下全局快捷键后，可以搜索应用和文件、计算表达式、翻译文本、打开自定义网络搜索，以及运行本地脚本命令。

项目当前处于 MVP 增量开发阶段。稳定、可构建的基线保存在 `master`，日常开发在长期分支 `dev` 上进行。

## 产品目标范围

- Windows x64，macOS Apple Silicon / Intel；
- Windows 优先使用 Everything，失败时回退到 Suo 限定目录索引；
- macOS 使用 Spotlight；
- 计算器、可切换的 Microsoft / Google / 有道翻译、自定义 HTTP/HTTPS 搜索；
- Python、PowerShell、Bash 和可执行文件命令；
- 当前支持纯文本单结果脚本输出；`suo-json-v1` 多结果协议仍在后续计划中；
- 搜索界面与设置界面各自独立的三套内置皮肤，以及可导入、导出、实时预览的自定义皮肤。

详细产品边界和验收标准见 [产品需求文档](./docs/PRODUCT_REQUIREMENTS.md)，当前交互视觉稿见 [UI 方案索引](./docs/README.md)。

## `dev` 当前状态

当前 `dev` 已实现 Windows 首轮可运行闭环，并于 2026-08-08 完成 macOS Apple Silicon 首轮实机验证：

- 默认以 Windows `Alt+Space`、macOS `Command+Space` 唤起/隐藏无边框窗口，`Esc` 关闭；通用设置可点击录制并更换组合键，冲突时保留原快捷键；
- Windows 与 macOS 均启用单实例保护；正常运行时再次启动会唤醒已有主窗口；
- macOS 搜索框使用不会激活 Suo 的原生面板：唤起后继续保留原应用的菜单栏和输入焦点显示，同时搜索框可接收键盘输入；显式打开设置页时仍会正常激活 Suo。Windows 保持原有搜索窗口聚焦行为；
- 常驻系统托盘；左键唤醒主窗口，右键菜单可显示 Suo、打开设置或退出进程；macOS 使用随菜单栏明暗自动反色的中空模板图标，并可选择只在设置窗口打开期间显示 Dock 图标；Windows 保留彩色托盘图标，快捷键搜索窗口不占任务栏，设置窗口仍显示任务栏图标；
- 搜索开始菜单应用和桌面、文档、下载目录文件；中文应用名称支持拼音全拼和首字母；macOS 同时读取 `.app` 的 Bundle 名称、URL scheme 和中文本地化名称，因此 `微信` / `weixin` 可命中 WeChat，`飞书` / `feishu` 可命中 Lark；
- `f <关键词>` 通过官方 `ES.exe` 连接已有 Everything，IPC 不可用时自动回退限定目录索引；
- macOS 的 `f <关键词>` 通过 Spotlight 文件名索引搜索，失败时同样回退限定目录索引；
- 通用设置可分别配置空输入和非空输入的尾沿防抖（均为 0–60000 ms，默认 0 / 50 ms），并取消或丢弃过期查询；即时脚本继续逐项配置自己的 20–60000 ms 执行延迟；
- 通用设置可在 560–1200 px 宽、320–720 px 高范围内调整启动器，并相对原始居中偏上位置微调水平/垂直偏移；多显示器上会自动夹紧到目标工作区，首次快捷键显示前即由原生层应用已保存的尺寸、紧凑高度和位置，避免默认居中窗口一闪再移动；
- `11+1` 本地计算；脚本命令可在设置中增删改并选择可选本地图标、空参数提示，支持 Python、PowerShell、Bash 和可执行文件、安全多 argv、即时或 Enter 执行；返回文本默认在再次按 Enter 时复制，也可显式选择高风险的“执行返回的 Shell 命令”；脚本路径旁可打开所在文件夹并选中文件；参数数量与含义由脚本决定；
- 网络搜索可在设置中增删改，并选择可选本地图标、空参数提示；不含占位符的 HTTP/HTTPS URL 是固定直达链接，只输入关键词即可生成打开结果；`{query}` 表示无需引号的整段参数，`{query0}`、`{query1}`…表示位置参数，所有链接都只在按 Enter 后交给浏览器；
- `fy hello` 使用当前选择的 Microsoft Translator、Google 翻译或有道翻译，支持 `fy:ja hello`；各提供方凭据彼此独立，只存入系统凭据库而非 JSON；
- 搜索界面与设置界面拥有完全独立的午夜、纸张、森林主题库、自定义皮肤和强调色；分别使用严格校验的 `suo-launcher-theme-v1` / `suo-settings-theme-v1` 导入导出文件，支持实时预览、可读性提示、经受限解码的本地背景图和平台覆盖；
- 搜索皮肤可分别调整窗口和搜索框边框、输入文字、普通/选中结果颜色与字号、结果行高和图标尺寸，并可隐藏放大镜、Suo Logo 与来源标签；
- 应用搜索结果异步加载系统原生图标，并限制并发、缓存、像素大小与可访问路径；文件夹使用统一文件夹图标，其余文件使用统一文件图标；Windows 已通过微信开始菜单快捷方式的真实图标提取验证；
- 命令、网络搜索和翻译服务采用摘要卡片，点击后展开编辑；每项支持可选描述和独立启用开关，开启为绿色、关闭为灰色；设置页可选择最终统一“保存设置”或在合法修改后自动保存；
- 通用设置可开启“空输入时仅显示搜索框”；空查询收缩原生窗口，开始输入后恢复完整窗口；
- `setting`、`settings` 或 `设置` 显示设置入口，按 Enter 打开设置窗口；
- 设置页在说明下方整行显示当前 `config.json` 的完整路径，可打开所在文件夹、选择空目录迁移或恢复平台默认位置；切换前先复制并校验，原位置文件保留为恢复副本；
- 通用设置可开启跨平台开机自启；登录后只在后台建立托盘/菜单栏与全局快捷键，不主动弹出搜索窗口；快捷键录制期间不会触发当前全局组合，Windows 录制 `Alt+Space` 时拦截系统菜单；
- 查询取消、陈旧结果保护、可配置脚本超时、1 MB 流式输出上限和进程树终止。

平台验证状态：macOS Apple Silicon 已完成至配置 v14 的核心功能与原生 arm64 构建，v15 开机自启等待 Mac 实机验证。Windows 10 x64 已完成 v10 基线；v15 已通过 x64 编译、102 项测试和旧配置只读启动检查，开关与快捷键录制仍待人工交互验收；v11–v14 的其余增量清单保持待验证。详细证据与执行清单分别见 [`handoff/MACOS.md`](handoff/MACOS.md) 和 [`handoff/WINDOWS.md`](handoff/WINDOWS.md)。

### 命令参数约定

- 脚本命令后的内容按引号感知规则拆成 argv，Suo 不限定参数数量或含义。`ts 1786082576069 +8` 会传入 `["1786082576069", "+8"]`，由 `timestamp.py` 将第二项解释为时区偏移；
- 网络搜索 `{query}` 表示关键词后的完整文本，因此 `google test codex` 无需引号即可得到一个 `test codex` 值；
- `{query0}`、`{query1}`…表示位置参数。同一输入配合 `?q={query0}&v={query1}` 会分别填入 `test`、`codex`；只有单个位置参数本身包含空格时才需要引号。
- URL 不含占位符时作为固定直达链接，例如将 `mydoc` 配置为 `https://bytedance.feishu.cn/drive/home/` 后，只输入 `mydoc` 并回车即可打开。

### 脚本返回值动作

不知道脚本入口、参数和 stdout/stderr 应该怎样组织时，直接从带逐行注释的 [`examples/script_template.py`](examples/script_template.py) 开始；设置字段、argv 拆分、执行时机、退出码以及两种返回值动作见 [`examples/README.md`](examples/README.md)。

每条脚本命令的“返回值动作”默认是“复制返回文本”，所以现有 `ts` 等配置无需修改。选择“执行返回的 Shell 命令（高风险）”后，脚本本身仍先按安全 argv 模式运行；它的 stdout 只会显示成一个待执行结果，不会自动执行。用户必须再次点击该结果或按 `Enter`，Suo 才会在 macOS 使用 `/bin/bash -lc`、在 Windows 使用 `powershell.exe -NoLogo -NoProfile -NonInteractive -Command` 执行返回文本。

示例 [`examples/open_path.py`](examples/open_path.py) 接收一个文件或目录参数并生成平台命令。可在“设置 → 命令与服务 → 脚本命令”新增：关键词 `open_file`、运行时 `Python`、路径 `examples/open_path.py`、执行方式“按 Enter 执行”、返回值动作“执行返回的 Shell 命令”。输入 `open_file ~` 后，第一次 `Enter` 运行 Python 并显示命令，第二次 `Enter` 才打开目录。

该模式等同于执行可信本地代码，只应为本人可审查的本地脚本开启。Suo 使用一次性、不透明的结果授权，查询变化、配置变化或首次执行后立即失效，并继续应用非 root/非管理员限制、脚本超时、输出上限和进程树取消；但它不会尝试判断返回命令是否安全。

### 翻译 Provider 接入指南

三个 Provider 共用同一个 `fy` 功能，不会创建额外命令。进入“设置 → 命令与服务 → 服务 → 翻译”，在“翻译能力提供方”中选择服务，填写该服务的凭据，再完成编辑并保存设置。`fy hello` 使用当前 Provider，`fy:ja hello` 临时指定目标语言。切换 Provider 不会删除其他 Provider 已保存的凭据，切回后可以继续使用。

#### Microsoft Translator

1. 按 [Microsoft 官方指引](https://learn.microsoft.com/en-us/azure/ai-services/translator/text-translation/how-to/use-rest-api) 在 Azure Portal 创建单服务 Translator 或 Azure AI multi-service 资源。
2. 打开资源的 “Keys and Endpoint”，复制一个 Key，填入 Suo 的“Azure API Key”。
3. multi-service 或区域资源还要填写资源 Region，例如 `eastasia`；单服务全局 Translator 资源可以留空。Suo 当前调用经过验证的 Translator Text API v3.0。

#### Google 翻译

1. 按 [Google Cloud Translation 设置指南](https://cloud.google.com/translate/docs/setup) 创建或选择 Google Cloud 项目，启用 Cloud Translation API，并为项目配置结算账号。
2. 创建 API Key；建议在 Google Cloud Console 中把该 Key 的 API 限制收紧到 Cloud Translation API。
3. 在 Suo 选择“Google 翻译”，把 Key 填入“Google Cloud API Key”。Suo 使用 [Cloud Translation Basic v2 `translate`](https://cloud.google.com/translate/docs/reference/rest/v2/translate)，不需要填写项目 ID 或 Region。

#### 有道翻译

1. 登录有道智云 AI 开放平台，按 [文本翻译 API 官方文档](https://ai.youdao.com/DOCSIRMA/html/trans/api/wbfy/index.html) 创建 API 类型应用，并为应用绑定文本翻译服务。
2. 在应用管理中取得“应用 ID”和“应用密钥”。
3. 在 Suo 选择“有道翻译”，分别填入这两个字段后点击“保存凭据”。请求使用 HTTPS、源语言自动识别和 v3 SHA-256 签名；默认简体中文代码会自动转换成有道所需的 `zh-CHS`。

Microsoft / Google 的 API Key，以及有道的应用 ID / 应用密钥，都只写入 macOS Keychain 或 Windows Credential Manager。它们不会进入 `config.json`、`.bak`、主题导出、日志或截图；更改配置文件位置也不会移动或导出凭据。删除凭据只删除当前选中的 Provider，不影响另外两家。

### 配置文件位置

默认配置文件路径：

- macOS：`~/Library/Application Support/io.github.dqgod.suo/config.json`
- Windows：`%APPDATA%\io.github.dqgod.suo\config.json`

“设置 → 通用 → 配置文件位置”会显示当前实际路径，并可直接打开文件夹或选择新的空目录。Suo 先在目标目录原子写入并校验 `config.json`，再更新默认目录中固定保留的 `config-location.json` 位置指针；目标已有 `config.json` 或 `.bak` 时拒绝覆盖。旧位置文件不会自动删除，可用于手动恢复。三家翻译 Provider 的凭据仍只保存在 macOS Keychain / Windows Credential Manager，不随普通配置迁移。

## TODO / 迭代路线图

以下顺序以个人日常可用、跨平台稳定和可发布性为优先。完成项必须同时具备代码验证和受影响平台的真实场景证据；具体执行记录继续放在 [`handoff/`](handoff/README.md)，不在本节堆积机器日志。

### P0：发布质量与稳定性

- [ ] 清完当前跨平台验证债务：完成 Windows v11–v15 回归、macOS v15 开机自启实测，并同步清理 `handoff/` 中已过期的待办状态；
- [ ] 建立可复现的发布流程：产出 Windows 安装版/便携版和 macOS 对应架构应用，补齐代码签名、公证、卸载、升级保留配置及回滚检查；当前阶段不接自动更新；
- [ ] 增加脚本首次运行确认；脚本路径、内容或解释器发生变化后重新确认，保留现有 argv、超时、输出上限、无提权和进程树终止边界；
- [ ] 增加有界滚动日志、崩溃恢复和“复制诊断信息”：默认只记元数据，按大小/保留期轮转并批量写入，导出时脱敏用户目录、查询、参数和凭据；
- [ ] 补齐配置与索引损坏恢复：保留损坏副本，优先从可验证的 `.bak` 恢复，只重建可再生索引，不静默覆盖用户命令；
- [ ] 建立性能基线与回归门槛：记录冷/热启动、快捷键到首帧、查询 P50/P95、空闲内存/CPU，以及 10 万索引项和 100 次连续显隐测试；常见查询 P95 目标不超过 200 ms；
- [ ] 完成发布前无障碍验收：全键盘操作、屏幕阅读器语义、系统高对比度、减少动态效果和 200% 缩放下的核心流程均可用。

### P1：核心体验优化

- [ ] 增加“搜索目录管理”：可添加、移除、暂停和排除目录，展示索引范围、条目数、最近完成时间与失败原因，并支持单目录重建；
- [ ] 将限定目录索引持久化并增量更新，监听新增、重命名和删除；应用安装/卸载后也能刷新目录，而不是必须重启 Suo；
- [ ] 为结果增加统一的次级动作和键盘入口：打开、在 Finder/Explorer 中显示、复制路径；后续可在同一入口加入固定、忽略和清除学习记录；
- [ ] 增加完全本地的常用项排序、固定项和最近使用；默认不保存完整查询文本，设置页可查看、清除或关闭学习数据；
- [ ] 增加应用别名与忽略列表，让用户为应用补充短命令/拼音别名，并隐藏卸载器等不需要的结果；
- [ ] 改进翻译接入体验：提供各 Provider 的官方创建入口、字段说明、连接测试和鉴权/限流/网络错误分类；凭据仍只进入 Keychain / Credential Manager；
- [ ] 实现严格校验的 `suo-json-v1` 脚本多结果协议，支持一条脚本返回多项标题、说明和受限动作，同时保持纯文本协议兼容与现有安全边界；
- [ ] 为命令、网络搜索、主题和普通设置增加不含密钥的配置导入/导出；导入的脚本默认禁用，必须逐项审查后开启。

### P2：可选能力

- [ ] 增加离线单位、日期和时区转换 Provider，与计算器共用即时结果体验，不依赖网络服务；
- [ ] 增加设置内搜索，能按“快捷键”“索引”“翻译”“主题”等关键词直接定位配置项；
- [ ] 增加可选文件快速预览，仅对明确选中的本地文件按需读取，并限制文件类型、大小、耗时和权限范围；
- [ ] 在不引入插件市场的前提下，提供经过版本校验的本地配置包，方便个人在多台设备间手工迁移命令与主题；密钥、历史和机器路径默认不导出。

### 当前不做

除非先修改产品需求和安全边界，否则当前不加入账号/云同步、插件市场、远程脚本、文件正文检索、CLI、自动更新、自动遥测、AI 对话或剪贴板历史。

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

## 平台验证与交接

- macOS Apple Silicon：核心功能已验证至配置 v14，最终原生 arm64 测试和 `.app` 构建通过；v15 LaunchAgent、真实场景证据及尚需人工按键的项目见 [`handoff/MACOS.md`](./handoff/MACOS.md)。
- Windows x64：v10 基线已完成；v15 已完成 x64 构建和自动化检查，开机自启、快捷键录制及路径布局待人工交互验收；v11–v14 的其余清单仍需实机验证，详见 [`handoff/WINDOWS.md`](./handoff/WINDOWS.md)。
- 已知跨平台工具链、配置迁移与平台隔离问题见 [`handoff/CROSS_PLATFORM.md`](./handoff/CROSS_PLATFORM.md)。README 仅维护当前状态，不保存逐轮测试流水。

## 分支约定

- `master`：始终保持可构建，用于稳定基线和发布；
- `dev`：长期开发分支，Windows 技术验证和后续功能在此推进。

## 数据与隐私

本地应用和文件查询不会上传。翻译请求只发送给用户当前选择的服务商；Microsoft / Google API Key 和有道应用凭据写入系统凭据存储。Suo 默认不保存完整查询文本，也不自动上传遥测。

## License

[MIT](./LICENSE)
