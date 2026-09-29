import { test, expect } from "playwright/test";
import { fixtureConfig, installMockBridge } from "./bridge.mjs";

async function openAppearance(page, config = fixtureConfig()) {
  await installMockBridge(page, { config });
  await page.goto("/");
  await page.locator('.settings-sidebar button[aria-current="page"]').waitFor();
  await page.locator(".settings-sidebar button", { hasText: "外观" }).click();
  await expect(page.locator(".appearance-editor")).toBeVisible();
}

test("appearance chooser spans both columns with compact tabs and a theme thumbnail", async ({ page }) => {
  await openAppearance(page);
  const tabs = page.locator(".appearance-scope-tabs button");
  await expect(tabs).toHaveText(["搜索界面", "设置界面"]);
  await expect(page.getByRole("tab", { name: "搜索界面皮肤" })).toHaveAttribute("aria-selected", "true");
  await expect(page.locator(".appearance-active-theme")).toHaveText("当前使用：午夜");

  const chooser = page.locator(".appearance-theme-picker");
  await expect(chooser.locator(".appearance-theme-copy small")).toHaveText("正在预览");
  await expect(chooser.locator(".appearance-theme-copy strong")).toHaveText("午夜");
  await expect(chooser.locator(".appearance-theme-identity > span")).toHaveText("内置");
  await expect(chooser.locator(".appearance-theme-actions small")).toHaveText("5 款皮肤");
  await expect(chooser.locator(".appearance-theme-thumbnail > *")).toHaveCount(3);

  const layout = await page.locator(".appearance-editor").evaluate((editor) => {
    const card = editor.querySelector(".appearance-theme-picker").getBoundingClientRect();
    const workbench = editor.querySelector(".appearance-workbench").getBoundingClientRect();
    const edit = editor.querySelector(".appearance-edit-panel").getBoundingClientRect();
    const preview = editor.querySelector(".appearance-preview-panel").getBoundingClientRect();
    const thumbnail = editor.querySelector(".appearance-theme-thumbnail").getBoundingClientRect();
    return {
      cardBottom: card.bottom, cardLeft: card.left, cardRight: card.right,
      workbenchLeft: workbench.left, workbenchRight: workbench.right,
      editTop: edit.top, previewTop: preview.top,
      thumbnailWidth: thumbnail.width, thumbnailHeight: thumbnail.height,
    };
  });
  expect(layout.cardBottom).toBeLessThan(layout.editTop);
  expect(layout.cardBottom).toBeLessThan(layout.previewTop);
  expect(Math.abs(layout.cardLeft - layout.workbenchLeft)).toBeLessThan(2);
  expect(Math.abs(layout.cardRight - layout.workbenchRight)).toBeLessThan(2);
  expect(layout.thumbnailWidth).toBe(68);
  expect(layout.thumbnailHeight).toBe(48);
});

test("theme browsing updates preview while active name waits for Apply in each scope", async ({ page }) => {
  const config = fixtureConfig();
  config.saveSettingsManually = false;
  await openAppearance(page, config);
  const thumbnail = page.locator(".appearance-theme-thumbnail");
  const midnightColor = await thumbnail.evaluate((element) => getComputedStyle(element).backgroundColor);
  await page.getByRole("button", { name: "更换皮肤" }).click();
  let dialog = page.getByRole("dialog", { name: "皮肤库" });
  await dialog.getByRole("button", { name: /纸张/ }).click();
  await expect(page.locator(".appearance-theme-copy strong")).toHaveText("纸张");
  expect(await thumbnail.evaluate((element) => getComputedStyle(element).backgroundColor)).not.toBe(midnightColor);
  await expect(page.locator(".appearance-active-theme")).toHaveText("当前使用：午夜");
  expect(await page.evaluate(() => window.__suoMock.config.launcherTheme.theme)).toBe("midnight");
  await page.locator(".appearance-edit-footer .primary").click();
  await expect(page.locator(".appearance-active-theme")).toHaveText("当前使用：纸张");
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.launcherTheme.theme)).toBe("paper");

  await page.getByRole("tab", { name: "设置界面皮肤" }).click();
  await expect(page.locator(".appearance-active-theme")).toHaveText("当前使用：森林");
  await expect(page.locator(".appearance-theme-copy strong")).toHaveText("森林");
  await page.getByRole("button", { name: "更换皮肤" }).click();
  dialog = page.getByRole("dialog", { name: "皮肤库" });
  await dialog.getByRole("button", { name: /Test Settings/ }).click();
  await expect(page.locator(".appearance-theme-copy strong")).toHaveText("Test Settings");
  await expect(page.locator(".appearance-theme-identity > span")).toHaveText("自定义");
  await expect(page.locator(".appearance-active-theme")).toHaveText("当前使用：森林");
  await expect(page.locator(".appearance-theme-actions small")).toHaveText("5 款皮肤");
});

test("chooser wraps at minimum width with a large settings font", async ({ page }) => {
  const config = fixtureConfig();
  config.settingsTheme.theme = "custom:settings-test";
  config.settingsTheme.customThemes[0].baseFontSizePx = 32;
  await openAppearance(page, config);
  await page.getByRole("tab", { name: "设置界面皮肤" }).click();
  const result = await page.evaluate(() => {
    const card = document.querySelector(".appearance-theme-picker").getBoundingClientRect();
    return { viewport: innerWidth, document: document.documentElement.scrollWidth, cardRight: card.right };
  });
  expect(result.document).toBeLessThanOrEqual(result.viewport);
  expect(result.cardRight).toBeLessThanOrEqual(result.viewport + 1);
  await expect(page.getByRole("button", { name: "更换皮肤" })).toBeVisible();
});
