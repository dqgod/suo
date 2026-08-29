# Windows `v0.1.0` validation handoff

状态：**待 Windows x64 真实编译与回归。** macOS 已从相同 tag 生成 arm64 Pre-release；Windows 通过后可把安装包追加到同一个 GitHub Release。

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

- [ ] 从真实 v14 配置启动，确认 v15 的 `startAtLogin` 默认关闭，其他字段保持，加载迁移不会自动覆盖原文件。
- [ ] 打开开机自启并保存，确认 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 写入带引号的完整 EXE 路径和 `--autostart`；登录启动后只保留托盘/快捷键，不主动显示搜索窗口；关闭开关后 Run 项移除。
- [ ] 录制当前已生效组合时只完成录制、不唤起搜索框；录制 `Alt+Space` 不弹系统菜单。冲突或保存失败必须继续保留旧快捷键。
- [ ] `Alt+Space` 搜索窗口始终不进任务栏；设置窗口进入任务栏且可切回；关闭设置后任务栏入口消失，彩色托盘图标持续存在。
- [ ] 完全退出后冷启动，第一次快捷键直接显示在保存的尺寸与位置，不先闪现默认中心帧。

### 最新共享 UI 修复

- [ ] 快速连续输入无匹配字符，状态区不得在“正在加载/等待输入”和“已索引/没有匹配结果”之间逐键交替；普通查询应保持上一稳定帧并一次更新。
- [ ] 配置一个超过 250 ms 的即时脚本或慢查询，确认真正等待时仍会显示延迟加载反馈。
- [ ] 等待新查询期间点击旧结果或按 Enter 不得执行旧动作；最新结果返回后点击和 Enter 恢复正常。

### 核心跨平台回归

- [ ] `微信`、`weixin`、`飞书` 命中真实安装应用并显示原生图标；Everything 主路径、无 Everything 回退和普通文件搜索均正常。
- [ ] 固定 URL 关键词 `mydoc` 可直接打开；带 `{query}` / `{query0}` 的模板正确展开，缺参仍产生不可执行错误。
- [ ] 脚本与网络命令的自定义图标、空参数提示、移除恢复和非法图片拒绝均正常。
- [ ] 共用 `fy` 可切换 Microsoft、Google、有道；三家 Credential Manager 项彼此独立，任何 JSON、日志、截图或交接文档都不得出现密钥。
- [ ] `open_file <目录>` 第一次 Enter 只生成返回命令，第二次才通过非交互 PowerShell 执行；同一 action 只能消费一次，查询变化后立即失效。
- [ ] 配置路径显示清晰；迁移到含空格/中文的空目录、重启读取、恢复默认和失败回滚均通过，凭据不随 JSON 移动。

## 4. 不要重复的跨平台问题

- 不得把 macOS `NSPanel`、AppKit 队列、Dock、Spotlight、plist 或 template 图标编进 Windows 路径。
- 不得把 `showDockIcon` 解释为 Windows taskbar 开关；Windows 的搜索/设置窗口角色由独立 adapter 控制。
- 遇到 native module 缺失先检查 Node 架构和 PATH，不删除 lockfile；遇到链接错误先检查 MSVC `link.exe`。
- Windows-only import、参数和常量必须正确放入 `cfg(windows)`，不得让 macOS 再出现未使用告警。
- Windows Graphics Capture 的 `SetIsBorderRequired ... 0x80004002` 是已知自动化工具限制，不能替代真实肉眼验收，也不等于产品窗口失败。

## 5. Release 资产与回报

全部通过后记录 Windows 版本、Node/pnpm、Rust host、MSVC linker、测试数、PE 架构、真实场景、配置恢复结果和安装包 SHA-256。若无需代码修复，可把 NSIS 安装包追加到现有 Pre-release：

```powershell
$installer = Get-ChildItem src-tauri\target\release\bundle\nsis\*.exe | Select-Object -First 1
Get-FileHash $installer.FullName -Algorithm SHA256
gh release upload v0.1.0 $installer.FullName
```

如果发现需要代码修复：回到 `dev` 新建修复提交，重新完成两端验证并发布新版本；**不要移动 `v0.1.0` tag，也不要用不同源码覆盖同名资产。** 完成后更新本文件和 [`README.md`](README.md)，将本轮明细移入 `archive/`。
