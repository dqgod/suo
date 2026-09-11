# Windows release validation handoff

状态：**Windows x64 `v0.1.2` 候选已完成安装包与 SHA-256 构建，正在关闭独立复审，尚未创建 tag 或 GitHub Release；最新安装包的真实界面复测仍有待办。** `v0.1.2` 修复脚本管道中文输出、支持受控的多行/类型化结果，以配置 v16 把可编辑模板迁移到用户脚本目录，并以 v17 增加默认本地二维码命令。发布后 macOS 需从同一不可变 tag 补跨平台构建回归。

## 0.1 `v0.1.2` 脚本输出与类型化结果（2026-09-09 至 2026-09-11）

- [x] Python 子进程显式使用 UTF-8 stdout/stderr；普通文本严格 UTF-8 解码失败后，Windows 使用当前 ANSI 系统代码页回退，避免 CP936 中文被替换为 `�`。
- [x] 纯文本仍去除首尾空白，但中间换行会原样进入结果标题与复制动作；启动器使用 `pre-line` 并限制为最多两行，普通单行结果保持不变。
- [x] v16 阶段新增 UTF-8 中文、多行、Python 输出环境、CP936 回退、`SearchResult.title`/`CopyText` 换行、模板不覆盖、v15 路径迁移、已知模板回退和 `SUO_RESULT:` 兼容过滤测试；当时的 116 项常规 Rust 测试、前端生产构建和 `cargo check --all-targets` 通过，最终结论以后续 v17 候选门禁与复审为准。
- [x] 最终配置 v17 候选已通过前端生产构建、`cargo check --locked --all-targets` 与 125 项 Rust 测试（123 通过、2 项依赖本机应用目录而忽略）。
- [x] 已重新生成包含用户脚本初始化、类型化结果、UTF-8 ECI 二维码、完整脚本协议模板、完整图片缩略图、Windows 文件定位修复、删除二次确认及严格时间戳参数校验的 x64 NSIS 安装包 `Suo_0.1.2_x64-setup.exe`（4,023,263 bytes）；Product/File Version 均为 `0.1.2`，PE 为 `8664` / x64，SHA-256 为 `88C4E797300B828821701483E8B83E72E99755895E910B51AA3E96FFEF0D0FB6`，当前仍未签名。对应 release 可执行文件 `suo.exe` 为 15,486,976 bytes，SHA-256 `291A729A2F5838E3619AE0413FA26DA2309A433E19AAF07E5D58AD5D911D402D`。
- [ ] 独立复审关闭后创建不可变 `v0.1.2` tag 与同名 Pre-release，并上传 `Suo_0.1.2_x64-setup.exe` 与对应 `.sha256`；不得用不同源码覆盖同名资产。
- [ ] 从新 `v0.1.2` NSIS 安装包重装后，以用户当前两行中文 `timestamp.py` 完成真实启动器肉眼验收；先确认旧脚本已经迁入 `%APPDATA%\io.github.dqgod.suo\scripts`，重装后内容与哈希必须保持。
- [x] 本机旧安装目录脚本已在重装前复制到 `%APPDATA%\io.github.dqgod.suo\scripts\timestamp.py`，源/目标 SHA-256 均为 `F75C2C0604CFFE02FA0E268EC192CA70EAB4165BC07F448B05BD4918E3927EBE`；复制使用目标不存在门禁，未覆盖任何既有用户脚本。
- [x] 当时的 v16 release 候选已真实启动并完成首次初始化：缺失的 `open_path.py`、`script_template.py`、`README.md` 均创建成功，既有 `timestamp.py` 启动前后 SHA-256 保持 `F75C2C0604CFFE02FA0E268EC192CA70EAB4165BC07F448B05BD4918E3927EBE`；测试后已恢复原安装版后台进程，最终结论以后续 v17 候选为准。
- [x] 当前用户目录的 `script_template.py` 与增强版 `timestamp.py` 已在旧哈希门禁及备份后显式升级为 `emit_result()`；新版 no-bundle exe 已替换到安装目录并成功后台启动。原始 timestamp 备份仍保留在 `D:\ai_repo\suo-user-scripts\timestamp-2026-09-09.py`。
- [ ] Windows“在文件夹中显示”已从不稳定的 `explorer.exe /select` 参数切换为初始化 COM 后调用 `SHOpenFolderAndSelectItems`；实现、内存释放和错误分支已复审并通过编译，待用户从设置页点击真实 `ts` 项确认 Explorer 打开用户脚本目录且选中 `timestamp.py`。
- [x] `SUO_RESULT:text:` / `image:` / `qrcode:` 与旧 `SUO_RESULT:`、无前缀 stdout 的兼容回归通过；图片经 Rust 解码限制和 WebView data URL 白名单双重校验，图片/二维码不会进入结果 Shell 执行路径。
- [x] 二维码最多接收 500 个 UTF-8 字节，在 Byte mode 前写入 ECI 26；后端原图的 500-byte 边界保持 Version 17 或以下、372 px 且含静区仍为 4 px/模块。启动器中的所有脚本图片采用最大 240 px、随可用窗口高度继续缩小的完整缩略图，避免覆盖说明和底栏。默认 `qr.py` 的 500/501-byte 和中文 emoji 示例通过。
- [x] 脚本命令与网络搜索的删除按钮只打开应用内二次确认框；必须再次点击红色“确认删除”才变更草稿，取消、点击遮罩或 Esc 均不删除。手动保存模式仍须右上角保存，自动保存模式在确认后保存。
- [x] `timestamp.py` 参数矩阵通过：空参数、10/13 位时间戳、日期时间、`+08:30` / `-0530` 均输出带类型前缀的正确结果；非法时区、短时间戳以及 `now` / `help` 后的多余参数均以退出码 2 返回可读 stderr，不出现 traceback。
- [ ] 本机当前 `config.json.bak` 仍保留误删前的 `timestamp-example`，但不得用全局迁移自动复活用户明确删除的命令；安装候选后由用户确认是否从备份只恢复 `ts` 条目，再验证定位。当前运行中的旧安装版尚未被本轮构建替换。

