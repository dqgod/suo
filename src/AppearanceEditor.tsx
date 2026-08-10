import { useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import {
  builtinThemeIds,
  buildLauncherThemeBundle,
  buildSettingsThemeBundle,
  createLauncherTheme,
  createSettingsTheme,
  parseLauncherThemeBundle,
  parseSettingsThemeBundle,
  resolveLauncherTheme,
  resolveSettingsTheme,
  validateWallpaperImageDataUrl,
  type LauncherCustomThemeConfig,
  type LauncherThemeConfig,
  type SearchBorderStyle,
  type SettingsCustomThemeConfig,
  type SettingsThemeConfig,
  type ThemeBackgroundConfig,
  type ThemeSelection,
} from "./config";
import { zhCN } from "./i18n/zh-CN";
import "./AppearanceEditor.css";

type ThemeScope = "launcher" | "settings";
type AppearanceEditorProps = {
  launcherTheme: LauncherThemeConfig;
  settingsTheme: SettingsThemeConfig;
  onChange: (themes: { launcherTheme: LauncherThemeConfig; settingsTheme: SettingsThemeConfig }) => Promise<boolean>;
  saveSettingsManually: boolean;
  readOnly: boolean;
  saving?: boolean;
};

type ThemeTarget = {
  scope: ThemeScope;
  themeId: string;
};

const t = zhCN.appearanceEditor;
const MAX_CUSTOM_THEMES = 12;
const MAX_THEME_BUNDLE_BYTES = Math.floor(2.5 * 1024 * 1024);
const MAX_WALLPAPER_BYTES = Math.floor(1.5 * 1024 * 1024);
const builtinLabels = { midnight: t.midnight, paper: t.paper, forest: t.forest } as const;

function cloneBackground(theme: ThemeBackgroundConfig): ThemeBackgroundConfig {
  return { ...theme, platformOverrides: { ...theme.platformOverrides } };
}

function cloneLauncherScope(scope: LauncherThemeConfig): LauncherThemeConfig {
  return { ...scope, customThemes: scope.customThemes.map((theme) => ({ ...theme, ...cloneBackground(theme) })) };
}

function cloneSettingsScope(scope: SettingsThemeConfig): SettingsThemeConfig {
  return { ...scope, customThemes: scope.customThemes.map((theme) => ({ ...theme, ...cloneBackground(theme) })) };
}

function customId(theme: string) {
  return theme.startsWith("custom:") ? theme.slice("custom:".length) : null;
}

function sameId(left: string, right: string | null) {
  return right !== null && left.toLowerCase() === right.toLowerCase();
}

function themeName(selection: ThemeSelection, customThemes: Array<{ id: string; name: string }>) {
  const id = customId(selection);
  return id
    ? customThemes.find((theme) => sameId(theme.id, id))?.name ?? t.unnamedTheme
    : builtinLabels[selection as keyof typeof builtinLabels];
}

function isHexColor(value: string) {
  return /^#[\da-f]{6}$/i.test(value);
}

function colorLuminance(hex: string) {
  const channels = [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16) / 255);
  const linear = channels.map((channel) => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrastRatio(left: string, right: string) {
  const [bright, dark] = [colorLuminance(left), colorLuminance(right)].sort((a, b) => b - a);
  return (bright + 0.05) / (dark + 0.05);
}

function previewWallpaper(dataUrl: string) {
  return /^data:image\/(?:png|jpeg|webp);base64,[A-Za-z0-9+/]+={0,2}$/.test(dataUrl)
    ? `url("${dataUrl}")`
    : "none";
}

function previewMaterial(theme: ThemeBackgroundConfig) {
  const overrides = theme.platformOverrides;
  const isMac = /Mac/i.test(navigator.platform);
  const isWindows = /Win/i.test(navigator.platform);
  return {
    blurPx: overrides.enabled && isMac ? overrides.macosBlurPx : overrides.enabled && isWindows ? overrides.windowsBlurPx : theme.blurPx,
    opacity: overrides.enabled && isMac ? overrides.macosOpacity : overrides.enabled && isWindows ? overrides.windowsOpacity : theme.windowOpacity,
  };
}

function needsSceneContrastReview(theme: ThemeBackgroundConfig) {
  const material = previewMaterial(theme);
  return material.opacity < 100 || (Boolean(theme.wallpaperDataUrl) && theme.wallpaperOpacity > 0);
}

type ContrastCheck = { label: string; ratio: number; minimum: number; kind: "text" | "border" };

function launcherChecks(theme: LauncherCustomThemeConfig): ContrastCheck[] {
  const checks: ContrastCheck[] = [
    { label: t.accentColor, ratio: Math.min(contrastRatio(theme.accentColor, theme.searchBackground), contrastRatio(theme.accentColor, theme.selectedRowBackground)), minimum: 3, kind: "border" },
    { label: t.searchText, ratio: contrastRatio(theme.searchTextColor, theme.searchBackground), minimum: 4.5, kind: "text" },
    { label: t.normalPrimary, ratio: contrastRatio(theme.normalPrimaryColor, theme.normalRowBackground), minimum: 4.5, kind: "text" },
    { label: t.normalSecondary, ratio: contrastRatio(theme.normalSecondaryColor, theme.normalRowBackground), minimum: 4.5, kind: "text" },
    { label: t.selectedPrimary, ratio: contrastRatio(theme.selectedPrimaryColor, theme.selectedRowBackground), minimum: 4.5, kind: "text" },
    { label: t.selectedSecondary, ratio: contrastRatio(theme.selectedSecondaryColor, theme.selectedRowBackground), minimum: 4.5, kind: "text" },
  ];
  if (theme.windowBorderWidthPx > 0) checks.push({ label: t.windowBorder, ratio: contrastRatio(theme.windowBorder, theme.windowBackground), minimum: 3, kind: "border" });
  if (theme.searchBorderWidthPx > 0 && theme.searchBorderStyle !== "none") checks.push({ label: t.searchBorder, ratio: contrastRatio(theme.searchBorder, theme.searchBackground), minimum: 3, kind: "border" });
  return checks;
}

function settingsChecks(theme: SettingsCustomThemeConfig): ContrastCheck[] {
  return [
    { label: t.accentColor, ratio: Math.min(contrastRatio(theme.accentColor, theme.contentBackground), contrastRatio(theme.accentColor, theme.cardBackground), contrastRatio(theme.accentColor, theme.sidebarBackground)), minimum: 3, kind: "border" },
    { label: t.primaryText, ratio: contrastRatio(theme.primaryTextColor, theme.contentBackground), minimum: 4.5, kind: "text" },
    { label: t.secondaryText, ratio: contrastRatio(theme.secondaryTextColor, theme.contentBackground), minimum: 4.5, kind: "text" },
    { label: t.navText, ratio: contrastRatio(theme.navTextColor, theme.sidebarBackground), minimum: 4.5, kind: "text" },
    { label: t.borderColor, ratio: contrastRatio(theme.borderColor, theme.cardBackground), minimum: 3, kind: "border" },
  ];
}

function ColorControl({ label, value, disabled, onChange }: { label: string; value: string; disabled: boolean; onChange: (value: string) => void }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const commit = () => {
    const next = text.trim();
    if (isHexColor(next)) onChange(next.toLowerCase());
    else setText(value);
  };
  return <label className="appearance-control">
    <span className="appearance-control-copy"><strong>{label}</strong></span>
    <span className="appearance-color-control">
      <input aria-label={t.colorAriaLabel.replace("{label}", label)} type="color" value={value} disabled={disabled} onChange={(event) => onChange(event.target.value)} />
      <input aria-label={t.hexColorAriaLabel.replace("{label}", label)} value={text} disabled={disabled} onChange={(event) => setText(event.target.value)} onBlur={commit} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} />
    </span>
  </label>;
}

function RangeControl({ label, value, minimum, maximum, unit = " px", disabled, onChange }: { label: string; value: number; minimum: number; maximum: number; unit?: string; disabled: boolean; onChange: (value: number) => void }) {
  return <label className="appearance-control">
    <span className="appearance-control-copy"><strong>{label}</strong></span>
    <span className="appearance-range-control"><input aria-label={label} type="range" min={minimum} max={maximum} value={value} disabled={disabled} onChange={(event) => onChange(Number(event.target.value))} /><output>{value}{unit}</output></span>
  </label>;
}

function ToggleControl({ label, checked, disabled, onChange }: { label: string; checked: boolean; disabled: boolean; onChange: (value: boolean) => void }) {
  return <label className="appearance-control"><span className="appearance-control-copy"><strong>{label}</strong></span><input aria-label={label} className="appearance-switch" type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /></label>;
}

function Section({ title, hint, children, open = false }: { title: string; hint: string; children: React.ReactNode; open?: boolean }) {
  return <details className="appearance-section" open={open}>
    <summary><span><strong>{title}</strong><small>{hint}</small></span><span aria-hidden="true">⌄</span></summary>
    <div className="appearance-control-grid">{children}</div>
  </details>;
}

function BackgroundControls({ theme, disabled, onChange, onWallpaper, onRemoveWallpaper }: { theme: ThemeBackgroundConfig; disabled: boolean; onChange: (update: (theme: ThemeBackgroundConfig) => ThemeBackgroundConfig) => void; onWallpaper: (file: File) => void; onRemoveWallpaper: () => void }) {
  const wallpaperRef = useRef<HTMLInputElement>(null);
  const updatePlatform = (field: keyof ThemeBackgroundConfig["platformOverrides"], value: number | boolean) => onChange((current) => ({ ...current, platformOverrides: { ...current.platformOverrides, [field]: value } }));
  return <Section title={t.advanced} hint={t.advancedHint}>
    <RangeControl label={t.windowOpacity} value={theme.windowOpacity} minimum={70} maximum={100} unit="%" disabled={disabled} onChange={(value) => onChange((current) => ({ ...current, windowOpacity: value }))} />
    <RangeControl label={t.blur} value={theme.blurPx} minimum={0} maximum={40} disabled={disabled} onChange={(value) => onChange((current) => ({ ...current, blurPx: value }))} />
    <RangeControl label={t.shadow} value={theme.shadowPercent} minimum={0} maximum={80} unit="%" disabled={disabled} onChange={(value) => onChange((current) => ({ ...current, shadowPercent: value }))} />
    <label className="appearance-control"><span className="appearance-control-copy"><strong>{t.wallpaper}</strong><small>{theme.wallpaperDataUrl ? t.wallpaperLoaded : t.wallpaperEmpty}</small></span><span className="appearance-inline-actions"><input ref={wallpaperRef} className="appearance-hidden-input" type="file" accept="image/png,image/jpeg,image/webp" disabled={disabled} onChange={(event) => { const file = event.target.files?.[0]; if (file) onWallpaper(file); event.currentTarget.value = ""; }} /><button type="button" disabled={disabled} onClick={() => wallpaperRef.current?.click()}>{t.chooseImage}</button>{theme.wallpaperDataUrl && <button type="button" disabled={disabled} onClick={onRemoveWallpaper}>{t.remove}</button>}</span></label>
    <RangeControl label={t.wallpaperOpacity} value={theme.wallpaperOpacity} minimum={0} maximum={60} unit="%" disabled={disabled} onChange={(value) => onChange((current) => ({ ...current, wallpaperOpacity: value }))} />
    <div className="appearance-wide-control"><ToggleControl label={t.platformOverrides} checked={theme.platformOverrides.enabled} disabled={disabled} onChange={(value) => updatePlatform("enabled", value)} />
      {theme.platformOverrides.enabled && <div className="appearance-platform-grid"><div><strong>{t.windows}</strong><RangeControl label={t.blur} value={theme.platformOverrides.windowsBlurPx} minimum={0} maximum={40} disabled={disabled} onChange={(value) => updatePlatform("windowsBlurPx", value)} /><RangeControl label={t.windowOpacity} value={theme.platformOverrides.windowsOpacity} minimum={70} maximum={100} unit="%" disabled={disabled} onChange={(value) => updatePlatform("windowsOpacity", value)} /></div><div><strong>{t.macos}</strong><RangeControl label={t.blur} value={theme.platformOverrides.macosBlurPx} minimum={0} maximum={40} disabled={disabled} onChange={(value) => updatePlatform("macosBlurPx", value)} /><RangeControl label={t.windowOpacity} value={theme.platformOverrides.macosOpacity} minimum={70} maximum={100} unit="%" disabled={disabled} onChange={(value) => updatePlatform("macosOpacity", value)} /></div></div>}
    </div>
  </Section>;
}

function ContrastAudit({ checks, sceneReview }: { checks: ContrastCheck[]; sceneReview: boolean }) {
  return <div className="appearance-contrast-audit" aria-live="polite"><strong>{t.contrastTitle}</strong><small>{sceneReview ? t.contrastSceneHint : t.contrastHint}</small><div>{checks.map((check) => { const ok = check.ratio >= check.minimum; const verified = ok && !sceneReview; return <span key={check.label} className={verified ? "ok" : "warning"}>{check.kind === "text" ? t.contrastText : t.contrastBorder} · {check.label} {check.ratio.toFixed(1)}:1 {verified ? t.contrastPass : ok ? t.contrastSceneReview : t.contrastAdjust}</span>; })}</div></div>;
}

function LauncherControls({ theme, disabled, update, onWallpaper, onRemoveWallpaper }: { theme: LauncherCustomThemeConfig; disabled: boolean; update: (update: (theme: LauncherCustomThemeConfig) => LauncherCustomThemeConfig) => void; onWallpaper: (file: File) => void; onRemoveWallpaper: () => void }) {
  return <>
    <Section title={t.launcherStructure} hint={t.launcherStructureHint} open>
      <ColorControl label={t.accentColor} value={theme.accentColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, accentColor: value }))} />
      <ColorControl label={t.windowBackground} value={theme.windowBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, windowBackground: value }))} />
      <ColorControl label={t.windowBorder} value={theme.windowBorder} disabled={disabled} onChange={(value) => update((current) => ({ ...current, windowBorder: value }))} />
      <RangeControl label={t.windowBorderWidth} value={theme.windowBorderWidthPx} minimum={0} maximum={4} disabled={disabled} onChange={(value) => update((current) => ({ ...current, windowBorderWidthPx: value }))} />
      <RangeControl label={t.windowWidth} value={theme.windowWidthPx} minimum={620} maximum={900} disabled={disabled} onChange={(value) => update((current) => ({ ...current, windowWidthPx: value, searchWidthPx: Math.min(current.searchWidthPx, value) }))} />
      <RangeControl label={t.windowRadius} value={theme.windowRadiusPx} minimum={0} maximum={28} disabled={disabled} onChange={(value) => update((current) => ({ ...current, windowRadiusPx: value }))} />
      <ColorControl label={t.searchBackground} value={theme.searchBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, searchBackground: value }))} />
      <ColorControl label={t.searchBorder} value={theme.searchBorder} disabled={disabled} onChange={(value) => update((current) => ({ ...current, searchBorder: value }))} />
      <label className="appearance-control"><span className="appearance-control-copy"><strong>{t.searchBorderStyle}</strong></span><select value={theme.searchBorderStyle} disabled={disabled} onChange={(event) => update((current) => ({ ...current, searchBorderStyle: event.target.value as SearchBorderStyle }))}><option value="solid">{t.solid}</option><option value="dashed">{t.dashed}</option><option value="dotted">{t.dotted}</option><option value="double">{t.double}</option><option value="none">{t.none}</option></select></label>
      <RangeControl label={t.searchBorderWidth} value={theme.searchBorderWidthPx} minimum={0} maximum={4} disabled={disabled} onChange={(value) => update((current) => ({ ...current, searchBorderWidthPx: value }))} />
      <RangeControl label={t.searchWidth} value={theme.searchWidthPx} minimum={320} maximum={theme.windowWidthPx} disabled={disabled} onChange={(value) => update((current) => ({ ...current, searchWidthPx: value }))} />
      <ColorControl label={t.searchText} value={theme.searchTextColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, searchTextColor: value }))} />
      <RangeControl label={t.searchFontSize} value={theme.searchFontSizePx} minimum={12} maximum={24} disabled={disabled} onChange={(value) => update((current) => ({ ...current, searchFontSizePx: value }))} />
      <ToggleControl label={t.showSearchIcon} checked={theme.showSearchIcon} disabled={disabled} onChange={(value) => update((current) => ({ ...current, showSearchIcon: value }))} />
      <ToggleControl label={t.showLogo} checked={theme.showLogo} disabled={disabled} onChange={(value) => update((current) => ({ ...current, showLogo: value }))} />
    </Section>
    <Section title={t.normalResults} hint={t.normalResultsHint}>
      <ColorControl label={t.normalRowBackground} value={theme.normalRowBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, normalRowBackground: value }))} />
      <ColorControl label={t.normalPrimary} value={theme.normalPrimaryColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, normalPrimaryColor: value }))} />
      <RangeControl label={t.normalPrimarySize} value={theme.normalPrimaryFontSizePx} minimum={12} maximum={20} disabled={disabled} onChange={(value) => update((current) => ({ ...current, normalPrimaryFontSizePx: value }))} />
      <ColorControl label={t.normalSecondary} value={theme.normalSecondaryColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, normalSecondaryColor: value }))} />
      <RangeControl label={t.normalSecondarySize} value={theme.normalSecondaryFontSizePx} minimum={10} maximum={18} disabled={disabled} onChange={(value) => update((current) => ({ ...current, normalSecondaryFontSizePx: value }))} />
      <RangeControl label={t.rowHeight} value={theme.normalRowHeightPx} minimum={44} maximum={84} disabled={disabled} onChange={(value) => update((current) => ({ ...current, normalRowHeightPx: value }))} />
    </Section>
    <Section title={t.selectedAndIcons} hint={t.selectedAndIconsHint}>
      <ColorControl label={t.selectedBackground} value={theme.selectedRowBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, selectedRowBackground: value }))} />
      <ColorControl label={t.selectedPrimary} value={theme.selectedPrimaryColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, selectedPrimaryColor: value }))} />
      <RangeControl label={t.selectedPrimarySize} value={theme.selectedPrimaryFontSizePx} minimum={12} maximum={20} disabled={disabled} onChange={(value) => update((current) => ({ ...current, selectedPrimaryFontSizePx: value }))} />
      <ColorControl label={t.selectedSecondary} value={theme.selectedSecondaryColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, selectedSecondaryColor: value }))} />
      <RangeControl label={t.selectedSecondarySize} value={theme.selectedSecondaryFontSizePx} minimum={10} maximum={18} disabled={disabled} onChange={(value) => update((current) => ({ ...current, selectedSecondaryFontSizePx: value }))} />
      <RangeControl label={t.iconSize} value={theme.iconSizePx} minimum={16} maximum={64} disabled={disabled} onChange={(value) => update((current) => ({ ...current, iconSizePx: value }))} />
      <label className="appearance-control"><span className="appearance-control-copy"><strong>{t.maxResults}</strong></span><select value={theme.maxResults} disabled={disabled} onChange={(event) => update((current) => ({ ...current, maxResults: Number(event.target.value) as LauncherCustomThemeConfig["maxResults"] }))}>{([6, 8, 10, 12] as const).map((value) => <option key={value} value={value}>{value}</option>)}</select></label>
      <ToggleControl label={t.showSourceBadge} checked={theme.showSourceBadge} disabled={disabled} onChange={(value) => update((current) => ({ ...current, showSourceBadge: value }))} />
    </Section>
    <BackgroundControls theme={theme} disabled={disabled} onWallpaper={onWallpaper} onRemoveWallpaper={onRemoveWallpaper} onChange={(updater) => update((current) => ({ ...current, ...updater(current) }))} />
  </>;
}

