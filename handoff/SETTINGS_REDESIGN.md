# 设置改版跨平台交接（`dev` 0.1.4 / 配置 v19）

实现提交：`f6f0c25`（设置改版、极简黑、皮肤显隐与 v2 导入导出；文档更新另行提交）。

状态（2026-09-28）：Windows 前端正式构建、137 项 Rust 测试（2 项系统目录集成测试忽略）、all-targets check、格式检查及正式 NSIS 安装包构建通过。界面回归包含搜索与设置两类极简黑，最新界面回归 36/36 通过，详见下表。macOS v19 尚未编译或实机验证。2026-09-23 的 macOS 0.1.4 本地包只含 v18，不能用于验收本轮改版。设计依据是 [`docs/PRODUCT_REQUIREMENTS.md`](../docs/PRODUCT_REQUIREMENTS.md) 和 [B「分栏工具」原型](../docs/ui-settings-redesign-proposal.html)；原型不是运行时代码。

## 当前行为与不变量

- 设置页采用分栏布局。命令与服务的展开编辑器保留独立草稿；“完成编辑”先走与保存相同的 Rust 配置校验。手动模式将有效草稿纳入页面待保存配置，最后由全局“保存设置”写盘；自动模式在完成编辑时写盘。导航后保留未完成草稿并提供返回入口，关闭或丢弃时保护未保存内容。自动保存失败要明确提示并允许重试。
- 搜索与设置皮肤各有午夜、纸张、森林、极简黑四款内置项和最多 12 个自定义项。紧凑入口打开有界、可按名称搜索和按内置/自定义筛选的皮肤库；皮肤库自身滚动，浏览只更新预览，显式“应用”才更换正在使用的皮肤。新配置的设置皮肤是浅色内容、深色侧栏的 `forest`，搜索皮肤仍是 `midnight`；升级不得改写已有主题选择或自定义皮肤。
- 视觉参数的合法范围：全部字号及结果图标 1–255 px；圆角 0–255 px；结果行高 1–1024 px；主题宽度和启动器宽度 256–2560 px；搜索框宽度 128–2560 px 且不得大于该主题窗口；启动器高度 160–2160 px；水平/垂直偏移 −8192–8192 px。滑块常用区间不是保存上限；精确输入、前端严格导入、Rust 保存校验须一致。最终原生窗口按目标显示器工作区约束，不得截断存储值或破坏物理像素/DPI 换算。图片大小/格式、透明度、防抖、超时与其他安全界限不变。
- `settingsIconStyle` 的合法值是 `transparentColor`、`monochrome`、`original`，新旧配置默认前者。它只改变启动器/设置窗口内部品牌图形；Windows 彩色托盘、macOS 菜单栏与 Dock、系统应用图标不随之改变。
- v18→v19 迁移补图标偏好、为旧搜索自定义皮肤补上默认显示的 `showProviderStatus` / `showFooterHints` 并升版本；原有终端目标和 `>` 启用状态、翻译/脚本/网络搜索命令、主题选择与全部自定义皮肤保持。v19 文件缺少或含未知图标值应拒绝。旧程序必须对 v19 文件继续只读，不可回写覆盖未知字段。严格主题导入继续拒绝跨 scope、废弃的 `suo-theme-v1`、未知/缺失字段、非法图片与越界数值；完整的搜索皮肤 v1 仍可导入并补默认值，失败时工作草稿不变。

## 目前证据与缺口

| 平台 | 已确认 | 待确认 |
| --- | --- | --- |
| Windows x64 | 前端构建；UI 回归 36/36；Rust 137 通过、2 忽略；all-targets check；fmt；正式 NSIS 安装包构建。隔离实例的数值草稿、Alt+F4 关闭保护、Esc 返回保留草稿及 Alt+Space 创建启动器窗口有原生控件树证据。 | 安装/升级/卸载；原生完整视觉、鼠标操作、焦点/搜索全过程、实际 DPI/多显示器；截图与点击工具失败，不能用浏览器回归代替。 |
| macOS arm64 | v18 曾有本地安装与定向终端/字号实测，均为历史代码。 | v19 的 arm64 工具链、前端/UI/Rust、`.app` 构建及以下真实窗口验收，均未完成。 |

### 本轮验证细节

- 前端 `pnpm test:ui` 使用隔离的 Tauri 内存接口，不读取真实配置、凭据或运行用户脚本。覆盖 1180×720 / 680×520、字号 8/32/255 与非法值、命令草稿/保存失败/取消启用、皮肤库 16 项的搜索与滚动、两类极简黑的独立应用，以及实际 Launcher 组件的黑色输入框与结果区。最终 `pnpm test:ui` 36/36 通过（含实际搜索框组件的两个视口用例）。
- 极简黑（`black`）两侧均为纯黑底、灰白文字、100% 不透明、0 模糊、0 阴影；搜索侧隐藏 Logo、放大镜、来源标签、来源状态栏和底部快捷键栏，Suo 自带结果字形去掉彩色渐变，原生应用图标保留。上下两栏通过皮肤制作中的通用开关控制，边框设置为 0 / none 同时移除输入框聚焦描边；旧皮肤的两栏仍默认显示。搜索皮肤导出升级到 `suo-launcher-theme-v2` / version 2，严格旧 v1 导入会补默认值；设置皮肤仍为 v1。复制为自定义皮肤和导入后，灰度配色仍保留中性编辑控件。
- 原生检查使用 `Suo UI Validation` / `io.github.dqgod.suo.ui-validation` 独立产品名和配置路径。记录到宽度 720→680 后出现未保存状态且保存模式禁用；Alt+F4 出现“保留这次修改？”；Esc 返回后仍为 680；Alt+Space 后窗口列表新增启动器。未将这些观测描述为原生视觉全量通过。
- Windows 截图工具返回 `FrameArrived timed out` / `window capture timed out`，点击返回 `coordinate input geometry is unavailable`；后续启动器输入尝试返回 `window is not a usable app window`，因此真实搜索输入、鼠标和焦点全过程未验收。窗口工具第一次启动还误匹配已安装的 Suo；终止该正式实例被自动审批拒绝，随后改用明确绝对路径启动隔离副本，未对正式实例进行测试性编辑。
- 本地浏览器截图在忽略的 `test-results/` 和 `logs/ui-review/`；Rust/构建日志在 `logs/native-validation-black/`。生成物和用户数据不提交。正式产物为 `src-tauri/target/release/suo.exe`，不能将 `logs/native-validation-20260928/qa-app/` 的旧验证副本当作最终产物。

