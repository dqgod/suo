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

export type AppearanceConfig = {
  theme: "midnight" | "paper" | "forest";
  accentColor: string;
};

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
  document.documentElement.dataset.theme = appearance.theme;
  document.documentElement.style.setProperty("--accent", appearance.accentColor);
}

export function aliasesFromText(value: string) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}