## 0. `v0.1.0` 之后的 `dev` 验证（2026-08-30）

- [x] Windows 搜索框仍获得真实前台焦点并接收键盘输入，但显示后通过 `ITaskbarList::ActivateTab` 只恢复原前台窗口的任务栏选中外观。以 ChatGPT 为前台执行真实 `Alt+Space` 后，前台句柄由 ChatGPT 切换到 `Suo / 梭`，而任务栏左侧 400×160 区域前后 64,000 个像素完全一致；搜索窗口仍没有自己的任务栏入口。
- [x] Windows 在后台调用固定系统 PowerShell 的 `Get-StartApps` 补充 MSIX/UWP/打包应用，完成后刷新当前查询，不阻塞全局快捷键注册；失败时保留传统开始菜单 `.lnk/.exe` 结果。
- [x] `chatgpt`、`xbox`、`microsoft store` 在真实 dev release 中均命中“Windows 应用”，并显示各自不同的原生 Shell 图标；自动化实测也确认三个 AUMID 均能发现并产生非空 RGBA 图标。
- [x] 打包应用使用经校验的 AUMID 在原生 `IApplicationActivationManager` 中启动；前端只收到 SHA-256 目录摘要，伪造 result id 无法解析启动目标或图标路径。真实按 Enter 后 Microsoft Store 的 `WinStore.App` 新进程启动。
- [x] 同名传统快捷方式与打包应用不会仅因显示名称相同而互相覆盖；回归测试同时保留两个独立动作。
- [x] 109 项常规 Rust 单元测试、2 项本机打包应用发现/图标测试、`cargo check --all-targets`、前端生产构建和 NSIS 正式构建通过。`Suo_0.1.1_x64-setup.exe` 为 x64、Product/File Version 均为 `0.1.1`，SHA-256 为 `CADD589EF27644E40CEDB9A06E5F59DC433283219511962E9895B3B602A09F07`；仍未签名，符合当前 Pre-release 限制。
- [x] GitHub Release <https://github.com/dqgod/suo/releases/tag/v0.1.1> 已上传 `Suo_0.1.1_x64-setup.exe`（3,983,883 bytes）与对应 `.sha256` 文件；GitHub digest 为 `sha256:cadd589ef27644e40cedb9a06e5f59dc433283219511962e9895b3b602a09f07`，与本地一致。不可变 tag 指向 `f367c5fd59bd1784b56fe1429b34d91cbb2da710`。

这轮改动全部由 `cfg(windows)` adapter 隔离。macOS 接手时仍需运行完整构建，确认 `CatalogEntry` 新字段、`ResultAction::LaunchApplication` 序列化分支和前端刷新监听不会造成交叉平台编译或运行回归；不得把 Windows taskbar/AUMID/PowerShell 逻辑移入 macOS 路径。

## 1. 固定发布来源

在 **Visual Studio Developer PowerShell** 中从不可变 tag 验证，不要直接从可能继续前进的 `dev` 构建发布资产：

```powershell
git fetch origin --tags
git switch --detach v0.1.0
git rev-parse HEAD
git rev-list -n 1 v0.1.0
git status --short --branch
node --version
node -p "process.arch"
pnpm --version
rustc -vV
cargo --version
Get-Command link.exe | Format-List Source
```

预期为 x64 Node、`x86_64-pc-windows-msvc` Rust host 和 Visual Studio/MSVC 的 `link.exe`，不得使用 Git for Windows 的同名程序。若 tag 尚未出现在本地，先确认远程发布已完成，不要自行创建另一个同名 tag。

