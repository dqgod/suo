import { test, expect } from "playwright/test";
import { fixtureConfig, installMockBridge, launcherSkin, settingsSkin } from "./bridge.mjs";

async function boot(page, options = {}) {
  await installMockBridge(page, options);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "通用", exact: true })).toBeVisible();
}

async function appearance(page) {
  await page.locator('.settings-sidebar button[aria-current="page"]').waitFor();
  await page.locator('.settings-sidebar button', { hasText: "外观" }).click();
  await expect(page.locator(".appearance-editor")).toBeVisible();
}

async function library(page) {
  await page.getByRole("button", { name: "更换皮肤" }).click();
  return page.getByRole("dialog", { name: "皮肤库" });
}

async function nav(page, label) {
  await page.locator(".settings-sidebar button", { hasText: label }).click();
}

async function openCustomSkin(page, scope, name) {
  if (scope === "settings") await page.getByRole("tab", { name: /设置界面皮肤/ }).click();
  const dialog = await library(page);
  await dialog.getByRole("searchbox", { name: "搜索皮肤名称" }).fill(name);
  await dialog.locator(".appearance-library-results > button").click();
  await expect(page.locator(".appearance-theme-copy strong")).toHaveText(name);
}

test("settings loads through the isolated Tauri bridge and fits the viewport", async ({ page }, testInfo) => {
  await boot(page);
  await expect(page.locator(".settings-brand svg")).toBeVisible();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth);
  expect(overflow).toBe(false);
  await page.screenshot({ path: testInfo.outputPath("settings.png") });
  await appearance(page);
  await page.screenshot({ path: testInfo.outputPath("appearance.png") });
  expect(await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth)).toBe(false);
});

test("library has bounded scroll, search focus, filters, and preview-only selection", async ({ page }, testInfo) => {
  const config = fixtureConfig();
  config.launcherTheme.customThemes = Array.from({ length: 12 }, (_, index) => launcherSkin(`launcher-${index}`, `Launcher Skin ${String(index + 1).padStart(2, "0")}`));
  config.settingsTheme.customThemes = Array.from({ length: 12 }, (_, index) => settingsSkin(`settings-${index}`, `Settings Skin ${String(index + 1).padStart(2, "0")}`));
  await boot(page, { config });
  await appearance(page);
  const dialog = await library(page);
  await expect(dialog.getByRole("searchbox", { name: "搜索皮肤名称" })).toBeFocused();
  await expect(dialog.locator(".appearance-library-results > button")).toHaveCount(16);
  const sizes = await dialog.locator(".appearance-library-results").evaluate((element) => ({ client: element.clientHeight, scroll: element.scrollHeight }));
  expect(sizes.scroll).toBeGreaterThan(sizes.client);
  const bounds = await dialog.boundingBox();
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(page.viewportSize().width + 1);
  await page.screenshot({ path: testInfo.outputPath("library.png") });
  await dialog.getByRole("button", { name: "自定义", exact: true }).click();
  await expect(dialog.locator(".appearance-library-results > button")).toHaveCount(12);
  await dialog.getByRole("searchbox", { name: "搜索皮肤名称" }).fill("Launcher Skin 11");
  await expect(dialog.locator(".appearance-library-results > button")).toHaveCount(1);
  await dialog.locator(".appearance-library-results > button").click();
  await expect(dialog).not.toBeVisible();
  await expect(page.locator(".appearance-theme-copy strong")).toHaveText("Launcher Skin 11");
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
  expect(await page.evaluate(() => window.__suoMock.config.launcherTheme.theme)).toBe("midnight");
  await page.locator(".appearance-edit-footer .primary").click();
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.launcherTheme.theme)).toBe("custom:launcher-10");
});

