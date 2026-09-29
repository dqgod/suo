import { invoke } from "@tauri-apps/api/core";

export type LauncherConfig = {
  globalHotkey: string;
  startAtLogin: boolean;
  closeOnBlur: boolean;
  keepLastInput: boolean;
  compactWhenEmpty: boolean;
  showDockIcon: boolean;
  emptyQueryDebounceMs: number;
  nonEmptyQueryDebounceMs: number;
  windowWidthPx: number | null;
  windowHeightPx: number;
  horizontalOffsetPx: number;
  verticalOffsetPx: number;
  terminal: TerminalCommandConfig;
};

export type WindowsTerminalShell = "powerShell" | "commandPrompt";

export type TerminalCommandConfig = {
  enabled: boolean;
  windowsShell: WindowsTerminalShell;
  macosTerminalApplication: string;
};

export type TranslationProvider = "microsoft" | "google" | "youdao";
export type SettingsIconStyle = "transparentColor" | "monochrome" | "original";

export type TranslationConfig = {
  enabled: boolean;
  keyword: string;
  description: string;
  aliases: string[];
  provider: TranslationProvider;
  region: string;
  defaultTargetLanguage: string;
  chineseTargetLanguage: string;
};

export type ScriptRuntime = "python" | "powerShell" | "bash" | "executable";
export type ScriptResultAction = "copy" | "executeShell";

export type ScriptCommandConfig = {
  id: string;
  name: string;
  keyword: string;
  description: string;
  iconDataUrl: string;
  inputHint: string;
  aliases: string[];
  enabled: boolean;
  runtime: ScriptRuntime;
  scriptPath: string;
  resultAction: ScriptResultAction;
  immediate: boolean;
  debounceMs: number;
  timeoutMs: number;
};

export type WebSearchConfig = {
  id: string;
  name: string;
  keyword: string;
  description: string;
  iconDataUrl: string;
  inputHint: string;
  aliases: string[];
  enabled: boolean;
  urlTemplate: string;
};

export type BuiltinThemeId = "midnight" | "paper" | "forest" | "black";
export type ThemeSelection = BuiltinThemeId | `custom:${string}`;
export type SearchBorderStyle = "solid" | "dashed" | "dotted" | "double" | "none";

export type PlatformThemeOverrides = {
  enabled: boolean;
  windowsBlurPx: number;
  windowsOpacity: number;
  macosBlurPx: number;
  macosOpacity: number;
};

/** Shared material properties are copied per scope and never shared by reference. */
export type ThemeBackgroundConfig = {
  windowOpacity: number;
  blurPx: number;
  shadowPercent: number;
  wallpaperDataUrl: string;
  wallpaperOpacity: number;
  platformOverrides: PlatformThemeOverrides;
};

export type LauncherCustomThemeConfig = ThemeBackgroundConfig & {
  id: string;
  name: string;
  accentColor: string;
  windowBackground: string;
  windowBorder: string;
  windowBorderWidthPx: number;
  windowWidthPx: number;
  windowRadiusPx: number;
  searchBackground: string;
  searchBorder: string;
  searchBorderWidthPx: number;
  searchBorderStyle: SearchBorderStyle;
  searchLeftSpacePx: number;
  searchVerticalSpacePx: number;
  searchWidthPx: number;
  searchTextColor: string;
  searchFontSizePx: number;
  normalRowBackground: string;
  normalPrimaryColor: string;
  normalSecondaryColor: string;
  normalPrimaryFontSizePx: number;
  normalSecondaryFontSizePx: number;
  normalRowHeightPx: number;
  selectedRowBackground: string;
  selectedPrimaryColor: string;
  selectedSecondaryColor: string;
  selectedPrimaryFontSizePx: number;
  selectedSecondaryFontSizePx: number;
  iconSizePx: number;
  showSearchIcon: boolean;
  showLogo: boolean;
  /** Kept when v5 local configuration is migrated; launcher-only. */
  showSourceBadge: boolean;
  showProviderStatus: boolean;
  showFooterHints: boolean;
  maxResults: 6 | 8 | 10 | 12;
};

export type SettingsCustomThemeConfig = ThemeBackgroundConfig & {
  id: string;
  name: string;
  accentColor: string;
  windowBackground: string;
  titlebarBackground: string;
  sidebarBackground: string;
  contentBackground: string;
  cardBackground: string;
  borderColor: string;
  primaryTextColor: string;
  secondaryTextColor: string;
  navTextColor: string;
  selectedNavBackground: string;
  baseFontSizePx: number;
  radiusPx: number;
};

export type LauncherThemeConfig = {
  theme: ThemeSelection;
  accentColor: string;
  customThemes: LauncherCustomThemeConfig[];
};

export type SettingsThemeConfig = {
  theme: ThemeSelection;
  accentColor: string;
  customThemes: SettingsCustomThemeConfig[];
};

export const builtinThemeIds: readonly BuiltinThemeId[] = ["midnight", "paper", "forest", "black"];

/** Persisted visual limits shared by the editor and strict theme imports. */
export const visualBounds = {
  launcher: {
    windowWidthPx: { min: 256, max: 2560 },
    windowHeightPx: { min: 160, max: 2160 },
    horizontalOffsetPx: { min: -8192, max: 8192 },
    verticalOffsetPx: { min: -8192, max: 8192 },
  },
  launcherTheme: {
    windowWidthPx: { min: 256, max: 2560 },
    windowRadiusPx: { min: 0, max: 255 },
    searchBorderWidthPx: { min: 0, max: 32 },
    searchLeftSpacePx: { min: 0, max: 128 },
    searchVerticalSpacePx: { min: 0, max: 64 },
    searchWidthPx: { min: 128, max: 2560 },
    searchFontSizePx: { min: 1, max: 255 },
    normalPrimaryFontSizePx: { min: 1, max: 255 },
    normalSecondaryFontSizePx: { min: 1, max: 255 },
    normalRowHeightPx: { min: 1, max: 1024 },
    selectedPrimaryFontSizePx: { min: 1, max: 255 },
    selectedSecondaryFontSizePx: { min: 1, max: 255 },
    iconSizePx: { min: 1, max: 255 },
  },
  settingsTheme: {
    baseFontSizePx: { min: 1, max: 255 },
    radiusPx: { min: 0, max: 255 },
  },
} as const;

