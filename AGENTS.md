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

```powershell
pnpm install
pnpm build
pnpm tauri dev
pnpm tauri build
```

Run Rust tests from `src-tauri`:

```powershell
cargo test
```

Before pushing, run the frontend build and Rust tests. When platform integration changes, also run the relevant desktop application and verify the real hotkey, focus, search, and process behavior.

## Architecture Rules

- Route queries through providers with cancellation/query IDs so stale results cannot replace newer input.
- Exact command keywords and valid calculations take precedence; apps and files are ranked together afterward.
- Windows file search order is existing Everything, bundled Everything for the installed build, then Suo's limited-directory index. The portable build must not install a service.
- macOS file search uses Spotlight; permission requests are deferred until a protected path needs them.
- Custom command keywords and aliases share one case-insensitive global namespace.
- Script execution defaults to argument arrays. Shell mode is explicit and high-risk.
- Query scripts may run immediately; action scripts run on Enter by default. Both must support timeout and process-tree cancellation.
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