test("script advanced settings open by default and command edits survive page navigation", async ({ page }) => {
  await boot(page);
  await nav(page, "脚本命令");
  await page.locator(".configuration-item .configuration-summary-main", { hasText: "Sample Script" }).click();
  const editor = page.locator(".configuration-editor");
  await expect(editor).toBeVisible();
  await expect(editor.locator("details.script-advanced")).toHaveAttribute("open", "");
  await editor.locator(".form-field", { hasText: "名称" }).locator("input").fill("Changed Script");
  await nav(page, "通用");
  await expect(page.getByRole("button", { name: /返回编辑：Changed Script/ })).toBeVisible();
  await nav(page, "脚本命令");
  await expect(editor.locator(".form-field", { hasText: "名称" }).locator("input")).toHaveValue("Changed Script");
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
});

test("new command switch guard keeps draft and save failure keeps the form", async ({ page }) => {
  await boot(page);
  await nav(page, "脚本命令");
  await page.getByRole("button", { name: /添加脚本命令/ }).click();
  const editor = page.locator(".configuration-editor");
  await editor.locator(".form-field", { hasText: "名称" }).locator("input").fill("Unsaved New Script");
  await editor.locator('input[aria-labelledby^="script-path-label-"]').fill("scripts/new.py");
  await nav(page, "网络搜索");
  await expect(page.getByRole("button", { name: /返回编辑：Unsaved New Script/ })).toBeVisible();
  await page.locator(".configuration-item .configuration-summary-main", { hasText: "Sample Web" }).click();
  const guard = page.getByRole("dialog", { name: "保留这次修改？" });
  await expect(guard).toBeVisible();
  await guard.getByRole("button", { name: "继续编辑" }).click();
  await nav(page, "脚本命令");
  await expect(editor.locator(".form-field", { hasText: "名称" }).locator("input")).toHaveValue("Unsaved New Script");
  await page.evaluate(() => { window.__suoMock.failNextSave = true; });
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect(editor.locator(".form-field", { hasText: "名称" }).locator("input")).toHaveValue("Unsaved New Script");
  await expect(page.getByRole("alert")).toContainText("Mock save failed");
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
});

test("manual and automatic settings saves keep the three icon styles synchronized", async ({ page }) => {
  await boot(page);
  const icons = page.getByRole("group", { name: "Suo 图标样式" });
  await icons.getByRole("button", { name: "主题单色" }).click();
  await expect(icons.getByRole("button", { name: "主题单色" })).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".settings-brand svg g")).toHaveAttribute("stroke", "currentColor");
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.settingsIconStyle)).toBe("monochrome");
  await icons.getByRole("button", { name: "原版底板" }).click();
  await expect(page.locator(".settings-brand img")).toBeVisible();
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.settingsIconStyle)).toBe("original");
  await page.getByRole("checkbox", { name: "统一保存设置" }).uncheck();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.saveSettingsManually)).toBe(false);
  await icons.getByRole("button", { name: "透明彩色" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.settingsIconStyle)).toBe("transparentColor");
  await expect(page.locator(".settings-brand svg linearGradient")).toHaveCount(1);
});