const defaultPlatformOverrides = (): PlatformThemeOverrides => ({
  enabled: false,
  windowsBlurPx: 18,
  windowsOpacity: 94,
  macosBlurPx: 18,
  macosOpacity: 94,
});

const defaultBackground = (opacity: number, blurPx: number, shadowPercent: number): ThemeBackgroundConfig => ({
  windowOpacity: opacity,
  blurPx,
  shadowPercent,
  wallpaperDataUrl: "",
  wallpaperOpacity: 0,
  platformOverrides: defaultPlatformOverrides(),
});

type ThemePalette = {
  name: string;
  window: string;
  panel: string;
  field: string;
  text: string;
  muted: string;
  accent: string;
  selected: string;
  border: string;
  opacity: number;
  blur: number;
  shadow: number;
};

const palettes: Record<BuiltinThemeId, ThemePalette> = {
  black: {
    name: "Minimal Black", window: "#000000", panel: "#000000", field: "#000000", text: "#eeeeee",
    muted: "#a0a0a0", accent: "#707070", selected: "#181818", border: "#303030", opacity: 100, blur: 0, shadow: 0,
  },
  midnight: {
    name: "Midnight", window: "#0b1222", panel: "#101a30", field: "#161f39", text: "#f5f7ff",
    muted: "#91a0c7", accent: "#8a78ff", selected: "#302b63", border: "#66728f", opacity: 96, blur: 18, shadow: 45,
  },
  paper: {
    name: "Paper", window: "#e9e5dd", panel: "#ffffff", field: "#eee9df", text: "#272b35",
    muted: "#586176", accent: "#5a61e6", selected: "#d9dcf8", border: "#85817a", opacity: 98, blur: 8, shadow: 30,
  },
  forest: {
    name: "Forest", window: "#071610", panel: "#102b20", field: "#142f25", text: "#eefaf3",
    muted: "#94b8a5", accent: "#62d89c", selected: "#184b37", border: "#587d69", opacity: 96, blur: 18, shadow: 45,
  },
};

function builtinLauncherTheme(id: BuiltinThemeId): LauncherCustomThemeConfig {
  const palette = palettes[id];
  return {
    id,
    name: palette.name,
    accentColor: palette.accent,
    windowBackground: palette.window,
    windowBorder: palette.border,
    windowBorderWidthPx: 1,
    windowWidthPx: id === "black" ? 560 : 720,
    windowRadiusPx: id === "black" ? 10 : 18,
    searchBackground: palette.field,
    searchBorder: palette.border,
    searchBorderWidthPx: id === "black" ? 0 : 1,
    searchBorderStyle: id === "black" ? "none" : "solid",
    searchLeftSpacePx: 30,
    searchVerticalSpacePx: 9,
    searchWidthPx: id === "black" ? 560 : 720,
    searchTextColor: palette.text,
    searchFontSizePx: 20,
    normalRowBackground: palette.window,
    normalPrimaryColor: palette.text,
    normalSecondaryColor: palette.muted,
    normalPrimaryFontSizePx: 14,
    normalSecondaryFontSizePx: 12,
    normalRowHeightPx: 58,
    selectedRowBackground: palette.selected,
    selectedPrimaryColor: palette.text,
    selectedSecondaryColor: palette.muted,
    selectedPrimaryFontSizePx: 14,
    selectedSecondaryFontSizePx: 12,
    iconSizePx: id === "black" ? 24 : 32,
    showSearchIcon: id !== "black",
    showLogo: id !== "black",
    showSourceBadge: id !== "black",
    showProviderStatus: id !== "black",
    showFooterHints: id !== "black",
    maxResults: 8,
    ...defaultBackground(palette.opacity, palette.blur, palette.shadow),
    ...(id === "black" ? { platformOverrides: { enabled: false, windowsBlurPx: 0, windowsOpacity: 100, macosBlurPx: 0, macosOpacity: 100 } } : {}),
  };
}

function builtinSettingsTheme(id: BuiltinThemeId): SettingsCustomThemeConfig {
  const palette = palettes[id];
  // The Forest settings canvas follows the light-content/dark-sidebar layout.
  // Launcher Forest keeps its independent dark palette.
  if (id === "forest") return {
    id, name: palette.name, accentColor: "#236749",
    windowBackground: "#fcfcfa", titlebarBackground: "#ffffff",
    sidebarBackground: "#192b25", contentBackground: "#fcfcfa",
    cardBackground: "#ffffff", borderColor: "#e3e8e2",
    primaryTextColor: "#25342d", secondaryTextColor: "#737f76",
    navTextColor: "#bcc9c1", selectedNavBackground: "#31483b",
    baseFontSizePx: 14, radiusPx: 10,
    ...defaultBackground(100, 0, 30),
  };
  return {
    id,
    name: palette.name,
    accentColor: palette.accent,
    windowBackground: palette.window,
    titlebarBackground: palette.panel,
    sidebarBackground: palette.field,
    contentBackground: palette.window,
    cardBackground: palette.panel,
    borderColor: palette.border,
    primaryTextColor: palette.text,
    secondaryTextColor: palette.muted,
    navTextColor: palette.text,
    selectedNavBackground: palette.selected,
    baseFontSizePx: 14,
    radiusPx: id === "black" ? 10 : 18,
    ...defaultBackground(palette.opacity, palette.blur, palette.shadow),
    ...(id === "black" ? { platformOverrides: { enabled: false, windowsBlurPx: 0, windowsOpacity: 100, macosBlurPx: 0, macosOpacity: 100 } } : {}),
  };
}

function cloneBackground(background: ThemeBackgroundConfig): ThemeBackgroundConfig {
  return { ...background, platformOverrides: { ...background.platformOverrides } };
}

function cloneLauncherTheme(theme: LauncherCustomThemeConfig): LauncherCustomThemeConfig {
  return { ...theme, ...cloneBackground(theme) };
}

function cloneSettingsTheme(theme: SettingsCustomThemeConfig): SettingsCustomThemeConfig {
  return { ...theme, ...cloneBackground(theme) };
}

