# Cross-machine handoff

本目录只保存当前跨平台交接状态和仍需执行的验证。长期规则见根目录 [`AGENTS.md`](../AGENTS.md)，已经完成的逐版本执行记录已移入 [`archive/`](archive/README.md)，不再作为新一轮操作清单。

最后更新：2026-08-30。`v0.1.0` 已发布为 Pre-release，配置协议为 v15；Windows 打包应用目录和任务栏视觉状态修复已形成 `v0.1.1` Windows x64 发布候选，既有 tag 与资产不移动、不覆盖。

## 当前状态

| 范围 | 状态 | 下一步 |
| --- | --- | --- |
| macOS Apple Silicon `v0.1.0` | **发布候选已完成** | 以 Pre-release 发布 arm64 ZIP；构建证据、限制和剩余人工项见 [`MACOS.md`](MACOS.md)。 |
| Windows x64 `v0.1.0` | **发布候选已完成** | 已从 `v0.1.0` tag 完成 x64 构建、真实回归、安装/卸载/重装与真实注销登录验收；NSIS 安装包及 SHA-256 文件已追加到同一 Pre-release。 |
| Windows x64 `v0.1.1` | **发布候选已完成** | 已补 ChatGPT/Xbox/Microsoft Store 等打包应用发现、启动和图标，以及保持原应用 taskbar 选中外观；Windows 安装包已构建并通过自动化门禁，macOS 仍需从 `v0.1.1` tag 完成跨平台构建回归。 |
| 跨平台约束 | **持续有效** | 修改平台代码前阅读 [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)，不得为一端编译而削弱另一端行为或安全边界。 |

## 当前交接文件

- [`WINDOWS.md`](WINDOWS.md)：Windows x64 构建、真实回归、注销登录验收与 Release 资产证据。
- [`MACOS.md`](MACOS.md)：当前 macOS 发布候选的构建证据、发布格式和未关闭限制。
- [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)：仍然有效的平台隔离、配置迁移、焦点、窗口时序和工具链经验。
- [`archive/`](archive/README.md)：2026-08-11 以前的逐版本执行证据，仅供追溯，不应整份照搬执行。

## 维护规则

1. 开始平台工作前先读 `AGENTS.md`、本文件、目标平台文件和 `CROSS_PLATFORM.md`。
2. 当前文件只保留“待验证”“阻塞”或最新“已完成”状态；完成后把过时的逐步日志移入 `archive/`。
3. 配置迁移测试前备份真实 `config.json`、`.bak` 和位置指针并记录哈希；测试后恢复或明确记录保留结果。
4. 不提交密钥、token、用户查询、截图、用户配置、构建目录或安装包。
5. GitHub Release 的 tag 是不可变构建来源。若 Windows 验证需要代码修复，不得覆盖 `v0.1.0` 资产，应在 `dev` 修复并发布新版本。

## 交接完成条件

- 当前 tag/commit、操作系统、目标架构和工具链明确；
- 前端构建、Rust 测试、all-targets check 和目标平台原生构建通过；
- 快捷键、焦点、窗口、系统托盘/任务栏、脚本动作和配置迁移完成真实平台验证；
- 用户配置和系统启动项已恢复或留下可审计结果；
- 更新目标平台文件和本状态表，删除或归档已失效说明。