function SettingsControls({ theme, disabled, update, onWallpaper, onRemoveWallpaper }: { theme: SettingsCustomThemeConfig; disabled: boolean; update: (update: (theme: SettingsCustomThemeConfig) => SettingsCustomThemeConfig) => void; onWallpaper: (file: File) => void; onRemoveWallpaper: () => void }) {
  return <>
    <Section title={t.settingsStructure} hint={t.settingsStructureHint} open>
      <ColorControl label={t.accentColor} value={theme.accentColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, accentColor: value }))} />
      <ColorControl label={t.windowBackground} value={theme.windowBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, windowBackground: value }))} />
      <ColorControl label={t.titlebarBackground} value={theme.titlebarBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, titlebarBackground: value }))} />
      <ColorControl label={t.sidebarBackground} value={theme.sidebarBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, sidebarBackground: value }))} />
      <ColorControl label={t.contentBackground} value={theme.contentBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, contentBackground: value }))} />
      <ColorControl label={t.cardBackground} value={theme.cardBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, cardBackground: value }))} />
      <ColorControl label={t.borderColor} value={theme.borderColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, borderColor: value }))} />
    </Section>
    <Section title={t.settingsText} hint={t.settingsTextHint}>
      <ColorControl label={t.primaryText} value={theme.primaryTextColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, primaryTextColor: value }))} />
      <ColorControl label={t.secondaryText} value={theme.secondaryTextColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, secondaryTextColor: value }))} />
      <ColorControl label={t.navText} value={theme.navTextColor} disabled={disabled} onChange={(value) => update((current) => ({ ...current, navTextColor: value }))} />
      <ColorControl label={t.selectedNav} value={theme.selectedNavBackground} disabled={disabled} onChange={(value) => update((current) => ({ ...current, selectedNavBackground: value }))} />
      <RangeControl label={t.baseFontSize} value={theme.baseFontSizePx} minimum={12} maximum={20} disabled={disabled} onChange={(value) => update((current) => ({ ...current, baseFontSizePx: value }))} />
      <RangeControl label={t.windowRadius} value={theme.radiusPx} minimum={0} maximum={28} disabled={disabled} onChange={(value) => update((current) => ({ ...current, radiusPx: value }))} />
    </Section>
    <BackgroundControls theme={theme} disabled={disabled} onWallpaper={onWallpaper} onRemoveWallpaper={onRemoveWallpaper} onChange={(updater) => update((current) => ({ ...current, ...updater(current) }))} />
  </>;
}

