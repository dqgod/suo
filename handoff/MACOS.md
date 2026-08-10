# macOS Apple Silicon handoff

状态：**已完成（2026-08-10）**。

## 已完成硬件基线

- [x] Apple Silicon `arm64`、macOS 15.6.1（24G90）首轮真实硬件验证。
- [x] `pnpm build`、全部 Rust 测试、`tauri build --no-bundle` 与 `.app` bundle 构建。
- [x] 最终主程序和 `Suo.app/Contents/MacOS/suo` 均由 `file` 确认为 `arm64`。
- [x] `/Applications`、`/System/Applications`、`~/Applications` 应用发现、原生 `.app` 图标与打开行为。
- [x] Spotlight `mdfind -name`、快速输入取消与限定目录回退。
- [x] Python、Bash、executable argv、超时与进程组取消。
- [x] 单实例、`Esc`、失焦、重复显隐、透明圆角与 Retina 缩放。

## 2026-08-09 `dev` 增量

- [x] v7 → v8：空/非空查询防抖默认 `0/50 ms`，原字段保留。
- [x] v8 → v9：宽度跟随皮肤、高度 `520`、偏移 `0/0`，原字段保留。
- [x] v9 → v10：`showDockIcon=true`，原字段保留。
- [x] 通用设置宽高与位置范围、`800 × 600`、`1200 × 320`、越界夹紧和紧凑 `74 px`。
- [x] 选中结果左侧 inset 强调条已从真实启动器与外观预览移除。
- [x] macOS 菜单栏使用独立 44×44 透明 template 图标；Windows 彩色资产未改。
- [x] Dock 开关实时完成 `Foreground → UIElement → Foreground`，隐藏状态重启后保持。
- [x] 最终 Rust 测试 69 项通过，用户配置与 `.bak` 在验证后按 SHA-256 原样恢复。

## 2026-08-10 v11 增量

状态：**已完成（2026-08-10）**。

- [x] 配置 v10 → v11 的平台默认快捷键迁移、规范化和非法组合拒绝已有单元测试。
- [x] 固定 HTTP/HTTPS URL 不含占位符时可只输入关键词生成可执行结果；动态模板的缺参行为保持。
- [x] 已安装 `WeChat.app` / `Lark.app` 的 Bundle 名称、URL scheme、UTF-8/UTF-16 中文本地化名称读取通过实机测试；`微信`、`weixin`、`飞书` 的匹配链路已有集成测试。
- [x] 使用原生 arm64 Node 24.14.0 / Rust 1.97.1 完成前端、83 项 Rust 测试（另有 1 项安装 Bundle 测试按预期忽略）、all-targets check、no-bundle 和 `.app` 构建；两个最终二进制均由 `file` 确认为 `arm64`。
- [x] 在真实设置页录制 `Ctrl+Shift+K`，后端注册成功、启动器显示 `Ctrl + Shift + K 已就绪`，配置规范化保存为 `shift+control+KeyK`。验收后主配置与 `.bak` 已按 SHA-256 原样恢复。
- [x] 在真实启动器分别输入 `微信`、`weixin`、`飞书`，均命中 `/Applications` 下对应的 `WeChat.app` / `Lark.app` 并生成可执行的打开动作；未实际启动第三方应用。
- [x] 新建关键词 `mydoc`、固定链接 `https://bytedance.feishu.cn/drive/home/` 后，只输入关键词即生成“打开 飞书文档”结果；验收停在 Enter 前，未改变浏览器状态。
- [x] 通用设置显示默认配置文件 `/Users/bytedance/Library/Application Support/io.github.dqgod.suo/config.json`，“打开文件夹”由 Finder 正确打开对应目录。
- [x] 使用原生目录选择器迁移到 `/private/tmp` 下的空测试目录；目标生成 v11 `config.json`，默认目录生成 v1 `config-location.json`，迁移前默认位置的主配置与 `.bak` 哈希保持不变。
- [x] 退出并重新启动真实 `.app` 后仍显示并读取自定义位置；点击“恢复默认”并再次重启后恢复默认路径，自定义位置文件继续保留。
- [x] 验收结束后正常退出 Suo，主配置与 `.bak` 分别按 SHA-256 `dc6572a8...bd9`、`251374dc...85a` 原样恢复；测试产生的位置指针已移入临时备份目录，默认目录恢复为无指针状态。

