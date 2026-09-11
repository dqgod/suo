# macOS Apple Silicon current handoff

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
