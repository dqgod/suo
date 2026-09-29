// An isolated Tauri bridge for browser-only UI regression tests. No disk,
// credential store, process, or native window command is performed.
const material = {
  windowOpacity: 100, blurPx: 0, shadowPercent: 30,
  wallpaperDataUrl: "", wallpaperOpacity: 0,
  platformOverrides: { enabled: false, windowsBlurPx: 0, windowsOpacity: 100, macosBlurPx: 0, macosOpacity: 100 },
};

export function launcherSkin(id, name) {
  return {
    id, name, accentColor: "#8a78ff", windowBackground: "#0b1222", windowBorder: "#66728f",
    windowBorderWidthPx: 1, windowWidthPx: 720, windowRadiusPx: 18,
    searchBackground: "#161f39", searchBorder: "#66728f", searchBorderWidthPx: 1,
    searchBorderStyle: "solid", searchLeftSpacePx: 30, searchVerticalSpacePx: 9,
    searchWidthPx: 720, searchTextColor: "#f5f7ff", searchFontSizePx: 20,
    normalRowBackground: "#0b1222", normalPrimaryColor: "#f5f7ff", normalSecondaryColor: "#91a0c7",
    normalPrimaryFontSizePx: 14, normalSecondaryFontSizePx: 12, normalRowHeightPx: 58,
    selectedRowBackground: "#302b63", selectedPrimaryColor: "#f5f7ff", selectedSecondaryColor: "#91a0c7",
    selectedPrimaryFontSizePx: 14, selectedSecondaryFontSizePx: 12, iconSizePx: 32,
    showSearchIcon: true, showLogo: true, showSourceBadge: true, showProviderStatus: true, showFooterHints: true, maxResults: 8,
    ...structuredClone(material),
  };
}

export function settingsSkin(id, name) {
  return {
    id, name, accentColor: "#236749", windowBackground: "#fcfcfa", titlebarBackground: "#ffffff",
    sidebarBackground: "#192b25", contentBackground: "#fcfcfa", cardBackground: "#ffffff",
    borderColor: "#e3e8e2", primaryTextColor: "#25342d", secondaryTextColor: "#737f76",
    navTextColor: "#bcc9c1", selectedNavBackground: "#31483b", baseFontSizePx: 14, radiusPx: 10,
    ...structuredClone(material),
  };
}

export function fixtureConfig(overrides = {}) {
  const config = {
    version: 20, saveSettingsManually: true, settingsIconStyle: "transparentColor",
    launcher: {
      globalHotkey: "alt+Space", startAtLogin: false, closeOnBlur: true, keepLastInput: false,
      compactWhenEmpty: false, showDockIcon: true, emptyQueryDebounceMs: 0, nonEmptyQueryDebounceMs: 50,
      windowWidthPx: null, windowHeightPx: 520, horizontalOffsetPx: 0, verticalOffsetPx: 0,
      terminal: { enabled: true, windowsShell: "powerShell", macosTerminalApplication: "Terminal" },
    },
    translation: {
      enabled: true, keyword: "fy", description: "Translate text", aliases: [], provider: "microsoft",
      region: "", defaultTargetLanguage: "zh-Hans", chineseTargetLanguage: "en",
    },
    scriptCommands: [{
      id: "sample-script", name: "Sample Script", keyword: "sample", description: "Local example",
      iconDataUrl: "", inputHint: "", aliases: [], enabled: true, runtime: "python", scriptPath: "scripts/sample.py",
      resultAction: "copy", immediate: true, debounceMs: 50, timeoutMs: 3000,
    }],
    webSearches: [{
      id: "sample-web", name: "Sample Web", keyword: "sampleweb", description: "Example web command",
      iconDataUrl: "", inputHint: "", aliases: [], enabled: true, urlTemplate: "https://example.com/?q={query}",
    }],
    launcherTheme: { theme: "midnight", accentColor: "#8a78ff", customThemes: [launcherSkin("launcher-test", "Test Launcher")] },
    settingsTheme: { theme: "forest", accentColor: "#236749", customThemes: [settingsSkin("settings-test", "Test Settings")] },
  };
  return { ...config, ...overrides };
}

export async function installMockBridge(page, options = {}) {
  const config = fixtureConfig(options.config);
  await page.addInitScript(({ initialConfig, platform, windowLabel, searchResults }) => {
    if (platform === "mac") {
      Object.defineProperty(navigator, "platform", { configurable: true, get: () => "MacIntel" });
      Object.defineProperty(navigator, "userAgent", { configurable: true, get: () => "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_0) AppleWebKit/537.36 Chrome/120 Safari/537.36" });
    }
    if (platform === "windows") Object.defineProperty(navigator, "platform", { configurable: true, get: () => "Win32" });
    const clone = (value) => structuredClone(value);
    let current = clone(initialConfig);
    let eventId = 0;
    const callbacks = new Map();
    const listeners = new Map();
    const calls = [];
    const saves = [];
    const view = () => ({
      config: clone(current), configFilePath: "/suo-test/config.json", configDirectory: "/suo-test",
      defaultConfigFilePath: "/suo-test/config.json", defaultConfigDirectory: "/suo-test",
      usingDefaultConfigLocation: true, configLocationNeedsReset: false,
      translationCredentialStatus: { microsoft: false, google: false, youdao: false },
      credentialStoreError: null, configLoadWarning: null, needsLegacyPreferencesMigration: false,
      configReadOnly: false,
    });
    window.__suoMock = {
      calls, saves, get config() { return clone(current); }, failNextSave: false,
      emit(event, payload = null) {
        for (const callbackId of listeners.get(event) ?? []) callbacks.get(callbackId)?.({ event, payload, id: 1, windowLabel: "settings" });
      },
    };
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: windowLabel } },
      transformCallback: (callback) => { const id = ++eventId; callbacks.set(id, callback); return id; },
      invoke: async (command, args = {}) => {
        calls.push({ command, args: clone(args) });
        if (command === "plugin:event|listen") {
          const id = ++eventId;
          listeners.set(args.event, [...(listeners.get(args.event) ?? []), args.handler]);
          return id;
        }
        if (command === "plugin:event|unlisten") return undefined;
        if (command === "get_app_config") return view();
        if (command === "get_index_status") return { indexing: false, indexedFileCount: 0 };
        if (command === "cancel_search") return { actionEpoch: args.generation };
        if (command === "set_launcher_compact" || command === "hide_launcher") return undefined;
        if (command === "get_app_icon") return null;
        if (command === "search_launcher") return {
          query: args.query, provider: "本地搜索", providerDetail: "应用与文件", hotkeyStatus: "Alt + Space 已就绪",
          indexing: false, indexedFileCount: 0, actionEpoch: args.generation,
          results: args.query ? clone(searchResults) : [],
        };
        if (command === "validate_app_config") return undefined;
        if (command === "save_app_config") {
          if (window.__suoMock.failNextSave) { window.__suoMock.failNextSave = false; throw new Error("Mock save failed"); }
          current = clone(args.config); saves.push(clone(current)); return view();
        }
        if (command === "hide_settings" || command === "set_hotkey_recording" || command === "rebuild_file_index") return undefined;
        throw new Error(`Unexpected Tauri command: ${command}`);
      },
    };
  }, { initialConfig: config, platform: options.platform ?? "windows", windowLabel: options.windowLabel ?? "settings", searchResults: options.searchResults ?? [] });
}