## 2026-08-10 v12 增量

状态：**已完成（2026-08-10）**。

- [x] 配置 v11 → v12 为每个脚本命令和网络搜索增加可选 `iconDataUrl`、`inputHint`；旧配置迁移后字段为空，既有命令和其他设置保持。
- [x] 图标仅接受本地 PNG/JPEG/WebP data URL；前后端共同限制 256 KiB、单边 512 px 和总像素数，并拒绝伪造 MIME、畸形 base64 与超限图片。
- [x] 真实设置页为默认时间戳脚本选择 128×128 PNG，并输入自定义提示；保存后真实启动器运行 `ts 1786082576069`，结果左侧显示所选图标且脚本输出正常。
- [x] 真实设置页为 Google 网络搜索选择同一图标并设置“输入 Google 搜索关键词”；输入 `google` 时提示结果显示自定义文案和图标，输入 `google codex` 时可执行结果继续显示该图标；未按 Enter，因此未改变浏览器状态。
- [x] “打开设置时显示 Dock 图标”开启后仅在设置窗口可见期间切换为 `Foreground`，关闭设置后回到 `UIElement`；关闭偏好时默认一直是 `UIElement`。
- [x] 在设置窗口中实时关闭 Dock 开关后，设置窗口经过即时、80 ms 与 1.1 s 延迟回调仍保持可见和可操作，没有因 Dock 隐藏而退出。
- [x] Tauri `app.macOSPrivateApi` 已开启，使主窗口的 `transparent=true` 真正在 NSWindow/WKWebView 层生效；紧凑与完整启动器的四角不再露出白色矩形底。该实现不兼容 Mac App Store，符合当前发布范围。
- [x] 使用原生 arm64 Node 24.14.0 / Rust 1.97.1 完成前端构建、90 项 Rust 测试（89 通过、1 项安装 Bundle 集成测试按预期忽略）、no-bundle 和 `.app` 构建；最终二进制均为 `arm64`。
- [x] 验收后正常退出真实 `.app`；用户 `config.json` 与 `.bak` 已分别恢复到 SHA-256 `d16cbc40...32bc`、`71da414b...d2f`，默认配置目录仍无 `config-location.json`。

## 仍需人工观察或在不同机器补测

- [ ] SystemUIServer 无法通过当前辅助功能接口稳定截图；菜单栏模板图标在浅色/深色菜单栏上的最终视觉仍建议肉眼检查。
- [ ] 本机 `Command+Space` 可直接注册，因此没有复现系统 Spotlight 占用时的冲突提示；需要在保留 macOS 默认 Spotlight 快捷键的机器补测。
- [ ] 当前辅助功能自动化不能向其他应用注入系统级全局快捷键；本轮已验证录制、注册返回、运行状态和持久化，真实 `Ctrl+Shift+K` 唤起仍建议人工按键回归。
- [ ] 多显示器不同缩放比例下的最终位置建议再做一次真实拖屏/唤起检查；纯函数和单屏工作区夹紧已经通过。
- [ ] Signing、notarization、DMG、自动更新和登录启动仍在 MVP 范围外。

以上项目没有完成时必须保持“待补测”，不要把编译通过描述为真实硬件通过。

## 后续 macOS 回归入口

```bash
git switch dev
git pull --ff-only origin dev
uname -m
sw_vers
node --version
node -p process.arch
pnpm --version
rustc -vV
cargo --version
xcode-select -p
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --no-bundle
pnpm tauri build --bundles app
file src-tauri/target/release/suo
file src-tauri/target/release/bundle/macos/Suo.app/Contents/MacOS/suo
```

必须从 `src-tauri/target/release/bundle/macos/Suo.app` 启动真实 bundle；不要只运行单元测试或裸二进制就宣称窗口、Dock、菜单栏或 Keychain 已验证。不要使用 `sudo` 运行 Suo 或脚本测试。