function createThemeId(scope: "launcher" | "settings") {
  const uuid = globalThis.crypto?.randomUUID?.();
  return uuid ? `${scope}-theme-${uuid}` : `${scope}-theme-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function asBuiltinThemeId(value: string): BuiltinThemeId {
  return builtinThemeIds.includes(value as BuiltinThemeId) ? value as BuiltinThemeId : "midnight";
}

export function createLauncherTheme(sourceId: string = "midnight"): LauncherCustomThemeConfig {
  const theme = builtinLauncherTheme(asBuiltinThemeId(sourceId));
  return { ...theme, id: createThemeId("launcher"), name: "Custom launcher theme" };
}

export function createSettingsTheme(sourceId: string = "forest"): SettingsCustomThemeConfig {
  const theme = builtinSettingsTheme(asBuiltinThemeId(sourceId));
  return { ...theme, id: createThemeId("settings"), name: "Custom settings theme" };
}

export function resolveLauncherTheme(scope: LauncherThemeConfig): LauncherCustomThemeConfig {
  if (scope.theme.startsWith("custom:")) {
    const id = scope.theme.slice("custom:".length).toLowerCase();
    const custom = scope.customThemes.find((theme) => theme.id.toLowerCase() === id);
    if (custom) return cloneLauncherTheme(custom);
  }
  const builtin = builtinLauncherTheme(asBuiltinThemeId(scope.theme));
  // The scope-level accent remains a compatibility override for migrated
  // built-ins. Custom themes carry their own accent and never inherit it.
  return { ...builtin, accentColor: scope.accentColor || builtin.accentColor };
}

export function resolveSettingsTheme(scope: SettingsThemeConfig): SettingsCustomThemeConfig {
  if (scope.theme.startsWith("custom:")) {
    const id = scope.theme.slice("custom:".length).toLowerCase();
    const custom = scope.customThemes.find((theme) => theme.id.toLowerCase() === id);
    if (custom) return cloneSettingsTheme(custom);
  }
  const builtin = builtinSettingsTheme(asBuiltinThemeId(scope.theme));
  return { ...builtin, accentColor: scope.accentColor || builtin.accentColor };
}

export type AppConfig = {
  version: number;
  saveSettingsManually: boolean;
  settingsIconStyle: SettingsIconStyle;
  launcher: LauncherConfig;
  translation: TranslationConfig;
  scriptCommands: ScriptCommandConfig[];
  webSearches: WebSearchConfig[];
  launcherTheme: LauncherThemeConfig;
  settingsTheme: SettingsThemeConfig;
};

export type AppConfigView = {
  config: AppConfig;
  configFilePath: string;
  configDirectory: string;
  defaultConfigFilePath: string;
  defaultConfigDirectory: string;
  usingDefaultConfigLocation: boolean;
  configLocationNeedsReset: boolean;
  translationCredentialStatus: Record<TranslationProvider, boolean>;
  credentialStoreError: string | null;
  configLoadWarning: string | null;
  needsLegacyPreferencesMigration: boolean;
  configReadOnly: boolean;
};

const legacyPreferencesKey = "suo.launcher.preferences.v1";

export async function loadAppConfig() {
  const view = await invoke<AppConfigView>("get_app_config");
  if (!view.needsLegacyPreferencesMigration) return view;
  const legacy = window.localStorage.getItem(legacyPreferencesKey);
  if (!legacy) return view;
  try {
    const preferences = JSON.parse(legacy) as Partial<LauncherConfig>;
    const config: AppConfig = {
      ...view.config,
      launcher: {
        ...view.config.launcher,
        closeOnBlur: typeof preferences.closeOnBlur === "boolean" ? preferences.closeOnBlur : view.config.launcher.closeOnBlur,
        keepLastInput: typeof preferences.keepLastInput === "boolean" ? preferences.keepLastInput : view.config.launcher.keepLastInput,
      },
    };
    const migrated = await invoke<AppConfigView>("save_app_config", { config });
    window.localStorage.removeItem(legacyPreferencesKey);
    return migrated;
  } catch {
    return view;
  }
}

function platformMaterial(theme: ThemeBackgroundConfig) {
  const isMac = /Mac/i.test(navigator.platform);
  const isWindows = /Win/i.test(navigator.platform);
  const overrides = theme.platformOverrides;
  return {
    blurPx: overrides.enabled && isMac ? overrides.macosBlurPx : overrides.enabled && isWindows ? overrides.windowsBlurPx : theme.blurPx,
    opacity: overrides.enabled && isMac ? overrides.macosOpacity : overrides.enabled && isWindows ? overrides.windowsOpacity : theme.windowOpacity,
  };
}

function wallpaperImage(dataUrl: string) {
  return /^data:image\/(?:png|jpeg|webp);base64,[A-Za-z0-9+/]+={0,2}$/.test(dataUrl) ? `url("${dataUrl}")` : "none";
}

function setCssVariables(values: Record<string, string>) {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(values)) root.style.setProperty(name, value);
}

/** Applies only launcher variables; it cannot mutate settings tokens. */
export function applyLauncherAppearance(scope: LauncherThemeConfig, launcher?: LauncherConfig) {
  const theme = resolveLauncherTheme(scope);
  const material = platformMaterial(theme);
  const windowWidthPx = launcher?.windowWidthPx ?? theme.windowWidthPx;
  const searchHorizontalMargin = Math.max(0, theme.windowWidthPx - theme.searchWidthPx);
  const searchWidthPx = Math.max(visualBounds.launcherTheme.searchWidthPx.min, Math.min(windowWidthPx, windowWidthPx - searchHorizontalMargin));
  const searchSideGapPx = Math.min(12, theme.searchLeftSpacePx);
  const root = document.documentElement;
  root.dataset.launcherNeutralChrome = String([theme.accentColor, theme.windowBackground, theme.searchBackground].every(isNeutralColor));
  root.dataset.launcherShowSearchIcon = String(theme.showSearchIcon);
  root.dataset.launcherShowLogo = String(theme.showLogo);
  root.dataset.launcherShowSourceBadge = String(theme.showSourceBadge);
  root.dataset.launcherShowProviderStatus = String(theme.showProviderStatus);
  root.dataset.launcherShowFooterHints = String(theme.showFooterHints);
  root.dataset.launcherSearchBorderEnabled = String(theme.searchBorderWidthPx > 0 && theme.searchBorderStyle !== "none");
  setCssVariables({
    "--launcher-window-bg": theme.windowBackground,
    "--launcher-window-border": theme.windowBorder,
    "--launcher-window-border-width": `${theme.windowBorderWidthPx}px`,
    "--launcher-window-radius": `${theme.windowRadiusPx}px`,
    "--launcher-window-opacity": `${material.opacity}%`,
    "--launcher-window-blur": `${material.blurPx}px`,
    "--launcher-window-shadow-opacity": String(theme.shadowPercent / 100),
    "--launcher-wallpaper-image": wallpaperImage(theme.wallpaperDataUrl),
    "--launcher-wallpaper-opacity": String(theme.wallpaperOpacity / 100),
    "--launcher-search-bg": theme.searchBackground,
    "--launcher-search-border": theme.searchBorder,
    "--launcher-search-border-width": `${theme.searchBorderWidthPx}px`,
    "--launcher-search-border-style": theme.searchBorderStyle,
    "--launcher-search-side-gap": `${searchSideGapPx}px`,
    "--launcher-search-left-padding": `${theme.searchLeftSpacePx - searchSideGapPx}px`,
    "--launcher-search-vertical-gap": `${theme.searchVerticalSpacePx}px`,
    "--launcher-search-row-min-height": `${74 + 2 * (theme.searchVerticalSpacePx - 9)}px`,
    "--launcher-search-width": `${searchWidthPx}px`,
    "--launcher-search-text": theme.searchTextColor,
    "--launcher-search-font-size": `${theme.searchFontSizePx}px`,
    "--launcher-result-bg": theme.normalRowBackground,
    "--launcher-result-primary": theme.normalPrimaryColor,
    "--launcher-result-secondary": theme.normalSecondaryColor,
    "--launcher-result-primary-size": `${theme.normalPrimaryFontSizePx}px`,
    "--launcher-result-secondary-size": `${theme.normalSecondaryFontSizePx}px`,
    "--launcher-result-height": `${theme.normalRowHeightPx}px`,
    "--launcher-selected-bg": theme.selectedRowBackground,
    "--launcher-selected-primary": theme.selectedPrimaryColor,
    "--launcher-selected-secondary": theme.selectedSecondaryColor,
    "--launcher-selected-primary-size": `${theme.selectedPrimaryFontSizePx}px`,
    "--launcher-selected-secondary-size": `${theme.selectedSecondaryFontSizePx}px`,
    "--launcher-icon-size": `${theme.iconSizePx}px`,
    // Accent is carried by each custom theme, keeping launcher bundles
    // visually self-contained instead of inheriting a settings-side token.
    "--launcher-accent": theme.accentColor,
    "--launcher-width": `${windowWidthPx}px`,
  });
  return theme;
}

function isNeutralColor(hex: string) {
  return /^#[0-9a-f]{6}$/i.test(hex) && hex.slice(1, 3).toLowerCase() === hex.slice(3, 5).toLowerCase() && hex.slice(3, 5).toLowerCase() === hex.slice(5, 7).toLowerCase();
}

/** Applies only settings variables; it cannot mutate launcher tokens. */
export function applySettingsAppearance(scope: SettingsThemeConfig) {
  const theme = resolveSettingsTheme(scope);
  const material = platformMaterial(theme);
  const neutralChrome = [theme.accentColor, theme.windowBackground, theme.sidebarBackground, theme.cardBackground].every(isNeutralColor);
  setCssVariables({
    "--settings-window-bg": theme.windowBackground,
    "--settings-titlebar-bg": theme.titlebarBackground,
    "--settings-sidebar-bg": theme.sidebarBackground,
    "--settings-content-bg": theme.contentBackground,
    "--settings-card-bg": theme.cardBackground,
    "--settings-border": theme.borderColor,
    "--settings-primary-text": theme.primaryTextColor,
    "--settings-secondary-text": theme.secondaryTextColor,
    "--settings-nav-text": theme.navTextColor,
    "--settings-nav-selected-bg": theme.selectedNavBackground,
    "--settings-font-size": `${theme.baseFontSizePx}px`,
    "--settings-radius": `${theme.radiusPx}px`,
    "--settings-opacity": `${material.opacity}%`,
    "--settings-blur": `${material.blurPx}px`,
    "--settings-shadow-opacity": String(theme.shadowPercent / 100),
    "--settings-wallpaper-image": wallpaperImage(theme.wallpaperDataUrl),
    "--settings-wallpaper-opacity": String(theme.wallpaperOpacity / 100),
    "--settings-accent": theme.accentColor,
    // Neutral skins keep editor chrome monochrome, including copies/imports.
    // Native app icons and semantic danger/warning colours remain independent.
    "--settings-scope-launcher": neutralChrome ? theme.primaryTextColor : "#8a78ff",
    "--settings-scope-settings": neutralChrome ? theme.primaryTextColor : "#48d5b0",
    "--settings-success": neutralChrome ? "#a0a0a0" : "#65dca9",
    "--settings-success-foreground": neutralChrome ? "#000000" : "#ffffff",
  });
  return theme;
}

type LauncherThemeImport = Omit<LauncherCustomThemeConfig, "id">;
type LauncherThemeImportV1 = Omit<LauncherThemeImport, "showProviderStatus" | "showFooterHints" | "searchLeftSpacePx" | "searchVerticalSpacePx">;
type LauncherThemeImportV2 = Omit<LauncherThemeImport, "searchLeftSpacePx" | "searchVerticalSpacePx">;
type SettingsThemeImport = Omit<SettingsCustomThemeConfig, "id">;

export type LauncherThemeBundleV1 = {
  schema: "suo-launcher-theme-v1";
  version: 1;
  theme: LauncherThemeImportV1;
};

export type LauncherThemeBundleV2 = {
  schema: "suo-launcher-theme-v2";
  version: 2;
  theme: LauncherThemeImportV2;
};

export type LauncherThemeBundleV3 = {
  schema: "suo-launcher-theme-v3";
  version: 3;
  theme: LauncherThemeImport;
};

export type SettingsThemeBundleV1 = {
  schema: "suo-settings-theme-v1";
  version: 1;
  theme: SettingsThemeImport;
};

const launcherBundleFieldsV1 = [
  "name", "accentColor", "windowBackground", "windowBorder", "windowBorderWidthPx", "windowWidthPx", "windowRadiusPx",
  "searchBackground", "searchBorder", "searchBorderWidthPx", "searchBorderStyle", "searchWidthPx", "searchTextColor", "searchFontSizePx",
  "normalRowBackground", "normalPrimaryColor", "normalSecondaryColor", "normalPrimaryFontSizePx", "normalSecondaryFontSizePx", "normalRowHeightPx",
  "selectedRowBackground", "selectedPrimaryColor", "selectedSecondaryColor", "selectedPrimaryFontSizePx", "selectedSecondaryFontSizePx",
  "iconSizePx", "showSearchIcon", "showLogo", "showSourceBadge", "maxResults",
  "windowOpacity", "blurPx", "shadowPercent", "wallpaperDataUrl", "wallpaperOpacity", "platformOverrides",
] as const;
const launcherBundleFieldsV2 = [...launcherBundleFieldsV1, "showProviderStatus", "showFooterHints"] as const;
const launcherBundleFieldsV3 = [...launcherBundleFieldsV2, "searchLeftSpacePx", "searchVerticalSpacePx"] as const;

const settingsBundleFields = [
  "name", "accentColor", "windowBackground", "titlebarBackground", "sidebarBackground", "contentBackground", "cardBackground", "borderColor",
  "primaryTextColor", "secondaryTextColor", "navTextColor", "selectedNavBackground", "baseFontSizePx", "radiusPx",
  "windowOpacity", "blurPx", "shadowPercent", "wallpaperDataUrl", "wallpaperOpacity", "platformOverrides",
] as const;

const platformOverrideFields = ["enabled", "windowsBlurPx", "windowsOpacity", "macosBlurPx", "macosOpacity"] as const;
const colorPattern = /^#[0-9a-fA-F]{6}$/;
const imagePattern = /^data:image\/(png|jpeg|webp);base64,([A-Za-z0-9+/]+={0,2})$/;
const maxWallpaperBytes = 1_572_864;
const maxWallpaperDimension = 4_096;
const maxWallpaperPixels = maxWallpaperDimension * maxWallpaperDimension;
const maxCommandIconBytes = 256 * 1024;
const maxCommandIconDimension = 512;
const maxCommandIconPixels = maxCommandIconDimension * maxCommandIconDimension;
const wallpaperDecodeTimeoutMs = 3_000;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function assertExactFields(value: Record<string, unknown>, expected: readonly string[], label: string) {
  const actual = Object.keys(value);
  if (actual.length !== expected.length || actual.some((field) => !expected.includes(field)) || expected.some((field) => !(field in value))) {
    throw new Error(`${label} fields must be complete and contain no unknown values`);
  }
}

function assertString(value: unknown, label: string): asserts value is string {
  if (typeof value !== "string") throw new Error(`${label} must be a string`);
}

function assertBoolean(value: unknown, label: string): asserts value is boolean {
  if (typeof value !== "boolean") throw new Error(`${label} must be a boolean`);
}

function assertIntegerInRange(value: unknown, minimum: number, maximum: number, label: string): asserts value is number {
  if (!Number.isInteger(value) || (value as number) < minimum || (value as number) > maximum) {
    throw new Error(`${label} must be an integer from ${minimum} to ${maximum}`);
  }
}

function assertColor(value: unknown, label: string) {
  assertString(value, label);
  if (!colorPattern.test(value)) throw new Error(`${label} must use #RRGGBB`);
}