function LauncherPreview({ theme }: { theme: LauncherCustomThemeConfig }) {
  const material = previewMaterial(theme);
  const style = {
    "--preview-accent": theme.accentColor, "--preview-window": theme.windowBackground, "--preview-window-border": theme.windowBorder, "--preview-window-border-width": `${theme.windowBorderWidthPx}px`, "--preview-window-width": `${(theme.windowWidthPx / 900) * 100}%`, "--preview-radius": `${theme.windowRadiusPx}px`, "--preview-window-opacity": `${material.opacity}%`, "--preview-blur": `${material.blurPx}px`, "--preview-shadow-opacity": String(theme.shadowPercent / 100), "--preview-wallpaper": previewWallpaper(theme.wallpaperDataUrl), "--preview-wallpaper-opacity": String(theme.wallpaperOpacity / 100), "--preview-search": theme.searchBackground, "--preview-search-border": theme.searchBorder, "--preview-search-border-width": `${theme.searchBorderWidthPx}px`, "--preview-search-border-style": theme.searchBorderStyle, "--preview-search-width": `${Math.min(100, (theme.searchWidthPx / theme.windowWidthPx) * 100)}%`, "--preview-search-text": theme.searchTextColor, "--preview-search-size": `${theme.searchFontSizePx}px`, "--preview-row": theme.normalRowBackground, "--preview-row-primary": theme.normalPrimaryColor, "--preview-row-secondary": theme.normalSecondaryColor, "--preview-row-primary-size": `${theme.normalPrimaryFontSizePx}px`, "--preview-row-secondary-size": `${theme.normalSecondaryFontSizePx}px`, "--preview-row-height": `${theme.normalRowHeightPx}px`, "--preview-selected": theme.selectedRowBackground, "--preview-selected-primary": theme.selectedPrimaryColor, "--preview-selected-secondary": theme.selectedSecondaryColor, "--preview-selected-primary-size": `${theme.selectedPrimaryFontSizePx}px`, "--preview-selected-secondary-size": `${theme.selectedSecondaryFontSizePx}px`, "--preview-icon-size": `${theme.iconSizePx}px`,
  } as CSSProperties;
  const sampleRows = [
    { kind: "wechat", title: t.previewAppTitle, path: t.previewAppPath, badge: t.previewAppBadge, selected: true },
    { kind: "folder", title: t.previewFolderTitle, path: t.previewFolderPath, badge: t.previewFolderBadge },
    { kind: "file", title: t.previewFileTitle, path: t.previewFilePath, badge: t.previewFileBadge },
  ];
  const rows = Array.from({ length: theme.maxResults }, (_, index) => ({ ...sampleRows[index % sampleRows.length], selected: index === 0 }));
  return <div className="appearance-launcher-preview" style={style}>
    <div className="appearance-live-search">{theme.showSearchIcon && <span aria-hidden="true">⌕</span>}<strong>{t.previewQuery}</strong>{theme.showLogo && <i aria-hidden="true">◇</i>}</div>
    <div className="appearance-live-results">{rows.map((row, index) => <div key={`${row.kind}-${index}`} className={`appearance-live-row ${row.selected ? "selected" : ""}`}><PreviewIcon kind={row.kind} /><span><strong>{row.title}</strong><small>{row.path}</small></span>{theme.showSourceBadge && <em>{row.badge}</em>}</div>)}</div>
    <p className="appearance-native-note">{t.previewLauncherDimensions.replace("{width}", String(theme.windowWidthPx)).replace("{count}", String(theme.maxResults))}<br />{t.nativeIconNote}</p>
  </div>;
}

