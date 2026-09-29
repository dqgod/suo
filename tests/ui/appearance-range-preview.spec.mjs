import { test, expect } from "playwright/test";
import { fixtureConfig, installMockBridge, launcherSkin } from "./bridge.mjs";

async function openWideBlackCopy(page) {
  const config = fixtureConfig();
  config.launcherTheme.theme = "custom:black-copy";
  config.launcherTheme.customThemes = [{
    ...launcherSkin("black-copy", "极简黑 副本"),
    windowWidthPx: 560,
    searchWidthPx: 560,
    searchFontSizePx: 24,
    showProviderStatus: false,
    showFooterHints: false,
    maxResults: 8,
  }];
  await installMockBridge(page, { config });
  await page.goto("/");
  await page.locator('.settings-sidebar button', { hasText: "外观" }).click();
  await expect(page.locator('.appearance-theme-copy strong')).toHaveText("极简黑 副本");
}

test("search width and text size can grow past the initial slider ends", async ({ page }) => {
  await openWideBlackCopy(page);
  const width = page.locator(".appearance-number-control", { hasText: "搜索框宽度" });
  const windowWidth = page.getByRole("spinbutton", { name: "窗口宽度精确数值" });
  const textSize = page.locator(".appearance-number-control", { hasText: "输入文字大小" });
  const widthNumber = width.locator('input[type="number"]');
  const textNumber = textSize.locator('input[type="number"]');

  expect(Number(await width.locator('input[type="range"]').getAttribute("max"))).toBeGreaterThan(560);
  await widthNumber.fill("900");
  await expect(widthNumber).toHaveValue("900");
  await expect(windowWidth).toHaveValue("900");
  await expect(widthNumber).toHaveAttribute("aria-invalid", "false");

  expect(Number(await textSize.locator('input[type="range"]').getAttribute("max"))).toBeGreaterThan(24);
  await textSize.locator('input[type="range"]').focus();
  await textSize.locator('input[type="range"]').press("End");
  expect(Number(await textNumber.inputValue())).toBeGreaterThan(24);
  await textNumber.fill("255");
  await expect(textNumber).toHaveValue("255");
  await expect(textNumber).toHaveAttribute("aria-invalid", "false");
  await page.locator(".appearance-edit-footer .primary").click();
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => {
    const saved = window.__suoMock.config.launcherTheme.customThemes[0];
    return [saved.windowWidthPx, saved.searchWidthPx, saved.searchFontSizePx];
  })).toEqual([900, 900, 255]);
});

test("launcher preview shows a representative set without an inner scrollbar", async ({ page }, testInfo) => {
  await openWideBlackCopy(page);
  const preview = page.locator(".appearance-launcher-preview");
  const results = preview.locator(".appearance-live-results");
  await expect(results.locator(".appearance-live-row")).toHaveCount(3);
  expect(await results.evaluate(el => getComputedStyle(el).overflowY)).toBe("hidden");
  expect(await results.evaluate(el => el.scrollHeight <= el.clientHeight)).toBe(true);
  expect(await preview.evaluate(el => el.scrollHeight <= el.clientHeight)).toBe(true);
  await expect(page.locator(".appearance-native-note")).toContainText("8");
  await page.screenshot({ path: testInfo.outputPath("preview-three-rows.png") });
});

test("skin spacing moves search content left and shrinks paired top and bottom gaps", async ({ page }) => {
  await openWideBlackCopy(page);
  const preview = page.locator(".appearance-live-search");
  const query = preview.locator("strong");
  const before = await query.boundingBox();
  const left = page.getByRole("spinbutton", { name: "搜索框左侧留白精确数值" });
  const vertical = page.getByRole("spinbutton", { name: "搜索框上下留白精确数值" });
  await left.fill("0");
  await vertical.fill("0");
  await expect(left).toHaveValue("0");
  await expect(vertical).toHaveValue("0");
  const after = await query.boundingBox();
  expect(after.x).toBeLessThan(before.x - 10);
  expect(after.y).toBeLessThan(before.y - 5);
  await expect(preview).toHaveCSS("--preview-search-vertical-gap", "0px");

  await page.locator(".appearance-edit-footer .primary").click();
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => {
    const saved = window.__suoMock.config.launcherTheme.customThemes[0];
    return [saved.searchLeftSpacePx, saved.searchVerticalSpacePx];
  })).toEqual([0, 0]);
});

test("border colour and width become visible from an initially hidden border", async ({ page }) => {
  await openWideBlackCopy(page);
  const colour = page.getByRole("textbox", { name: "搜索框边框十六进制颜色" });
  const style = page.getByRole("combobox", { name: "搜索框边框样式" });
  const width = page.getByRole("spinbutton", { name: "搜索框边框宽度精确数值" });
  const preview = page.locator(".appearance-live-search");
  await style.selectOption("none");
  await width.fill("0");
  await colour.fill("#FF4A4A");
  await colour.blur();
  await expect(style).toHaveValue("solid");
  await expect(width).toHaveValue("2");
  await expect(preview).toHaveCSS("border-top-color", "rgb(255, 74, 74)");
  await width.fill("32");
  await expect(width).toHaveAttribute("aria-invalid", "false");
  expect(parseFloat(await preview.evaluate(el => getComputedStyle(el).borderTopWidth))).toBeGreaterThan(4);
  await style.selectOption("none");
  await expect(preview).toHaveCSS("border-top-style", "none");
});

test("compact native-sized search moves equally at the top and bottom", async ({ page }) => {
  const config = fixtureConfig();
  config.launcher.compactWhenEmpty = true;
  config.launcherTheme.theme = "custom:black-copy";
  config.launcherTheme.customThemes = [{ ...launcherSkin("black-copy", "极简黑 副本"), windowWidthPx: 560, searchWidthPx: 560, searchBorderWidthPx: 0, searchBorderStyle: "none", searchLeftSpacePx: 30, searchVerticalSpacePx: 9 }];
  await page.setViewportSize({ width: 560, height: 74 });
  await installMockBridge(page, { config, windowLabel: "main" });
  await page.goto("/");
  const input = page.locator(".search-box input");
  const before = await input.boundingBox();
  await page.evaluate(() => {
    const next = window.__suoMock.config;
    next.launcherTheme.customThemes[0].searchLeftSpacePx = 0;
    next.launcherTheme.customThemes[0].searchVerticalSpacePx = 0;
    window.__suoMock.emit("app-config-updated", next);
  });
  await page.setViewportSize({ width: 560, height: 56 });
  await expect(page.locator(".launcher")).toHaveCSS("--launcher-search-row-min-height", "56px");
  const after = await input.boundingBox();
  expect(after.x).toBeLessThan(before.x - 20);
  expect(after.y).toBeLessThan(before.y - 6);
  expect(Math.abs(after.y - (56 - after.y - after.height))).toBeLessThan(2);
  expect(await page.locator(".launcher").evaluate(el => el.scrollHeight <= el.clientHeight)).toBe(true);
});