function assertWallpaper(value: unknown) {
  assertString(value, "wallpaperDataUrl");
  if (!value) return;
  const match = value.match(imagePattern);
  if (!match || match[2].length % 4 !== 0 || /=[^=]/.test(match[2])) {
    throw new Error("wallpaperDataUrl must be a PNG, JPEG, or WebP data URL");
  }
  const payload = match[2];
  const padding = payload.endsWith("==") ? 2 : payload.endsWith("=") ? 1 : 0;
  if ((payload.length / 4) * 3 - padding > maxWallpaperBytes) throw new Error("wallpaperDataUrl is too large");
  const lastSextet = base64Sextet(payload[payload.length - padding - 1]);
  if (lastSextet === null || (padding === 2 && (lastSextet & 0x0f) !== 0) || (padding === 1 && (lastSextet & 0x03) !== 0)) {
    throw new Error("wallpaperDataUrl must use canonical Base64 padding");
  }
  try {
    const bytes = decodeBase64(payload);
    if (!bytes || bytes.length !== (payload.length / 4) * 3 - padding) throw new Error("invalid Base64 payload");
    const valid = match[1] === "png" ? validatePng(bytes)
      : match[1] === "jpeg" ? validateJpeg(bytes)
        : validateWebp(bytes);
    if (!valid) throw new Error("incomplete image payload");
  } catch {
    throw new Error("wallpaperDataUrl must contain a complete PNG, JPEG, or WebP payload");
  }
}