function PreviewIcon({ kind }: { kind: string }) {
  if (kind === "wechat") return <span className="appearance-live-icon wechat" aria-hidden="true"><svg viewBox="0 0 48 48"><path d="M8 23c0-8 7-14 15-14s15 6 15 14-7 14-15 14c-2 0-4 0-6-1l-6 4 2-6c-3-3-5-7-5-11Z" fill="#20bf63"/><circle cx="18" cy="21" r="2" fill="white"/><circle cx="27" cy="21" r="2" fill="white"/><path d="M25 31c5 0 10-4 10-9 0-5-5-9-10-9" fill="none" stroke="white" strokeWidth="2" opacity=".9"/></svg></span>;
  if (kind === "folder") return <span className="appearance-live-icon folder" aria-hidden="true">▰</span>;
  return <span className="appearance-live-icon file" aria-hidden="true">▤</span>;
}

function SettingsPreview({ theme }: { theme: SettingsCustomThemeConfig }) {
  const material = previewMaterial(theme);
  const style = { "--preview-settings-accent": theme.accentColor, "--preview-settings-window": theme.windowBackground, "--preview-settings-titlebar": theme.titlebarBackground, "--preview-settings-sidebar": theme.sidebarBackground, "--preview-settings-content": theme.contentBackground, "--preview-settings-card": theme.cardBackground, "--preview-settings-border": theme.borderColor, "--preview-settings-primary": theme.primaryTextColor, "--preview-settings-secondary": theme.secondaryTextColor, "--preview-settings-nav": theme.navTextColor, "--preview-settings-selected": theme.selectedNavBackground, "--preview-settings-font-size": `${theme.baseFontSizePx}px`, "--preview-settings-radius": `${theme.radiusPx}px`, "--preview-window-opacity": `${material.opacity}%`, "--preview-blur": `${material.blurPx}px`, "--preview-shadow-opacity": String(theme.shadowPercent / 100), "--preview-wallpaper": previewWallpaper(theme.wallpaperDataUrl), "--preview-wallpaper-opacity": String(theme.wallpaperOpacity / 100) } as CSSProperties;
  return <div className="appearance-settings-preview" style={style}><header><strong>{t.previewSettingsTitle}</strong><span>×</span></header><div><aside><span>{t.previewGeneral}</span><span>{t.previewSearch}</span><span>{t.previewCommands}</span><strong>{t.previewAppearance}</strong></aside><main><h3>{t.previewPageTitle}</h3><p>{t.previewPageCopy}</p><section><strong>{t.previewCardTitle}</strong><p>{t.previewCardCopy}</p><i /><i /><i /></section></main></div></div>;
}

