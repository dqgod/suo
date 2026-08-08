import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import {
  builtinThemeIds,
  createCustomTheme,
  resolveTheme,
  type AppearanceConfig,
  type BuiltinThemeId,
  type CustomThemeConfig,
} from "./config";
import { zhCN } from "./i18n/zh-CN";
import "./AppearanceEditor.css";

type AppearanceEditorProps = {
  appearance: AppearanceConfig;
  onChange: (appearance: AppearanceConfig) => void;
  readOnly: boolean;
};

type EditorTab = "colors" | "layout" | "platform";
type PreviewKind = "launcher" | "settings";

type ThemeBundleV1 = {
  schema: "suo-theme-v1";
  version: 1;
  theme: Omit<CustomThemeConfig, "id">;
};

const MAX_WALLPAPER_BYTES = Math.floor(1.5 * 1024 * 1024);
const MAX_THEME_BUNDLE_BYTES = Math.floor(2.5 * 1024 * 1024);
const t = zhCN.appearanceEditor;
const builtinThemeLabels: Record<BuiltinThemeId, string> = {
  midnight: t.midnight,
  paper: t.paper,
  forest: t.forest,
};
const colorFields = [
  ["windowColor", t.colors.window.label, t.colors.window.description],
  ["panelColor", t.colors.panel.label, t.colors.panel.description],
  ["fieldColor", t.colors.field.label, t.colors.field.description],
  ["accentColor", t.colors.accent.label, t.colors.accent.description],
  ["selectionColor", t.colors.selection.label, t.colors.selection.description],
  ["textColor", t.colors.text.label, t.colors.text.description],
  ["mutedColor", t.colors.muted.label, t.colors.muted.description],
  ["borderColor", t.colors.border.label, t.colors.border.description],
] as const;

const themeFieldNames = [
  "name",
  "windowColor",
  "panelColor",
  "fieldColor",
  "textColor",
  "mutedColor",
  "accentColor",
  "selectionColor",
  "borderColor",
  "windowOpacity",
  "blurPx",
  "shadowPercent",
  "wallpaperDataUrl",
  "wallpaperOpacity",
  "radiusPx",
  "fontFamily",
  "fontSizePx",
  "launcherWidthPx",
  "resultDensity",
  "maxResults",
  "iconSizePx",
  "showSourceBadge",
  "platformOverrides",
] as const;

const platformOverrideFieldNames = [
  "enabled",
  "windowsBlurPx",
  "windowsOpacity",
  "macosBlurPx",
  "macosOpacity",
] as const;

function cloneAppearance(appearance: AppearanceConfig): AppearanceConfig {
  return {
    ...appearance,
    customThemes: appearance.customThemes.map((theme) => ({
      ...theme,
      platformOverrides: { ...theme.platformOverrides },
    })),
  };
}

function customThemeId(theme: string) {
  return theme.startsWith("custom:") ? theme.slice("custom:".length) : null;
}

function sameThemeId(left: string, right: string | null) {
  return right !== null && left.toLowerCase() === right.toLowerCase();
}

function fallbackBuiltinTheme() {
  return builtinThemeIds[0] as BuiltinThemeId;
}

