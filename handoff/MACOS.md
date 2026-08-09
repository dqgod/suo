# macOS Apple Silicon handoff

状态：**已完成（2026-08-09）**。

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

## 仍需人工观察或在不同机器补测

- [ ] SystemUIServer 无法通过当前辅助功能接口稳定截图；菜单栏模板图标在浅色/深色菜单栏上的最终视觉仍建议肉眼检查。
- [ ] 本机 `Command+Space` 可直接注册，因此没有复现系统 Spotlight 占用时的冲突提示；需要在保留 macOS 默认 Spotlight 快捷键的机器补测。
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