export default function AppearanceEditor({ launcherTheme, settingsTheme, onChange, saveSettingsManually, readOnly, saving = false }: AppearanceEditorProps) {
  const [scope, setScope] = useState<ThemeScope>("launcher");
  const [workingLauncher, setWorkingLauncher] = useState(() => cloneLauncherScope(launcherTheme));
  const [workingSettings, setWorkingSettings] = useState(() => cloneSettingsScope(settingsTheme));
  const [editingLauncherTheme, setEditingLauncherTheme] = useState<ThemeSelection>(launcherTheme.theme);
  const [editingSettingsTheme, setEditingSettingsTheme] = useState<ThemeSelection>(settingsTheme.theme);
  const [notice, setNotice] = useState("");
  const [importing, setImporting] = useState(false);
  const [committing, setCommitting] = useState(false);
  const importRef = useRef<HTMLInputElement>(null);
  const importRequestRef = useRef(0);
  const tabRefs = useRef<Record<ThemeScope, HTMLButtonElement | null>>({ launcher: null, settings: null });
  const wallpaperRequestsRef = useRef(new Map<string, number>());
  const launcherLibrarySignature = useMemo(() => JSON.stringify(launcherTheme.customThemes), [launcherTheme.customThemes]);
  const settingsLibrarySignature = useMemo(() => JSON.stringify(settingsTheme.customThemes), [settingsTheme.customThemes]);
  const disabled = readOnly || saving || importing || committing;

  useEffect(() => {
    importRequestRef.current += 1;
    setImporting(false);
    for (const [key, version] of wallpaperRequestsRef.current) {
      if (key.startsWith("launcher:")) wallpaperRequestsRef.current.set(key, version + 1);
    }
    setWorkingLauncher((current) => ({
      ...current,
      customThemes: launcherTheme.customThemes.map((theme) => ({ ...theme, ...cloneBackground(theme) })),
    }));
    setEditingLauncherTheme((current) => {
      const id = customId(current);
      return id && !launcherTheme.customThemes.some((theme) => sameId(theme.id, id))
        ? launcherTheme.theme
        : current;
    });
  }, [launcherLibrarySignature]);
  useEffect(() => {
    importRequestRef.current += 1;
    setImporting(false);
    for (const [key, version] of wallpaperRequestsRef.current) {
      if (key.startsWith("settings:")) wallpaperRequestsRef.current.set(key, version + 1);
    }
    setWorkingSettings((current) => ({
      ...current,
      customThemes: settingsTheme.customThemes.map((theme) => ({ ...theme, ...cloneBackground(theme) })),
    }));
    setEditingSettingsTheme((current) => {
      const id = customId(current);
      return id && !settingsTheme.customThemes.some((theme) => sameId(theme.id, id))
        ? settingsTheme.theme
        : current;
    });
  }, [settingsLibrarySignature]);

  useEffect(() => {
    setWorkingLauncher((current) => ({ ...current, theme: launcherTheme.theme, accentColor: launcherTheme.accentColor }));
  }, [launcherTheme.theme, launcherTheme.accentColor]);

  useEffect(() => {
    setWorkingSettings((current) => ({ ...current, theme: settingsTheme.theme, accentColor: settingsTheme.accentColor }));
  }, [settingsTheme.theme, settingsTheme.accentColor]);

  const selectedLauncherId = customId(editingLauncherTheme);
  const selectedLauncher = selectedLauncherId ? workingLauncher.customThemes.find((theme) => sameId(theme.id, selectedLauncherId)) ?? null : null;
  const committedLauncher = selectedLauncherId ? launcherTheme.customThemes.find((theme) => sameId(theme.id, selectedLauncherId)) ?? null : null;
  const launcherEditingHasChanges = selectedLauncher !== null
    && (committedLauncher === null || JSON.stringify(selectedLauncher) !== JSON.stringify(committedLauncher));
  const launcherPreview = resolveLauncherTheme({ ...workingLauncher, theme: editingLauncherTheme });
  const selectedSettingsId = customId(editingSettingsTheme);
  const selectedSettings = selectedSettingsId ? workingSettings.customThemes.find((theme) => sameId(theme.id, selectedSettingsId)) ?? null : null;
  const committedSettings = selectedSettingsId ? settingsTheme.customThemes.find((theme) => sameId(theme.id, selectedSettingsId)) ?? null : null;
  const settingsEditingHasChanges = selectedSettings !== null
    && (committedSettings === null || JSON.stringify(selectedSettings) !== JSON.stringify(committedSettings));
  const settingsPreview = resolveSettingsTheme({ ...workingSettings, theme: editingSettingsTheme });
  const checks = scope === "launcher" ? launcherChecks(launcherPreview) : settingsChecks(settingsPreview);
  const checksPass = checks.every((check) => check.ratio >= check.minimum);
  const sceneReview = needsSceneContrastReview(scope === "launcher" ? launcherPreview : settingsPreview);
  const contrastVerified = checksPass && !sceneReview;
  const selectedCustom = scope === "launcher" ? selectedLauncher : selectedSettings;
  const editingHasChanges = scope === "launcher" ? launcherEditingHasChanges : settingsEditingHasChanges;

  const commitThemes = async (themes: { launcherTheme: LauncherThemeConfig; settingsTheme: SettingsThemeConfig }) => {
    setCommitting(true);
    try {
      return await onChange(themes);
    } catch {
      return false;
    } finally {
      setCommitting(false);
    }
  };

  const advanceWallpaperRequest = (target: ThemeTarget) => {
    const key = `${target.scope}:${target.themeId.toLowerCase()}`;
    const version = (wallpaperRequestsRef.current.get(key) ?? 0) + 1;
    wallpaperRequestsRef.current.set(key, version);
    return { key, version };
  };

  const currentWallpaperTarget = (): ThemeTarget | null => {
    const themeId = customId(scope === "launcher" ? editingLauncherTheme : editingSettingsTheme);
    return themeId ? { scope, themeId } : null;
  };

  const invalidateCurrentWallpaperRequest = () => {
    const target = currentWallpaperTarget();
    if (target) advanceWallpaperRequest(target);
  };

  const updateLauncher = (updater: (theme: LauncherCustomThemeConfig) => LauncherCustomThemeConfig) => {
    if (!selectedLauncher || disabled) return;
    setWorkingLauncher((current) => ({ ...current, customThemes: current.customThemes.map((theme) => sameId(theme.id, customId(editingLauncherTheme)) ? updater(theme) : theme) }));
  };
  const updateSettings = (updater: (theme: SettingsCustomThemeConfig) => SettingsCustomThemeConfig) => {
    if (!selectedSettings || disabled) return;
    setWorkingSettings((current) => ({ ...current, customThemes: current.customThemes.map((theme) => sameId(theme.id, customId(editingSettingsTheme)) ? updater(theme) : theme) }));
  };
  const chooseScope = (next: ThemeScope) => {
    if (disabled) return;
    invalidateCurrentWallpaperRequest();
    setScope(next);
    setNotice("");
  };
  const onScopeKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight" && event.key !== "Home" && event.key !== "End") return;
    event.preventDefault();
    const next = event.key === "ArrowLeft" || event.key === "Home" ? "launcher" : "settings";
    chooseScope(next); tabRefs.current[next]?.focus();
  };
  const chooseBuiltin = (id: typeof builtinThemeIds[number]) => {
    if (disabled) return;
    if (scope === "launcher") setEditingLauncherTheme(id);
    else setEditingSettingsTheme(id);
    setNotice("");
  };
  const chooseCustom = (id: string) => {
    if (disabled) return;
    if (scope === "launcher") setEditingLauncherTheme(`custom:${id}`);
    else setEditingSettingsTheme(`custom:${id}`);
    setNotice("");
  };
  const chooseEditingTheme = (selection: ThemeSelection) => {
    const current = scope === "launcher" ? editingLauncherTheme : editingSettingsTheme;
    if (selection === current) return;
    if (editingHasChanges) {
      setNotice(t.saveBeforeSwitchingTheme);
      return;
    }
    invalidateCurrentWallpaperRequest();
    const id = customId(selection);
    if (id) chooseCustom(id);
    else chooseBuiltin(selection as typeof builtinThemeIds[number]);
  };

  const chooseActiveTheme = async (selection: ThemeSelection) => {
    if (disabled) return;
    const commitScope = scope;
    let themes: { launcherTheme: LauncherThemeConfig; settingsTheme: SettingsThemeConfig };
    if (commitScope === "launcher") {
      const next = {
        ...cloneLauncherScope(launcherTheme),
        theme: selection,
        accentColor: customId(selection) ? launcherTheme.accentColor : createLauncherTheme(selection).accentColor,
      };
      themes = { launcherTheme: next, settingsTheme: cloneSettingsScope(settingsTheme) };
    } else {
      const next = {
        ...cloneSettingsScope(settingsTheme),
        theme: selection,
        accentColor: customId(selection) ? settingsTheme.accentColor : createSettingsTheme(selection).accentColor,
      };
      themes = { launcherTheme: cloneLauncherScope(launcherTheme), settingsTheme: next };
    }
    const saved = await commitThemes(themes);
    if (!saved) {
      setNotice(t.themeSaveFailed);
      return;
    }
    if (commitScope === "launcher") {
      setWorkingLauncher((current) => ({ ...current, theme: themes.launcherTheme.theme, accentColor: themes.launcherTheme.accentColor }));
    } else {
      setWorkingSettings((current) => ({ ...current, theme: themes.settingsTheme.theme, accentColor: themes.settingsTheme.accentColor }));
    }
    setNotice(saveSettingsManually ? t.activeThemeAddedToDraft : t.activeThemeApplied);
  };
  const createTheme = () => {
    if (disabled) return;
    if (editingHasChanges) return setNotice(t.saveBeforeSwitchingTheme);
    if (scope === "launcher") {
      if (workingLauncher.customThemes.length >= MAX_CUSTOM_THEMES) return setNotice(t.customThemeLimit.replace("{max}", String(MAX_CUSTOM_THEMES)));
      const seed = createLauncherTheme();
      const source = selectedLauncher;
      const builtin = (customId(editingLauncherTheme) ? "midnight" : editingLauncherTheme) as keyof typeof builtinLabels;
      const next = source ? { ...source, id: seed.id, name: t.copyName.replace("{name}", source.name), ...cloneBackground(source) } : { ...createLauncherTheme(builtin), name: t.copyName.replace("{name}", builtinLabels[builtin] ?? t.midnight) };
      setWorkingLauncher((current) => ({ ...current, customThemes: [...current.customThemes, next] }));
      setEditingLauncherTheme(`custom:${next.id}`);
    } else {
      if (workingSettings.customThemes.length >= MAX_CUSTOM_THEMES) return setNotice(t.customThemeLimit.replace("{max}", String(MAX_CUSTOM_THEMES)));
      const seed = createSettingsTheme();
      const source = selectedSettings;
      const builtin = (customId(editingSettingsTheme) ? "midnight" : editingSettingsTheme) as keyof typeof builtinLabels;
      const next = source ? { ...source, id: seed.id, name: t.copyName.replace("{name}", source.name), ...cloneBackground(source) } : { ...createSettingsTheme(builtin), name: t.copyName.replace("{name}", builtinLabels[builtin] ?? t.midnight) };
      setWorkingSettings((current) => ({ ...current, customThemes: [...current.customThemes, next] }));
      setEditingSettingsTheme(`custom:${next.id}`);
    }
    setNotice(t.createdTheme);
  };
  const deleteTheme = async () => {
    if (!selectedCustom || disabled) return;
    invalidateCurrentWallpaperRequest();
    const commitScope = scope;
    if (commitScope === "launcher") {
      const deletedId = customId(editingLauncherTheme);
      if (!deletedId) return;
      if (!committedLauncher) {
        setWorkingLauncher((current) => ({ ...current, customThemes: current.customThemes.filter((theme) => !sameId(theme.id, deletedId)) }));
        setEditingLauncherTheme("midnight");
        setNotice(t.deletedUnsavedTheme);
        return;
      }
      const next = {
        ...cloneLauncherScope(launcherTheme),
        ...(customId(launcherTheme.theme) === deletedId ? { theme: "midnight" as const, accentColor: createLauncherTheme("midnight").accentColor } : {}),
        customThemes: launcherTheme.customThemes.filter((theme) => !sameId(theme.id, deletedId)),
      };
      const saved = await commitThemes({ launcherTheme: next, settingsTheme: cloneSettingsScope(settingsTheme) });
      if (!saved) return setNotice(t.themeSaveFailed);
      setWorkingLauncher(cloneLauncherScope(next));
      setEditingLauncherTheme("midnight");
    } else {
      const deletedId = customId(editingSettingsTheme);
      if (!deletedId) return;
      if (!committedSettings) {
        setWorkingSettings((current) => ({ ...current, customThemes: current.customThemes.filter((theme) => !sameId(theme.id, deletedId)) }));
        setEditingSettingsTheme("midnight");
        setNotice(t.deletedUnsavedTheme);
        return;
      }
      const next = {
        ...cloneSettingsScope(settingsTheme),
        ...(customId(settingsTheme.theme) === deletedId ? { theme: "midnight" as const, accentColor: createSettingsTheme("midnight").accentColor } : {}),
        customThemes: settingsTheme.customThemes.filter((theme) => !sameId(theme.id, deletedId)),
      };
      const saved = await commitThemes({ launcherTheme: cloneLauncherScope(launcherTheme), settingsTheme: next });
      if (!saved) return setNotice(t.themeSaveFailed);
      setWorkingSettings(cloneSettingsScope(next));
      setEditingSettingsTheme("midnight");
    }
    setNotice(saveSettingsManually ? t.deletedThemeToDraft : t.deletedTheme);
  };
  const resetTheme = () => {
    if (!selectedCustom || disabled) return;
    invalidateCurrentWallpaperRequest();
    if (scope === "launcher") updateLauncher((theme) => ({ ...createLauncherTheme(), id: theme.id, name: theme.name }));
    else updateSettings((theme) => ({ ...createSettingsTheme(), id: theme.id, name: theme.name }));
    setNotice(t.resetTheme);
  };
  const importError = (targetScope: ThemeScope, value: unknown, error: unknown) => {
    const expectedSchema = targetScope === "launcher" ? "suo-launcher-theme-v1" : "suo-settings-theme-v1";
    const schema = value && typeof value === "object" && "schema" in value ? (value as { schema?: unknown }).schema : "";
    const version = value && typeof value === "object" && "version" in value ? (value as { version?: unknown }).version : undefined;
    const detail = error instanceof Error ? error.message.trim() : "";
    if (error instanceof SyntaxError) return t.invalidJson;
    if (schema === "suo-theme-v1") return t.legacySchema;
    if (schema === (targetScope === "launcher" ? "suo-settings-theme-v1" : "suo-launcher-theme-v1")) return t.wrongScope.replace("{scope}", targetScope === "launcher" ? t.settingsScope : t.launcherScope);
    if (schema === expectedSchema && version !== 1) {
      return typeof version === "number"
        ? t.unsupportedThemeVersion.replace("{version}", `v${version}`)
        : t.invalidThemeVersion;
    }
    if (schema === expectedSchema) {
      return detail
        ? t.invalidThemeFieldsWithReason.replace("{reason}", detail)
        : t.invalidThemeFields;
    }
    return t.invalidSchema.replace("{schema}", expectedSchema);
  };
  const importTheme = (file: File) => {
    if (disabled || importing) return;
    if (editingHasChanges) return setNotice(t.saveBeforeSwitchingTheme);
    if (file.size > MAX_THEME_BUNDLE_BYTES) return setNotice(t.bundleTooLarge);
    if ((scope === "launcher" ? workingLauncher : workingSettings).customThemes.length >= MAX_CUSTOM_THEMES) return setNotice(t.customThemeLimit.replace("{max}", String(MAX_CUSTOM_THEMES)));
    // FileReader completes asynchronously. Keep the destination fixed even if
    // the user switches scope while the file is being read.
    const importScope = scope;
    const importRequest = ++importRequestRef.current;
    setImporting(true);
    const reader = new FileReader();
    reader.onload = async () => {
      let value: unknown;
      try {
        if (importRequestRef.current !== importRequest) return;
        value = JSON.parse(String(reader.result));
        if (importScope === "launcher") {
          const bundle = parseLauncherThemeBundle(value);
          await validateWallpaperImageDataUrl(bundle.theme.wallpaperDataUrl);
          if (importRequestRef.current !== importRequest) return;
          const theme: LauncherCustomThemeConfig = { ...bundle.theme, id: createLauncherTheme().id, platformOverrides: { ...bundle.theme.platformOverrides } };
          setWorkingLauncher((current) => current.customThemes.length >= MAX_CUSTOM_THEMES ? current : { ...current, customThemes: [...current.customThemes, theme] });
          setEditingLauncherTheme(`custom:${theme.id}`);
          setNotice(t.importedTheme.replace("{name}", theme.name));
        } else {
          const bundle = parseSettingsThemeBundle(value);
          await validateWallpaperImageDataUrl(bundle.theme.wallpaperDataUrl);
          if (importRequestRef.current !== importRequest) return;
          const theme: SettingsCustomThemeConfig = { ...bundle.theme, id: createSettingsTheme().id, platformOverrides: { ...bundle.theme.platformOverrides } };
          setWorkingSettings((current) => current.customThemes.length >= MAX_CUSTOM_THEMES ? current : { ...current, customThemes: [...current.customThemes, theme] });
          setEditingSettingsTheme(`custom:${theme.id}`);
          setNotice(t.importedTheme.replace("{name}", theme.name));
        }
      } catch (error) {
        if (importRequestRef.current === importRequest) setNotice(t.importFailed.replace("{reason}", importError(importScope, value, error)));
      } finally {
        if (importRequestRef.current === importRequest) setImporting(false);
      }
    };
    reader.onerror = () => {
      if (importRequestRef.current !== importRequest) return;
      setImporting(false);
      setNotice(t.unableToReadBundle);
    };
    reader.readAsText(file, "utf-8");
  };
  const exportTheme = () => {
    try {
      const theme = scope === "launcher" ? launcherPreview : settingsPreview;
      const bundle = scope === "launcher" ? buildLauncherThemeBundle(theme as LauncherCustomThemeConfig) : buildSettingsThemeBundle(theme as SettingsCustomThemeConfig);
      const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob); const anchor = document.createElement("a");
      anchor.href = url; anchor.download = `${scope}-theme.json`; anchor.click(); window.setTimeout(() => URL.revokeObjectURL(url), 0);
      setNotice(t.exportedTheme);
    } catch {
      setNotice(t.invalidThemeDraft);
    }
  };
  const loadWallpaper = (file: File) => {
    if (disabled || !/^image\/(png|jpeg|webp)$/.test(file.type) || file.size > MAX_WALLPAPER_BYTES) return setNotice(t.wallpaperTooLarge);
    const target = currentWallpaperTarget();
    if (!target) return;
    const request = advanceWallpaperRequest(target);
    const reader = new FileReader();
    reader.onload = async () => {
      if (wallpaperRequestsRef.current.get(request.key) !== request.version) return;
      const dataUrl = String(reader.result);
      try {
        await validateWallpaperImageDataUrl(dataUrl);
      } catch {
        if (wallpaperRequestsRef.current.get(request.key) === request.version) setNotice(t.wallpaperInvalid);
        return;
      }
      if (wallpaperRequestsRef.current.get(request.key) !== request.version) return;
      if (target.scope === "launcher") {
        setWorkingLauncher((current) => {
          let updated = false;
          const customThemes = current.customThemes.map((theme) => {
            if (!sameId(theme.id, target.themeId)) return theme;
            updated = true;
            return { ...theme, wallpaperDataUrl: dataUrl };
          });
          return updated ? { ...current, customThemes } : current;
        });
      } else {
        setWorkingSettings((current) => {
          let updated = false;
          const customThemes = current.customThemes.map((theme) => {
            if (!sameId(theme.id, target.themeId)) return theme;
            updated = true;
            return { ...theme, wallpaperDataUrl: dataUrl };
          });
          return updated ? { ...current, customThemes } : current;
        });
      }
      setNotice(t.wallpaperLoaded);
    };
    reader.onerror = () => {
      if (wallpaperRequestsRef.current.get(request.key) === request.version) setNotice(t.wallpaperUnableToRead);
    };
    reader.readAsDataURL(file);
  };
  const removeWallpaper = () => {
    if (disabled) return;
    invalidateCurrentWallpaperRequest();
    if (scope === "launcher") updateLauncher((theme) => ({ ...theme, wallpaperDataUrl: "" }));
    else updateSettings((theme) => ({ ...theme, wallpaperDataUrl: "" }));
  };
  const applyDraft = async () => {
    if (disabled || !editingHasChanges) return;
    const commitScope = scope;
    const editingSelection = commitScope === "launcher" ? editingLauncherTheme : editingSettingsTheme;
    const activeSelection = commitScope === "launcher" ? launcherTheme.theme : settingsTheme.theme;
    try {
      if (commitScope === "launcher") buildLauncherThemeBundle(launcherPreview);
      else buildSettingsThemeBundle(settingsPreview);
    } catch {
      setNotice(t.invalidThemeDraft);
      return;
    }
    let themes: { launcherTheme: LauncherThemeConfig; settingsTheme: SettingsThemeConfig };
    if (commitScope === "launcher" && selectedLauncherId && selectedLauncher) {
      const next = cloneLauncherScope(launcherTheme);
      const found = next.customThemes.some((theme) => sameId(theme.id, selectedLauncherId));
      next.customThemes = found
        ? next.customThemes.map((theme) => sameId(theme.id, selectedLauncherId) ? { ...selectedLauncher, ...cloneBackground(selectedLauncher) } : theme)
        : [...next.customThemes, { ...selectedLauncher, ...cloneBackground(selectedLauncher) }];
      themes = { launcherTheme: next, settingsTheme: cloneSettingsScope(settingsTheme) };
    } else if (commitScope === "settings" && selectedSettingsId && selectedSettings) {
      const next = cloneSettingsScope(settingsTheme);
      const found = next.customThemes.some((theme) => sameId(theme.id, selectedSettingsId));
      next.customThemes = found
        ? next.customThemes.map((theme) => sameId(theme.id, selectedSettingsId) ? { ...selectedSettings, ...cloneBackground(selectedSettings) } : theme)
        : [...next.customThemes, { ...selectedSettings, ...cloneBackground(selectedSettings) }];
      themes = { launcherTheme: cloneLauncherScope(launcherTheme), settingsTheme: next };
    } else {
      setNotice(t.invalidThemeDraft);
      return;
    }
    const saved = await commitThemes(themes);
    if (!saved) {
      setNotice(t.themeSaveFailed);
      return;
    }
    if (commitScope === "launcher") setWorkingLauncher(cloneLauncherScope(themes.launcherTheme));
    else setWorkingSettings(cloneSettingsScope(themes.settingsTheme));
    const savedNotice = saveSettingsManually
      ? t.savedThemeToDraft
      : editingSelection === activeSelection ? t.savedThemeAndApplied : t.savedThemeWithoutSwitching;
    setNotice((!checksPass || sceneReview) ? `${savedNotice} ${t.savedWithReadabilityWarning}` : savedNotice);
  };
  const selectedBuiltin = scope === "launcher" ? (selectedLauncher ? null : editingLauncherTheme) : (selectedSettings ? null : editingSettingsTheme);
  const activeSelection = scope === "launcher" ? launcherTheme.theme : settingsTheme.theme;
  const editingSelection = scope === "launcher" ? editingLauncherTheme : editingSettingsTheme;
  const activeCustomThemes = scope === "launcher" ? launcherTheme.customThemes : settingsTheme.customThemes;
  const editingCustomThemes = scope === "launcher" ? workingLauncher.customThemes : workingSettings.customThemes;
  const editingName = themeName(editingSelection, editingCustomThemes);
  const editingIsActive = editingSelection === activeSelection;
  const hasThemeChanges = editingHasChanges;
  const canApplySavedInactiveTheme = !saveSettingsManually && !editingIsActive && !hasThemeChanges;
  const primaryActionLabel = canApplySavedInactiveTheme
    ? t.applyTheme
    : !saveSettingsManually && editingIsActive ? t.saveAndApply : t.saveTheme;
  const primaryActionHint = hasThemeChanges
    ? (saveSettingsManually ? t.saveThemeManualHint : editingIsActive ? t.saveActiveThemeHint : t.saveInactiveThemeHint)
    : canApplySavedInactiveTheme ? t.applySavedThemeHint : t.noThemeChanges;
  const runPrimaryAction = () => {
    if (canApplySavedInactiveTheme) {
      void chooseActiveTheme(editingSelection);
      return;
    }
    void applyDraft();
  };
  const editingSwatchStyle = selectedCustom
    ? { background: `linear-gradient(135deg, ${selectedCustom.windowBackground}, ${scope === "launcher" ? (selectedCustom as LauncherCustomThemeConfig).selectedRowBackground : (selectedCustom as SettingsCustomThemeConfig).selectedNavBackground})` }
    : undefined;
  return <section className="appearance-editor" aria-label={t.ariaLabel}>
    <header className="appearance-editor-heading"><div><h2>{t.title}</h2><p>{t.description}</p></div></header>
    <section className={`appearance-scope-zone ${scope}`}>
      <div>
        <span className="appearance-step-label">{t.chooseScopeStep}</span>
        <nav className="appearance-scope-tabs" role="tablist" aria-label={t.scopeTabs} onKeyDown={onScopeKeyDown}>
          {(["launcher", "settings"] as const).map((id) => <button ref={(node) => { tabRefs.current[id] = node; }} key={id} id={`appearance-${id}-tab`} type="button" role="tab" aria-selected={scope === id} aria-controls={`appearance-${id}-panel`} tabIndex={scope === id ? 0 : -1} className={`${id} ${scope === id ? "active" : ""}`} onClick={() => chooseScope(id)}><span aria-hidden="true">{id === "launcher" ? "⌕" : "⚙"}</span><span><strong>{id === "launcher" ? t.launcherScope : t.settingsScope}</strong><small>{id === "launcher" ? t.launcherScopeHint : t.settingsScopeHint}</small></span><em>{scope === id ? t.designing : t.switchScope}</em></button>)}
        </nav>
      </div>
      <label className="appearance-active-theme">
        <span><i className="appearance-active-dot" aria-hidden="true" /><strong>{scope === "launcher" ? t.launcherActiveTheme : t.settingsActiveTheme}</strong><small>{t.activeThemeHint}</small></span>
        <select value={activeSelection} disabled={disabled} onChange={(event) => void chooseActiveTheme(event.target.value as ThemeSelection)}>
          {builtinThemeIds.map((id) => <option key={id} value={id}>{builtinLabels[id]} · {t.builtin}</option>)}
          {activeCustomThemes.map((theme) => <option key={theme.id} value={`custom:${theme.id}`}>{theme.name} · {t.custom}</option>)}
        </select>
      </label>
      <p className="appearance-separation-note"><strong>{t.scopeRule}</strong>{t.separateNotice}</p>
    </section>
    <div id={`appearance-${scope}-panel`} role="tabpanel" aria-labelledby={`appearance-${scope}-tab`} className="appearance-workbench">
      <section className="appearance-edit-panel">
        <header className="appearance-library-heading"><div><span className="appearance-step-label">{t.chooseEditingStep}</span><h3>{scope === "launcher" ? t.launcherLibrary : t.settingsLibrary}</h3><p>{t.editingThemeHint}</p></div><span>{t.isolated}</span></header>
        <div className="appearance-toolbar"><input ref={importRef} className="appearance-hidden-input" type="file" accept="application/json,.json" disabled={disabled} onChange={(event) => { const file = event.target.files?.[0]; if (file) importTheme(file); event.currentTarget.value = ""; }} /><button type="button" disabled={disabled} onClick={() => importRef.current?.click()}>{t.importTheme}</button><button type="button" disabled={saving || importing} onClick={exportTheme}>{t.exportTheme}</button><button type="button" className="primary" disabled={disabled} onClick={createTheme}>{t.createTheme}</button></div>
        <div className="appearance-theme-picker">
          <span className={`appearance-theme-swatch ${selectedBuiltin ? `appearance-theme-swatch-${selectedBuiltin}` : ""}`} style={editingSwatchStyle} aria-hidden="true" />
          <label><span><strong>{t.chooseEditingTheme}</strong><small>{t.editingOnlyHint}</small></span><select value={editingSelection} disabled={disabled} onChange={(event) => chooseEditingTheme(event.target.value as ThemeSelection)}>{builtinThemeIds.map((id) => <option key={id} value={id}>{builtinLabels[id]} · {t.builtin}{id === activeSelection ? ` · ${t.inUse}` : ""}</option>)}{editingCustomThemes.map((theme) => { const selection = `custom:${theme.id}` as ThemeSelection; return <option key={theme.id} value={selection}>{theme.name} · {t.custom}{selection === activeSelection ? ` · ${t.inUse}` : ""}</option>; })}</select></label>
          <span className="appearance-theme-status"><em className="editing">✎ {t.editing}</em>{editingIsActive && <em className="in-use"><i className="appearance-active-dot" aria-hidden="true" />{t.inUse}</em>}<small>{selectedCustom ? t.custom : t.builtin} · {scope === "launcher" ? t.launcherTag : t.settingsTag}</small></span>
        </div>
        <header className="appearance-editing-banner"><span aria-hidden="true">✎</span><div><small>{t.editingStep}</small><strong>{t.editingThemeName.replace("{name}", editingName)}</strong><p>{editingIsActive ? t.editingActiveThemeHint : t.editingInactiveThemeHint}</p></div></header>
        {selectedCustom ? <div className="appearance-custom-editor"><header><label><span>{t.themeName}</span><input value={selectedCustom.name} maxLength={40} disabled={disabled} onChange={(event) => scope === "launcher" ? updateLauncher((theme) => ({ ...theme, name: event.target.value })) : updateSettings((theme) => ({ ...theme, name: event.target.value }))} /><small>{t.themeNameHint}</small></label><div><button type="button" disabled={disabled} onClick={resetTheme}>{t.restoreDefault}</button><button type="button" className="danger" disabled={disabled} onClick={deleteTheme}>{t.delete}</button></div></header>{scope === "launcher" ? <LauncherControls theme={selectedLauncher!} disabled={disabled} update={updateLauncher} onWallpaper={loadWallpaper} onRemoveWallpaper={removeWallpaper} /> : <SettingsControls theme={selectedSettings!} disabled={disabled} update={updateSettings} onWallpaper={loadWallpaper} onRemoveWallpaper={removeWallpaper} />}</div> : <div className="appearance-builtin-empty"><p>{t.builtinReadOnly.replace("{name}", builtinLabels[selectedBuiltin as keyof typeof builtinLabels] ?? t.midnight)}</p><button type="button" className="primary" disabled={disabled} onClick={createTheme}>{t.createFromBuiltin}</button></div>}
        <footer className="appearance-edit-footer"><span aria-live="polite">{notice || primaryActionHint}</span><button type="button" className={`primary ${canApplySavedInactiveTheme ? "apply" : ""}`} title={!hasThemeChanges && !canApplySavedInactiveTheme ? t.noThemeChanges : undefined} disabled={disabled || (!hasThemeChanges && !canApplySavedInactiveTheme)} onClick={runPrimaryAction}>{primaryActionLabel}</button></footer>
      </section>
      <aside className="appearance-preview-panel" aria-label={t.previewAriaLabel}><header><div><strong>{scope === "launcher" ? t.previewLauncher : t.previewSettings}：{editingName}</strong><small>{editingIsActive ? t.previewEditingActive : t.previewEditingInactive}</small></div><span className={contrastVerified ? "ok" : "warning"}>{contrastVerified ? t.contrastPass : checksPass ? t.contrastSceneReview : t.contrastAdjust}</span></header><div className="appearance-preview-canvas">{scope === "launcher" ? <LauncherPreview theme={launcherPreview} /> : <SettingsPreview theme={settingsPreview} />}</div><ContrastAudit checks={checks} sceneReview={sceneReview} /></aside>
    </div>
  </section>;
}
