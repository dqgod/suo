# Windows current release handoff

## 当前：`dev` 0.1.4 / 配置 v19

本轮设置改版、两类极简黑、数值范围与 v18→v19 迁移见 [`SETTINGS_REDESIGN.md`](SETTINGS_REDESIGN.md)。Windows x64 工具链为 VS 2022 Build Tools / Rust 1.98.1 MSVC；Build Tools 位于 `D:\Software\SuoBuildTools`。必须从 Developer PowerShell 运行原生命令并确认 MSVC `link.exe` 优先。

- [x] `cargo test --manifest-path src-tauri/Cargo.toml --locked`：138 通过、2 忽略、0 失败；包含新极简黑双 scope 往返、v19 迁移、数值边界与窗口工作区测试。
- [x] `cargo check --locked --all-targets`、`cargo fmt --check`、`pnpm build` 和正式 `pnpm tauri build --no-bundle` 通过。仅保留既有 `dock::visible_for_settings` dead_code 警告；未用测试标识构建正式产物。
- [x] `pnpm test:ui` 56/56 通过，覆盖完整设置流程和搜索/设置极简黑。最终 exe SHA-256 为 `0522DD34FBC5667CF6F60D80753ADACFF343797F3D7CB41CA55F43956219F58B`（15,680,000 bytes），产品名 `Suo`、0.1.4、正式应用标识；日志与明细见交接文档。
- [x] 正式 NSIS 安装包 `src-tauri/target/release/bundle/nsis/Suo_0.1.4_x64-setup.exe` 已生成，4,088,573 bytes；SHA-256 `99C5451C217ACDB7A23E640BB0241CD71186C26869D3C3189B07DEC0A38871D0`。构建脚本包含主程序与五个 examples 资源，未运行安装包或完成安装/升级/卸载验收。
- [x] 独立标识原生实例确认数值编辑进入草稿、保存策略锁定、Alt+F4 保护与 Esc 保留草稿、Alt+Space 唤出启动器窗口。
- [ ] 补原生截图/鼠标与搜索焦点全过程、两类极简黑观感、680×520 / 8/32 px、大尺寸和偏移、多显示器 DPI、模糊/透明、托盘/任务栏。窗口工具捕获与点击失败限制见 [`SETTINGS_REDESIGN.md`](SETTINGS_REDESIGN.md)，浏览器测试不能代替这一项。
- [ ] 用备份副本完成真实 v18→v19 迁移与重启保留；不触碰现有生产配置，不移动旧 tag；本轮按用户要求生成本地安装包，不发布 GitHub Release。

## 历史：`dev` 0.1.4 / 配置 v18 定向自动化（2026-09-12）

本轮新增可关闭的 `>` 内置终端命令，以及由设置皮肤基础字号派生的命令摘要/外观编辑器组件字号。历史代码基线是下文不可变的 `v0.1.3` tag；这些改动只能在 `dev` 验证，不能反向覆盖旧 tag。

2026-09-12 本机自动化证据：`pnpm build` 通过；Rust 132 项通过、2 项依赖真实系统应用目录而忽略；`cargo check --locked --all-targets` 与 `pnpm tauri build --no-bundle` 通过，生成 `src-tauri/target/release/suo.exe`（15,570,944 bytes；SHA-256 `C9EBD0C504F0B1E33123EA9F9BB574EA8CDBC0FD68A1ED311DFB64BD811EE7F9`）。隔离子进程已验证污染 `SystemRoot` 不会改变 Windows API 返回的系统目录，终端 token 也绑定创建时的完整配置。本轮未启动该 exe、未改真实配置，也未代替下面的终端窗口与视觉实测。当前工具链未安装 Clippy 组件，`cargo clippy` 未执行；这不是代码测试失败。