test("appearance exact values validate without clamping and drafts survive pages", async ({ page }) => {
  await boot(page);
  await appearance(page);
  await openCustomSkin(page, "settings", "Test Settings");
  await page.locator(".appearance-section summary", { hasText: "设置文字与导航" }).click();
  const number = page.locator(".appearance-number-control", { hasText: "基础字号" }).locator('input[type="number"]');
  await number.fill("8");
  await expect(number).toHaveValue("8");
  await number.fill("32");
  await expect(page.locator(".appearance-settings-preview")).toHaveCSS("font-size", "32px");
  await nav(page, "通用");
  await expect(page.getByRole("button", { name: "返回外观编辑" })).toBeVisible();
  await appearance(page);
  await expect(number).toHaveValue("32");
  await number.fill("0");
  await expect(number).toHaveAttribute("aria-invalid", "true");
  await expect(page.locator(".appearance-edit-footer .primary")).toBeDisabled();
  await number.fill("256");
  await expect(number).toHaveAttribute("aria-invalid", "true");
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
  await number.fill("255");
  await expect(number).toHaveAttribute("aria-invalid", "false");
  await expect(page.locator(".appearance-settings-preview")).toHaveCSS("font-size", "255px");
  await expect(page.locator(".appearance-edit-footer .primary")).toBeEnabled();
  await page.getByRole("tab", { name: /搜索界面皮肤/ }).click();
  await openCustomSkin(page, "launcher", "Test Launcher");
  const launcherNumber = page.locator(".appearance-number-control", { hasText: "输入文字大小" }).locator('input[type="number"]');
  await launcherNumber.fill("8");
  await page.getByRole("tab", { name: /设置界面皮肤/ }).click();
  await expect(number).toHaveValue("255");
  await page.getByRole("tab", { name: /搜索界面皮肤/ }).click();
  await expect(launcherNumber).toHaveValue("8");
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
});

test("cancel restores the enable switch for each command kind", async ({ page }) => {
  await boot(page);
  for (const [category, summary] of [["脚本命令", "Sample Script"], ["网络搜索", "Sample Web"], ["翻译", "fy"], ["内置命令", ">"]]) {
    await nav(page, category);
    const item = page.locator(".configuration-item", { has: page.locator(".configuration-summary-main", { hasText: summary }) }).first();
    await item.locator(".configuration-summary-main").click();
    const toggle = item.locator(".configuration-enable-switch input");
    await expect(toggle).toBeChecked();
    await item.locator(".configuration-enable-switch").click();
    await expect(toggle).not.toBeChecked();
    await page.locator(".configuration-editor").getByRole("button", { name: "取消" }).click();
    await expect(toggle).toBeChecked();
  }
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
});

test("Complete Edit validates and joins the manual draft before global save", async ({ page }) => {
  await boot(page);
  await nav(page, "脚本命令");
  await page.locator(".configuration-item .configuration-summary-main", { hasText: "Sample Script" }).click();
  const editor = page.locator(".configuration-editor");
  await editor.locator(".form-field", { hasText: "名称" }).locator("input").fill("Manual Name");
  await editor.getByRole("button", { name: "完成编辑" }).click();
  await expect(editor).not.toBeVisible();
  expect(await page.evaluate(() => window.__suoMock.calls.some((call) => call.command === "validate_app_config"))).toBe(true);
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.scriptCommands[0].name)).toBe("Manual Name");
});

test("Complete Edit validates then persists in automatic mode", async ({ page }) => {
  await boot(page, { config: fixtureConfig({ saveSettingsManually: false }) });
  await nav(page, "脚本命令");
  await page.locator(".configuration-item .configuration-summary-main", { hasText: "Sample Script" }).click();
  const editor = page.locator(".configuration-editor");
  await editor.locator(".form-field", { hasText: "名称" }).locator("input").fill("Automatic Name");
  await editor.getByRole("button", { name: "完成编辑" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.scriptCommands[0].name)).toBe("Automatic Name");
  const order = await page.evaluate(() => window.__suoMock.calls.map((call) => call.command));
  expect(order.indexOf("validate_app_config")).toBeGreaterThanOrEqual(0);
  expect(order.indexOf("save_app_config")).toBeGreaterThan(order.indexOf("validate_app_config"));
});

test("native close request protects a dirty draft and Escape keeps editing", async ({ page }) => {
  await boot(page);
  await page.getByRole("group", { name: "Suo 图标样式" }).getByRole("button", { name: "主题单色" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.calls.some((call) => call.command === "plugin:event|listen" && call.args.event === "settings-close-requested"))).toBe(true);
  await page.evaluate(() => window.__suoMock.emit("settings-close-requested"));
  const dialog = page.getByRole("dialog", { name: "保留这次修改？" });
  await expect(dialog).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
  await expect(page.getByRole("group", { name: "Suo 图标样式" }).getByRole("button", { name: "主题单色" })).toHaveAttribute("aria-pressed", "true");
  await page.evaluate(() => window.__suoMock.emit("settings-close-requested"));
  await dialog.getByRole("button", { name: "放弃并关闭" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.calls.some((call) => call.command === "hide_settings"))).toBe(true);
  expect(await page.evaluate(() => window.__suoMock.config.settingsIconStyle)).toBe("transparentColor");
});

