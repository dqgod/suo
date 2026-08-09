# Repository Guidelines

## Project Scope

Suo is a cross-platform desktop launcher for Windows and macOS. The stable branch is `master`; ongoing development belongs on `dev`. Keep the core local-first and avoid adding frozen-out MVP features such as a plugin marketplace, remote scripts, full-text file search, a CLI, automatic updates, or telemetry unless the product requirements are explicitly revised.

## Repository Layout

- `src/`: React and TypeScript launcher/settings UI.
- `src-tauri/src/`: Rust application core and platform adapters.
- `src-tauri/capabilities/`: Tauri permission declarations; grant the smallest capability set needed.
- `docs/`: product requirements, the interactive HTML prototype, and rendered previews.
- `examples/`: sample scripts used for end-to-end verification.

Keep platform-independent contracts in shared Rust modules. Put Windows Everything and application-discovery code behind Windows adapters; put Spotlight and macOS application discovery behind macOS adapters. The UI must consume typed commands/results instead of platform-specific paths or APIs.

## Build and Verification

Use pnpm for JavaScript dependencies.

```text
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Windows native commands should run from Visual Studio's Developer PowerShell so MSVC `link.exe` wins over Git for Windows tools:

```powershell
pnpm tauri dev
pnpm tauri build --no-bundle
```

Before pushing, run the frontend build and Rust tests. When platform integration changes, also run the relevant desktop application and verify the real hotkey, focus, search, and process behavior.

## macOS Apple Silicon Handoff

The first Apple Silicon hardware pass was completed on 2026-08-08. Treat the commands and scenarios below as the regression checklist for later cross-platform changes. Work from `dev`, do not use `sudo`, and do not commit `node_modules/`, `dist/`, `target/`, `.app`, or `.dmg` artifacts.

### 2026-08-09 dev Delta

The Windows pass for the current `dev` delta is complete. On macOS, do not stop at a successful compile: run the bundled `.app` and regression-test the following changes against a copy of an existing user configuration.

- Configuration is now version 8. Loading a version 7 configuration must preserve every existing command, service, theme and preference while supplying `0 ms` empty-query and `50 ms` non-empty-query debounce defaults. Saving once must produce a valid version 8 file; keep the original config and `.bak` until the migration is verified.
- In General settings, set visibly different empty/non-empty delays (for example `300 ms` and `600 ms`). Confirm an empty launcher waits for the empty delay, ordinary text uses the non-empty trailing-edge delay, each additional keystroke restarts that timer, and an enabled immediate script still uses only its own per-command delay. Restore the intended values afterward.
- Application-name pinyin matching is catalog-only and cross-platform. Find an installed application whose discovered display name contains Chinese characters and verify both full pinyin and initials; native English/name/path matching must still rank ahead of a pinyin fallback, and ordinary files must not gain pinyin aliases.
- For at least one application under `/Applications` and one under `~/Applications`, confirm the result first renders safely and then shows the real `.app` icon, repeated queries use the cache, and activating the row opens the same application. Directories and ordinary files must retain Suo's fixed folder/file icons. Do not loosen the discovered-path allowlist or accept symlink/remote icon sources to make an icon appear.
- Exercise both settings persistence modes. With unified save enabled, edits must remain page drafts until the top-right save succeeds. With it disabled, valid ordinary changes auto-save; an active custom theme uses one-click `Save and apply`, while an inactive custom theme is saved first and is applied only by a separate explicit action. An unchanged theme must keep its save button disabled.
- Verify launcher and settings theme libraries remain independent after restart, including active-theme selection, edited-theme selection, import/export scope checks, Retina sizing, wallpaper rendering and search/logo visibility. A save response must not overwrite edits typed while a previous save is in flight.

If a macOS-only fix is required, preserve the Windows acceptance baseline: `weixin` and `wx` find the Chinese WeChat shortcut with its native icon, `steam` shows the Steam icon, Everything fallback order is unchanged, and script cancellation still terminates the Windows Job Object tree.

Record the environment first:

```bash
git switch dev
git pull --ff-only origin dev
uname -m
sw_vers
node --version
pnpm --version
rustc --version
cargo --version
xcode-select -p
```

`uname -m` should report `arm64`. Install Xcode Command Line Tools, Node.js 22+, pnpm 11+, and stable Rust if any command is missing. Then establish the unmodified baseline:

```bash
pnpm install --frozen-lockfile
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml --locked
pnpm tauri build --no-bundle
pnpm tauri build --bundles app
file src-tauri/target/release/suo
pnpm tauri dev
```

Verify these behaviors on the real machine:

- `Command+Space` registration and conflict reporting. macOS normally reserves it for Spotlight; first confirm Suo reports the conflict, then temporarily disable or remap the macOS Spotlight shortcut and restart Suo before testing successful invocation. The Suo hotkey recorder is not implemented yet.
- The menu-bar tray icon, its Show/Settings/Quit menu, the Dock icon, single-instance wake-up, `Esc`, focus loss, and repeated show/hide behavior.
- Rounded transparent launcher/settings windows on Retina scaling, with no square halo or invisible click region. Enable `Settings -> General -> 空输入时仅显示搜索框`; an empty query must shrink the native window to the search row, typing must restore the full window, and clearing must shrink it again.
- General settings expose independent empty/non-empty query debounce values (0–60000 ms, defaults 0/50). Verify trailing-edge reset while typing and clearing; immediate scripts must continue using their own per-command 20–60000 ms delay.
- Launcher and settings themes must remain independent on macOS: switch and edit each scope separately, reject cross-scope/legacy imports, verify launcher width and icon-size changes on Retina, and confirm hiding the search icon or Suo logo does not affect the settings window. Wallpaper readability warnings are only a preview aid; verify translucent or image-backed themes against the real desktop before accepting them.
- Application discovery under `/Applications`, `/System/Applications`, and `~/Applications`; results should show native `.app` icons and open the selected application. File search results must use the fixed directory/file icons, and replacing a discovered `.app` with a symlink must not make it an approved native-icon source.
- `f <name>` through `/usr/bin/mdfind -name`, including cancellation under rapid typing. Record any privacy prompt, timeout, or fallback to the Desktop/Documents/Downloads limited index; do not broaden protected-directory scanning just to silence a prompt.
- Calculator, web search, and Microsoft Translator. Translation credentials must land in macOS Keychain and must never appear in logs, screenshots, fixtures, or commits.
- Python (`python3` fallback), Bash, and executable script commands, including argv quoting, timeout/cancel process-group termination, and the `examples/timestamp.py` sample. Never run Suo or its script tests as root.
- A locally built `.app` launched from `src-tauri/target/release/bundle/macos/Suo.app`. Signing, notarization, DMG distribution, automatic update, and login-at-startup are still outside the current MVP validation.

For each failure, preserve the exact command, exit code, stderr, macOS version, architecture, and a screenshot when the issue is visual. Keep fixes platform-scoped behind `cfg(target_os = "macos")` where appropriate; do not weaken the verified Windows Everything, icon extraction, or Job Object paths. After a fix, rerun the frontend build, all Rust tests, the no-bundle build, and the affected real-app scenario. Update README status after the hardware pass; update the product requirements only if behavior or scope changes.

## Architecture Rules

- Route queries through providers with cancellation/query IDs so stale results cannot replace newer input.
- Exact command keywords and valid calculations take precedence; apps and files are ranked together afterward.
- Windows file search order is existing Everything, bundled Everything for the installed build, then Suo's limited-directory index. The portable build must not install a service.
- macOS file search uses Spotlight; permission requests are deferred until a protected path needs them.
- Custom command keywords and aliases share one case-insensitive global namespace.
- Script execution defaults to argument arrays. Shell mode is explicit and high-risk.
- Script argument count and meaning are owned by the script; Suo only performs quote-aware argv splitting and does not declare a parameter schema in the current MVP.
- Query scripts may run immediately with a per-command 20–60000 ms debounce (default 50 ms); action scripts run on Enter by default. Both must support timeout and process-tree cancellation.
- Ordinary launcher queries use separate configurable empty/non-empty trailing-edge debounce values (0–60000 ms, defaults 0/50); these settings must never override an immediate script's per-command delay.
- Web search `{query}` expands the complete post-keyword text without requiring quotes. `{query0}`, `{query1}`… expand quote-aware positional arguments; missing arguments must produce a non-actionable error result.
- Launcher and settings themes are independent scopes, including their custom theme libraries and accent colours. Imports use the strict `suo-launcher-theme-v1` and `suo-settings-theme-v1` contracts and must reject the wrong scope, the retired `suo-theme-v1`, missing/unknown fields, unsupported versions, and images that fail bounded decoding. Only fixed scope-specific CSS variables may be populated; wallpaper input is limited to local PNG/JPEG/WebP data URLs no larger than 1.5 MB or 4096 px per side, and must never load remote content or execute code.
- Launcher result icons follow a stable type contract: applications discovered in the runtime catalog may request native icons from their validated local paths, directories use the built-in folder icon, and other files use the built-in file icon.
- `suo-json-v1` is UTF-8. Plain text output may use a configured encoding and is capped independently for stdout/stderr.

## Security and Privacy

- Never concatenate untrusted arguments into a shell command in normal execution mode.
- Never place API keys, proxy credentials, or tokens in JSON config, logs, tests, fixtures, or screenshots.
- Use the operating system credential store for secrets.
- Reject remote script URLs and administrator/root script execution in the MVP.
- Log metadata by default, not full queries, arguments, stdout, or stderr. Preserve rotation and size limits.
- Imported bundles containing scripts remain disabled until explicitly reviewed.

## Code and Documentation Style

- Prefer small modules, explicit types, and deterministic behavior over framework abstractions.
- Keep user-facing Chinese copy in localization resources rather than hard-coding new text across components.
- Document platform-specific behavior and the fallback path next to its implementation.
- Update `docs/PRODUCT_REQUIREMENTS.md` when an accepted product decision changes behavior or scope.

## Commits

Use focused commits with imperative subjects. Do not mix generated assets, dependency upgrades, and behavior changes without a clear reason. Preserve a buildable `master`; feature work and technical experiments belong on `dev`.
