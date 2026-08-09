# Windows agent handoff

状态：**已完成（2026-08-09）**。当前 `dev` 已完成 Windows x64 编译、自动化测试、全局快捷键唤起和用户手工视觉/交互验收；搜索窗口与设置窗口的任务栏角色也已验证通过。

## 本轮目标

在 Windows 上确认当前 `dev` 可编译、可运行，并验证以下新增行为没有破坏既有 Windows 路径：

- 配置 v8 → v9 → v10 迁移；
- 通用设置中的启动器宽高与位置偏移；
- 选中结果不再显示左侧 inset 强调条；
- Windows 继续使用彩色托盘/任务栏图标，不加载 macOS 单色模板图；
- macOS 专属“在 Dock 中显示图标”不出现在 Windows 设置页面，但 `showDockIcon` 字段可以安全往返保存；
- Everything、Windows 应用图标提取、拼音搜索和 Job Object 取消路径保持原样。

## 1. 环境与分支

必须在 **Visual Studio Developer PowerShell** 中执行，先确认 MSVC 工具链：

```powershell
git switch dev
git pull --ff-only origin dev
git status --short --branch
node --version
node -p "process.arch"
pnpm --version
rustc -vV
cargo --version
Get-Command link.exe | Format-List Source
```

预期：

- Node.js 22+、pnpm 11+；
- 标准 x64 Windows 开发机上 `process.arch` 为 `x64`，Rust host 为 `x86_64-pc-windows-msvc`；若机器本身是 Windows on ARM，则 Node、Rust target 与最终验收目标必须明确一致，不能混用；
- `link.exe` 必须来自 Visual Studio/MSVC，不得来自 Git for Windows。

## 2. 干净基线

```powershell
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --no-bundle
pnpm tauri dev
```

不要提交 `node_modules/`、`dist/`、`target/`、`.exe`、`.msi` 或安装包产物。

## 3. 当前增量验收

### 配置 v10

- [x] 复制现有用户配置与 `.bak` 后再测试，记录校验值。
- [x] 从不含几何字段的 v8 配置启动：宽度跟随搜索皮肤，高度为 `520`，水平/垂直偏移为 `0/0`。
- [x] 从不含 `showDockIcon` 的 v9 配置启动：迁移到 v10 后该字段为 `true`，其他命令、服务、主题和偏好逐项保持。
- [x] Windows 设置页面不显示 Dock 开关；保存其他设置时 `showDockIcon` 不丢失。
- [x] 测试完成后明确保留已验证的 v10 配置；原始 v8 配置另有临时备份。

### 启动器尺寸与位置

- [x] 通用设置显示宽度 `560–1200 px`、高度 `320–720 px`、水平偏移 `-400–400 px`、垂直偏移 `-240–240 px`。
- [x] 默认 `720 × 520` 与原始居中偏上位置一致。
- [x] 测试 `800 × 600` 以及上下限；窗口、结果区和搜索框同步调整。
- [x] 极端偏移仍被夹在目标显示器工作区内，Windows 任务栏不会遮住窗口。
- [x] 至少在一个不同缩放比例或第二显示器上验证逻辑像素到物理像素转换。
- [x] 开启“空输入时仅显示搜索框”后，空输入仍收起到 `74 px`，输入后恢复配置高度。

### 图标与选中态

- [x] Windows 托盘和任务栏继续显示彩色 Suo 图标；不得使用 `tray-macos-template.png`。
- [x] 搜索 Obsidian/Steam/微信等应用，选中行只有背景与边框，没有左侧叠加条。
- [x] `steam` 显示 Steam 原生图标；`weixin` 与 `wx` 命中中文“微信”并显示原生图标。
- [x] 目录和普通文件仍使用固定目录/文件图标，应用图标缓存正常。

### Windows 原有能力回归

- [x] 文件搜索顺序仍为：已有 Everything → 安装版内置 Everything → 限定目录索引；portable 不安装服务。
- [x] 普通文件不获得拼音别名。
- [x] “统一保存设置”开启时几何修改只进入草稿，点击保存后才生效；关闭时合法修改自动保存且不会被较早的保存响应覆盖。
- [x] 搜索界面与设置界面的主题库、当前主题、导入/导出和 Logo 显隐仍彼此独立。
- [x] 空/非空查询防抖仍分别使用配置值，即时脚本只使用自己的逐命令 delay。
- [x] Python、PowerShell、Bash（若配置）和 executable argv 引号行为正常。
- [x] timeout/cancel 继续终止完整 Windows Job Object 进程树。
- [x] `Alt+Space`、单实例唤醒、`Esc`、失焦与连续显隐正常。

## 4. 不要重复的跨平台问题

详细原因见 [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)，Windows 本轮尤其注意：

1. 遇到 Rollup/native module 缺失时，先核对 Node 架构与 PATH，不要第一反应删除 lockfile 或整个依赖目录。
2. 发布前核对最终 `.exe` 架构与 Rust host；“能编译”不等于目标架构正确。
3. 新 Windows 修复必须留在 `cfg(target_os = "windows")` 或 Windows adapter 中，不得为了编译改弱 macOS Spotlight、Dock 或模板托盘实现。
4. 不得把 macOS `set_dock_visibility` 变成 Windows taskbar 隐藏逻辑；当前产品决定只是 macOS Dock 偏好。
5. Windows-only import、常量和参数也要正确放在 `cfg(target_os = "windows")` 后面；不要把另一平台的 unused warning 当作无关噪音。
6. 不要用硬编码路径、反斜杠字符串或 Windows API 类型污染共享前端/配置协议。

## 5. 完成后回报

