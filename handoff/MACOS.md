# macOS Apple Silicon release handoff

状态：**正在准备 `v0.1.3` / 配置协议 v17 的 macOS Apple Silicon Pre-release。该版本包含原生全屏 Space 覆盖修复；Windows 必须从同一不可变 tag 重新构建，不能把 `v0.1.2` 安装包改名复用。** 仍无 Developer ID 签名、未公证，不是正式稳定版。

## `v0.1.3` 原生全屏 Space 修复（2026-09-11）

- 问题：从 macOS 原生全屏应用所在的独立 Space 按全局快捷键时，快捷键会触发，但单一启动器面板仍停留在普通桌面，当前页面看不到搜索框。
- 修复：`src-tauri/src/focus.rs` 在既有非激活 `NSPanel` 上增加 `CanJoinAllSpaces | FullScreenAuxiliary`，并移除冲突的 Space/全屏角色；继续沿用鼠标所在显示器的定位、非激活焦点策略和 AppKit 几何队列，不改变设置窗口或 Windows 行为。
- 自动化：`pnpm install --frozen-lockfile`、`pnpm build`、120 个 Rust 测试（119 通过、1 个安装应用探测测试 ignored）、`cargo check --locked --all-targets` 和 `pnpm tauri build --bundles app` 通过；主程序与 `.app` 内可执行文件均为 Mach-O arm64，最低系统版本仍为 macOS 13.0。
- 实机：从 LaunchServices 启动工作树 `.app`，把活动监视器置于原生全屏 Space 后按 `Command+Space`，搜索框能在其上层显示并接收输入；关闭搜索框后活动监视器保持当前全屏应用，没有切换 Space 或激活 Suo。验证后已恢复原 `/Applications/Suo.app`，真实 `config.json` 与 `.bak` 的 SHA-256 与测试前一致，测试时创建的 `scripts/` 已移到临时备份。
- 限制：当前机器的 CoreGraphics 只报告 1 块活动显示器，因此“双物理显示器、各自普通/全屏 Space、不同缩放”的组合仍需连接第二块显示器后手工关闭。`v0.1.3` tag 一旦发布即不可移动或重建。

## `v0.1.3` 发布目标（2026-09-11）

- 来源：先 `git fetch origin --tags`，确认本地与远程 `v0.1.3` tag 指向同一发布提交。验证和发布产物必须使用该不可变 tag；若发现必须修改代码，回到 `dev` 修复并协调更高版本。
- 产品版本：`0.1.3`；配置协议：v17；目标：Apple Silicon / `arm64`，macOS 13+。
- 本轮共享能力：用户脚本目录与只创建不覆盖的模板、UTF-8/多行脚本输出、类型化文字/图片/二维码结果、默认 `qr` 命令、完整图片缩略图、脚本/网络搜索删除二次确认。
- Windows-only 的 `SHOpenFolderAndSelectItems`、PIDL、COM 和路径分隔符修复不得进入 macOS 分支；macOS“在文件夹中显示”继续使用 `/usr/bin/open -R`。

## 拉取与原生工具链门禁

```bash
git fetch origin --tags
git switch dev
git pull --ff-only origin dev
git rev-parse HEAD
git rev-list -n 1 v0.1.3
git status --short --branch
uname -m
node -p process.arch
rustc -vV
cargo --version
pnpm --version
```

`uname -m`、Node 与 Rust host 都应为 `arm64` / `aarch64-apple-darwin`。先在 `dev` 阅读本文件；正式产物必须从 `v0.1.3` tag 的源码构建。如果 `dev` 仅比 tag 多交接文档提交，可以阅读最新文档后切回 tag；不得从含未提交代码的工作区发布。

## 自动化与构建

```bash
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo check --manifest-path src-tauri/Cargo.toml --locked --all-targets
pnpm tauri build --bundles app
file src-tauri/target/release/suo
file src-tauri/target/release/bundle/macos/Suo.app/Contents/MacOS/suo
```

所有非 ignored 测试必须通过。确认 `base64`、`qrcode` 与共享 `image` 依赖在 arm64 正常编译；不得为了通过 macOS 编译删除 Windows feature 或降低图片校验。最终两个 Mach-O 均必须是原生 arm64。

## 配置与用户脚本迁移