test("geometry exact field accepts legal values and blocks out-of-range save", async ({ page }) => {
  await boot(page);
  const width = page.getByRole("spinbutton", { name: "启动器宽度 (px)" });
  await width.fill("2000");
  await width.blur();
  await expect(width).toHaveValue("2000");
  await width.fill("3000");
  await width.blur();
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => width.evaluate((input) => !input.checkValidity())).toBe(true);
  expect(await page.evaluate(() => window.__suoMock.saves.length)).toBe(0);
});

test("Mac UI exposes Dock and terminal application controls", async ({ page }) => {
  await boot(page, { platform: "mac" });
  await expect(page.getByText("打开设置时显示 Dock 图标", { exact: true })).toBeVisible();
  await nav(page, "内置命令");
  await page.locator(".configuration-item .configuration-summary-main").first().click();
  await expect(page.getByText("macOS 终端应用", { exact: true })).toBeVisible();
  await expect(page.getByText("Windows 命令终端", { exact: true })).toHaveCount(0);
});

test("applied 32 px settings theme keeps command fields and Complete Edit reachable", async ({ page }, testInfo) => {
  const config = fixtureConfig();
  config.settingsTheme.theme = "custom:settings-test";
  config.settingsTheme.customThemes[0].baseFontSizePx = 32;
  await boot(page, { config });
  await expect(page.locator('.settings-stage[data-large-text="true"]')).toBeVisible();
  await nav(page, "脚本命令");
  await page.locator(".configuration-item .configuration-summary-main", { hasText: "Sample Script" }).click();
  const editor = page.locator(".configuration-editor");
  const targets = [
    editor.locator(".form-field", { hasText: "名称" }).locator("input"),
    editor.locator('input[aria-labelledby^="script-path-label-"]'),
    editor.getByRole("button", { name: "完成编辑" }),
  ];
  for (const target of targets) {
    await target.scrollIntoViewIfNeeded();
    const reachable = await target.evaluate((element) => {
      const box = element.getBoundingClientRect();
      const x = Math.min(window.innerWidth - 1, Math.max(0, box.left + box.width / 2));
      const y = Math.min(window.innerHeight - 1, Math.max(0, box.top + box.height / 2));
      const hit = document.elementFromPoint(x, y);
      return box.width > 0 && box.height > 0 && box.top >= 0 && box.bottom <= window.innerHeight
        && (hit === element || element.contains(hit));
    });
    expect(reachable).toBe(true);
  }
  await page.screenshot({ path: testInfo.outputPath("large-font-editor.png") });
});


