# Settings UI regression tests

Run after `pnpm install --frozen-lockfile`:

```text
pnpm test:ui
```

The Playwright suite starts the Vite development server on `127.0.0.1:1420` when needed. It uses installed Microsoft Edge on Windows. On macOS, install Playwright Chromium once with `pnpm exec playwright install chromium` before running the suite. Tests cover 1180×720 and the supported 680×520 settings window size, plus the actual launcher at 256, 480, 600, 620, 621, 680 and 720 px widths. Both independent Minimal Black skins are covered. Launcher layout checks cover custom Midnight and Minimal Black skins, selected/unselected rows, hidden source badges, and oversized icons; icon/text alignment is checked on both sides of the former 620 px wrapping breakpoint.

`bridge.mjs` supplies an in-memory Tauri API and sample configuration. No native commands, credential store, scripts, network search, or user files are used. This suite verifies React behavior and layout; the platform checks in `handoff/` remain necessary for real desktop integration. Screenshots, traces, and the HTML report go to ignored `test-results/` and `playwright-report/` directories.

Compact-mode checks use the native 74 px height and the 560 px Minimal Black width, then verify expanding and collapsing as input changes. Custom 255 px search text with 4 px outer borders is tested with visible and hidden search borders at the matching native heights. Appearance-header checks verify the full-width chooser above both columns, theme-derived thumbnails, separate active/previewed names, scope switching, and large-font wrapping.