1. 测试前备份真实 `config.json`、`.bak`、`config-location.json`（若存在）以及 `~/Library/Application Support/io.github.dqgod.suo/scripts/`，记录哈希；不要提交这些文件。
2. 从 v15 配置启动：仅内置 `timestamp-example` 的旧 `examples/timestamp.py` 路径迁移为 `scripts/timestamp.py`，版本最终为 v17，其他命令、皮肤、保存模式和启动器设置保持。
3. 从 v16 配置启动：仅在 `qr` 关键字/别名及 `qr-example` ID 均未占用时新增默认二维码命令；存在冲突时不抢占，脚本数量达到上限时不追加。
4. 缺失的 `timestamp.py`、`qr.py`、`script_template.py`、`open_path.py` 和 `README.md` 初始化到用户脚本目录；对其中任一文件加入探针后重启/升级，内容与哈希必须保持，不能被 bundle 模板覆盖。
5. 用户把 `config.json` 迁到自定义位置时，脚本目录仍固定属于默认 Application Support 目录，不跟随 JSON 移动。

## 类型化脚本结果验收

- `timestamp.py`：让 stdout 同时包含普通日志与 `SUO_RESULT:text:`，确认只显示标记文字；中文无乱码，多行按原换行复制。补测 `+08:30` / `-0530` 的完整时区标签，以及 `now +99`、`now -5 extra`、`help extra` 的退出码 2 和无 traceback。完全没有前缀的旧脚本仍返回完整 stdout。
- `qr www.google.com`：本地生成并完整显示二维码；结果缩略图最大 240 px，窗口变矮时继续等比例缩小，标题、整张图片、说明和底栏不能互相遮挡；按 Enter 复制 `www.google.com`，不是复制 PNG data URL。
- `qr 你好，Suo 🚀`：用实际扫码器确认内容按 UTF-8 解码，无乱码。输入超过 500 个 UTF-8 字节时应得到可操作错误，不能联网请求二维码服务。
- 临时图片脚本：输出一张受限的 `SUO_RESULT:image: data:image/png;base64,...`，确认完整缩略图与宽高比；按 Enter 复制 data URL。再验证 raw PNG Base64 可用，而远程 URL、SVG、损坏 Base64、超出 512 KB / 1024 px、多张图片以及文字/图片混合均被拒绝。
- 把临时图片脚本的“返回值动作”改为执行 Shell，确认图片/二维码结果只显示拒绝提示，绝不进入 Bash。stderr 和二次 Shell 输出不能套用结果前缀过滤。
- [`examples/script_template.py`](../examples/script_template.py) 的 `emit_text_result`、`emit_image_result`、`emit_qrcode_result` 分别做一次临时验证；单次执行只能选择一种结果类型。测试后删除临时命令和文件。

## Finder 定位与删除确认

- 在设置中对真实 `ts`、`qr` 各点击一次“在文件夹中显示”：Finder 必须打开用户脚本目录并选中对应文件；补测含空格和中文的绝对路径。失败时记录完整配置路径和 Finder 行为，但不要把用户目录写进公开截图。
- 复制一个临时脚本命令与一个临时网络搜索进行删除测试，**不要使用真实 `ts`**：第一次点击“删除”只能打开应用内确认框，条目仍存在；取消、Esc、点击遮罩均不删除。
- 自动保存关闭（统一保存设置开启）时，确认删除只修改页面草稿，必须再点右上角“保存设置”才持久化；直接关闭设置应丢弃草稿。自动保存开启时，第二次点击红色“确认删除”后才立即保存。两种模式均需重启确认。

## macOS 核心回归

- Command+Space、非激活 `NSPanel`、原应用菜单栏/输入光标、Esc、失焦策略和单实例保持既有行为；设置窗口仍是普通可聚焦窗口。
- 在每块目标显示器上分别从普通 Space 和原生全屏应用的独立 Space 唤起：搜索框必须出现在鼠标所在显示器及当前 Space 的全屏应用上层，关闭后不得切换 Space、退出全屏或激活 Suo；不同缩放的双显示器必须分别验证定位和工作区夹紧。
- 搜索窗口不错误显示 Dock 图标；设置窗口打开时按用户配置显示 Dock 图标；菜单栏模板图标在浅色/深色下可见，退出/设置菜单正常。
- 圆角透明窗口不能重新出现白色直角，`app.macOSPrivateApi=true` 必须保留；紧凑空输入与完整结果窗口都要检查。
- `startAtLogin` LaunchAgent 开关、后台冷启动、退出清理至少做一次真实用户会话验证；不得使用 `sudo`。
- 应用/文件/文件夹搜索、原生应用图标、拼音应用搜索、计算器、网络搜索和翻译做基础回归，确认共享 `SearchResult` 新字段未破坏旧结果。

