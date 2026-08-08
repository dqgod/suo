import { invoke } from "@tauri-apps/api/core";

export type LauncherConfig = {
  closeOnBlur: boolean;
  keepLastInput: boolean;
  compactWhenEmpty: boolean;
};

export type TranslationConfig = {
  enabled: boolean;
  keyword: string;
  description: string;
  aliases: string[];
  region: string;
  defaultTargetLanguage: string;
  chineseTargetLanguage: string;
};

export type ScriptRuntime = "python" | "powerShell" | "bash" | "executable";

export type ScriptCommandConfig = {
  id: string;
  name: string;
  keyword: string;
  description: string;
  aliases: string[];
  enabled: boolean;
  runtime: ScriptRuntime;
  scriptPath: string;
  immediate: boolean;
  debounceMs: number;
  timeoutMs: number;
};

export type WebSearchConfig = {
  id: string;
  name: string;
  keyword: string;
  description: string;
  aliases: string[];
  enabled: boolean;
  urlTemplate: string;
};

export type BuiltinThemeId = "midnight" | "paper" | "forest";
export type ThemeFontFamily = "system" | "cjk" | "mono";
export type ResultDensity = "compact" | "comfortable" | "loose";

export type PlatformThemeOverrides = {
  enabled: boolean;
  windowsBlurPx: number;
  windowsOpacity: number;
  macosBlurPx: number;
  macosOpacity: number;
};

export type CustomThemeConfig = {
  id: string;
  name: string;
  windowColor: string;
  panelColor: string;
  fieldColor: string;
  textColor: string;
  mutedColor: string;
  accentColor: string;
  selectionColor: string;
  borderColor: string;
  windowOpacity: number;
  blurPx: number;
  shadowPercent: number;
  wallpaperDataUrl: string;
  wallpaperOpacity: number;
  radiusPx: number;
  fontFamily: ThemeFontFamily;
  fontSizePx: number;
  launcherWidthPx: number;
  resultDensity: ResultDensity;
  maxResults: 6 | 8 | 10 | 12;
  iconSizePx: number;
  showSourceBadge: boolean;
  platformOverrides: PlatformThemeOverrides;
};

export type AppearanceConfig = {
  theme: string;
  accentColor: string;
  customThemes: CustomThemeConfig[];
};

export const builtinThemeIds: BuiltinThemeId[] = ["midnight", "paper", "forest"];

const defaultPlatformOverrides = (): PlatformThemeOverrides => ({
  enabled: false,
  windowsBlurPx: 18,
  windowsOpacity: 94,
  macosBlurPx: 18,
  macosOpacity: 94,
});

const builtinThemes: Record<BuiltinThemeId, CustomThemeConfig> = {
  midnight: {
    id: "midnight",
    name: "Midnight",
    windowColor: "#0b1222",
    panelColor: "#101a30",
    fieldColor: "#161f39",
    textColor: "#f5f7ff",
    mutedColor: "#91a0c7",
    accentColor: "#8a78ff",
    selectionColor: "#302b63",
    borderColor: "#343d5a",
    windowOpacity: 96,
    blurPx: 18,
    shadowPercent: 45,
    wallpaperDataUrl: "",
    wallpaperOpacity: 0,
    radiusPx: 18,
    fontFamily: "system",
    fontSizePx: 14,
    launcherWidthPx: 720,
    resultDensity: "comfortable",
    maxResults: 8,
    iconSizePx: 32,
    showSourceBadge: true,
    platformOverrides: defaultPlatformOverrides(),
  },
  paper: {
    id: "paper",
    name: "Paper",
    windowColor: "#e9e5dd",
    panelColor: "#ffffff",
    fieldColor: "#eee9df",
    textColor: "#272b35",
    mutedColor: "#596378",
    accentColor: "#5a61e6",
    selectionColor: "#d9dcf8",
    borderColor: "#b9b6ae",
    windowOpacity: 98,
    blurPx: 8,
    shadowPercent: 30,
    wallpaperDataUrl: "",
    wallpaperOpacity: 0,
    radiusPx: 18,
    fontFamily: "system",
    fontSizePx: 14,
    launcherWidthPx: 720,
    resultDensity: "comfortable",
    maxResults: 8,
    iconSizePx: 32,
    showSourceBadge: true,
    platformOverrides: defaultPlatformOverrides(),
  },
  forest: {
    id: "forest",
    name: "Forest",
    windowColor: "#071610",
    panelColor: "#102b20",
    fieldColor: "#142f25",
    textColor: "#eefaf3",
    mutedColor: "#91b5a2",
    accentColor: "#62d89c",
    selectionColor: "#184b37",
    borderColor: "#315b48",
    windowOpacity: 96,
    blurPx: 18,
    shadowPercent: 45,
    wallpaperDataUrl: "",
    wallpaperOpacity: 0,
    radiusPx: 18,
    fontFamily: "system",
    fontSizePx: 14,
    launcherWidthPx: 720,
    resultDensity: "comfortable",
    maxResults: 8,
    iconSizePx: 32,
    showSourceBadge: true,
    platformOverrides: defaultPlatformOverrides(),
  },
};

