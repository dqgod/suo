# Cross-machine handoff

本目录只保存当前跨平台交接状态和仍需执行的验证。长期规则见根目录 [`AGENTS.md`](../AGENTS.md)，已经完成的逐版本执行记录已移入 [`archive/`](archive/README.md)，不再作为新一轮操作清单。

最后更新：2026-09-11。[`v0.1.3`](https://github.com/dqgod/suo/releases/tag/v0.1.3) 是当前最新 Pre-release，配置协议仍为 v17；macOS arm64 已发布原生全屏 Space 覆盖修复，Windows x64 必须从同一不可变 tag 重新构建并追加资产。已经完成的构建、实机和旧版本流水均已移入 [`archive/`](archive/README.md)，当前平台文件只保留未完成项。

## 当前状态

| 范围 | 状态 | 下一步 |
| --- | --- | --- |
| macOS Apple Silicon `v0.1.3` | **资产已发布** | arm64 ZIP、SHA-256、最终解包冷启动和单显示器原生全屏实机验证已完成；只保留双物理显示器、v17 真实迁移/脚本矩阵及原生集成待办，见 [`MACOS.md`](MACOS.md)。 |
| Windows x64 `v0.1.3` | **待构建与验证** | 从提交 `b027d774a6aa9aa61fea3f325e221f34e3dc7735` 的不可变 tag 构建 NSIS，执行 [`WINDOWS.md`](WINDOWS.md) 当前清单后向同一 Release 追加资产。 |
| 跨平台约束 | **持续有效** | 修改平台代码前阅读 [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)，不得为一端编译而削弱另一端行为或安全边界。 |

## 当前交接文件

- [`WINDOWS.md`](WINDOWS.md)：`v0.1.3` Windows x64 当前构建、真实回归和 Release 追加清单。
- [`MACOS.md`](MACOS.md)：`v0.1.3` macOS 仍未关闭的多显示器、v17 真实迁移和原生集成验证。
- [`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)：仍然有效的平台隔离、配置迁移、焦点、窗口时序和工具链经验。
- [`archive/`](archive/README.md)：已经完成或被新版本接替的逐版本执行证据，仅供追溯，不应整份照搬执行。

## 维护规则

1. 开始平台工作前先读 `AGENTS.md`、本文件、目标平台文件和 `CROSS_PLATFORM.md`。
2. 当前文件只保留“待验证”“阻塞”或最新“已完成”状态；完成后把过时的逐步日志移入 `archive/`。
3. 配置迁移测试前备份真实 `config.json`、`.bak` 和位置指针并记录哈希；测试后恢复或明确记录保留结果。
4. 不提交密钥、token、用户查询、截图、用户配置、构建目录或安装包。
5. GitHub Release 的 tag 是不可变构建来源。若 Windows 验证需要代码修复，不得移动 `v0.1.3` tag 或覆盖现有 macOS 资产，应在 `dev` 修复并发布新版本。

## 交接完成条件

- 当前 tag/commit、操作系统、目标架构和工具链明确；
- 前端构建、Rust 测试、all-targets check 和目标平台原生构建通过；
- 快捷键、焦点、窗口、系统托盘/任务栏、脚本动作和配置迁移完成真实平台验证；
- 用户配置和系统启动项已恢复或留下可审计结果；
- 更新目标平台文件和本状态表，删除或归档已失效说明。