/**
 * Re-validates an already strict data URL through the browser decoder before
 * an import/selection UI accepts it. The Rust save path performs the matching
 * decode with allocation limits, so this is an early UI failure rather than a
 * security boundary.
 */
export async function validateWallpaperImageDataUrl(value: unknown): Promise<void> {
  assertWallpaper(value);
  if (value === "") return;
  const { width, height } = await decodeLocalImageDataUrl(
    value as string,
    "wallpaperDataUrl must decode to a complete PNG, JPEG, or WebP image",
  );
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width > maxWallpaperDimension || height > maxWallpaperDimension || width * height > maxWallpaperPixels) {
    throw new Error("wallpaper image dimensions exceed the allowed limit");
  }
}

export async function validateCommandIconImageDataUrl(value: unknown): Promise<void> {
  assertWallpaper(value);
  if (value === "") return;
  const match = (value as string).match(imagePattern);
  if (!match) throw new Error("command icon must be a PNG, JPEG, or WebP data URL");
  const payload = match[2];
  const padding = payload.endsWith("==") ? 2 : payload.endsWith("=") ? 1 : 0;
  if ((payload.length / 4) * 3 - padding > maxCommandIconBytes) {
    throw new Error("command icon is too large");
  }
  const { width, height } = await decodeLocalImageDataUrl(
    value as string,
    "command icon must decode to a complete PNG, JPEG, or WebP image",
  );
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width > maxCommandIconDimension || height > maxCommandIconDimension || width * height > maxCommandIconPixels) {
    throw new Error("command icon dimensions exceed the allowed limit");
  }
}

async function decodeLocalImageDataUrl(value: string, failure: string) {
  if (typeof Image === "undefined") throw new Error(failure);
  const image = new Image();
  image.decoding = "async";
  image.src = value;
  let timeout: ReturnType<typeof globalThis.setTimeout> | undefined;
  try {
    await Promise.race([
      image.decode(),
      new Promise<void>((_resolve, reject) => {
        timeout = globalThis.setTimeout(
          () => reject(new Error("wallpaper image decoding timed out")),
          wallpaperDecodeTimeoutMs,
        );
      }),
    ]);
    return { width: image.naturalWidth, height: image.naturalHeight };
  } catch {
    throw new Error(failure);
  } finally {
    if (timeout !== undefined) globalThis.clearTimeout(timeout);
    image.src = "";
  }
}

