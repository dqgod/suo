# Settings UI regression tests

Run after `pnpm install --frozen-lockfile`:

```text
pnpm test:ui
```

The Playwright suite starts the Vite development server on `127.0.0.1:1420` when needed. It uses installed Microsoft Edge on Windows. On macOS, install Playwright Chromium once with `pnpm exec playwright install chromium` before running the suite. Tests cover 1180×720 and the supported 680×520 settings window size, plus the actual launcher at 720×520 and 680×520. Both independent Minimal Black skins are covered.

`bridge.mjs` supplies an in-memory Tauri API and sample configuration. No native commands, credential store, scripts, network search, or user files are used. This suite verifies React behavior and layout; the platform checks in `handoff/` remain necessary for real desktop integration. Screenshots, traces, and the HTML report go to ignored `test-results/` and `playwright-report/` directories.