## 2. 干净构建与产物

```powershell
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
pnpm tauri build --no-bundle
pnpm tauri build --bundles nsis
dumpbin /headers src-tauri\target\release\suo.exe
Get-ChildItem src-tauri\target\release\bundle\nsis
```

- 最终 PE machine 必须是 `8664` / x64。
- NSIS 安装包必须真实安装、启动、卸载一次；升级/卸载仍未形成正式支持承诺，发现问题要记录。
- 不提交 `dist/`、`target/`、`.exe`、安装包或真实配置。

## 3. 当前版本必须完成的人工闭环

### 快捷键、开机自启与窗口角色

- [x] 从真实 v14 配置启动：运行时迁移为 v15 且 `startAtLogin=false`，其他关键字段保持；磁盘 v14 文件 SHA-256 `5A08A062620E5121DCD4435149A01D26C971C6A30D971E7AF0F4E706649CD3D4` 在加载前后不变。
- [x] 开机自启开关双向生效：Run 项精确写入 `"D:\Software\Suo\suo.exe" --autostart`，关闭时移除；使用该精确参数冷启动后进程正常且可见 Suo 顶层窗口数为 0。
- [x] 2026-08-29 用户真实注销 Windows 并重新登录后确认：Suo 按 Run 项在后台自动启动，没有主动弹出搜索窗口，托盘与全局快捷键行为均正常。
- [x] 录制当前已生效组合时只完成录制、不唤起搜索框；录制 `Alt+Space` 不弹系统菜单。另一个进程真实占用 `Alt+B` 后，Suo 显示注册失败并保留磁盘中的 `Alt+Space`；恢复录制后界面与配置均回到 `Alt+Space`。
- [x] `Alt+Space` 搜索窗口不进任务栏；设置窗口进入任务栏且可切回；关闭设置后任务栏入口消失，彩色托盘图标持续存在。
- [x] 完全退出后冷启动，第一次快捷键直接显示在保存的尺寸与位置，没有先闪现默认中心帧。

### 最新共享 UI 修复

- [x] 快速连续输入无匹配字符时保持稳定帧并一次更新，没有逐键加载闪烁。
- [x] 将即时脚本临时设为 600 ms 后，超过 250 ms 时显示延迟加载反馈；验收后恢复为 80 ms。
- [x] 等待新查询期间按 Enter 和立即点击旧结果均不执行旧动作；最新结果返回后，重新生成的动作通过 Enter 或点击恢复正常。自动化测试 `launcher::tests::a_new_search_invalidates_pending_script_output_actions` 同时覆盖查询变化撤销动作令牌。

### 核心跨平台回归

- [x] `微信`、`weixin`、`wx`、`飞书` 命中真实安装应用并显示原生图标；Everything 主路径正常。临时同时移除安装目录与源码回退位置的 ES 客户端后，`f` 查询明确切换为“Suo 限定目录索引”并命中文档探针；恢复后重新连接 Everything IPC。
- [x] 固定 URL 关键词 `mydoc` 产生可执行结果；`{query}` 与 `{query0}/{query1}` 正确展开；缺少第 2 个参数时显示错误且按 Enter 不执行。
- [x] 网络命令导入 32×32 PNG 后在启动器真实显示 data URL 图标；移除后恢复内置网络图标；Markdown 文件被拒绝。临时脚本也真实显示同一 32×32 自定义图标，并在无参数时显示“请输入目录”。自动化测试 `config::tests::validates_command_icons_and_normalizes_input_hints`、`launcher::tests::configured_web_search_hint_and_icon_reach_results` 与 `launcher::tests::configured_script_hint_only_replaces_empty_argument_subtitle` 同时覆盖校验、图标和空参数提示边界。
- [x] 共用 `fy` 可切换 Microsoft、Google、有道；三组明确标记的假凭据同时存入三个独立 Credential Manager 项，切换互不删除，JSON 与 `.bak` 均未出现假凭据或密钥；验收后全部清除并恢复 Microsoft 选择。
- [x] `open_file D:\Software\Suo` 第一次 Enter 只生成 `Invoke-Item` 命令且资源管理器窗口数仍为 0；第二次才通过非交互 PowerShell 打开目录；再次 Enter 不重复执行。补测中，查询变化后立即点击旧动作不执行，重新生成的新动作点击后只打开一次目录；临时脚本已删除。自动化测试 `launcher::tests::script_output_action_defaults_to_copy_and_shell_tokens_are_one_time` 覆盖默认复制与 Shell 一次性令牌。
- [x] 配置路径在说明下一行清晰显示；目标目录 `D:\ai_repo\suo-validation\配置 迁移` 在迁移前已确认为空，真实迁移、重启读取和界面恢复默认均通过，凭据未随 JSON 移动。`config::tests::relocating_config_is_transactional_and_keeps_recovery_copies` 会注入已占用目标，断言不覆盖目标、配置路径不变，并验证恢复默认后主配置与备份副本仍存在。

