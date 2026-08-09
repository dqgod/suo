# Windows agent handoff

状态：**当前 `dev` 待验证**。pre-v9 Windows 基线已完成，但 macOS 期间新增的 v9/v10 配置与界面变更还没有在 Windows 真机上闭环。

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

- [ ] 复制现有用户配置与 `.bak` 后再测试，记录校验值。
- [ ] 从不含几何字段的 v8 配置启动：宽度跟随搜索皮肤，高度为 `520`，水平/垂直偏移为 `0/0`。
- [ ] 从不含 `showDockIcon` 的 v9 配置启动：迁移到 v10 后该字段为 `true`，其他命令、服务、主题和偏好逐项保持。
- [ ] Windows 设置页面不显示 Dock 开关；保存其他设置时 `showDockIcon` 不丢失。
- [ ] 测试完成后恢复用户配置，或明确保留已经验证的 v10 迁移结果。

### 启动器尺寸与位置

- [ ] 通用设置显示宽度 `560–1200 px`、高度 `320–720 px`、水平偏移 `-400–400 px`、垂直偏移 `-240–240 px`。
- [ ] 默认 `720 × 520` 与原始居中偏上位置一致。
- [ ] 测试 `800 × 600` 以及上下限；窗口、结果区和搜索框同步调整。
- [ ] 极端偏移仍被夹在目标显示器工作区内，Windows 任务栏不会遮住窗口。
- [ ] 至少在一个不同缩放比例或第二显示器上验证逻辑像素到物理像素转换。
- [ ] 开启“空输入时仅显示搜索框”后，空输入仍收起到 `74 px`，输入后恢复配置高度。

### 图标与选中态

- [ ] Windows 托盘和任务栏继续显示彩色 Suo 图标；不得使用 `tray-macos-template.png`。
- [ ] 搜索 Obsidian/Steam/微信等应用，选中行只有背景与边框，没有左侧叠加条。
- [ ] `steam` 显示 Steam 原生图标；`weixin` 与 `wx` 命中中文“微信”并显示原生图标。
- [ ] 目录和普通文件仍使用固定目录/文件图标，应用图标缓存正常。

### Windows 原有能力回归

- [ ] 文件搜索顺序仍为：已有 Everything → 安装版内置 Everything → 限定目录索引；portable 不安装服务。
- [ ] 普通文件不获得拼音别名。
- [ ] “统一保存设置”开启时几何修改只进入草稿，点击保存后才生效；关闭时合法修改自动保存且不会被较早的保存响应覆盖。
- [ ] 搜索界面与设置界面的主题库、当前主题、导入/导出和 Logo 显隐仍彼此独立。
- [ ] 空/非空查询防抖仍分别使用配置值，即时脚本只使用自己的逐命令 delay。
- [ ] Python、PowerShell、Bash（若配置）和 executable argv 引号行为正常。
- [ ] timeout/cancel 继续终止完整 Windows Job Object 进程树。
- [ ] `Alt+Space`、单实例唤醒、`Esc`、失焦与连续显隐正常。

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
