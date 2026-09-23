# macOS Apple Silicon current handoff

## `dev` 0.1.4 / 配置 v18 待验证

本轮新增可关闭的 `>` 内置终端命令，以及由设置皮肤基础字号派生的命令摘要/外观编辑器组件字号。发布基线仍是下文不可变的 `v0.1.3`；这些改动只能在 `dev` 验证，不能覆盖旧 tag。

Windows 端已完成共享前端构建、Rust 132 项单元测试与 all-targets check，但 `cfg(target_os = "macos")` 的 `/usr/bin/open`、文件权限、清理行为和真实终端兼容性仍必须在 macOS 编译并实测，不能用 Windows 通过替代。

本机后台验证（2026-09-23，起点 `dev` `1917ea0`）：Node 与 Rust host 均为 arm64；前端构建、Rust 测试（127 通过、1 项本机 Bundle 集成测试跳过）、all-targets check 和 macOS `.app` 构建通过，产物为 arm64。新增真实文件系统测试确认过期 `suo-terminal-*` 文件清理不会删除近期文件或无关文件。Tauri 生成的 `.app` 原始 ad-hoc 签名未通过严格校验，本机重新签名后通过 `codesign --verify --deep --strict`；这不代表公网下载可通过 Gatekeeper。用户要求暂停前台操作，本轮没有完成 `>`、设置页和终端的图形实测；真实 v17 配置及 `.bak` 哈希保持不变。

- [ ] 在普通用户权限启动 `dev` arm64 构建，输入 `>` 确认只有无动作提示；输入 `> pwd`、`> printf '你好\n'` 但不回车时不得打开终端，按 Enter 后应通过默认 Terminal 以 `/bin/bash` 执行，工作目录为 `$HOME`，完成后窗口保持可见。
- [ ] 在“设置 → 命令与服务 → 内置命令”填写至少一个机器上已安装且支持打开 `.command` 的其他终端应用名称或绝对 `.app` 路径并实测；无效应用要给出明确错误并清理临时文件。关闭/开启、手动统一保存和自动保存都要重启复核。
- [ ] 验证应用缓存 `terminal-commands` 权限为 `0700`，载荷 `.sh` 为 `0600`、包装 `.command` 为 `0700`；执行成功、`open` 失败后均无本轮残留。人为准备仅属于 `suo-terminal-*` 且超过 24 小时的隔离测试文件，确认只清理过期自有文件；不得使用 `sudo` 或触碰真实无关文件。
- [ ] root 运行路径用自动化/隔离方式验证拒绝，不要以 root 启动真实 GUI 会话；快速变更查询、关闭窗口、保存终端目标后旧结果不能执行，同一结果第二次激活失败，默认日志不出现完整命令或终端输出。
- [ ] 用 v17 配置副本迁移到 v18：无冲突时默认启用 Terminal；脚本、网络搜索、翻译关键词或别名已使用 `>` 时必须原样保留用户项并关闭内置命令；新版本配置仍保持旧版本只读保护。
- [ ] 把设置皮肤基础字号分别调到 12、14、20 px，验证脚本/网络搜索/服务/内置命令摘要及外观编辑器步骤、说明同步变化且核心操作不溢出；默认 14 px 时说明文字不小于 12 px。右侧主题预览仍使用被预览皮肤自身字号。

状态：**`v0.1.3` macOS Apple Silicon 资产已发布到 [GitHub Pre-release](https://github.com/dqgod/suo/releases/tag/v0.1.3)。** 不可变 tag 指向 `b027d774a6aa9aa61fea3f325e221f34e3dc7735`；产品版本 0.1.3，配置协议 v17，支持 macOS 13+ arm64。完整构建、实机、配置恢复和资产哈希已经移入 [`archive/MACOS_V0.1.3_2026-09-11.md`](archive/MACOS_V0.1.3_2026-09-11.md)，当前文件只保留未完成项。

## 当前待验证

- [ ] 连接第二块物理显示器，在 A/B 两屏分别准备普通窗口 Space 与原生全屏应用 Space；把鼠标移到目标屏幕后触发快捷键，确认搜索框始终出现在该屏当前 Space 的全屏应用上层。两屏缩放不同时还要确认位置、尺寸和工作区夹紧正确，关闭后不切换 Space、不退出全屏、不激活 Suo。
- [ ] 用真实 v15、v16 配置副本完成一次 v17 迁移：只迁移内置 `timestamp-example` 路径，只在 `qr` 关键字/别名与 ID 均未占用时新增默认命令，用户脚本目录中的已有文件不得被覆盖；测试前后恢复真实配置并核对哈希。
- [ ] 真实验证 `SUO_RESULT:text:`、`image:`、`qrcode:`：中文多行复制、完整图片缩略图、raw PNG Base64、非法/超限图片拒绝、二维码 UTF-8 扫码与 500/501-byte 边界；图片/二维码不得进入二次 Shell 执行。
- [ ] 在设置页验证 `ts`、`qr` 的 Finder 定位，以及脚本/网络搜索删除确认在手动保存和自动保存两种模式下的持久化行为。
- [ ] 真实验证 Dock 设置页显隐、菜单栏模板图标、失焦/Esc、单实例、`startAtLogin` LaunchAgent 创建、登录后台启动和关闭清理；不得使用 `sudo`。
- [ ] Developer ID 签名、公证和 DMG 仍未提供；当前 ZIP 只有链接器 ad-hoc 签名，不得宣称可绕过 Gatekeeper 或支持 Mac App Store。

## 不可变来源与复验命令

```bash
git fetch origin --tags
git switch --detach v0.1.3
git rev-parse HEAD
git rev-list -n 1 v0.1.3
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
pnpm tauri build --bundles app
file src-tauri/target/release/bundle/macos/Suo.app/Contents/MacOS/suo
```

若复验需要代码修复，回到 `dev` 提交并发布更高版本，不能移动 `v0.1.3` tag 或覆盖现有资产。ZIP 必须使用 `ditto -c -k --sequesterRsrc --keepParent` 从真实 `.app` 创建，再解包并通过 LaunchServices 冷启动；不能用裸执行 `Contents/MacOS/suo` 代替窗口验收。
