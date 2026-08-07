export type LauncherPreferences = {
  closeOnBlur: boolean;
  keepLastInput: boolean;
};

const storageKey = "suo.launcher.preferences.v1";

export const defaultLauncherPreferences: LauncherPreferences = {
  closeOnBlur: true,
  keepLastInput: false,
};

export function loadLauncherPreferences(): LauncherPreferences {
  try {
    const stored = window.localStorage.getItem(storageKey);
    if (!stored) return defaultLauncherPreferences;
    const parsed = JSON.parse(stored) as Partial<LauncherPreferences>;
    return {
      closeOnBlur:
        typeof parsed.closeOnBlur === "boolean"
          ? parsed.closeOnBlur
          : defaultLauncherPreferences.closeOnBlur,
      keepLastInput:
        typeof parsed.keepLastInput === "boolean"
          ? parsed.keepLastInput
          : defaultLauncherPreferences.keepLastInput,
    };
  } catch {
    return defaultLauncherPreferences;
  }
}

export function saveLauncherPreferences(preferences: LauncherPreferences) {
  window.localStorage.setItem(storageKey, JSON.stringify(preferences));
}
