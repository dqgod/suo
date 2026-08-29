# macOS Apple Silicon release handoff

状态：**`v0.1.0` arm64 发布候选已完成，作为 GitHub Pre-release 分发；无 Developer ID 签名、未公证，不是正式稳定版。**

## 发布范围

- 来源：`v0.1.0` tag（tag 创建前必须与最终发布提交一致）。
- 产品版本：`0.1.0`；配置协议：v15。
- 目标：Apple Silicon / `arm64`，macOS 13+。
- 产物：`Suo_0.1.0_macos_arm64.zip` 与对应 `.sha256`。
- 分发限制：当前只有链接器生成的 ad-hoc 签名，没有 Developer ID 签名、Apple notarization 或 DMG；首次打开可能触发 Gatekeeper 提示。不得宣称支持 Mac App Store，`app.macOSPrivateApi=true` 仍是透明无边框窗口所必需。

## 本轮发布验证

- [x] `uname -m`、Node `process.arch`、Rust host 和最终 Mach-O 均核对为原生 arm64。
- [x] `pnpm install --frozen-lockfile`、`pnpm build`、全部 Rust 测试、`cargo check --locked --all-targets` 和 `.app` bundle 构建通过。
- [x] macOS 编译不再保留 Windows-only `AppHandle` 未使用告警。
- [x] 解包后的 `Info.plist` 明确声明 `LSMinimumSystemVersion=13.0`，与产品支持范围一致。
- [x] ZIP 使用 `ditto --sequesterRsrc --keepParent` 从真实 `.app` bundle 创建，发布前重新解压并核对 bundle 主程序架构。
- [x] `Suo_0.1.0_macos_arm64.zip` SHA-256：`12109c4678f0556e1c9b1d529d92587489d1ab3c41f26bb3353731125433d90c`；发布时同时上传 `.sha256`。
- [x] 从最终 ZIP 重新解压后经 Finder/LaunchServices 冷启动，进程路径确认属于解包后的 `Suo.app`；再次打开可显示搜索框，输入无副作用查询 `release-smoke-xyz` 后稳定返回“没有匹配结果”。
- [x] 对同一最终 bundle 快速替换输入为 `rapid-smoke-abcdef`，在 80 ms、260 ms 和 910 ms 三个观察点均未出现“正在加载/稳定状态”交替，输入框焦点持续保留。

## 已知限制与后续人工项

- `startAtLogin` 的 LaunchAgent 创建、后台登录启动和关闭清理仍需一次真实用户会话手测；默认关闭，Pre-release 不以此项已验收为前提。
- 系统级全局快捷键无法由当前 Computer Use 注入。正式版前仍应做 20 轮冷启动/立即按键，并检查保存位置首帧、非激活菜单栏和原应用光标。
- 菜单栏模板图标浅色/深色最终观感、多显示器不同缩放、三家翻译真实鉴权/限流仍是人工或凭据受限项目。
- 输入普通字符时，状态区不应在“正在加载”和稳定结果之间逐键交替；250 ms 内完成的查询保留稳定帧，真正慢查询才显示延迟反馈。

## 可重复构建

```bash
git fetch origin --tags
git switch --detach v0.1.0
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
pnpm tauri build --bundles app
file src-tauri/target/release/suo
file src-tauri/target/release/bundle/macos/Suo.app/Contents/MacOS/suo
```

必须从 `.app` bundle 经 Finder/LaunchServices 启动；裸执行 `Contents/MacOS/suo` 不能替代窗口、Dock、菜单栏或 Keychain 验收。不要使用 `sudo` 运行 Suo。
