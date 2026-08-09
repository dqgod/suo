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
} from "./config";
import { zhCN } from "./i18n/zh-CN";
import "./AppearanceEditor.css";

type ThemeScope = "launcher" | "settings";
type AppearanceEditorProps = {
  launcherTheme: LauncherThemeConfig;
  settingsTheme: SettingsThemeConfig;
  onChange: (themes: { launcherTheme: LauncherThemeConfig; settingsTheme: SettingsThemeConfig }) => void;
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

export default function AppearanceEditor({ launcherTheme, settingsTheme, onChange, readOnly, saving = false }: AppearanceEditorProps) {
  const [scope, setScope] = useState<ThemeScope>("launcher");
  const [workingLauncher, setWorkingLauncher] = useState(() => cloneLauncherScope(launcherTheme));
  const [workingSettings, setWorkingSettings] = useState(() => cloneSettingsScope(settingsTheme));
  const [notice, setNotice] = useState("");
  const [warnedScope, setWarnedScope] = useState<ThemeScope | null>(null);
  const [importing, setImporting] = useState(false);
  const importRef = useRef<HTMLInputElement>(null);
  const importRequestRef = useRef(0);
  const tabRefs = useRef<Record<ThemeScope, HTMLButtonElement | null>>({ launcher: null, settings: null });
  const wallpaperRequestsRef = useRef(new Map<string, number>());
  const launcherSignature = useMemo(() => JSON.stringify(launcherTheme), [launcherTheme]);
  const settingsSignature = useMemo(() => JSON.stringify(settingsTheme), [settingsTheme]);
  const disabled = readOnly || saving || importing;

  useEffect(() => {
    importRequestRef.current += 1;
    setImporting(false);
    for (const [key, version] of wallpaperRequestsRef.current) {
      if (key.startsWith("launcher:")) wallpaperRequestsRef.current.set(key, version + 1);
    }
    setWorkingLauncher(cloneLauncherScope(launcherTheme));
    setWarnedScope(null);
  }, [launcherSignature]);
  useEffect(() => {
    importRequestRef.current += 1;
    setImporting(false);
    for (const [key, version] of wallpaperRequestsRef.current) {
      if (key.startsWith("settings:")) wallpaperRequestsRef.current.set(key, version + 1);
    }
    setWorkingSettings(cloneSettingsScope(settingsTheme));
    setWarnedScope(null);
  }, [settingsSignature]);

  const selectedLauncherId = customId(workingLauncher.theme);
  const selectedLauncher = selectedLauncherId ? workingLauncher.customThemes.find((theme) => sameId(theme.id, selectedLauncherId)) ?? null : null;
  const launcherPreview = resolveLauncherTheme(workingLauncher);
  const selectedSettingsId = customId(workingSettings.theme);
  const selectedSettings = selectedSettingsId ? workingSettings.customThemes.find((theme) => sameId(theme.id, selectedSettingsId)) ?? null : null;
  const settingsPreview = resolveSettingsTheme(workingSettings);
  const checks = scope === "launcher" ? launcherChecks(launcherPreview) : settingsChecks(settingsPreview);
  const checksPass = checks.every((check) => check.ratio >= check.minimum);
  const sceneReview = needsSceneContrastReview(scope === "launcher" ? launcherPreview : settingsPreview);
  const contrastVerified = checksPass && !sceneReview;
  const selectedCustom = scope === "launcher" ? selectedLauncher : selectedSettings;

  const advanceWallpaperRequest = (target: ThemeTarget) => {
    const key = `${target.scope}:${target.themeId.toLowerCase()}`;
    const version = (wallpaperRequestsRef.current.get(key) ?? 0) + 1;
    wallpaperRequestsRef.current.set(key, version);
    return { key, version };
  };

  const currentWallpaperTarget = (): ThemeTarget | null => {
    const themeId = customId(scope === "launcher" ? workingLauncher.theme : workingSettings.theme);
    return themeId ? { scope, themeId } : null;
  };

  const invalidateCurrentWallpaperRequest = () => {
    const target = currentWallpaperTarget();
    if (target) advanceWallpaperRequest(target);
  };

  const updateLauncher = (updater: (theme: LauncherCustomThemeConfig) => LauncherCustomThemeConfig) => {
    if (!selectedLauncher || disabled) return;
    setWarnedScope(null);
    setWorkingLauncher((current) => ({ ...current, customThemes: current.customThemes.map((theme) => sameId(theme.id, customId(current.theme)) ? updater(theme) : theme) }));
  };
  const updateSettings = (updater: (theme: SettingsCustomThemeConfig) => SettingsCustomThemeConfig) => {
    if (!selectedSettings || disabled) return;
    setWarnedScope(null);
    setWorkingSettings((current) => ({ ...current, customThemes: current.customThemes.map((theme) => sameId(theme.id, customId(current.theme)) ? updater(theme) : theme) }));
  };
  const chooseScope = (next: ThemeScope) => { setScope(next); setNotice(""); setWarnedScope(null); };
  const onScopeKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight" && event.key !== "Home" && event.key !== "End") return;
    event.preventDefault();
    const next = event.key === "ArrowLeft" || event.key === "Home" ? "launcher" : "settings";
    chooseScope(next); tabRefs.current[next]?.focus();
  };
  const chooseBuiltin = (id: typeof builtinThemeIds[number]) => {
    if (disabled) return;
    setWarnedScope(null);
    if (scope === "launcher") setWorkingLauncher((current) => ({ ...current, theme: id, accentColor: createLauncherTheme(id).accentColor }));
    else setWorkingSettings((current) => ({ ...current, theme: id, accentColor: createSettingsTheme(id).accentColor }));
  };
  const chooseCustom = (id: string) => {
    if (disabled) return;
    setWarnedScope(null);
    if (scope === "launcher") setWorkingLauncher((current) => ({ ...current, theme: `custom:${id}` }));
    else setWorkingSettings((current) => ({ ...current, theme: `custom:${id}` }));
  };
  const createTheme = () => {
    if (disabled) return;
    if (scope === "launcher") {
      if (workingLauncher.customThemes.length >= MAX_CUSTOM_THEMES) return setNotice(t.customThemeLimit.replace("{max}", String(MAX_CUSTOM_THEMES)));
      const seed = createLauncherTheme();
      const source = selectedLauncher;
      const next = source ? { ...source, id: seed.id, name: t.copyName.replace("{name}", source.name), ...cloneBackground(source) } : { ...createLauncherTheme(workingLauncher.theme), name: t.copyName.replace("{name}", builtinLabels[workingLauncher.theme as keyof typeof builtinLabels] ?? t.midnight) };
      setWorkingLauncher((current) => ({ ...current, theme: `custom:${next.id}`, customThemes: [...current.customThemes, next] }));
    } else {
      if (workingSettings.customThemes.length >= MAX_CUSTOM_THEMES) return setNotice(t.customThemeLimit.replace("{max}", String(MAX_CUSTOM_THEMES)));
      const seed = createSettingsTheme();
      const source = selectedSettings;
      const next = source ? { ...source, id: seed.id, name: t.copyName.replace("{name}", source.name), ...cloneBackground(source) } : { ...createSettingsTheme(workingSettings.theme), name: t.copyName.replace("{name}", builtinLabels[workingSettings.theme as keyof typeof builtinLabels] ?? t.midnight) };
      setWorkingSettings((current) => ({ ...current, theme: `custom:${next.id}`, customThemes: [...current.customThemes, next] }));
    }
    setWarnedScope(null); setNotice(t.createdTheme);
  };
  const deleteTheme = () => {
    if (!selectedCustom || disabled) return;
    invalidateCurrentWallpaperRequest();
    if (scope === "launcher") setWorkingLauncher((current) => ({ ...current, theme: "midnight", accentColor: createLauncherTheme("midnight").accentColor, customThemes: current.customThemes.filter((theme) => !sameId(theme.id, customId(current.theme))) }));
    else setWorkingSettings((current) => ({ ...current, theme: "midnight", accentColor: createSettingsTheme("midnight").accentColor, customThemes: current.customThemes.filter((theme) => !sameId(theme.id, customId(current.theme))) }));
    setWarnedScope(null); setNotice(t.deletedTheme);
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
          setWorkingLauncher((current) => current.customThemes.length >= MAX_CUSTOM_THEMES ? current : { ...current, theme: `custom:${theme.id}`, customThemes: [...current.customThemes, theme] });
          setNotice(t.importedTheme.replace("{name}", theme.name));
        } else {
          const bundle = parseSettingsThemeBundle(value);
          await validateWallpaperImageDataUrl(bundle.theme.wallpaperDataUrl);
          if (importRequestRef.current !== importRequest) return;
          const theme: SettingsCustomThemeConfig = { ...bundle.theme, id: createSettingsTheme().id, platformOverrides: { ...bundle.theme.platformOverrides } };
          setWorkingSettings((current) => current.customThemes.length >= MAX_CUSTOM_THEMES ? current : { ...current, theme: `custom:${theme.id}`, customThemes: [...current.customThemes, theme] });
          setNotice(t.importedTheme.replace("{name}", theme.name));
        }
        setWarnedScope(null);
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
  const applyDraft = () => {
    if (disabled) return;
    try {
      if (scope === "launcher") buildLauncherThemeBundle(launcherPreview);
      else buildSettingsThemeBundle(settingsPreview);
    } catch {
      setNotice(t.invalidThemeDraft);
      return;
    }
    if ((!checksPass || sceneReview) && warnedScope !== scope) { setWarnedScope(scope); setNotice(checksPass ? t.pendingSceneReadability : t.pendingReadability); return; }
    onChange(scope === "launcher"
      ? { launcherTheme: cloneLauncherScope(workingLauncher), settingsTheme: cloneSettingsScope(settingsTheme) }
      : { launcherTheme: cloneLauncherScope(launcherTheme), settingsTheme: cloneSettingsScope(workingSettings) });
    setNotice(t.appliedTheme); setWarnedScope(null);
  };
  const selectedBuiltin = scope === "launcher" ? (selectedLauncher ? null : workingLauncher.theme) : (selectedSettings ? null : workingSettings.theme);
  return <section className="appearance-editor" aria-label={t.ariaLabel}>
    <header className="appearance-editor-heading"><div><h2>{t.title}</h2><p>{t.description}</p></div></header>
    <nav className="appearance-scope-tabs" role="tablist" aria-label={t.scopeTabs} onKeyDown={onScopeKeyDown}>
      {(["launcher", "settings"] as const).map((id) => <button ref={(node) => { tabRefs.current[id] = node; }} key={id} id={`appearance-${id}-tab`} type="button" role="tab" aria-selected={scope === id} aria-controls={`appearance-${id}-panel`} tabIndex={scope === id ? 0 : -1} className={scope === id ? "active" : ""} onClick={() => chooseScope(id)}><span aria-hidden="true">{id === "launcher" ? "⌕" : "⚙"}</span><span><strong>{id === "launcher" ? t.launcherScope : t.settingsScope}</strong><small>{id === "launcher" ? t.launcherScopeHint : t.settingsScopeHint}</small></span></button>)}
    </nav>
    <p className="appearance-separation-note">{t.separateNotice}</p>
    <div id={`appearance-${scope}-panel`} role="tabpanel" aria-labelledby={`appearance-${scope}-tab`} className="appearance-workbench">
      <section className="appearance-edit-panel">
        <header className="appearance-library-heading"><div><h3>{scope === "launcher" ? t.launcherLibrary : t.settingsLibrary}</h3><p>{scope === "launcher" ? t.launcherLibraryHint : t.settingsLibraryHint}</p></div><span>{t.isolated}</span></header>
        <div className="appearance-toolbar"><input ref={importRef} className="appearance-hidden-input" type="file" accept="application/json,.json" disabled={disabled} onChange={(event) => { const file = event.target.files?.[0]; if (file) importTheme(file); event.currentTarget.value = ""; }} /><button type="button" disabled={disabled} onClick={() => importRef.current?.click()}>{t.importTheme}</button><button type="button" disabled={saving || importing} onClick={exportTheme}>{t.exportTheme}</button><button type="button" className="primary" disabled={disabled} onClick={createTheme}>{t.createTheme}</button></div>
        <div className="appearance-theme-grid" aria-label={scope === "launcher" ? t.launcherLibrary : t.settingsLibrary}>{builtinThemeIds.map((id) => <button key={id} type="button" className={`appearance-theme-card ${selectedBuiltin === id ? "selected" : ""}`} aria-pressed={selectedBuiltin === id} disabled={disabled} onClick={() => chooseBuiltin(id)}><span className={`appearance-theme-swatch appearance-theme-swatch-${id}`} /><span><strong>{builtinLabels[id]}</strong><small>{t.builtin} · {scope === "launcher" ? t.launcherTag : t.settingsTag}</small></span></button>)}{(scope === "launcher" ? workingLauncher.customThemes : workingSettings.customThemes).map((theme) => <button key={theme.id} type="button" className={`appearance-theme-card ${selectedCustom?.id === theme.id ? "selected" : ""}`} aria-pressed={selectedCustom?.id === theme.id} disabled={disabled} onClick={() => chooseCustom(theme.id)}><span className="appearance-theme-swatch" style={{ background: `linear-gradient(135deg, ${theme.windowBackground}, ${scope === "launcher" ? (theme as LauncherCustomThemeConfig).selectedRowBackground : (theme as SettingsCustomThemeConfig).selectedNavBackground})` }} /><span><strong>{theme.name}</strong><small>{t.custom} · {scope === "launcher" ? t.launcherTag : t.settingsTag}</small></span></button>)}</div>
        {selectedCustom ? <div className="appearance-custom-editor"><header><label><span>{t.themeName}</span><input value={selectedCustom.name} maxLength={40} disabled={disabled} onChange={(event) => scope === "launcher" ? updateLauncher((theme) => ({ ...theme, name: event.target.value })) : updateSettings((theme) => ({ ...theme, name: event.target.value }))} /><small>{t.themeNameHint}</small></label><div><button type="button" disabled={disabled} onClick={resetTheme}>{t.restoreDefault}</button><button type="button" className="danger" disabled={disabled} onClick={deleteTheme}>{t.delete}</button></div></header>{scope === "launcher" ? <LauncherControls theme={selectedLauncher!} disabled={disabled} update={updateLauncher} onWallpaper={loadWallpaper} onRemoveWallpaper={removeWallpaper} /> : <SettingsControls theme={selectedSettings!} disabled={disabled} update={updateSettings} onWallpaper={loadWallpaper} onRemoveWallpaper={removeWallpaper} />}</div> : <div className="appearance-builtin-empty"><p>{t.builtinReadOnly.replace("{name}", builtinLabels[selectedBuiltin as keyof typeof builtinLabels] ?? t.midnight)}</p><button type="button" className="primary" disabled={disabled} onClick={createTheme}>{t.createFromBuiltin}</button></div>}
        <footer className="appearance-edit-footer"><span>{notice}</span><button type="button" className="primary" disabled={disabled} onClick={applyDraft}>{t.applyToDraft}</button></footer>
      </section>
      <aside className="appearance-preview-panel" aria-label={t.previewAriaLabel}><header><div><strong>{scope === "launcher" ? t.previewLauncher : t.previewSettings}</strong><small>{t.previewDraftOnly}</small></div><span className={contrastVerified ? "ok" : "warning"}>{contrastVerified ? t.contrastPass : checksPass ? t.contrastSceneReview : t.contrastAdjust}</span></header><div className="appearance-preview-canvas">{scope === "launcher" ? <LauncherPreview theme={launcherPreview} /> : <SettingsPreview theme={settingsPreview} />}</div><ContrastAudit checks={checks} sceneReview={sceneReview} /></aside>
    </div>
  </section>;
}