function base64Sextet(value: string | undefined) {
  if (!value) return null;
  const code = value.charCodeAt(0);
  if (code >= 65 && code <= 90) return code - 65;
  if (code >= 97 && code <= 122) return code - 97 + 26;
  if (code >= 48 && code <= 57) return code - 48 + 52;
  return value === "+" ? 62 : value === "/" ? 63 : null;
}

function decodeBase64(payload: string) {
  const decoded = globalThis.atob(payload);
  const bytes = new Uint8Array(decoded.length);
  for (let index = 0; index < decoded.length; index += 1) bytes[index] = decoded.charCodeAt(index);
  return bytes;
}

function readU32BE(bytes: Uint8Array, offset: number) {
  return bytes[offset] * 0x1_000000 + (bytes[offset + 1] << 16) + (bytes[offset + 2] << 8) + bytes[offset + 3];
}

function readU32LE(bytes: Uint8Array, offset: number) {
  return bytes[offset] + (bytes[offset + 1] << 8) + (bytes[offset + 2] << 16) + bytes[offset + 3] * 0x1_000000;
}

function fourCc(bytes: Uint8Array, offset: number) {
  return String.fromCharCode(bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]);
}

function pngCrc32(bytes: Uint8Array, start: number, end: number) {
  let crc = 0xffff_ffff;
  for (let index = start; index < end; index += 1) {
    crc ^= bytes[index];
    for (let bit = 0; bit < 8; bit += 1) crc = (crc & 1) === 0 ? crc >>> 1 : (crc >>> 1) ^ 0xedb8_8320;
  }
  return (crc ^ 0xffff_ffff) >>> 0;
}

function validatePngIhdr(data: Uint8Array) {
  if (data.length !== 13 || readU32BE(data, 0) === 0 || readU32BE(data, 4) === 0 || data[10] !== 0 || data[11] !== 0 || (data[12] !== 0 && data[12] !== 1)) return false;
  const bitDepth = data[8];
  const colorType = data[9];
  return (colorType === 0 && [1, 2, 4, 8, 16].includes(bitDepth))
    || ((colorType === 2 || colorType === 4 || colorType === 6) && (bitDepth === 8 || bitDepth === 16))
    || (colorType === 3 && [1, 2, 4, 8].includes(bitDepth));
}

function validatePng(bytes: Uint8Array) {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < signature.length || signature.some((value, index) => bytes[index] !== value)) return false;
  let position = signature.length;
  let sawIhdr = false;
  let sawPlte = false;
  let sawIdat = false;
  let leftIdat = false;
  let indexedColor = false;
  while (position < bytes.length) {
    if (position + 12 > bytes.length) return false;
    const length = readU32BE(bytes, position);
    const dataStart = position + 8;
    const dataEnd = dataStart + length;
    const chunkEnd = dataEnd + 4;
    if (!Number.isSafeInteger(chunkEnd) || chunkEnd > bytes.length) return false;
    const kind = fourCc(bytes, position + 4);
    const data = bytes.subarray(dataStart, dataEnd);
    if (pngCrc32(bytes, position + 4, dataEnd) !== readU32BE(bytes, dataEnd)) return false;
    if (!sawIhdr) {
      if (kind !== "IHDR" || !validatePngIhdr(data)) return false;
      indexedColor = data[9] === 3;
      sawIhdr = true;
    } else if (kind === "IHDR") {
      return false;
    } else if (kind === "PLTE") {
      if (sawIdat || data.length === 0 || data.length % 3 !== 0 || data.length > 768) return false;
      sawPlte = true;
    } else if (kind === "IDAT") {
      if (leftIdat || data.length === 0) return false;
      sawIdat = true;
    } else if (kind === "IEND") {
      return data.length === 0 && sawIdat && (!indexedColor || sawPlte) && chunkEnd === bytes.length;
    } else if (sawIdat) {
      leftIdat = true;
    }
    position = chunkEnd;
  }
  return false;
}

function jpegMarker(bytes: Uint8Array, position: number): [number, number] | null {
  if (bytes[position] !== 0xff) return null;
  let markerEnd = position + 1;
  while (bytes[markerEnd] === 0xff) markerEnd += 1;
  const marker = bytes[markerEnd];
  return marker === undefined || marker === 0 ? null : [marker, markerEnd + 1];
}

function isJpegFrameMarker(marker: number) {
  return (marker >= 0xc0 && marker <= 0xc3) || (marker >= 0xc5 && marker <= 0xc7) || (marker >= 0xc9 && marker <= 0xcb) || (marker >= 0xcd && marker <= 0xcf);
}

function validateJpegFrame(segment: Uint8Array) {
  if (segment.length < 6) return false;
  const components = segment[5];
  const height = (segment[1] << 8) | segment[2];
  const width = (segment[3] << 8) | segment[4];
  return components > 0 && height > 0 && width > 0 && segment.length === 6 + components * 3;
}

function validateJpegScanHeader(segment: Uint8Array) {
  const components = segment[0];
  return components > 0 && segment.length === 4 + components * 2;
}

function jpegScanEnd(bytes: Uint8Array, position: number): number | -1 | null {
  while (position < bytes.length) {
    if (bytes[position] !== 0xff) {
      position += 1;
      continue;
    }
    const markerStart = position;
    position += 1;
    while (bytes[position] === 0xff) position += 1;
    const marker = bytes[position];
    if (marker === undefined) return null;
    if (marker === 0) {
      position += 1;
      continue;
    }
    if (marker >= 0xd0 && marker <= 0xd7) {
      position += 1;
      continue;
    }
    if (marker === 0xd9) return position + 1 === bytes.length ? -1 : null;
    return markerStart;
  }
  return null;
}

