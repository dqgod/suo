# Cross-machine handoff

本目录只保存当前跨平台交接状态和仍需执行的验证。长期规则见根目录 [`AGENTS.md`](../AGENTS.md)，已经完成的逐版本执行记录已移入 [`archive/`](archive/README.md)，不再作为新一轮操作清单。

最后更新：2026-09-23。GitHub 当前可查到的最新 Release 为 [`v0.1.2`](https://github.com/dqgod/suo/releases/tag/v0.1.2)；`v0.1.3` tag 仍在，但 `gh release view v0.1.3` 返回 `release not found`，不能再按“向现有 Release 追加资产”执行。`dev` 的 0.1.4 / 配置 v18 已在 macOS arm64 完成定向实机验证并制作本地可用安装包，Windows 实机与两端其余清单仍待完成。已经完成的旧版本流水保留在 [`archive/`](archive/README.md)。

## 当前状态

| 范围 | 状态 | 下一步 |
| --- | --- | --- |
| `dev` 0.1.4 / 配置 v18 | **macOS 定向验证通过，Windows 待实机验证** | macOS 本地 arm64 ZIP 与 `/Applications/Suo.app` 已验证；一次性授权、迁移冲突和其余未完成项见 [`MACOS.md`](MACOS.md) 与 [`WINDOWS.md`](WINDOWS.md)。 |
| macOS Apple Silicon `v0.1.3` | **历史构建已归档，当前 Release 缺失** | 不可变 tag 和历史验证证据仍在；当前本机已安装 0.1.4。双物理显示器及旧配置/脚本矩阵待办见 [`MACOS.md`](MACOS.md)。 |
| Windows x64 `v0.1.3` | **待构建与验证** | 如仍要发布 0.1.3 Windows 包，从 tag 对应提交 `b027d774a6aa9aa61fea3f325e221f34e3dc7735` 构建并完成 [`WINDOWS.md`](WINDOWS.md) 清单；先明确 Release 恢复/新建计划。 |
| 跨平台约束 | **持续有效** | 修改平台代码前阅读 [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)，不得为一端编译而削弱另一端行为或安全边界。 |

## 当前交接文件

- [`WINDOWS.md`](WINDOWS.md)：`dev` 0.1.4 新能力实机清单，以及 `v0.1.3` Windows x64 构建和真实回归清单。
- [`MACOS.md`](MACOS.md)：0.1.4 本地包、安装与定向实机证据，以及仍未关闭的 macOS 验证。
- [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)：仍然有效的平台隔离、配置迁移、焦点、窗口时序和工具链经验。
- [`archive/`](archive/README.md)：已经完成或被新版本接替的逐版本执行证据，仅供追溯，不应整份照搬执行。

## 维护规则

1. 开始平台工作前先读 `AGENTS.md`、本文件、目标平台文件和 `CROSS_PLATFORM.md`。
2. 当前文件只保留“待验证”“阻塞”或最新“已完成”状态；完成后把过时的逐步日志移入 `archive/`。
3. 配置迁移测试前备份真实 `config.json`、`.bak` 和位置指针并记录哈希；测试后恢复或明确记录保留结果。
4. 不提交密钥、token、用户查询、截图、用户配置、构建目录或安装包。
5. GitHub Release 的 tag 是不可变构建来源。若 Windows 验证需要代码修复，不得移动 `v0.1.3` tag；应在 `dev` 修复并发布新版本。发布资产前重新确认目标 Release 是否存在。

## 交接完成条件

- 当前 tag/commit、操作系统、目标架构和工具链明确；
- 前端构建、Rust 测试、all-targets check 和目标平台原生构建通过；
- 快捷键、焦点、窗口、系统托盘/任务栏、脚本动作和配置迁移完成真实平台验证；
- 用户配置和系统启动项已恢复或留下可审计结果；
- 更新目标平台文件和本状态表，删除或归档已失效说明。