## arm64 资产与上传

验证全部通过后，从 tag 构建出的真实 `.app` 创建 `Suo_0.1.3_macos_arm64.zip`：

```bash
ditto --sequesterRsrc --keepParent \
  src-tauri/target/release/bundle/macos/Suo.app \
  Suo_0.1.3_macos_arm64.zip
shasum -a 256 Suo_0.1.3_macos_arm64.zip
```

重新解压 ZIP，经 Finder/LaunchServices 冷启动并再次核对 Mach-O arm64 后，才创建 `v0.1.3` Pre-release 并上传 ZIP 与 `.sha256`。Windows 后续必须从同一 tag 追加 x64 安装包；不得复用旧版本资产，也不得用裸执行 `Contents/MacOS/suo` 代替 bundle 验收。完成后更新本文件与 [根 README](../README.md)，提交并推送到 `dev`。

## `v0.1.0` 历史发布证据

### 发布范围

- 来源：`v0.1.0` tag（tag 创建前必须与最终发布提交一致）。
- 产品版本：`0.1.0`；配置协议：v15。
- 目标：Apple Silicon / `arm64`，macOS 13+。
- 产物：`Suo_0.1.0_macos_arm64.zip` 与对应 `.sha256`。
- 分发限制：当前只有链接器生成的 ad-hoc 签名，没有 Developer ID 签名、Apple notarization 或 DMG；首次打开可能触发 Gatekeeper 提示。不得宣称支持 Mac App Store，`app.macOSPrivateApi=true` 仍是透明无边框窗口所必需。

### 已完成验证

- [x] `uname -m`、Node `process.arch`、Rust host 和最终 Mach-O 均核对为原生 arm64。
- [x] `pnpm install --frozen-lockfile`、`pnpm build`、全部 Rust 测试、`cargo check --locked --all-targets` 和 `.app` bundle 构建通过。
- [x] macOS 编译不再保留 Windows-only `AppHandle` 未使用告警。
- [x] 解包后的 `Info.plist` 明确声明 `LSMinimumSystemVersion=13.0`，与产品支持范围一致。
- [x] ZIP 使用 `ditto --sequesterRsrc --keepParent` 从真实 `.app` bundle 创建，发布前重新解压并核对 bundle 主程序架构。
- [x] `Suo_0.1.0_macos_arm64.zip` SHA-256：`12109c4678f0556e1c9b1d529d92587489d1ab3c41f26bb3353731125433d90c`；发布时同时上传 `.sha256`。
- [x] 从最终 ZIP 重新解压后经 Finder/LaunchServices 冷启动，进程路径确认属于解包后的 `Suo.app`；再次打开可显示搜索框，输入无副作用查询 `release-smoke-xyz` 后稳定返回“没有匹配结果”。
- [x] 对同一最终 bundle 快速替换输入为 `rapid-smoke-abcdef`，在 80 ms、260 ms 和 910 ms 三个观察点均未出现“正在加载/稳定状态”交替，输入框焦点持续保留。

### 当时的已知限制与后续人工项

- `startAtLogin` 的 LaunchAgent 创建、后台登录启动和关闭清理仍需一次真实用户会话手测；默认关闭，Pre-release 不以此项已验收为前提。
- 系统级全局快捷键无法由当前 Computer Use 注入。正式版前仍应做 20 轮冷启动/立即按键，并检查保存位置首帧、非激活菜单栏和原应用光标。
- 菜单栏模板图标浅色/深色最终观感、多显示器不同缩放、三家翻译真实鉴权/限流仍是人工或凭据受限项目。
- 输入普通字符时，状态区不应在“正在加载”和稳定结果之间逐键交替；250 ms 内完成的查询保留稳定帧，真正慢查询才显示延迟反馈。

### 可重复构建

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