function isHexColor(value: unknown): value is string {
  return typeof value === "string" && /^#[\da-f]{6}$/i.test(value);
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function hasOnlyKeys(value: Record<string, unknown>, keys: readonly string[]) {
  const actualKeys = Object.keys(value);
  return actualKeys.length === keys.length && actualKeys.every((key) => keys.includes(key));
}

function isSafeWallpaperDataUrl(value: unknown): value is string {
  if (value === "") return true;
  if (typeof value !== "string") return false;
  const match = /^data:(image\/(?:png|jpeg|webp));base64,([a-z\d+/]+={0,2})$/i.exec(value);
  if (!match) return false;
  const base64 = match[2];
  if (base64.length % 4 !== 0) return false;
  const padding = base64.endsWith("==") ? 2 : base64.endsWith("=") ? 1 : 0;
  const content = padding ? base64.slice(0, -padding) : base64;
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const lastSextet = alphabet.indexOf(content[content.length - 1] ?? "");
  if (lastSextet < 0 || (padding === 2 && (lastSextet & 0x0f) !== 0) || (padding === 1 && (lastSextet & 0x03) !== 0)) {
    return false;
  }
  const bytes = Math.floor((base64.length * 3) / 4) - padding;
  return bytes >= 0 && bytes <= MAX_WALLPAPER_BYTES;
}

function parseThemeBundle(value: unknown): Omit<CustomThemeConfig, "id"> {
  if (!isPlainRecord(value) || !hasOnlyKeys(value, ["schema", "version", "theme"])) {
    throw new Error(t.invalidBundleShape);
  }
  if (value.schema !== "suo-theme-v1" || value.version !== 1 || !isPlainRecord(value.theme)) {
    throw new Error(t.unsupportedBundle);
  }
  const theme = value.theme;
  if (!hasOnlyKeys(theme, themeFieldNames) || !isPlainRecord(theme.platformOverrides)) {
    throw new Error(t.invalidThemeFields);
  }
  if (!hasOnlyKeys(theme.platformOverrides, platformOverrideFieldNames)) {
    throw new Error(t.invalidPlatformFields);
  }

  const colors = colorFields.map(([key]) => theme[key]);
  if (!colors.every(isHexColor)) throw new Error(t.invalidColor);
  if (typeof theme.name !== "string" || Array.from(theme.name.trim()).length < 1 || Array.from(theme.name.trim()).length > 40) {
    throw new Error(t.invalidThemeName);
  }

  const isIntegerIn = (candidate: unknown, minimum: number, maximum: number) =>
    typeof candidate === "number" && Number.isInteger(candidate) && candidate >= minimum && candidate <= maximum;
  if (
    !isIntegerIn(theme.windowOpacity, 70, 100) ||
    !isIntegerIn(theme.blurPx, 0, 40) ||
    !isIntegerIn(theme.shadowPercent, 0, 80) ||
    !isIntegerIn(theme.wallpaperOpacity, 0, 60) ||
    !isIntegerIn(theme.radiusPx, 0, 28) ||
    !isIntegerIn(theme.fontSizePx, 12, 18) ||
    !isIntegerIn(theme.launcherWidthPx, 620, 900) ||
    !isIntegerIn(theme.iconSizePx, 28, 48)
  ) {
    throw new Error(t.outOfRange);
  }
  if (
    !["system", "cjk", "mono"].includes(String(theme.fontFamily)) ||
    !["compact", "comfortable", "loose"].includes(String(theme.resultDensity)) ||
    ![6, 8, 10, 12].includes(Number(theme.maxResults)) ||
    typeof theme.showSourceBadge !== "boolean" ||
    !isSafeWallpaperDataUrl(theme.wallpaperDataUrl)
  ) {
    throw new Error(t.unsupportedOptions);
  }

  const platform = theme.platformOverrides;
  if (
    typeof platform.enabled !== "boolean" ||
    !isIntegerIn(platform.windowsBlurPx, 0, 40) ||
    !isIntegerIn(platform.windowsOpacity, 70, 100) ||
    !isIntegerIn(platform.macosBlurPx, 0, 40) ||
    !isIntegerIn(platform.macosOpacity, 70, 100)
  ) {
    throw new Error(t.invalidPlatformRange);
  }

  // Runtime checks above narrow every imported field before this typed boundary.
  const checked = theme as unknown as Omit<CustomThemeConfig, "id">;
  return {
    ...checked,
    name: checked.name.trim(),
    platformOverrides: { ...checked.platformOverrides },
  };
}

function colorLuminance(hex: string) {
  const channels = [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16) / 255);
  const linear = channels.map((channel) =>
    channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrastRatio(left: string, right: string) {
  const [bright, dark] = [colorLuminance(left), colorLuminance(right)].sort((a, b) => b - a);
  return (bright + 0.05) / (dark + 0.05);
}

function colorWithOpacity(hex: string, opacity: number) {
  const [red, green, blue] = [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16));
  return `rgb(${red} ${green} ${blue} / ${opacity})`;
}

function ColorControl({
  label,
  description,
  value,
  disabled,
  onChange,
}: {
  label: string;
  description: string;
  value: string;
  disabled: boolean;
  onChange: (value: string) => void;
}) {
  const [textValue, setTextValue] = useState(value);

  useEffect(() => setTextValue(value), [value]);

  const commitText = () => {
    const next = textValue.trim();
    if (isHexColor(next)) onChange(next.toLowerCase());
    else setTextValue(value);
  };

  return (
    <div className="appearance-control-row">
      <div className="appearance-control-copy"><strong>{label}</strong><small>{description}</small></div>
      <div className="appearance-color-controls">
        <input aria-label={t.colorAriaLabel.replace("{label}", label)} type="color" value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)} />
        <input aria-label={t.hexColorAriaLabel.replace("{label}", label)} className="appearance-hex-input" value={textValue} disabled={disabled} onChange={(event) => setTextValue(event.target.value)} onBlur={commitText} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} />
      </div>
    </div>
  );
}

