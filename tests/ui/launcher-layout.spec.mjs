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
