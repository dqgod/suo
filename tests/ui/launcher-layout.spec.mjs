import { test, expect } from "playwright/test";
import { fixtureConfig, installMockBridge, launcherSkin } from "./bridge.mjs";

const searchResults = [
  { id: "folder", kind: "directory", title: "WeiXin", subtitle: "D:/Pictures/WeiXin", badge: "文件夹" },
  { id: "app", kind: "app", title: "微信", subtitle: "C:/ProgramData/Microsoft/Windows/Start Menu/Programs/微信/微信.lnk", badge: "应用" },
].map(result => ({ ...result, iconDataUrl: "", resultImageDataUrl: "", score: 1, action: { type: "copyText", text: result.title } }));

for (const theme of ["custom:midnight-copy", "black"]) {
  test(`launcher result alignment stays horizontal in narrow windows: ${theme}`, async ({ page }, testInfo) => {
    const config = fixtureConfig();
    config.launcherTheme.theme = theme;
    config.launcherTheme.customThemes = [launcherSkin("midnight-copy", "午夜 副本")];
    await page.setViewportSize({ width: 600, height: 520 });
    await installMockBridge(page, { config, windowLabel: "main", searchResults });
    await page.goto("/");
    await page.locator(".search-box input").fill("weixin");
    await expect(page.getByRole("option")).toHaveCount(2);
    for (const width of [600, 620, 621, 480, 256, 720]) {
      await page.setViewportSize({ width, height: 520 });
      for (const row of await page.getByRole("option").all()) {
        const icon = await row.locator(".result-icon").boundingBox();
        const copy = await row.locator(".result-copy").boundingBox();
        expect(copy.x, `text stays beside icon at ${width}px`).toBeGreaterThanOrEqual(icon.x + icon.width);
        expect(Math.abs(copy.y + copy.height / 2 - icon.y - icon.height / 2), `vertical alignment at ${width}px`).toBeLessThan(1);
        expect(copy.width).toBeGreaterThan(32);
        expect(await row.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
      }
      await page.locator(".search-box input").press("ArrowDown");
      const selected = page.getByRole("option", { selected: true });
      const hint = await selected.locator("kbd").boundingBox();
      const copy = await selected.locator(".result-copy").boundingBox();
      expect(hint.x).toBeGreaterThanOrEqual(copy.x + copy.width);
      if (width === 600) await page.screenshot({ path: testInfo.outputPath("launcher-600px.png") });
    }
  });
}

test("oversized launcher icons leave readable text in the narrowest window", async ({ page }) => {
  const config = fixtureConfig();
  config.launcherTheme.theme = "custom:large-icons";
  config.launcherTheme.customThemes = [{ ...launcherSkin("large-icons", "Large icons"), iconSizePx: 255 }];
  await page.setViewportSize({ width: 256, height: 520 });
  await installMockBridge(page, { config, windowLabel: "main", searchResults });
  await page.goto("/");
  await page.locator(".search-box input").fill("weixin");
  await expect(page.getByRole("option")).toHaveCount(2);
  for (const row of await page.getByRole("option").all()) {
    const icon = await row.locator(".result-icon").boundingBox();
    const copy = await row.locator(".result-copy").boundingBox();
    expect(Math.abs(icon.width - icon.height)).toBeLessThan(1);
    expect(copy.x).toBeGreaterThanOrEqual(icon.x + icon.width);
    expect(copy.width).toBeGreaterThan(32);
    expect(await row.evaluate(el => el.scrollWidth <= el.clientWidth)).toBe(true);
  }
});

for (const [theme, width] of [["black", 560], ["midnight", 720]]) {
  test(`compact empty launcher fits its native height without a scrollbar: ${theme}`, async ({ page }, testInfo) => {
    const config = fixtureConfig();
    config.launcher.compactWhenEmpty = true;
    config.launcherTheme.theme = theme;
    await page.setViewportSize({ width, height: 74 });
    await installMockBridge(page, { config, windowLabel: "main", searchResults });
    await page.goto("/");
    const launcher = page.locator(".launcher");
    await expect(launcher).toHaveClass(/compact-empty/);
    expect(await launcher.evaluate(el => ({ scroll: el.scrollHeight, client: el.clientHeight }))).toEqual({ scroll: 72, client: 72 });
    await expect(launcher).toHaveCSS("--launcher-width", `${width}px`);
    await expect(launcher).toHaveCSS("--launcher-search-width", `${width}px`);
    await expect(page.locator(".results")).toHaveCount(0);
    const input = page.locator(".search-box input");
    const inputBox = await input.boundingBox();
    expect(inputBox.y).toBeGreaterThanOrEqual(0);
    expect(inputBox.y + inputBox.height).toBeLessThanOrEqual(74);
    if (theme === "black") await page.screenshot({ path: testInfo.outputPath("minimal-black-compact-560px.png") });
    await page.setViewportSize({ width, height: 520 });
    await input.fill("weixin");
    await expect(page.getByRole("option")).toHaveCount(2);
    await expect(launcher).not.toHaveClass(/compact-empty/);
    await input.fill("");
    await expect(launcher).toHaveClass(/compact-empty/);
    await page.setViewportSize({ width, height: 74 });
    expect(await launcher.evaluate(el => el.scrollHeight <= el.clientHeight)).toBe(true);
    if (theme === "black") {
      await page.evaluate(() => {
        const next = window.__suoMock.config;
        next.launcher.windowWidthPx = 840;
        window.__suoMock.emit("app-config-updated", next);
      });
      await expect(launcher).toHaveCSS("--launcher-width", "840px");
      await expect(launcher).toHaveCSS("--launcher-search-width", "840px");
    }
  });
}

for (const [borderStyle, height] of [["solid", 338], ["none", 330]]) {
  test(`compact launcher fits large text and visible inner/outer borders: ${borderStyle}`, async ({ page }) => {
    const config = fixtureConfig();
    config.launcher.compactWhenEmpty = true;
    config.launcherTheme.theme = "custom:large-search";
    config.launcherTheme.customThemes = [{
      ...launcherSkin("large-search", "Large search text"), searchFontSizePx: 255,
      windowBorderWidthPx: 4, searchBorderWidthPx: 4, searchBorderStyle: borderStyle,
    }];
    // These are the corresponding native compact heights, including borders.
    await page.setViewportSize({ width: 720, height });
    await installMockBridge(page, { config, windowLabel: "main" });
    await page.goto("/");
    const launcher = page.locator(".launcher");
    await expect(launcher).toHaveClass(/compact-empty/);
    expect(await launcher.evaluate(el => el.scrollHeight <= el.clientHeight)).toBe(true);
    const input = await page.locator(".search-box input").boundingBox();
    expect(input.height).toBeGreaterThanOrEqual(255 * 1.15 + 8);
    expect(input.y).toBeGreaterThanOrEqual(4);
    expect(input.y + input.height).toBeLessThanOrEqual(height - 4);
  });
}