完成后把本文件顶部状态改为“**已完成：YYYY-MM-DD**”，勾选实际通过项，并在下方追加：

```text
Windows 版本：
架构：
Node / pnpm：
rustc host：
通过的命令：
真实场景：
未通过项（命令、退出码、stderr、截图）：
用户配置恢复/迁移结果：
```

同时更新 `handoff/README.md` 状态表和根 `README.md` 的平台验证状态。产品行为没有变化时不要改产品需求；若 Windows 修复改变了跨平台行为，再更新 `docs/PRODUCT_REQUIREMENTS.md`。

## 6. 2026-08-09 Windows 执行记录

- Windows 版本：Windows 10 Enterprise 22H2，build 19045.6332
- 架构：Node `x64`；Rust host 与最终 PE 均为 `x86_64/x64`
- Node / pnpm：Node 22.15.1；pnpm 11.16.0
- rustc host：Rust 1.97.1，`x86_64-pc-windows-msvc`
- MSVC linker：Visual Studio Community 2022，`VC/Tools/MSVC/14.41.34120/bin/Hostx64/x64/link.exe`

通过的命令与代码证据：

- `pnpm install --frozen-lockfile`、`pnpm build`、全部 68 项 Windows Rust 测试、`pnpm tauri build --no-bundle`；
- `dumpbin /headers` 确认 `src-tauri/target/release/suo.exe` 为 `8664 machine (x64)`；
- 配置迁移测试覆盖 v8 几何默认值、v9 `showDockIcon=true` 默认值、v10 范围校验和新字段双向序列化；
- 当前机器真实中文微信快捷方式的目标图标提取测试通过；全拼、首字母、普通文件不使用拼音和 top-k/cancel 测试通过；
- Windows Job Object 的继承管道后代超时终止测试通过；
- Windows 编译发现的 Unix-only 测试导入和 macOS-only `Reopen` 回调未使用警告已按平台隔离修正；`cargo check --all-targets` 无警告，仅 release 链接时保留 MSVC 生成导入库的 `linker_messages` 信息。

真实场景：

- 从备份前的 v8 用户配置启动新 v10 EXE，进程保持运行；`Alt+Space` 成功创建并显示标题为 `Suo / 梭` 的窗口；
- 启动前后真实 `config.json` 保持 v8 且 SHA-256 均为 `525DB6B588F10B18DAA98000069E85058C16D0FC6C1EB70CC140FFD16F3E0AE8`，说明只加载迁移不会擅自覆盖用户文件；原 `.bak` 哈希为 `E4A812BFB17FE28416520745198033AC44F325E325FB4BD4FB233A69230655AC`；
- 本机未运行 Everything 服务，仓库 `ES.exe` 返回 exit 8：`Everything IPC not found`。因此本轮只能确认 IPC 不可用条件和既有回退代码/测试，无法把 Everything 主路径标记为实机通过。

工具限制（不影响产品验收）：

- Windows Graphics Capture 在 Suo 透明无边框窗口上返回 `SetIsBorderRequired failed: 不支持此接口 (0x80004002)`。这是 Windows 10 22H2 未提供自动化工具所请求的可选捕获接口（`E_NOINTERFACE`），只阻止该工具截图；不影响 Suo 的显示、输入、圆角、托盘、快捷键或性能；
- 用户随后完成全部剩余手工项目，确认几何滑杆、真实尺寸、多屏/DPI、任务栏夹紧、紧凑 74 px、Dock 开关不显示、彩色托盘/任务栏图标、无左侧选中条、应用图标和设置保存模式均通过；
- `cargo clippy` 未执行：当前自定义 Rust 工具链未安装 `cargo-clippy`。这不影响已通过的编译和测试，但后续安装组件后可补跑。

用户配置恢复/迁移结果：保留已经手工验证的 v10 主配置，最终为 `720 × 520`、偏移 `0/0`、`showDockIcon=true`，SHA-256 为 `123C71374973BF58E2F33DC979067B0AD3CCC79F8451B0AFC6A11B1EDF2F543B`。当前 `.bak` 也为 v10，SHA-256 为 `C390ECB316AD497ED2589465B847B648DDD700593836D0A66ECF90D7F07D85BD`；原始 v8 主配置与 `.bak` 仍保留在系统临时备份目录。

## 7. Windows 窗口级任务栏策略

- [x] `Alt+Space` 显示搜索窗口时，任务栏不出现 Suo 图标；重复显隐也不应短暂闪现。
- [x] 从搜索结果、托盘菜单或其他入口打开设置时，任务栏显示彩色 Suo 图标，并可从任务栏切回设置窗口。
- [x] 关闭或隐藏设置窗口后，该任务栏入口消失；右下角系统托盘图标和菜单始终保留。
- [x] macOS Dock 的 `showDockIcon` 配置与行为不受 Windows taskbar 策略影响。

自动化证据：69 项 Windows Rust 测试、前端构建和 MSVC no-bundle 构建通过；真实执行 `Alt+Space → setting → Enter` 后，窗口列表由 `Suo / 梭` 切换为唯一的 `Suo 设置`。用户随后完成任务栏肉眼验收，确认以上四项全部通过。

排障说明：开发或验收期间若使用 `Stop-Process -Force`、任务管理器“结束任务”、崩溃退出，或在旧进程退出前替换构建，Windows Explorer 可能暂时保留失去宿主的通知区域图标。鼠标悬停后旧图标立即消失、且 `Get-Process -Name suo` 只有一个进程时，这是 Explorer 的“幽灵图标”缓存，不是 Suo 多实例；托盘菜单“退出”会走正常清理路径。只有悬停后仍存在多个图标或确有多个进程时，才按单实例/托盘重复创建缺陷继续排查。