## 4. 不要重复的跨平台问题

- 不得把 macOS `NSPanel`、AppKit 队列、Dock、Spotlight、plist 或 template 图标编进 Windows 路径。
- 不得把 `showDockIcon` 解释为 Windows taskbar 开关；Windows 的搜索/设置窗口角色由独立 adapter 控制。
- 遇到 native module 缺失先检查 Node 架构和 PATH，不删除 lockfile；遇到链接错误先检查 MSVC `link.exe`。
- Windows-only import、参数和常量必须正确放入 `cfg(windows)`，不得让 macOS 再出现未使用告警。
- Windows Graphics Capture 的 `SetIsBorderRequired ... 0x80004002` 是已知自动化工具限制，不能替代真实肉眼验收，也不等于产品窗口失败。

## 5. Release 资产与回报

2026-08-29 Windows 验证证据：

- 系统：Windows 10 Enterprise 22H2，build `19045.6332`，x64；
- 工具链：Node `v22.15.1` x64、pnpm `11.19.0`、rustc/cargo `1.97.1`、host `x86_64-pc-windows-msvc`；构建通过 Visual Studio 2022 Developer Command Prompt 使用 `C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\14.41.34120\bin\Hostx64\x64\link.exe`，未使用默认 PATH 中 Git for Windows 的同名程序；
- `pnpm install --frozen-lockfile`、`pnpm build`、102 项 Rust 测试、`cargo check --all-targets`、release no-bundle 和 NSIS 构建全部通过；Windows 仅保留 macOS dock helper 的 `dead_code` warning 与 MSVC import-library 信息 warning；
- `src-tauri\target\release\suo.exe` 由 `dumpbin` 确认为 `8664 machine (x64)`；
- 安装包：`Suo_0.1.0_x64-setup.exe`，SHA-256 `7EB2981F18827EDB07DB2A0381188D4EA6A8B43831F5831C1EA4C12A3BF345D5`；当前未签名，符合 Pre-release 已知限制；
- NSIS 真实安装、启动、静默卸载和原路径重装均成功。卸载退出码 0，安装目录、卸载项、开始菜单快捷方式与 Run 项均移除，用户配置未删除；重装启动后按 v15 配置恢复 Run 项；
- 真实注销并重新登录后，开机自启、后台不弹窗、托盘与快捷键均由用户确认通过；
- 最终用户状态：`config.json`、`.bak`、位置指针 SHA-256 分别为 `46CE1ED5A0AA1FADD7F025FEA07DF4F1455AEF6ED8EC7AE510FD450DADC5C298`、`46CE1ED5A0AA1FADD7F025FEA07DF4F1455AEF6ED8EC7AE510FD450DADC5C298`、`9D2E6AFDFE034AA091EBB323D7501F300E72913F61AEB1BABF9A344669B3F5C1`；主配置与备份内容一致且均不含临时脚本，翻译测试凭据已清除；安装版单实例正常响应，WebView 调试端口已关闭。
- GitHub Release 已上传 `Suo_0.1.0_x64-setup.exe`（3,956,056 bytes）和 `Suo_0.1.0_x64-setup.exe.sha256`；GitHub 返回的安装包 digest 为 `sha256:7eb2981f18827edb07db2a0381188d4ea6a8b43831f5831c1ea4c12a3bf345d5`，与本地一致。

本轮使用以下精确路径与哈希门禁上传；后续重跑时不得使用 `--clobber` 覆盖同名资产：

```powershell
$installer = Resolve-Path 'src-tauri\target\release\bundle\nsis\Suo_0.1.0_x64-setup.exe'
$expected = '7EB2981F18827EDB07DB2A0381188D4EA6A8B43831F5831C1EA4C12A3BF345D5'
$actual = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
if ($actual -ne $expected) { throw "安装包 SHA-256 不匹配：$actual" }
& 'D:\Develop\gh\bin\gh.exe' release upload v0.1.0 $installer `
  'src-tauri\target\release\bundle\nsis\Suo_0.1.0_x64-setup.exe.sha256' `
  --repo dqgod/suo
```

当前验证机使用便携版 GitHub CLI `v2.98.0`，路径为 `D:\Develop\gh\bin\gh.exe`。远程 Release：<https://github.com/dqgod/suo/releases/tag/v0.1.0>。

如果发现需要代码修复：回到 `dev` 新建修复提交，重新完成两端验证并发布新版本；**不要移动 `v0.1.0` tag，也不要用不同源码覆盖同名资产。** 完成后更新本文件和 [`README.md`](README.md)，将本轮明细移入 `archive/`。