function cloneTheme(theme: CustomThemeConfig): CustomThemeConfig {
  return {
    ...theme,
    platformOverrides: { ...theme.platformOverrides },
  };
}

function createThemeId() {
  const uuid = globalThis.crypto?.randomUUID?.();
  return uuid ? `theme-${uuid}` : `theme-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function createCustomTheme(sourceId: string = "midnight") {
  const source = builtinThemes[(builtinThemeIds.includes(sourceId as BuiltinThemeId)
    ? sourceId
    : "midnight") as BuiltinThemeId];
  return {
    ...cloneTheme(source),
    id: createThemeId(),
    name: "Custom theme",
    platformOverrides: { ...source.platformOverrides },
  };
}

export function resolveTheme(appearance: AppearanceConfig): CustomThemeConfig {
  const customId = appearance.theme.startsWith("custom:")
    ? appearance.theme.slice("custom:".length)
    : null;
  if (customId) {
    const normalizedCustomId = customId.toLowerCase();
    const custom = appearance.customThemes.find((theme) => theme.id.toLowerCase() === normalizedCustomId);
    if (custom) return cloneTheme(custom);
  }
  const builtinId = builtinThemeIds.includes(appearance.theme as BuiltinThemeId)
    ? appearance.theme as BuiltinThemeId
    : "midnight";
  const builtin = cloneTheme(builtinThemes[builtinId]);
  builtin.accentColor = appearance.accentColor || builtin.accentColor;
  return builtin;
}

export type AppConfig = {
  version: number;
  launcher: LauncherConfig;
  translation: TranslationConfig;
  scriptCommands: ScriptCommandConfig[];
  webSearches: WebSearchConfig[];
  appearance: AppearanceConfig;
};

export type AppConfigView = {
  config: AppConfig;
  translationApiKeyConfigured: boolean;
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
        closeOnBlur:
          typeof preferences.closeOnBlur === "boolean"
            ? preferences.closeOnBlur
            : view.config.launcher.closeOnBlur,
        keepLastInput:
          typeof preferences.keepLastInput === "boolean"
            ? preferences.keepLastInput
            : view.config.launcher.keepLastInput,
      },
    };
    const migrated = await invoke<AppConfigView>("save_app_config", { config });
    window.localStorage.removeItem(legacyPreferencesKey);
    return migrated;
  } catch {
    return view;
  }
}

export function applyAppearance(appearance: AppearanceConfig) {
  const theme = resolveTheme(appearance);
  const root = document.documentElement;
  const isMac = /Mac/i.test(navigator.platform);
  const isWindows = /Win/i.test(navigator.platform);
  const overrides = theme.platformOverrides;
  const blurPx = overrides.enabled && isMac
    ? overrides.macosBlurPx
    : overrides.enabled && isWindows
      ? overrides.windowsBlurPx
      : theme.blurPx;
  const windowOpacity = overrides.enabled && isMac
    ? overrides.macosOpacity
    : overrides.enabled && isWindows
      ? overrides.windowsOpacity
      : theme.windowOpacity;
  const densityHeights: Record<ResultDensity, number> = {
    compact: 48,
    comfortable: 58,
    loose: 68,
  };
  const fontFamilies: Record<ThemeFontFamily, string> = {
    system: 'Inter, ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif',
    cjk: '"PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", sans-serif',
    mono: '"SFMono-Regular", Consolas, "Liberation Mono", monospace',
  };
  const wallpaper = /^data:image\/(?:png|jpeg|webp);base64,[A-Za-z0-9+/=]+$/.test(theme.wallpaperDataUrl)
    ? `url("${theme.wallpaperDataUrl}")`
    : "none";

  root.dataset.theme = appearance.theme.startsWith("custom:") ? "custom" : appearance.theme;
  root.dataset.showSourceBadge = String(theme.showSourceBadge);
  const variables: Record<string, string> = {
    "--page": theme.windowColor,
    "--surface": theme.panelColor,
    "--surface-strong": `color-mix(in srgb, ${theme.panelColor} 92%, ${theme.textColor})`,
    "--field": theme.fieldColor,
    "--text": theme.textColor,
    "--muted": theme.mutedColor,
    "--subtle": `color-mix(in srgb, ${theme.mutedColor} 78%, ${theme.windowColor})`,
    "--line": theme.borderColor,
    "--accent": theme.accentColor,
    "--selection": theme.selectionColor,
    "--window-opacity": `${windowOpacity}%`,
    "--window-blur": `${blurPx}px`,
    "--window-radius": `${theme.radiusPx}px`,
    "--window-shadow-opacity": String(theme.shadowPercent / 100),
    "--base-font-size": `${theme.fontSizePx}px`,
    "--theme-font-family": fontFamilies[theme.fontFamily],
    "--result-row-height": `${densityHeights[theme.resultDensity]}px`,
    "--result-icon-size": `${theme.iconSizePx}px`,
    "--launcher-width": `${theme.launcherWidthPx}px`,
    "--wallpaper-image": wallpaper,
    "--wallpaper-opacity": String(theme.wallpaperOpacity / 100),
  };
  for (const [name, value] of Object.entries(variables)) {
    root.style.setProperty(name, value);
  }
  return theme;
}

export function aliasesFromText(value: string) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}
