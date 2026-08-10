# Repository Guidelines

## Project Scope

Suo is a cross-platform desktop launcher for Windows and macOS. The stable branch is `master`; ongoing development belongs on `dev`. Keep the core local-first and avoid adding frozen-out MVP features such as a plugin marketplace, remote scripts, full-text file search, a CLI, automatic updates, or telemetry unless the product requirements are explicitly revised.

## Repository Layout

- `src/`: React and TypeScript launcher/settings UI.
- `src-tauri/src/`: Rust application core and platform adapters.
- `src-tauri/capabilities/`: Tauri permission declarations; grant the smallest capability set needed.
- `docs/`: product requirements, the interactive HTML prototype, and rendered previews.
- `handoff/`: cross-machine status, platform validation checklists, and lessons for the next Windows/macOS agent.
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

## Cross-machine Handoff

Machine-specific status, completed hardware evidence, active platform TODOs and cross-platform lessons live in [`handoff/README.md`](handoff/README.md). Read that index and the target platform file before compiling, validating or changing native integration on another operating system.

Keep this file limited to durable repository rules. Update `handoff/` when work moves between Windows and macOS; mark completed items explicitly and remove stale instructions instead of accumulating dated execution logs here.

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
- The global launcher shortcut is persisted cross-platform and edited through a key recorder. Validate and register the replacement before persisting it; a conflict or save failure must keep or restore the previously working shortcut.
- Launcher geometry is shared configuration, not a platform-specific path: explicit width/height and relative offsets use logical pixels, while final positioning converts with the destination monitor scale factor and clamps to its work area. A missing explicit width continues to follow the active launcher theme.
- Dock visibility is a macOS presentation preference. Keep its typed config field cross-platform for portable configuration, but isolate native behavior behind the macOS adapter. On Windows, keep the shortcut-driven launcher out of the taskbar, keep Settings in the taskbar, and preserve the independent coloured tray icon.
- Web search `{query}` expands the complete post-keyword text without requiring quotes. `{query0}`, `{query1}`… expand quote-aware positional arguments; missing arguments must produce a non-actionable error result. A valid HTTP/HTTPS URL with no placeholder is a direct-link command and must be actionable when the user enters only its keyword.
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
