# Windows current release handoff

状态：**`v0.1.3` macOS arm64 资产已经发布到 [GitHub Pre-release](https://github.com/dqgod/suo/releases/tag/v0.1.3)，Windows x64 资产待从同一不可变 tag 构建、验证并追加。** tag/发布提交为 `b027d774a6aa9aa61fea3f325e221f34e3dc7735`；不能改名复用 `v0.1.2` 安装包。`v0.1.2` 历史已归档到 [`archive/WINDOWS_V0.1.2_2026-09-11.md`](archive/WINDOWS_V0.1.2_2026-09-11.md)。

## 本轮代码范围

- 0.1.3 的行为修复仅位于 macOS `focus.rs` adapter：启动器加入原生全屏 Space。Windows 不应编译或复制 AppKit collection behavior。
- 共享产品版本已升至 0.1.3，配置协议仍为 v17，没有新增配置迁移。
- `app_icon.rs` 只把 Windows 测试使用的 `Path` import 放到精确 `cfg(target_os = "windows")` 下，不改变运行时图标行为。

## Windows 待执行

- [ ] 在 Visual Studio Developer PowerShell 中拉取 tag，确认 `Get-Command link.exe` 指向 MSVC，并核对 Node/Rust 为 x64。
- [ ] 执行锁文件安装、前端构建、全部 Rust 测试、`cargo check --locked --all-targets` 和正式 NSIS 构建；安装包及 release 可执行文件的 Product/File Version 必须都是 0.1.3、PE 架构必须是 x64。
- [ ] 备份 `%APPDATA%\io.github.dqgod.suo` 的配置、位置指针和 `scripts\`，记录哈希；安装/升级/重装后不得覆盖用户脚本，配置仍按 v17 读取。
- [ ] 真实回归全局快捷键、搜索输入焦点、原应用 taskbar 选中外观、搜索窗口不显示任务栏入口、设置窗口显示任务栏入口、彩色托盘菜单和单实例。
- [ ] 真实回归微信、ChatGPT、Xbox、Microsoft Store 等传统/打包应用搜索、原生图标与启动，以及文件/文件夹、计算器、固定链接、参数化网络搜索和 `fy` 翻译。
- [ ] 验证用户目录的 `ts`、`qr` 与临时图片脚本：UTF-8/CP936、多行复制、类型化文字/图片/二维码、500/501-byte QR、超限拒绝和二次 Shell 隔离。
- [ ] 从设置页验证 Explorer 选中脚本文件、脚本/网络搜索删除二次确认、手动/自动保存模式、配置位置迁移和恢复。
- [ ] 验证 `startAtLogin` 的当前用户启动项、真实注销登录后台启动和关闭清理；不得使用管理员权限安装服务。
- [ ] 生成 `Suo_0.1.3_x64-setup.exe` 与 `.sha256`，从安装包完成安装/卸载/重装复测后，追加到现有 `v0.1.3` Pre-release；远程大小和 digest 必须与本地一致。

## 构建入口

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

发现必须修复的问题时回到 `dev` 并发布更高版本；不得移动 `v0.1.3` tag、覆盖 macOS 资产，或把 Windows-only PIDL/COM/AUMID/PowerShell 路径泄漏到 macOS。
