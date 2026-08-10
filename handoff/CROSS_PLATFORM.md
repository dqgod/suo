# Cross-platform lessons

这些问题来自 Windows 与 macOS 之间的真实交接，后续 agent 应把它们当作发布前检查，而不是只在失败后排查。

## 1. 同一台机器上的混合架构工具链

macOS 验证机同时安装了 Intel 与 Apple Silicon 工具链。默认 shell 曾解析到 x64 Node 26，Rollup 因而查找 `@rollup/rollup-darwin-x64`；随后 Cargo 也曾生成可运行但错误的 `x86_64` 产物。代码测试可能全部通过，最终架构仍然错误。

处理原则：

- 在安装依赖和发布构建前同时检查 Node 架构、Rust host 与目标产物；
- macOS 使用 `node -p process.arch`、`rustc -vV` 和 `file`；Windows 使用 `node -p "process.arch"`、`rustc -vV` 以及 PE/构建目标检查；
- native module 缺失时先修正 PATH/架构，再执行 `pnpm install --frozen-lockfile`；不要先删除 lockfile；
- 不要把 Rosetta/x64 构建“能启动”当作 Apple Silicon 验收，也不要把错误 Windows target 的可编译结果当作发布产物。

## 2. Windows `link.exe` 冲突

Git for Windows 也可能提供名为 `link.exe` 的程序。若它在 PATH 中优先于 MSVC linker，Tauri/Rust 原生构建会出现误导性链接错误。

- 使用 Visual Studio Developer PowerShell；
- 构建前执行 `Get-Command link.exe | Format-List Source`；
- 修复 PATH，而不是修改 Rust 代码绕过链接器错误。

## 3. 平台外观资源必须隔离

- macOS 菜单栏使用 `src-tauri/icons/tray-macos-template.png` 并启用 template mode；Windows 继续使用彩色默认应用图标。
- macOS 的无边框透明窗口必须保留 `tauri.conf.json > app.macOSPrivateApi=true`；仅写 `transparent=true` 会被 Tauri 忽略并在 CSS 圆角外露出白色原生直角。该私有 API 不能用于 Mac App Store 分发，但当前 MVP 不包含该渠道；Windows 不依赖此开关。
- macOS Dock 可见性通过 `src-tauri/src/dock.rs` 隔离；`showDockIcon=true` 只表示设置窗口打开期间显示，设置关闭和正常后台运行时必须隐藏，实时关闭开关不能连带隐藏设置窗口。Windows 不应把同一字段解释为 taskbar 隐藏。
- Windows taskbar 角色通过 `src-tauri/src/taskbar.rs` 隔离：`main` 搜索窗口跳过任务栏，`settings` 设置窗口保留任务栏入口；不得把这项策略泄漏到 macOS Dock。
- Dock 图标、菜单栏图标、搜索框 Logo 与设置页 Logo 是不同呈现位置，不要通过替换同一个资源顺带改变全部位置。
- 平台修复必须放在 adapter 或 `cfg(target_os = ...)` 后面，避免未使用 import、错误 API 或行为泄漏到另一平台。
- Windows 开发阶段曾让 `std::env` import 和仅 Windows 使用的参数暴露到 macOS 编译；此类 warning 应通过精确 `cfg` 或明确的跨平台接口处理，不要长期留在共享模块。

## 4. 配置版本不能只依赖 serde 默认值

新增持久字段后必须升级配置版本，否则旧二进制可能读取同版本的新文件并覆盖未知行为。

- v8：查询防抖；
- v9：启动器宽高与位置；
- v10：macOS Dock 可见性。
- v11：可配置全局快捷键；v10 及更早配置使用当前平台默认值迁移。
- v12：脚本命令和网络搜索新增可选 `iconDataUrl` / `inputHint`；v11 及更早迁移为空值。图标只允许受限解码的本地 PNG/JPEG/WebP data URL，不得改成跨机器失效的本地路径或远程 URL。

每次迁移都要测试：旧文件缺少新字段、默认值正确、所有旧字段保持、更新版本拒绝被旧程序覆盖、真实 `config.json`/`.bak` 可恢复。

快捷键变更还必须按运行时事务处理：先验证并注册新组合，再停用旧组合和持久化；注册冲突或保存失败时保留/恢复旧组合。不要让设置页显示已保存但进程仍监听另一个快捷键。

## 5. 可移动配置仍需要固定引导位置

macOS 默认配置是 `~/Library/Application Support/io.github.dqgod.suo/config.json`，Windows 默认配置是 `%APPDATA%\io.github.dqgod.suo\config.json`。用户把配置迁走后，程序不能靠被迁走的 `config.json` 记录自身位置；默认目录中的 `config-location.json` 是固定、版本化的启动指针。

迁移必须持有与普通保存相同的锁，先原子写入并验证目标 `config.json`，再原子替换位置指针，最后切换内存路径。自定义目标已存在 `config.json` 或 `.bak` 时拒绝覆盖；失败继续使用旧路径；成功后旧文件作为恢复副本保留。翻译密钥仍在 Keychain / Credential Manager，不得复制进目标 JSON。Windows 必须实测含空格和非 ASCII 的目录、Explorer 打开以及重启读取。

## 6. macOS 应用名不等于 `.app` 文件名

`WeChat.app` 的中文“微信”可能只存在于 `zh-Hans.lproj/InfoPlist.strings`，`Lark.app` 的 Feishu/飞书可能来自 `CFBundleName`、URL scheme 或中文本地化资源。macOS 应用搜索需要把可信 Bundle 元数据作为别名，并兼容 UTF-8、UTF-16 与 plist 格式；不要为单个应用硬编码映射。Windows 继续使用开始菜单入口，不应编译或读取 macOS plist 路径。

## 7. 默认位置与工作区不是同一个公式

为防止窗口越界，不能直接改成“按 work area 重新居中”，否则有菜单栏/Dock/任务栏时默认位置会漂移。当前实现先按旧的完整显示器公式计算默认位置，再把用户偏移后的结果夹到工作区。

Windows agent 修改多显示器或 DPI 逻辑时必须保留这个顺序，并验证任务栏位于顶部、侧边或不同屏幕时的结果。

## 8. 实机状态与编译状态分开记录

- Rust/TypeScript 测试只能证明代码路径；Dock、菜单栏、任务栏、全局快捷键、焦点、窗口透明区域和进程树需要真实平台验证。
- 视觉问题要保存截图；OS 级显隐可同时记录 LaunchServices/进程类型等系统证据。
- 无法自动化的项目明确标记“待人工观察”，不要用静态资源或单元测试代替最终观感结论。

## 9. 用户配置与密钥

- 真实配置测试前复制主文件与 `.bak` 并记录校验值；测试结束后恢复，或明确说明保留了哪次迁移。
- Microsoft Translator 密钥只进入操作系统凭据库；任何平台都不得把密钥写进 JSON、日志、截图、fixture 或 handoff。
- 不要为了绕开 macOS 权限提示扩大受保护目录扫描，也不要为了 Windows 调试降低路径/图标来源校验。