- [ ] 在普通用户权限启动 `dev` 构建，输入 `>` 确认只有无动作提示；输入 `> Get-Date`、`> ls` 但不回车时不得打开进程，按 Enter 后应打开新的可见 PowerShell、执行并保留窗口。
- [ ] 在“设置 → 命令与服务 → 内置命令”切换到 CMD，验证 `> dir` 通过系统 `cmd.exe /D /K` 执行；关闭开关后 `>` 不再作为内置命令，重新开启后恢复。手动统一保存和自动保存两种模式都要重启复核。
- [ ] 以 Windows 管理员身份启动的隔离测试必须拒绝 `>`；普通用户路径中即使 PATH 前置同名 `powershell.exe` / `cmd.exe`，也只能启动 SystemRoot 中的系统程序。不要在真实用户环境执行破坏性命令。
- [ ] 用 v17 配置副本迁移到 v18：无冲突时默认启用 PowerShell；脚本、网络搜索、翻译关键词或别名已使用 `>` 时必须原样保留用户项并关闭内置命令；新版本配置仍保持旧版本只读保护。
- [ ] 把设置皮肤基础字号分别调到 12、14、20 px，验证脚本/网络搜索/服务/内置命令摘要的名称、说明、关键词、徽标，以及外观编辑器步骤与说明都同步变化且不截断核心操作；默认 14 px 时说明文字不小于 12 px。右侧主题预览仍跟随被预览主题自身字号。
- [ ] 检查快速变更查询、关闭窗口、保存终端目标后旧结果均不能执行；同一结果第二次激活失败；默认日志不出现完整命令或终端输出。

状态：**`v0.1.3` tag 仍在，但 2026-09-23 的 GitHub Release 查询返回 `release not found`；不能按“向现有 Release 追加”执行。** tag 对应提交为 `b027d774a6aa9aa61fea3f325e221f34e3dc7735`。Windows x64 资产若仍需发布，应先完成同一不可变 tag 的构建与实机验证，再决定恢复 0.1.3 Release 或发布新版本；不能改名复用 `v0.1.2` 安装包。`v0.1.2` 历史已归档到 [`archive/WINDOWS_V0.1.2_2026-09-11.md`](archive/WINDOWS_V0.1.2_2026-09-11.md)。

## 历史 `v0.1.3` 代码范围

- 0.1.3 的行为修复仅位于 macOS `focus.rs` adapter：启动器加入原生全屏 Space。Windows 不应编译或复制 AppKit collection behavior。
- 共享产品版本已升至 0.1.3，配置协议仍为 v17，没有新增配置迁移。
- `app_icon.rs` 只把 Windows 测试使用的 `Path` import 放到精确 `cfg(target_os = "windows")` 下，不改变运行时图标行为。

## 历史 `v0.1.3` Windows 待执行

- [ ] 在 Visual Studio Developer PowerShell 中拉取 tag，确认 `Get-Command link.exe` 指向 MSVC，并核对 Node/Rust 为 x64。
- [ ] 执行锁文件安装、前端构建、全部 Rust 测试、`cargo check --locked --all-targets` 和正式 NSIS 构建；安装包及 release 可执行文件的 Product/File Version 必须都是 0.1.3、PE 架构必须是 x64。
- [ ] 备份 `%APPDATA%\io.github.dqgod.suo` 的配置、位置指针和 `scripts\`，记录哈希；安装/升级/重装后不得覆盖用户脚本，配置仍按 v17 读取。
- [ ] 真实回归全局快捷键、搜索输入焦点、原应用 taskbar 选中外观、搜索窗口不显示任务栏入口、设置窗口显示任务栏入口、彩色托盘菜单和单实例。
- [ ] 真实回归微信、ChatGPT、Xbox、Microsoft Store 等传统/打包应用搜索、原生图标与启动，以及文件/文件夹、计算器、固定链接、参数化网络搜索和 `fy` 翻译。
- [ ] 验证用户目录的 `ts`、`qr` 与临时图片脚本：UTF-8/CP936、多行复制、类型化文字/图片/二维码、500/501-byte QR、超限拒绝和二次 Shell 隔离。
- [ ] 从设置页验证 Explorer 选中脚本文件、脚本/网络搜索删除二次确认、手动/自动保存模式、配置位置迁移和恢复。
- [ ] 验证 `startAtLogin` 的当前用户启动项、真实注销登录后台启动和关闭清理；不得使用管理员权限安装服务。
- [ ] 若决定发布 0.1.3 Windows 资产，生成 `Suo_0.1.3_x64-setup.exe` 与 `.sha256`，从安装包完成安装/卸载/重装复测；发布前确认目标 Release 已恢复或创建，上传后核对远程大小和 digest。

## 历史 `v0.1.3` 构建入口

```powershell
git fetch origin --tags
git switch --detach v0.1.3
git rev-parse HEAD
git rev-list -n 1 v0.1.3
Get-Command link.exe | Format-List Source
node -p "process.arch"
rustc -vV
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
pnpm tauri build
```

发现必须修复的问题时回到 `dev` 并发布更高版本；不得移动 `v0.1.3` tag、覆盖任何已发布资产，或把 Windows-only PIDL/COM/AUMID/PowerShell 路径泄漏到 macOS。
