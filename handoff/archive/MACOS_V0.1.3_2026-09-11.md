# macOS v0.1.3 completed release evidence

本文件是已经完成的执行证据，不是后续 agent 的操作清单。当前待办以 [`../MACOS.md`](../MACOS.md) 为准。

## 发布来源与范围

- 不可变 tag：`v0.1.3`。
- 发布提交：`b027d774a6aa9aa61fea3f325e221f34e3dc7735`。
- 产品版本：0.1.3；配置协议：v17；目标：Apple Silicon arm64、macOS 13+。
- 关键修复：macOS 非激活启动器面板增加 `CanJoinAllSpaces | FullScreenAuxiliary`，可以覆盖原生全屏应用所在的独立 Space；设置窗口与 Windows 行为不变。
- 分发限制：只有链接器 ad-hoc 签名，没有 Developer ID 签名或公证，不支持 Mac App Store。

## 已完成门禁

- `pnpm install --frozen-lockfile` 通过供应链策略与锁文件校验。
- `pnpm build` 通过。
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`：119 通过、0 失败、1 ignored；ignored 项依赖本机安装的微信和飞书 Bundle。
- `cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets` 通过。
- 从 tag 执行 `pnpm tauri build --bundles app` 通过；裸主程序与 `.app` 内程序均为 Mach-O arm64，`CFBundleShortVersionString=0.1.3`，`LSMinimumSystemVersion=13.0`。
- 实际打包确认 `ditto` 必须使用 `-c -k --sequesterRsrc --keepParent`；缺少 `-c -k` 不会生成 ZIP，活跃交接模板已修正。

## 实机证据

- 从工作树 `.app` 启动，把活动监视器置于原生全屏 Space 后按 `Command+Space`：搜索框显示在全屏应用上层、输入框可用；关闭后没有切换 Space、退出全屏或激活 Suo。
- 当前 CoreGraphics 只报告一块活动显示器，因此本轮只关闭单显示器普通/全屏路径；双物理显示器和不同缩放组合留在活跃待办。
- 最终 ZIP 重新解压后通过 `open -n` / LaunchServices 冷启动，进程路径确认属于解包后的 `Suo.app`；无副作用查询 `final-v0.1.3-smoke` 正常显示稳定的“没有匹配结果”。
- 测试前后用户配置目录逐文件一致；`config.json` SHA-256 为 `831cfda68113c5e4ea36542c6c791b585f17fa339a456d277c9331224f0a6588`，`.bak` 为 `5177f406722b3303e3502977b2efd01fd7cd0eb02928cb261f74a40b187f790b`。

## GitHub Pre-release

- Release：<https://github.com/dqgod/suo/releases/tag/v0.1.3>。
- `Suo_0.1.3_macos_arm64.zip`：5,953,315 bytes。
- ZIP SHA-256：`365d802385f2838ca72b50efd560f02fd7a3f349909fc27848958c7f51f8ed23`；GitHub 远程 digest 与本地一致。
- 同名 `.sha256` 文件已经上传。
- Windows x64 资产未包含，必须从同一 tag 重新构建后追加。