test("minimal black is built in for both scopes with opaque neutral surfaces", async ({ page }, testInfo) => {
  await boot(page);
  await appearance(page);
  const launcherLibrary = await library(page);
  await launcherLibrary.getByRole("button", { name: "内置", exact: true }).click();
  await expect(launcherLibrary.locator(".appearance-library-results > button")).toHaveCount(4);
  await launcherLibrary.getByRole("button", { name: /极简黑/ }).click();
  await expect(page.locator(".appearance-launcher-preview")).toHaveCSS("--preview-window", "#000000");
  await expect(page.locator(".appearance-launcher-preview")).toHaveCSS("--preview-accent", "#707070");
  expect(await page.evaluate(() => window.__suoMock.config.launcherTheme.theme)).toBe("midnight");
  await page.locator(".appearance-edit-footer .primary").click();
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.launcherTheme.theme)).toBe("black");
  expect(await page.evaluate(() => window.__suoMock.config.settingsTheme.theme)).toBe("forest");
  await page.getByRole("tab", { name: /设置界面皮肤/ }).click();
  const settingsLibrary = await library(page);
  await settingsLibrary.getByRole("button", { name: /极简黑/ }).click();
  await expect(page.locator(".appearance-settings-preview")).toHaveCSS("--preview-settings-window", "#000000");
  await page.locator(".appearance-edit-footer .primary").click();
  await page.getByRole("button", { name: "保存设置" }).click();
  await expect.poll(() => page.evaluate(() => window.__suoMock.config.settingsTheme.theme)).toBe("black");
  await expect(page.locator(".settings-stage")).toHaveCSS("--settings-opacity", "100%");
  await expect(page.locator(".settings-stage")).toHaveCSS("--settings-blur", "0px");
  await expect(page.locator(".settings-stage")).toHaveCSS("--settings-shadow-opacity", "0");
  await expect(page.locator(".settings-stage")).toHaveCSS("--settings-scope-launcher", "#eeeeee");
  await page.screenshot({ path: testInfo.outputPath("minimal-black-appearance.png") });
  await nav(page, "通用");
  await expect(page.locator(".settings-card")).toHaveCSS("background-color", "rgb(0, 0, 0)");
  await expect(page.locator(".settings-sidebar")).toHaveCSS("background-color", "rgb(0, 0, 0)");
  await page.screenshot({ path: testInfo.outputPath("minimal-black-settings.png") });
});


test("minimal black renders the actual launcher search box and neutral results", async ({ page }, testInfo) => {
  await page.setViewportSize({ width: Math.min(720, page.viewportSize().width), height: 520 });
  const config = fixtureConfig();
  config.launcherTheme = { ...config.launcherTheme, theme: "black", accentColor: "#707070" };
  await installMockBridge(page, { config, windowLabel: "main", searchResults: [{
    id: "black-test-calculator", kind: "calculator", title: "12", subtitle: "11 + 1", badge: "计算",
    iconDataUrl: "", resultImageDataUrl: "", score: 1, action: { type: "copyText", text: "12" },
  }] });
  await page.goto("/");
  await expect(page.locator(".launcher")).toBeVisible();
  await expect(page.locator(".search-box")).toHaveCSS("background-color", "rgb(0, 0, 0)");
  await expect(page.locator(".launcher")).toHaveCSS("--launcher-window-opacity", "100%");
  await expect(page.locator(".launcher")).toHaveCSS("--launcher-window-blur", "0px");
  await expect(page.locator(".launcher")).toHaveCSS("--launcher-window-shadow-opacity", "0");
  await expect(page.locator(".brand-button")).toBeHidden();
  await expect(page.locator(".search-icon")).toBeHidden();
  await page.locator(".search-box input").fill("11+1");
  await expect(page.locator(".result-copy strong")).toHaveText("12");
  await expect(page.locator(".provider-row")).toBeHidden();
  await expect(page.locator(".launcher > footer")).toBeHidden();
  await expect(page.locator(".search-box")).toHaveCSS("box-shadow", "none");
  const frames = await page.evaluate(() => ({ search:document.querySelector(".search-box").getBoundingClientRect().bottom, results:document.querySelector(".results").getBoundingClientRect().top }));
  expect(frames.results - frames.search).toBeLessThan(15);
  await expect(page.locator(".result-badge")).toBeHidden();
  await expect(page.locator(".result-icon.calculator")).toHaveCSS("background-image", "none");
  await expect(page.locator(".result-icon.calculator")).toHaveCSS("background-color", "rgb(24, 24, 24)");
  await expect(page.locator(".toast")).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)).toBe(false);
  await page.screenshot({ path: testInfo.outputPath("minimal-black-launcher.png") });
});