function RangeControl({
  label,
  description,
  value,
  minimum,
  maximum,
  unit,
  disabled,
  onChange,
}: {
  label: string;
  description: string;
  value: number;
  minimum: number;
  maximum: number;
  unit: string;
  disabled: boolean;
  onChange: (value: number) => void;
}) {
  return (
    <div className="appearance-control-row">
      <div className="appearance-control-copy"><strong>{label}</strong><small>{description}</small></div>
      <label className="appearance-range-control">
        <input aria-label={label} type="range" min={minimum} max={maximum} value={value} disabled={disabled} onChange={(event) => onChange(Number(event.target.value))} />
        <output>{value}{unit}</output>
      </label>
    </div>
  );
}

export default function AppearanceEditor({ appearance, onChange, readOnly }: AppearanceEditorProps) {
  const [working, setWorking] = useState<AppearanceConfig>(() => cloneAppearance(appearance));
  const [tab, setTab] = useState<EditorTab>("colors");
  const [previewKind, setPreviewKind] = useState<PreviewKind>("launcher");
  const [notice, setNotice] = useState("");
  const [importing, setImporting] = useState(false);
  const importInputRef = useRef<HTMLInputElement>(null);
  const wallpaperInputRef = useRef<HTMLInputElement>(null);
  const externalSignature = useMemo(() => JSON.stringify(appearance), [appearance]);

  useEffect(() => {
    setWorking(cloneAppearance(appearance));
  }, [appearance, externalSignature]);

  const selectedCustomId = customThemeId(working.theme);
  const selectedCustomTheme = selectedCustomId
    ? working.customThemes.find((theme) => sameThemeId(theme.id, selectedCustomId)) ?? null
    : null;
  const selectedBuiltin = selectedCustomTheme ? null : (builtinThemeIds.includes(working.theme as BuiltinThemeId) ? working.theme as BuiltinThemeId : fallbackBuiltinTheme());
  const previewTheme = resolveTheme(working);
  const contrast = Math.min(
    contrastRatio(previewTheme.textColor, previewTheme.windowColor),
    contrastRatio(previewTheme.textColor, previewTheme.panelColor),
    contrastRatio(previewTheme.textColor, previewTheme.fieldColor),
    contrastRatio(previewTheme.textColor, previewTheme.selectionColor),
  );
  const contrastOk = contrast >= 4.5;
  const safeWallpaper = isSafeWallpaperDataUrl(previewTheme.wallpaperDataUrl) && previewTheme.wallpaperDataUrl ? previewTheme.wallpaperDataUrl : "";
  const currentPlatform = /Mac/i.test(navigator.platform) ? "macos" : /Win/i.test(navigator.platform) ? "windows" : null;
  const platformOverrides = previewTheme.platformOverrides;
  const previewBlurPx = platformOverrides.enabled && currentPlatform === "macos"
    ? platformOverrides.macosBlurPx
    : platformOverrides.enabled && currentPlatform === "windows"
      ? platformOverrides.windowsBlurPx
      : previewTheme.blurPx;
  const previewWindowOpacity = platformOverrides.enabled && currentPlatform === "macos"
    ? platformOverrides.macosOpacity
    : platformOverrides.enabled && currentPlatform === "windows"
      ? platformOverrides.windowsOpacity
      : previewTheme.windowOpacity;
  const previewStyle = {
    "--appearance-window": previewTheme.windowColor,
    "--appearance-panel": previewTheme.panelColor,
    "--appearance-field": previewTheme.fieldColor,
    "--appearance-text": previewTheme.textColor,
    "--appearance-muted": previewTheme.mutedColor,
    "--appearance-accent": previewTheme.accentColor,
    "--appearance-selection": previewTheme.selectionColor,
    "--appearance-border": previewTheme.borderColor,
    "--appearance-window-opacity": String(previewWindowOpacity / 100),
    "--appearance-window-composite": colorWithOpacity(previewTheme.windowColor, previewWindowOpacity / 100),
    "--appearance-blur": `${previewBlurPx}px`,
    "--appearance-wallpaper-opacity": String(previewTheme.wallpaperOpacity / 100),
    "--appearance-wallpaper-overlay": safeWallpaper ? String(1 - previewTheme.wallpaperOpacity / 100) : "0",
    "--appearance-radius": `${previewTheme.radiusPx}px`,
    "--appearance-font-size": `${previewTheme.fontSizePx}px`,
    "--appearance-icon-size": `${previewTheme.iconSizePx}px`,
    "--appearance-row-height": previewTheme.resultDensity === "compact" ? "48px" : previewTheme.resultDensity === "loose" ? "68px" : "58px",
    "--appearance-shadow-opacity": String(previewTheme.shadowPercent / 100),
    "--appearance-font-family": previewTheme.fontFamily === "mono" ? "ui-monospace, SFMono-Regular, Consolas, monospace" : previewTheme.fontFamily === "cjk" ? "PingFang SC, Microsoft YaHei, Hiragino Sans GB, sans-serif" : "Inter, ui-sans-serif, system-ui, sans-serif",
  } as CSSProperties;

  const updateSelectedCustom = (updater: (theme: CustomThemeConfig) => CustomThemeConfig) => {
    if (!selectedCustomTheme || readOnly) return;
    setWorking((current) => {
      const id = customThemeId(current.theme);
      if (!id) return current;
      const customThemes = current.customThemes.map((theme) => sameThemeId(theme.id, id) ? updater(theme) : theme);
      const selected = customThemes.find((theme) => sameThemeId(theme.id, id));
      return selected ? { ...current, customThemes, accentColor: selected.accentColor } : current;
    });
  };

  const chooseBuiltin = (id: BuiltinThemeId) => {
    if (readOnly) return;
    setWorking((current) => {
      const next = { ...current, theme: id };
      return { ...next, accentColor: resolveTheme(next).accentColor };
    });
  };

  const createTheme = () => {
    if (readOnly) return;
    if (working.customThemes.length >= 12) {
      setNotice(t.customThemeLimit.replace("{max}", "12"));
      return;
    }
    let created = false;
    setWorking((current) => {
      if (current.customThemes.length >= 12) return current;
      const source = customThemeId(current.theme)
        ? current.customThemes.find((theme) => sameThemeId(theme.id, customThemeId(current.theme)))
        : null;
      const generated = createCustomTheme(source ? undefined : current.theme);
      const nextTheme: CustomThemeConfig = source
        ? { ...source, id: generated.id, name: t.copyName.replace("{name}", source.name), platformOverrides: { ...source.platformOverrides } }
        : { ...generated, name: t.copyName.replace("{name}", builtinThemeLabels[current.theme as BuiltinThemeId] ?? t.defaultBuiltinName) };
      created = true;
      return { ...current, theme: `custom:${nextTheme.id}`, accentColor: nextTheme.accentColor, customThemes: [...current.customThemes, nextTheme] };
    });
    queueMicrotask(() => {
      setNotice(created ? t.createdTheme : t.customThemeLimit.replace("{max}", "12"));
      if (created) setTab("colors");
    });
  };

  const deleteTheme = () => {
    if (!selectedCustomTheme || readOnly) return;
    setWorking((current) => {
      const id = customThemeId(current.theme);
      const customThemes = current.customThemes.filter((theme) => !sameThemeId(theme.id, id));
      const next = { ...current, theme: fallbackBuiltinTheme(), customThemes };
      return { ...next, accentColor: resolveTheme(next).accentColor };
    });
    setNotice(t.deletedTheme);
  };

  const resetTheme = () => {
    if (!selectedCustomTheme || readOnly) return;
    const reset = createCustomTheme();
    updateSelectedCustom((theme) => ({ ...reset, id: theme.id, name: theme.name }));
    setNotice(t.resetTheme);
  };

  const applyToDraft = () => {
    if (readOnly) return;
    if (working.customThemes.some((theme) => !theme.name.trim())) {
      setNotice(t.themeNameRequired);
      return;
    }
    onChange(cloneAppearance(working));
    setNotice(t.appliedTheme);
  };

  const exportTheme = () => {
    const builtinId = selectedBuiltin ?? fallbackBuiltinTheme();
    const source = selectedCustomTheme ?? {
      ...createCustomTheme(builtinId),
      name: builtinThemeLabels[builtinId],
    };
    const { id: _id, ...theme } = source;
    const payload: ThemeBundleV1 = { schema: "suo-theme-v1", version: 1, theme };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${theme.name.replace(/[\\/:*?\"<>|]/g, "_") || "suo-theme"}.suo-theme.json`;
    anchor.click();
    window.setTimeout(() => URL.revokeObjectURL(url), 0);
    setNotice(t.exportedTheme);
  };

  const importTheme = (file: File) => {
    if (readOnly || importing) return;
    if (working.customThemes.length >= 12) {
      setNotice(t.importedThemeLimit.replace("{max}", "12"));
      return;
    }
    if (file.size > MAX_THEME_BUNDLE_BYTES) {
      setNotice(t.bundleTooLarge);
      return;
    }
    setImporting(true);
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const imported = parseThemeBundle(JSON.parse(String(reader.result)));
        const seed = createCustomTheme();
        const theme: CustomThemeConfig = { ...imported, id: seed.id, platformOverrides: { ...imported.platformOverrides } };
        let importedTheme = false;
        setWorking((current) => {
          if (current.customThemes.length >= 12) return current;
          importedTheme = true;
          return {
            ...current,
            theme: `custom:${theme.id}`,
            accentColor: theme.accentColor,
            customThemes: [...current.customThemes, theme],
          };
        });
        queueMicrotask(() => {
          setNotice(importedTheme ? t.importedTheme.replace("{name}", theme.name) : t.importedThemeLimit.replace("{max}", "12"));
          if (importedTheme) setTab("colors");
        });
      } catch (error) {
        setNotice(t.importFailed.replace("{reason}", error instanceof Error ? error.message : t.invalidBundle));
      } finally {
        setImporting(false);
      }
    };
    reader.onerror = () => {
      setImporting(false);
      setNotice(t.unableToReadBundle);
    };
    reader.readAsText(file, "utf-8");
  };

  const chooseWallpaper = (file: File) => {
    if (!selectedCustomTheme || readOnly) return;
    if (!/^image\/(png|jpeg|webp)$/.test(file.type) || file.size > MAX_WALLPAPER_BYTES) {
      setNotice(t.wallpaperTooLarge);
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = String(reader.result);
      if (!isSafeWallpaperDataUrl(dataUrl)) {
        setNotice(t.invalidWallpaper);
        return;
      }
      updateSelectedCustom((theme) => ({ ...theme, wallpaperDataUrl: dataUrl }));
      setNotice(t.wallpaperLoaded.replace("{name}", file.name));
    };
    reader.onerror = () => setNotice(t.unableToReadWallpaper);
    reader.readAsDataURL(file);
  };

  return (
    <section className="appearance-editor" aria-label={t.ariaLabel}>
      <header className="appearance-editor-heading">
        <div>
          <h2>{t.title}</h2>
          <p>{t.description}</p>
        </div>
        <div className="appearance-editor-actions">
          <input ref={importInputRef} className="appearance-hidden-input" type="file" accept="application/json,.json" onChange={(event) => { const file = event.target.files?.[0]; if (file) importTheme(file); event.currentTarget.value = ""; }} />
          <button type="button" className="appearance-secondary-button" disabled={readOnly || importing} onClick={() => importInputRef.current?.click()}>{t.importTheme}</button>
          <button type="button" className="appearance-secondary-button" onClick={exportTheme}>{t.exportJson}</button>
          <button type="button" className="appearance-primary-button" disabled={readOnly} onClick={createTheme}>{t.createTheme}</button>
        </div>
      </header>

      <div className="appearance-theme-grid" aria-label={t.themeLibrary}>
        {builtinThemeIds.map((id) => (
          <button key={id} type="button" className={`appearance-theme-card ${selectedBuiltin === id ? "selected" : ""}`} aria-pressed={selectedBuiltin === id} disabled={readOnly} onClick={() => chooseBuiltin(id)}>
            <span className={`appearance-theme-swatch appearance-theme-swatch-${id}`} aria-hidden="true" />
            <span><strong>{builtinThemeLabels[id]}</strong><small>{t.builtin}</small></span>
          </button>
        ))}
        {working.customThemes.map((theme) => (
          <button key={theme.id} type="button" className={`appearance-theme-card appearance-theme-card-custom ${selectedCustomTheme?.id === theme.id ? "selected" : ""}`} aria-pressed={selectedCustomTheme?.id === theme.id} disabled={readOnly} onClick={() => setWorking((current) => ({ ...current, theme: `custom:${theme.id}`, accentColor: theme.accentColor }))}>
            <span className="appearance-theme-swatch" style={{ background: `linear-gradient(135deg, ${theme.windowColor}, ${theme.accentColor})` }} aria-hidden="true" />
            <span><strong>{theme.name}</strong><small>{t.custom}</small></span>
          </button>
        ))}
      </div>

      <div className="appearance-workbench">
        <section className="appearance-edit-panel">
          <div className="appearance-panel-heading">
            <div>
              {selectedCustomTheme ? (
                <label className="appearance-theme-name-editor">
                  <span>{t.themeName}</span>
                  <input
                    aria-label={t.themeName}
                    value={selectedCustomTheme.name}
                    disabled={readOnly}
                    onChange={(event) => {
                      const name = Array.from(event.target.value).slice(0, 40).join("");
                      updateSelectedCustom((theme) => ({ ...theme, name }));
                    }}
                  />
                  <small>{t.themeNameHint}</small>
                </label>
              ) : <strong>{t.builtinReadOnly.replace("{name}", builtinThemeLabels[selectedBuiltin ?? fallbackBuiltinTheme()])}</strong>}
              <small>{selectedCustomTheme ? t.customPreviewHint : t.builtinPreviewHint}</small>
            </div>
            {selectedCustomTheme && <div className="appearance-inline-buttons"><button type="button" disabled={readOnly} onClick={resetTheme}>{t.restoreDefault}</button><button type="button" className="appearance-danger-button" disabled={readOnly} onClick={deleteTheme}>{t.delete}</button></div>}
          </div>

          {!selectedCustomTheme ? (
            <div className="appearance-builtin-empty"><p>{t.builtinEmpty}</p><button type="button" className="appearance-primary-button" disabled={readOnly} onClick={createTheme}>{t.createFromBuiltin}</button></div>
          ) : (
            <>
              <div className="appearance-tabs" role="tablist" aria-label={t.editorTabs}>
                {([ ["colors", t.colorsAndMaterial], ["layout", t.layoutAndComponents], ["platform", t.platformOverrides] ] as const).map(([id, label]) => (
                  <button key={id} id={`appearance-tab-${id}`} type="button" role="tab" aria-selected={tab === id} aria-controls={`appearance-panel-${id}`} className={tab === id ? "active" : ""} onClick={() => setTab(id)}>{label}</button>
                ))}
              </div>

              {tab === "colors" && <div id="appearance-panel-colors" role="tabpanel" aria-labelledby="appearance-tab-colors" className="appearance-tab-panel">
                <h3>{t.primaryColors}</h3>
                {colorFields.map(([key, label, description]) => <ColorControl key={key} label={label} description={description} value={selectedCustomTheme[key]} disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, [key]: value }))} />)}
                <h3>{t.material}</h3>
                <RangeControl label={t.windowOpacity} description={t.windowOpacityHint} value={selectedCustomTheme.windowOpacity} minimum={70} maximum={100} unit="%" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, windowOpacity: value }))} />
                <RangeControl label={t.backgroundBlur} description={t.blurHint} value={selectedCustomTheme.blurPx} minimum={0} maximum={40} unit=" px" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, blurPx: value }))} />
                <div className="appearance-control-row">
                  <div className="appearance-control-copy"><strong>{t.wallpaper}</strong><small>{selectedCustomTheme.wallpaperDataUrl ? t.wallpaperLoadedHint : t.wallpaperEmptyHint}</small></div>
                  <div className="appearance-inline-buttons"><input ref={wallpaperInputRef} className="appearance-hidden-input" type="file" accept="image/png,image/jpeg,image/webp" onChange={(event) => { const file = event.target.files?.[0]; if (file) chooseWallpaper(file); event.currentTarget.value = ""; }} /><button type="button" disabled={readOnly} onClick={() => wallpaperInputRef.current?.click()}>{t.chooseImage}</button>{selectedCustomTheme.wallpaperDataUrl && <button type="button" disabled={readOnly} onClick={() => updateSelectedCustom((theme) => ({ ...theme, wallpaperDataUrl: "" }))}>{t.remove}</button>}</div>
                </div>
                <RangeControl label={t.wallpaperOpacity} description={t.wallpaperOpacityHint} value={selectedCustomTheme.wallpaperOpacity} minimum={0} maximum={60} unit="%" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, wallpaperOpacity: value }))} />
              </div>}

              {tab === "layout" && <div id="appearance-panel-layout" role="tabpanel" aria-labelledby="appearance-tab-layout" className="appearance-tab-panel">
                <h3>{t.window}</h3>
                <RangeControl label={t.launcherWidth} description={t.launcherWidthHint} value={selectedCustomTheme.launcherWidthPx} minimum={620} maximum={900} unit=" px" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, launcherWidthPx: value }))} />
                <RangeControl label={t.radius} description={t.radiusHint} value={selectedCustomTheme.radiusPx} minimum={0} maximum={28} unit=" px" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, radiusPx: value }))} />
                <RangeControl label={t.shadow} description={t.shadowHint} value={selectedCustomTheme.shadowPercent} minimum={0} maximum={80} unit="%" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, shadowPercent: value }))} />
                <h3>{t.density}</h3>
                <div className="appearance-control-row"><div className="appearance-control-copy"><strong>{t.resultDensity}</strong><small>{t.resultDensityHint}</small></div><div className="appearance-segmented">{([ ["compact", t.compact], ["comfortable", t.comfortable], ["loose", t.loose] ] as const).map(([value, label]) => <button key={value} type="button" className={selectedCustomTheme.resultDensity === value ? "active" : ""} aria-pressed={selectedCustomTheme.resultDensity === value} disabled={readOnly} onClick={() => updateSelectedCustom((theme) => ({ ...theme, resultDensity: value }))}>{label}</button>)}</div></div>
                <RangeControl label={t.fontSize} description={t.fontSizeHint} value={selectedCustomTheme.fontSizePx} minimum={12} maximum={18} unit=" px" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, fontSizePx: value }))} />
                <RangeControl label={t.iconSize} description={t.iconSizeHint} value={selectedCustomTheme.iconSizePx} minimum={28} maximum={48} unit=" px" disabled={readOnly} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, iconSizePx: value }))} />
                <label className="appearance-control-row"><span className="appearance-control-copy"><strong>{t.maxResults}</strong><small>{t.maxResultsHint}</small></span><select value={selectedCustomTheme.maxResults} disabled={readOnly} onChange={(event) => updateSelectedCustom((theme) => ({ ...theme, maxResults: Number(event.target.value) as CustomThemeConfig["maxResults"] }))}>{[6, 8, 10, 12].map((value) => <option key={value} value={value}>{t.resultCount.replace("{count}", String(value))}</option>)}</select></label>
                <label className="appearance-control-row"><span className="appearance-control-copy"><strong>{t.fontFamily}</strong><small>{t.fontFamilyHint}</small></span><select value={selectedCustomTheme.fontFamily} disabled={readOnly} onChange={(event) => updateSelectedCustom((theme) => ({ ...theme, fontFamily: event.target.value as CustomThemeConfig["fontFamily"] }))}><option value="system">{t.systemFont}</option><option value="cjk">{t.cjkFont}</option><option value="mono">{t.monoFont}</option></select></label>
                <label className="appearance-control-row"><span className="appearance-control-copy"><strong>{t.showSourceBadge}</strong><small>{t.showSourceBadgeHint}</small></span><input className="appearance-switch" type="checkbox" checked={selectedCustomTheme.showSourceBadge} disabled={readOnly} onChange={(event) => updateSelectedCustom((theme) => ({ ...theme, showSourceBadge: event.target.checked }))} /></label>
              </div>}

              {tab === "platform" && <div id="appearance-panel-platform" role="tabpanel" aria-labelledby="appearance-tab-platform" className="appearance-tab-panel">
                <h3>{t.platformOverrideValues}</h3>
                <label className="appearance-control-row"><span className="appearance-control-copy"><strong>{t.enablePlatformOverrides}</strong><small>{t.enablePlatformOverridesHint}</small></span><input className="appearance-switch" type="checkbox" checked={selectedCustomTheme.platformOverrides.enabled} disabled={readOnly} onChange={(event) => updateSelectedCustom((theme) => ({ ...theme, platformOverrides: { ...theme.platformOverrides, enabled: event.target.checked } }))} /></label>
                <div className="appearance-platform-grid" aria-disabled={!selectedCustomTheme.platformOverrides.enabled}>
                  <div><h4>Windows</h4><RangeControl label={t.backgroundBlur} description={t.unavailableFallback} value={selectedCustomTheme.platformOverrides.windowsBlurPx} minimum={0} maximum={40} unit=" px" disabled={readOnly || !selectedCustomTheme.platformOverrides.enabled} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, platformOverrides: { ...theme.platformOverrides, windowsBlurPx: value } }))} /><RangeControl label={t.windowOpacity} description={t.windowsOnly} value={selectedCustomTheme.platformOverrides.windowsOpacity} minimum={70} maximum={100} unit="%" disabled={readOnly || !selectedCustomTheme.platformOverrides.enabled} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, platformOverrides: { ...theme.platformOverrides, windowsOpacity: value } }))} /></div>
                  <div><h4>macOS</h4><RangeControl label={t.backgroundBlur} description={t.unavailableFallback} value={selectedCustomTheme.platformOverrides.macosBlurPx} minimum={0} maximum={40} unit=" px" disabled={readOnly || !selectedCustomTheme.platformOverrides.enabled} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, platformOverrides: { ...theme.platformOverrides, macosBlurPx: value } }))} /><RangeControl label={t.windowOpacity} description={t.macosOnly} value={selectedCustomTheme.platformOverrides.macosOpacity} minimum={70} maximum={100} unit="%" disabled={readOnly || !selectedCustomTheme.platformOverrides.enabled} onChange={(value) => updateSelectedCustom((theme) => ({ ...theme, platformOverrides: { ...theme.platformOverrides, macosOpacity: value } }))} /></div>
                </div>
                <p className="appearance-safety-note">{t.platformSafetyHint}</p>
              </div>}
            </>
          )}

          <footer className="appearance-edit-footer"><span>{notice || t.defaultHint}</span><button type="button" className="appearance-primary-button" disabled={readOnly} onClick={applyToDraft}>{t.applyToDraft}</button></footer>
        </section>

        <section className="appearance-preview-panel" aria-label={t.previewAriaLabel}>
          <div className="appearance-preview-heading"><div className="appearance-preview-switch" role="group" aria-label={t.previewContent}><button type="button" className={previewKind === "launcher" ? "active" : ""} aria-pressed={previewKind === "launcher"} onClick={() => setPreviewKind("launcher")}>{t.launcherPreview}</button><button type="button" className={previewKind === "settings" ? "active" : ""} aria-pressed={previewKind === "settings"} onClick={() => setPreviewKind("settings")}>{t.settingsPreview}</button></div><span className={`appearance-contrast ${contrastOk ? "ok" : "warning"}`}>{contrastOk ? t.contrastPass : t.contrastLow} · {contrast.toFixed(1)}:1</span></div>
          <div className="appearance-preview-canvas" style={previewStyle}>
            <div className="appearance-preview-surface" style={safeWallpaper ? { backgroundImage: `url("${safeWallpaper}")` } : undefined}>
              {previewKind === "launcher" ? <LauncherPreview theme={previewTheme} /> : <SettingsPreview />}
            </div>
          </div>
          <p className="appearance-preview-note">{t.previewHint}{safeWallpaper ? t.wallpaperContrastHint : ""}</p>
        </section>
      </div>
    </section>
  );
}