function validateJpeg(bytes: Uint8Array) {
  if (bytes.length < 4 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return false;
  let position = 2;
  let sawFrame = false;
  let sawScan = false;
  while (position < bytes.length) {
    const markerData = jpegMarker(bytes, position);
    if (!markerData) return false;
    const [marker, markerEnd] = markerData;
    position = markerEnd;
    if (marker === 0xd9) return sawFrame && sawScan && position === bytes.length;
    if (marker === 0xd8) return false;
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
    if (position + 2 > bytes.length) return false;
    const length = (bytes[position] << 8) | bytes[position + 1];
    if (length < 2 || position + length > bytes.length) return false;
    const segmentEnd = position + length;
    const segment = bytes.subarray(position + 2, segmentEnd);
    if (isJpegFrameMarker(marker)) {
      if (!validateJpegFrame(segment)) return false;
      sawFrame = true;
    }
    if (marker === 0xda) {
      if (!sawFrame || !validateJpegScanHeader(segment)) return false;
      sawScan = true;
      const scanEnd = jpegScanEnd(bytes, segmentEnd);
      if (scanEnd === -1) return true;
      if (scanEnd === null) return false;
      position = scanEnd;
    } else {
      position = segmentEnd;
    }
  }
  return false;
}

function validateWebpVp8(data: Uint8Array) {
  if (data.length < 10 || (data[0] & 1) !== 0 || data[3] !== 0x9d || data[4] !== 0x01 || data[5] !== 0x2a) return false;
  const width = ((data[7] << 8) | data[6]) & 0x3fff;
  const height = ((data[9] << 8) | data[8]) & 0x3fff;
  return width > 0 && height > 0;
}

function validateWebpVp8l(data: Uint8Array) {
  if (data.length < 5 || data[0] !== 0x2f) return false;
  const dimensions = readU32LE(data, 1);
  return (dimensions >>> 29) === 0;
}

function validateWebpVp8x(data: Uint8Array) {
  return data.length === 10 && (data[0] & 0x01) === 0 && data[1] === 0 && data[2] === 0 && data[3] === 0;
}

function validateWebpChunks(bytes: Uint8Array) {
  let position = 0;
  let sawImage = false;
  while (position < bytes.length) {
    if (position + 8 > bytes.length) return false;
    const length = readU32LE(bytes, position + 4);
    const dataStart = position + 8;
    const dataEnd = dataStart + length;
    const next = dataEnd + length % 2;
    if (!Number.isSafeInteger(next) || next > bytes.length) return false;
    const kind = fourCc(bytes, position);
    const data = bytes.subarray(dataStart, dataEnd);
    const valid = kind === "VP8 " ? validateWebpVp8(data)
      : kind === "VP8L" ? validateWebpVp8l(data)
        : kind === "VP8X" ? validateWebpVp8x(data)
          : kind === "ANMF" ? data.length >= 16 && validateWebpChunks(data.subarray(16))
            : true;
    if (!valid) return false;
    if (kind === "VP8 " || kind === "VP8L" || kind === "ANMF") sawImage = true;
    position = next;
  }
  return sawImage;
}

function validateWebp(bytes: Uint8Array) {
  if (bytes.length < 20 || fourCc(bytes, 0) !== "RIFF" || fourCc(bytes, 8) !== "WEBP" || readU32LE(bytes, 4) + 8 !== bytes.length) return false;
  return validateWebpChunks(bytes.subarray(12));
}

function assertBackground(theme: Record<string, unknown>) {
  assertIntegerInRange(theme.windowOpacity, 70, 100, "windowOpacity");
  assertIntegerInRange(theme.blurPx, 0, 40, "blurPx");
  assertIntegerInRange(theme.shadowPercent, 0, 80, "shadowPercent");
  assertIntegerInRange(theme.wallpaperOpacity, 0, 60, "wallpaperOpacity");
  assertWallpaper(theme.wallpaperDataUrl);
  if (!isRecord(theme.platformOverrides)) throw new Error("platformOverrides must be an object");
  assertExactFields(theme.platformOverrides, platformOverrideFields, "platformOverrides");
  assertBoolean(theme.platformOverrides.enabled, "platformOverrides.enabled");
  assertIntegerInRange(theme.platformOverrides.windowsBlurPx, 0, 40, "platformOverrides.windowsBlurPx");
  assertIntegerInRange(theme.platformOverrides.windowsOpacity, 70, 100, "platformOverrides.windowsOpacity");
  assertIntegerInRange(theme.platformOverrides.macosBlurPx, 0, 40, "platformOverrides.macosBlurPx");
  assertIntegerInRange(theme.platformOverrides.macosOpacity, 70, 100, "platformOverrides.macosOpacity");
}

function assertName(value: unknown) {
  assertString(value, "name");
  if (!value.trim() || [...value].length > 40) throw new Error("name must contain 1–40 characters");
}

function assertLauncherTheme(theme: Record<string, unknown>, requireVisibility = true, requireSpacing = true) {
  assertName(theme.name);
  for (const field of ["accentColor", "windowBackground", "windowBorder", "searchBackground", "searchBorder", "searchTextColor", "normalRowBackground", "normalPrimaryColor", "normalSecondaryColor", "selectedRowBackground", "selectedPrimaryColor", "selectedSecondaryColor"]) assertColor(theme[field], field);
  assertIntegerInRange(theme.windowBorderWidthPx, 0, 4, "windowBorderWidthPx");
  assertIntegerInRange(theme.windowWidthPx, visualBounds.launcherTheme.windowWidthPx.min, visualBounds.launcherTheme.windowWidthPx.max, "windowWidthPx");
  assertIntegerInRange(theme.windowRadiusPx, visualBounds.launcherTheme.windowRadiusPx.min, visualBounds.launcherTheme.windowRadiusPx.max, "windowRadiusPx");
  assertIntegerInRange(theme.searchBorderWidthPx, visualBounds.launcherTheme.searchBorderWidthPx.min, visualBounds.launcherTheme.searchBorderWidthPx.max, "searchBorderWidthPx");
  if (theme.searchBorderStyle !== "solid" && theme.searchBorderStyle !== "dashed" && theme.searchBorderStyle !== "dotted" && theme.searchBorderStyle !== "double" && theme.searchBorderStyle !== "none") throw new Error("searchBorderStyle is invalid");
  if (requireSpacing) {
    assertIntegerInRange(theme.searchLeftSpacePx, visualBounds.launcherTheme.searchLeftSpacePx.min, visualBounds.launcherTheme.searchLeftSpacePx.max, "searchLeftSpacePx");
    assertIntegerInRange(theme.searchVerticalSpacePx, visualBounds.launcherTheme.searchVerticalSpacePx.min, visualBounds.launcherTheme.searchVerticalSpacePx.max, "searchVerticalSpacePx");
  }
  assertIntegerInRange(theme.searchWidthPx, visualBounds.launcherTheme.searchWidthPx.min, visualBounds.launcherTheme.searchWidthPx.max, "searchWidthPx");
  if ((theme.searchWidthPx as number) > (theme.windowWidthPx as number)) throw new Error("searchWidthPx cannot exceed windowWidthPx");
  assertIntegerInRange(theme.searchFontSizePx, visualBounds.launcherTheme.searchFontSizePx.min, visualBounds.launcherTheme.searchFontSizePx.max, "searchFontSizePx");
  assertIntegerInRange(theme.normalPrimaryFontSizePx, visualBounds.launcherTheme.normalPrimaryFontSizePx.min, visualBounds.launcherTheme.normalPrimaryFontSizePx.max, "normalPrimaryFontSizePx");
  assertIntegerInRange(theme.normalSecondaryFontSizePx, visualBounds.launcherTheme.normalSecondaryFontSizePx.min, visualBounds.launcherTheme.normalSecondaryFontSizePx.max, "normalSecondaryFontSizePx");
  assertIntegerInRange(theme.normalRowHeightPx, visualBounds.launcherTheme.normalRowHeightPx.min, visualBounds.launcherTheme.normalRowHeightPx.max, "normalRowHeightPx");
  assertIntegerInRange(theme.selectedPrimaryFontSizePx, visualBounds.launcherTheme.selectedPrimaryFontSizePx.min, visualBounds.launcherTheme.selectedPrimaryFontSizePx.max, "selectedPrimaryFontSizePx");
  assertIntegerInRange(theme.selectedSecondaryFontSizePx, visualBounds.launcherTheme.selectedSecondaryFontSizePx.min, visualBounds.launcherTheme.selectedSecondaryFontSizePx.max, "selectedSecondaryFontSizePx");
  assertIntegerInRange(theme.iconSizePx, visualBounds.launcherTheme.iconSizePx.min, visualBounds.launcherTheme.iconSizePx.max, "iconSizePx");
  if (theme.maxResults !== 6 && theme.maxResults !== 8 && theme.maxResults !== 10 && theme.maxResults !== 12) throw new Error("maxResults is invalid");
  assertBoolean(theme.showSearchIcon, "showSearchIcon");
  assertBoolean(theme.showLogo, "showLogo");
  assertBoolean(theme.showSourceBadge, "showSourceBadge");
  if (requireVisibility) {
    assertBoolean(theme.showProviderStatus, "showProviderStatus");
    assertBoolean(theme.showFooterHints, "showFooterHints");
  }
  assertBackground(theme);
}

function assertSettingsTheme(theme: Record<string, unknown>) {
  assertName(theme.name);
  for (const field of ["accentColor", "windowBackground", "titlebarBackground", "sidebarBackground", "contentBackground", "cardBackground", "borderColor", "primaryTextColor", "secondaryTextColor", "navTextColor", "selectedNavBackground"]) assertColor(theme[field], field);
  assertIntegerInRange(theme.baseFontSizePx, visualBounds.settingsTheme.baseFontSizePx.min, visualBounds.settingsTheme.baseFontSizePx.max, "baseFontSizePx");
  assertIntegerInRange(theme.radiusPx, visualBounds.settingsTheme.radiusPx.min, visualBounds.settingsTheme.radiusPx.max, "radiusPx");
  assertBackground(theme);
}

function parseThemeBundle<T>(value: unknown, schema: string, version: 1 | 2 | 3, fields: readonly string[], validate: (theme: Record<string, unknown>) => void): T {
  if (!isRecord(value)) throw new Error("theme bundle must be an object");
  assertExactFields(value, ["schema", "version", "theme"], "theme bundle");
  if (value.schema !== schema || value.version !== version) throw new Error(`only ${schema} v${version} is supported`);
  if (!isRecord(value.theme)) throw new Error("theme bundle theme must be an object");
  assertExactFields(value.theme, fields, "theme bundle theme");
  validate(value.theme);
  return value.theme as unknown as T;
}

export function parseLauncherThemeBundle(value: unknown): LauncherThemeBundleV3 {
  if (isRecord(value) && value.schema === "suo-launcher-theme-v1" && value.version === 1) {
    const legacy = parseThemeBundle<LauncherThemeImportV1>(value, "suo-launcher-theme-v1", 1, launcherBundleFieldsV1, (theme) => assertLauncherTheme(theme, false, false));
    return { schema: "suo-launcher-theme-v3", version: 3, theme: { ...legacy, showProviderStatus: true, showFooterHints: true, searchLeftSpacePx: 30, searchVerticalSpacePx: 9 } };
  }
  if (isRecord(value) && value.schema === "suo-launcher-theme-v2" && value.version === 2) {
    const legacy = parseThemeBundle<LauncherThemeImportV2>(value, "suo-launcher-theme-v2", 2, launcherBundleFieldsV2, (theme) => assertLauncherTheme(theme, true, false));
    return { schema: "suo-launcher-theme-v3", version: 3, theme: { ...legacy, searchLeftSpacePx: 30, searchVerticalSpacePx: 9 } };
  }
  return { schema: "suo-launcher-theme-v3", version: 3, theme: parseThemeBundle(value, "suo-launcher-theme-v3", 3, launcherBundleFieldsV3, assertLauncherTheme) };
}

export function parseSettingsThemeBundle(value: unknown): SettingsThemeBundleV1 {
  return { schema: "suo-settings-theme-v1", version: 1, theme: parseThemeBundle(value, "suo-settings-theme-v1", 1, settingsBundleFields, assertSettingsTheme) };
}

function withoutId<T extends { id: string }>(theme: T): Omit<T, "id"> {
  const { id: _id, ...exported } = theme;
  return exported;
}

export function buildLauncherThemeBundle(theme: LauncherCustomThemeConfig): LauncherThemeBundleV3 {
  const bundle: LauncherThemeBundleV3 = { schema: "suo-launcher-theme-v3", version: 3, theme: withoutId(theme) };
  return parseLauncherThemeBundle(bundle);
}

export function buildSettingsThemeBundle(theme: SettingsCustomThemeConfig): SettingsThemeBundleV1 {
  const bundle: SettingsThemeBundleV1 = { schema: "suo-settings-theme-v1", version: 1, theme: withoutId(theme) };
  return parseSettingsThemeBundle(bundle);
}

export function aliasesFromText(value: string) {
  return value.split(",").map((item) => item.trim()).filter(Boolean);
}
