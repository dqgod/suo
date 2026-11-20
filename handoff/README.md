# Cross-machine handoff

本目录只保存当前跨平台交接状态和仍需执行的验证。长期规则见根目录 [`AGENTS.md`](../AGENTS.md)，已经完成的逐版本执行记录已移入 [`archive/`](archive/README.md)，不再作为新一轮操作清单。

最后更新：2026-11-20。当前 `dev` 为产品 0.1.4 / 配置 v20。macOS 版本基于 `57e276f`，本轮自动化结果、安装包和回退位置见 [macOS 交接](MACOS.md)。Windows 的 NSIS 与 66 项 UI 回归记录对应 `e36a4a9`，需要用本轮提交重跑。公共发行状态须在发布前重新核对。

## 当前状态

| 范围 | 状态 | 下一步 |
| --- | --- | --- |
| `dev` 0.1.4 / 配置 v20 | **macOS 修复版已通过 74 项 UI 回归；Windows 待同步本轮修复** | 先读 [`SETTINGS_REDESIGN.md`](SETTINGS_REDESIGN.md)，再按 [`MACOS.md`](MACOS.md) / [`WINDOWS.md`](WINDOWS.md) 的当前清单验收。 |
| `dev` 0.1.4 / 配置 v18 | **macOS 历史定向验证** | 当时的 arm64 本地 ZIP 与 `/Applications/Suo.app` 验证只覆盖 v18；原有证据保留在 [`MACOS.md`](MACOS.md)，不得给 v20 复用结论。 |
| macOS Apple Silicon `v0.1.3` | **历史构建已归档，当前 Release 缺失** | 不可变 tag 和历史验证证据仍在；当前本机已安装 0.1.4。双物理显示器及旧配置/脚本矩阵待办见 [`MACOS.md`](MACOS.md)。 |
| Windows x64 `v0.1.3` | **待构建与验证** | 如仍要发布 0.1.3 Windows 包，从 tag 对应提交 `b027d774a6aa9aa61fea3f325e221f34e3dc7735` 构建并完成 [`WINDOWS.md`](WINDOWS.md) 清单；先明确 Release 恢复/新建计划。 |
| 跨平台约束 | **持续有效** | 修改平台代码前阅读 [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)，不得为一端编译而削弱另一端行为或安全边界。 |

## 当前交接文件

- [`SETTINGS_REDESIGN.md`](SETTINGS_REDESIGN.md)：v20 设置改版行为、配置迁移与跨平台验收矩阵。
- [`WINDOWS.md`](WINDOWS.md)：`dev` v20 自动化状态及 Windows 实机待办；保留 v18 与 `v0.1.3` 历史入口。
- [`MACOS.md`](MACOS.md)：`dev` v20 待验步骤；旧 v18 本地包与定向实机证据明确标为历史。
- [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)：仍然有效的平台隔离、配置迁移、焦点、窗口时序和工具链经验。
- [`archive/`](archive/README.md)：已经完成或被新版本接替的逐版本执行证据，仅供追溯，不应整份照搬执行。

## 维护规则

1. 开始平台工作前先读 `AGENTS.md`、本文件、目标平台文件和 `CROSS_PLATFORM.md`。
2. 当前文件只保留“待验证”“阻塞”或最新“已完成”状态；完成后把过时的逐步日志移入 `archive/`。
3. 配置迁移测试前备份真实 `config.json`、`.bak` 和位置指针并记录哈希；测试后恢复或明确记录保留结果。
4. 不提交密钥、token、用户查询、截图、用户配置、构建目录或安装包。
5. GitHub Release 的 tag 是不可变构建来源。若验证需要代码修复，不得移动 `v0.1.3` tag 或覆盖历史资产；应在 `dev` 修复并发布新版本。发布资产前重新确认目标 Release 是否存在。

## 交接完成条件

- 当前 tag/commit、操作系统、目标架构和工具链明确；
- 前端构建、Rust 测试、all-targets check 和目标平台原生构建通过；
- 快捷键、焦点、窗口、系统托盘/任务栏、脚本动作和配置迁移完成真实平台验证；
- 用户配置和系统启动项已恢复或留下可审计结果；
- 更新目标平台文件和本状态表，删除或归档已失效说明。
