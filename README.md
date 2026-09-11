# Suo / 梭

Suo 是一个面向 Windows 与 macOS 的轻量快捷启动器。按下全局快捷键后，可以搜索应用和文件、计算表达式、翻译文本、打开自定义网络搜索，以及运行本地脚本命令。

## 产品目标范围

- Windows x64，macOS Apple Silicon / Intel；
- Windows 优先使用 Everything，失败时回退到 Suo 限定目录索引；
- macOS 使用 Spotlight；
- 计算器、可切换的 Microsoft / Google / 有道翻译、自定义 HTTP/HTTPS 搜索；
- Python、PowerShell、Bash 和可执行文件命令；
- 当前支持带类型前缀的文本、图片与二维码单结果脚本输出；`suo-json-v1` 多结果协议仍在后续计划中；
- 搜索界面与设置界面各自独立的三套内置皮肤，以及可导入、导出、实时预览的自定义皮肤。

详细产品边界和验收标准见 [产品需求文档](./docs/PRODUCT_REQUIREMENTS.md)，开发进度与后续计划见 [开发状态与迭代路线图](./docs/ROADMAP.md)，当前交互视觉稿见 [UI 方案索引](./docs/README.md)，逐版本改动见 [更新记录](./CHANGELOG.md)。

## 使用与配置说明

### 命令参数约定

- 脚本命令后的内容按引号感知规则拆成 argv，Suo 不限定参数数量或含义。`ts 1786082576069 +8` 会传入 `["1786082576069", "+8"]`，由 `timestamp.py` 将第二项解释为时区偏移；
- 网络搜索 `{query}` 表示关键词后的完整文本，因此 `google test codex` 无需引号即可得到一个 `test codex` 值；
- `{query0}`、`{query1}`…表示位置参数。同一输入配合 `?q={query0}&v={query1}` 会分别填入 `test`、`codex`；只有单个位置参数本身包含空格时才需要引号。
- URL 不含占位符时作为固定直达链接，例如将 `mydoc` 配置为 `https://bytedance.feishu.cn/drive/home/` 后，只输入 `mydoc` 并回车即可打开。

### 脚本返回值动作

不知道脚本入口、参数和 stdout/stderr 应该怎样组织时，直接从带逐行注释的 [`examples/script_template.py`](examples/script_template.py) 开始；设置字段、argv 拆分、执行时机、退出码以及两种返回值动作见 [`examples/README.md`](examples/README.md)。

每条脚本命令的“返回值动作”默认是“复制返回文本”，所以现有 `ts` 等配置无需修改。选择“执行返回的 Shell 命令（高风险）”后，脚本本身仍先按安全 argv 模式运行；它的 stdout 只会显示成一个待执行结果，不会自动执行。用户必须再次点击该结果或按 `Enter`，Suo 才会在 macOS 使用 `/bin/bash -lc`、在 Windows 使用 `powershell.exe -NoLogo -NoProfile -NonInteractive -Command` 执行返回文本。

脚本可以用区分大小写的类型前缀明确选择返回内容：`SUO_RESULT:text:` 返回文本，旧 `SUO_RESULT:` 继续按文本兼容；`SUO_RESULT:image:` 返回一张 PNG/JPEG/WebP data URL，纯 Base64 则按 PNG 解释；`SUO_RESULT:qrcode:` 让 Suo 在本地把后续文字渲染为二维码 PNG。只要 stdout 中出现结果前缀，普通 `print()` 日志就会忽略；一次执行不能混合文本和图片类型。图片会在 Rust 中完成格式、完整性、尺寸和解码上限校验，再进入 WebView；不允许远程 URL 或 SVG。stderr 和脚本返回的 Shell 命令执行输出不应用此前缀解析。

默认 `qr` 命令指向 [`examples/qr.py`](examples/qr.py) 初始化出的 `scripts/qr.py`。输入 `qr www.google.com` 后脚本只返回 `qrcode` 类型和原始内容，二维码编码与 PNG 生成由 Suo 本地完成，不需要 `qrcode`、Pillow 等额外 Python 包，也不会访问二维码服务；内容上限为 500 个 UTF-8 字节，中文和 emoji 会写入 UTF-8 字符集声明；后端保留清晰原图，搜索结果中的图片缩略图限制在 240 px 内并随窗口高度进一步缩小，以保证整张图片与操作区域都可见；选中结果按 Enter 会复制原始文字或网址。

示例 [`examples/open_path.py`](examples/open_path.py) 接收一个文件或目录参数并生成平台命令。Suo 首次启动会把它初始化到用户脚本目录。可在“设置 → 命令与服务 → 脚本命令”新增：关键词 `open_file`、运行时 `Python`、路径 `scripts/open_path.py`、执行方式“按 Enter 执行”、返回值动作“执行返回的 Shell 命令”。输入 `open_file ~` 后，第一次 `Enter` 运行 Python 并显示命令，第二次 `Enter` 才打开目录。

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

### 用户脚本目录

Suo 自带的 [`examples/`](examples/) 仅作为安装包中的只读模板。首次启动或模板缺失时，Suo 会把模板分别初始化到：

- macOS：`~/Library/Application Support/io.github.dqgod.suo/scripts`
- Windows：`%APPDATA%\io.github.dqgod.suo\scripts`

默认 `ts` 命令使用 `scripts/timestamp.py`，默认 `qr` 命令使用 `scripts/qr.py`。Suo 仅创建不存在的模板文件，升级、重装和后续启动都不会覆盖同名用户脚本；即使把 `config.json` 迁移到其他目录，用户脚本目录也保持上述固定位置。旧版默认 `examples/timestamp.py` 配置会自动迁移，用户自行配置的绝对路径或其他相对路径保持不变；v16 升级到 v17 时，仅在 `qr` 关键字/别名和 `qr-example` ID 均未被占用时添加二维码示例。

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

- 当前版本、平台资产和待验证项统一维护在 [`handoff/README.md`](./handoff/README.md)。
- macOS 与 Windows 的执行清单分别见 [`handoff/MACOS.md`](./handoff/MACOS.md) 和 [`handoff/WINDOWS.md`](./handoff/WINDOWS.md)。
- 已知跨平台工具链、配置迁移与平台隔离问题见 [`handoff/CROSS_PLATFORM.md`](./handoff/CROSS_PLATFORM.md)。

## 分支约定

- `master`：始终保持可构建，用于稳定基线和发布；
- `dev`：长期开发分支，Windows 技术验证和后续功能在此推进。

## 数据与隐私

本地应用和文件查询不会上传。翻译请求只发送给用户当前选择的服务商；Microsoft / Google API Key 和有道应用凭据写入系统凭据存储。Suo 默认不保存完整查询文本，也不自动上传遥测。

## License

[MIT](./LICENSE)