test("custom launcher skins expose independent status and footer controls", async ({ page }) => {
  await boot(page);
  await appearance(page);
  await openCustomSkin(page, "launcher", "Test Launcher");
  const preview = page.locator(".appearance-launcher-preview");
  const statusToggle = page.getByRole("checkbox", { name: "显示来源状态栏" });
  const footerToggle = page.getByRole("checkbox", { name: "显示底部快捷键栏" });
  await expect(statusToggle).toBeChecked();
  await expect(footerToggle).toBeChecked();
  await statusToggle.uncheck();
  await expect(preview.locator(".appearance-live-provider")).toBeHidden();
  await expect(preview.locator(".appearance-live-footer")).toBeVisible();
  await footerToggle.uncheck();
  await expect(preview.locator(".appearance-live-footer")).toBeHidden();
  await page.locator(".appearance-edit-footer .primary").click();
  await page.getByRole("button", { name: "保存设置" }).click();
  const saved = await page.evaluate(() => window.__suoMock.config.launcherTheme.customThemes.find(t => t.id === "launcher-test"));
  expect(saved.showProviderStatus).toBe(false);
  expect(saved.showFooterHints).toBe(false);
});

test("custom skin capability flags control launcher rows without theme-name checks", async ({ page }) => {
  const config = fixtureConfig();
  config.launcherTheme.theme = "custom:launcher-test";
  await installMockBridge(page, { config, windowLabel:"main" });
  await page.goto("/");
  await expect(page.locator(".provider-row")).toBeVisible();
  await expect(page.locator(".launcher > footer")).toBeVisible();
  await page.evaluate(() => {
    const next = window.__suoMock.config;
    next.launcherTheme.customThemes[0].showProviderStatus = false;
    window.__suoMock.emit("app-config-updated", next);
  });
  await expect(page.locator(".provider-row")).toBeHidden();
  await expect(page.locator(".launcher > footer")).toBeVisible();
  await page.evaluate(() => {
    const next = window.__suoMock.config;
    next.launcherTheme.customThemes[0].showFooterHints = false;
    next.launcherTheme.customThemes[0].searchBorderStyle = "none";
    window.__suoMock.emit("app-config-updated", next);
  });
  await expect(page.locator(".provider-row")).toBeVisible();
  await expect(page.locator(".launcher > footer")).toBeHidden();
  await page.locator(".search-box input").focus();
  await expect(page.locator(".search-box")).toHaveCSS("box-shadow", "none");
});


test("legacy launcher bundles upgrade chrome flags and v2 exports preserve them", async ({ page }) => {
  await boot(page);
  await appearance(page);
  const legacy = launcherSkin("legacy", "Legacy Launcher");
  delete legacy.id;
  delete legacy.showProviderStatus;
  delete legacy.showFooterHints;
  await page.locator(".appearance-hidden-input[type=file]").setInputFiles({ name:"legacy.json", mimeType:"application/json", buffer:Buffer.from(JSON.stringify({ schema:"suo-launcher-theme-v1", version:1, theme:legacy })) });
  await expect(page.locator(".appearance-theme-copy strong")).toHaveText("Legacy Launcher");
  await expect(page.getByRole("checkbox", {name:"显示来源状态栏"})).toBeChecked();
  await expect(page.getByRole("checkbox", {name:"显示底部快捷键栏"})).toBeChecked();
  await page.getByRole("checkbox", {name:"显示底部快捷键栏"}).uncheck();
  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", {name:"导出当前界面皮肤"}).click();
  const download = await downloadPromise;
  const { readFile } = await import("node:fs/promises");
  const bundle = JSON.parse(await readFile(await download.path(), "utf8"));
  expect(bundle.schema).toBe("suo-launcher-theme-v2");
  expect(bundle.version).toBe(2);
  expect(bundle.theme.showProviderStatus).toBe(true);
  expect(bundle.theme.showFooterHints).toBe(false);
});