function LauncherPreview({ theme }: { theme: CustomThemeConfig }) {
  return (
    <div className="appearance-launcher-preview">
      <div className="appearance-preview-search"><span aria-hidden="true">⌕</span><strong>{t.previewQuery}</strong><kbd>Alt + Space</kbd></div>
      <div className="appearance-preview-provider"><span><b>●</b> {t.previewProvider}</span><span>{t.rebuildIndex}</span></div>
      <div className="appearance-preview-results">
        <PreviewResult icon={t.previewAppIcon} title={t.previewAppTitle} subtitle={t.previewAppPath} badge={t.previewAppBadge} selected showBadge={theme.showSourceBadge} />
        <PreviewResult icon={t.previewFileIcon} title={t.previewFileTitle} subtitle={t.previewFilePath} badge={t.previewFileBadge} showBadge={theme.showSourceBadge} />
        <PreviewResult icon="⌘" title={t.previewWebTitle} subtitle={t.previewWebSubtitle} badge={t.previewWebBadge} showBadge={theme.showSourceBadge} />
      </div>
      <div className="appearance-preview-footer"><span>{t.previewFooterLeft}</span><span>{t.previewFooterRight}</span></div>
    </div>
  );
}

function PreviewResult({ icon, title, subtitle, badge, selected = false, showBadge }: { icon: string; title: string; subtitle: string; badge: string; selected?: boolean; showBadge: boolean }) {
  return <div className={`appearance-preview-result ${selected ? "selected" : ""}`}><span className="appearance-preview-result-icon">{icon}</span><span><strong>{title}</strong><small>{subtitle}</small></span>{showBadge && <em>{badge}</em>}</div>;
}

function SettingsPreview() {
  return <div className="appearance-settings-preview"><header>{t.previewSettingsTitle}</header><div className="appearance-settings-preview-body"><aside><span>{t.previewGeneral}</span><span>{t.previewSearch}</span><span>{t.previewCommands}</span><strong>{t.previewAppearance}</strong></aside><main><h3>{t.previewAppearance}</h3><p>{t.previewSettingsDescription}</p><div className="appearance-settings-preview-card"><i /><i /><i /></div></main></div></div>;
}