## macOS 接手顺序

1. 先读 [`README.md`](README.md)、[`CROSS_PLATFORM.md`](CROSS_PLATFORM.md)、[`MACOS.md`](MACOS.md)。检查 `git status`，保留当前本地修改后再 `git fetch origin --tags`、切到 `dev`，仅在可快进时更新到 `origin/dev`；不要丢弃本地修改、移动旧 tag 或从 v18 安装包验证 v19。
2. 核对 `node -p process.arch` 为 `arm64`、`rustc -vV` 的 host 为 `aarch64-apple-darwin`；构建完成后用 `file` 核对 Mach-O。混合 Rosetta 环境先修正工具链架构，再安装锁文件依赖。
3. 运行 `pnpm install --frozen-lockfile`、`pnpm build`、`pnpm test:ui`、`cargo test --manifest-path src-tauri/Cargo.toml --locked`、`cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets`、`pnpm tauri build --bundles app`。当前 `playwright.config.mjs` 在 macOS 使用 Playwright Chromium（Windows 才用 Edge channel）；若提示缺浏览器，先运行 `pnpm exec playwright install chromium` 再重试 UI 测试。UI 用例覆盖桌面 1180×720 和最小 680×520 视口，但不替代原生窗口实测。
4. 从真实 `.app` 经 LaunchServices 冷启动，在 680×520 和常规窗口、浅/深皮肤中逐项检查：设置页导航与命令分栏编辑、皮肤库搜索/筛选/独立滚动、预览与应用边界、搜索框和设置页分别应用极简黑、8/32 px 字号和长文换行、极端宽高/偏移的工作区夹紧、不同缩放显示器上的几何、模糊/透明与背景图。窗口内图标三种样式的标题栏/侧栏/搜索 Logo 一致，菜单栏模板图标与 Dock 不受偏好影响。
5. 同一 `.app` 实测 Command+Space/自定义快捷键、搜索焦点、失焦、Dock 在设置打开期间显隐、菜单栏、原生全屏 Space；有第二物理显示器时复核鼠标目标屏与混合 DPI。关闭有未保存命令、皮肤或通用设置的窗口时检查原生关闭请求弹出的 dirty 对话框；自动/手动保存、失败重试、取消编辑和重启持久化都要分别观察。原有 `>` 命令的显式激活、一次性 token 与无提权边界仍需回归。
6. 真实配置迁移前备份默认/当前 `config.json`、`.bak` 与 `config-location.json` 并记录哈希，不在日志、截图、handoff 中输出密钥或完整私密内容。用副本测 v18→v19、旧版只读保护、原命令/终端/主题保留，再恢复并核对哈希。不要让 v18 二进制写 v19 文件，不要触碰生产配置来测试隔离构建。

记录完成时写明 commit、OS、Node/Rust 架构、可执行文件架构、命令结果、截图/日志位置和仍待人工观察项；只把实际完成的项标为通过。发布仍需单独核对签名、公证、Gatekeeper 与 Release 状态，不能把本地 ad-hoc 包当作公网发行包。

## 本轮正式 Windows 构建

- 安装包：`src-tauri/target/release/bundle/nsis/Suo_0.1.4_x64-setup.exe`，4,087,087 bytes，x64，产品名/版本 `Suo` / 0.1.4。
- 安装包 SHA-256：`A75A06793793A34E6A7467E91F10BB8417674603E224F9690A40822AF1925BC1`。
- 包内程序：`src-tauri/target/release/suo.exe`，15,683,072 bytes，x64，产品名 `Suo`，产品版本 0.1.4，标识 `io.github.dqgod.suo`。NSIS 打包会为程序写入 bundle 类型信息，故与此前 no-bundle 程序哈希不同。
- 程序 SHA-256：`8337E758F5ABF9CDDA8A35B1675452E8577057CDB57736702FA0FB5A8A1E7B74`。
- 最后构建日志：`logs/native-validation-final/tauri-nsis-build-proxy.log`，包括前端构建成功、官方 NSIS 工具下载哈希验证及 `makensis` 成功；同目录的 `artifact-manifest.txt` 记录版本、哈希及 NSIS 脚本中主程序与五个 examples 资源的路径。未运行安装包，未独立解包验证。
- Rust 测试日志 `logs/native-validation-black/cargo-test.log` 为 137 通过、2 忽略；格式检查通过，最新 all-targets check 日志在 `logs/native-validation-final/cargo-check-all-targets.log`。
- 本轮推送 `dev` 源码和文档，并应用户要求生成上述本地 Windows NSIS 安装包；不发布 GitHub Release，不向 Git 提交可执行文件。Mac 从最新 `origin/dev` 自行构建并执行上述清单；旧 0.1.4/v18 本地包不含这些改动。
