use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use image::{GenericImageView, ImageFormat, ImageReader, Limits};
use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{dock, hotkey, launcher::LauncherState, web_search};

const CONFIG_VERSION: u32 = 12;
const CONFIG_FILE_NAME: &str = "config.json";
const CONFIG_LOCATION_FILE_NAME: &str = "config-location.json";
const CONFIG_LOCATION_VERSION: u32 = 1;
const CREDENTIAL_SERVICE: &str = "io.github.dqgod.suo";
const TRANSLATOR_CREDENTIAL: &str = "microsoft-translator-api-key";
const MAX_COMMANDS: usize = 50;
const MAX_CUSTOM_THEMES: usize = 12;
const MAX_THEME_WALLPAPER_BYTES: usize = 1_572_864;
const MAX_THEME_WALLPAPER_DIMENSION: u32 = 4_096;
const MAX_THEME_WALLPAPER_PIXELS: u64 = 16_777_216;
// A supported PNG can use 16-bit RGBA (8 bytes per pixel). Cover the full
// 4096 x 4096 output plus bounded decoder scratch space so every image that
// passes the frontend's dimension contract can also pass the Rust decoder.
const MAX_THEME_WALLPAPER_ALLOCATION_BYTES: u64 = 160 * 1024 * 1024;
const MAX_COMMAND_ICON_BYTES: usize = 256 * 1024;
const MAX_COMMAND_ICON_DIMENSION: u32 = 512;
const MAX_COMMAND_ICON_PIXELS: u64 = 512 * 512;
const MAX_COMMAND_ICON_ALLOCATION_BYTES: u64 = 8 * 1024 * 1024;
const MISSING_CUSTOM_ACCENT_COLOR: &str = "\u{0}missing-custom-accent";
const MIN_QUERY_DEBOUNCE_MS: u64 = 0;
const MAX_QUERY_DEBOUNCE_MS: u64 = 60_000;
const MIN_LAUNCHER_WIDTH_PX: u32 = 560;
const MAX_LAUNCHER_WIDTH_PX: u32 = 1_200;
const MIN_LAUNCHER_HEIGHT_PX: u32 = 320;
const MAX_LAUNCHER_HEIGHT_PX: u32 = 720;
const MIN_LAUNCHER_HORIZONTAL_OFFSET_PX: i32 = -400;
const MAX_LAUNCHER_HORIZONTAL_OFFSET_PX: i32 = 400;
const MIN_LAUNCHER_VERTICAL_OFFSET_PX: i32 = -240;
const MAX_LAUNCHER_VERTICAL_OFFSET_PX: i32 = 240;
const MIN_SCRIPT_DEBOUNCE_MS: u64 = 20;
const MAX_SCRIPT_DEBOUNCE_MS: u64 = 60_000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub save_settings_manually: bool,
    pub launcher: LauncherConfig,
    pub translation: TranslationConfig,
    pub script_commands: Vec<ScriptCommandConfig>,
    pub web_searches: Vec<WebSearchConfig>,
    pub launcher_theme: LauncherThemeConfig,
    pub settings_theme: SettingsThemeConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfigWire {
    version: u32,
    #[serde(default)]
    save_settings_manually: Option<bool>,
    launcher: LauncherConfig,
    translation: TranslationConfig,
    script_commands: Vec<ScriptCommandConfig>,
    web_searches: Vec<WebSearchConfig>,
    #[serde(default)]
    launcher_theme: Option<LauncherThemeConfig>,
    #[serde(default)]
    settings_theme: Option<SettingsThemeConfig>,
    // v5 and older stored one appearance model. It is intentionally accepted
    // only while loading local configuration so that it can be split into two
    // independent scope models; it is not an import contract.
    #[serde(default)]
    appearance: Option<LegacyAppearanceConfig>,
}

impl<'de> Deserialize<'de> for AppConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AppConfigWire::deserialize(deserializer)?;
        let (mut launcher_theme, mut settings_theme) =
            match (wire.launcher_theme, wire.settings_theme) {
                (Some(launcher_theme), Some(settings_theme)) => (launcher_theme, settings_theme),
                (None, None) if wire.version <= 5 => {
                    let legacy = wire.appearance.unwrap_or_default();
                    (
                        LauncherThemeConfig::from_legacy(&legacy),
                        SettingsThemeConfig::from_legacy(&legacy),
                    )
                }
                (None, None) => {
                    return Err(D::Error::custom(
                        "v6 及更新配置必须同时包含 launcherTheme 和 settingsTheme",
                    ));
                }
                _ => {
                    return Err(D::Error::custom(
                        "launcherTheme 和 settingsTheme 必须同时存在",
                    ));
                }
            };
        // An intermediate local v6 build predated per-custom-theme accents.
        // Repair only the serde sentinel for an omitted field; an explicitly
        // empty or malformed accent still reaches normal validation and fails.
        launcher_theme.fill_missing_custom_accents();
        settings_theme.fill_missing_custom_accents();
        let save_settings_manually = match wire.save_settings_manually {
            Some(value) => value,
            None if wire.version <= 6 => true,
            None => {
                return Err(D::Error::custom("v7 配置必须包含 saveSettingsManually"));
            }
        };
        Ok(Self {
            version: wire.version,
            save_settings_manually,
            launcher: wire.launcher,
            translation: wire.translation,
            script_commands: wire.script_commands,
            web_searches: wire.web_searches,
            launcher_theme,
            settings_theme,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    #[serde(default = "hotkey::default_shortcut")]
    pub global_hotkey: String,
    pub close_on_blur: bool,
    pub keep_last_input: bool,
    #[serde(default)]
    pub compact_when_empty: bool,
    #[serde(default = "default_show_dock_icon")]
    pub show_dock_icon: bool,
    #[serde(default = "default_empty_query_debounce_ms")]
    pub empty_query_debounce_ms: u64,
    #[serde(default = "default_non_empty_query_debounce_ms")]
    pub non_empty_query_debounce_ms: u64,
    /// `None` keeps the active launcher theme's width. Once the user adjusts
    /// General settings, the explicit width remains stable across themes.
    #[serde(default)]
    pub window_width_px: Option<u32>,
    #[serde(default = "default_launcher_height_px")]
    pub window_height_px: u32,
    #[serde(default)]
    pub horizontal_offset_px: i32,
    #[serde(default)]
    pub vertical_offset_px: i32,
}

const fn default_empty_query_debounce_ms() -> u64 {
    0
}

const fn default_show_dock_icon() -> bool {
    true
}

const fn default_non_empty_query_debounce_ms() -> u64 {
    50
}

const fn default_launcher_height_px() -> u32 {
    520
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationConfig {
    pub enabled: bool,
    pub keyword: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub region: String,
    pub default_target_language: String,
    pub chinese_target_language: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCommandConfig {
    pub id: String,
    pub name: String,
    pub keyword: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_data_url: String,
    #[serde(default)]
    pub input_hint: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub enabled: bool,
    pub runtime: ScriptRuntime,
    pub script_path: String,
    pub immediate: bool,
    #[serde(default = "default_script_debounce_ms")]
    pub debounce_ms: u64,
    pub timeout_ms: u64,
}

const fn default_script_debounce_ms() -> u64 {
    50
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptRuntime {
    Python,
    PowerShell,
    Bash,
    Executable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchConfig {
    pub id: String,
    pub name: String,
    pub keyword: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_data_url: String,
    #[serde(default)]
    pub input_hint: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub enabled: bool,
    pub url_template: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherThemeConfig {
    pub theme: String,
    pub accent_color: String,
    #[serde(default)]
    pub custom_themes: Vec<LauncherCustomThemeConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherCustomThemeConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "missing_custom_accent_color")]
    pub accent_color: String,
    pub window_background: String,
    pub window_border: String,
    pub window_border_width_px: u8,
    pub window_width_px: u16,
    pub window_radius_px: u8,
    pub search_background: String,
    pub search_border: String,
    pub search_border_width_px: u8,
    pub search_border_style: String,
    pub search_width_px: u16,
    pub search_text_color: String,
    pub search_font_size_px: u8,
    pub normal_row_background: String,
    pub normal_primary_color: String,
    pub normal_secondary_color: String,
    pub normal_primary_font_size_px: u8,
    pub normal_secondary_font_size_px: u8,
    pub normal_row_height_px: u8,
    pub selected_row_background: String,
    pub selected_primary_color: String,
    pub selected_secondary_color: String,
    pub selected_primary_font_size_px: u8,
    pub selected_secondary_font_size_px: u8,
    pub icon_size_px: u8,
    pub show_search_icon: bool,
    pub show_logo: bool,
    #[serde(default = "default_show_source_badge")]
    pub show_source_badge: bool,
    pub max_results: u8,
    #[serde(flatten)]
    pub background: ThemeBackgroundConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsThemeConfig {
    pub theme: String,
    pub accent_color: String,
    #[serde(default)]
    pub custom_themes: Vec<SettingsCustomThemeConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsCustomThemeConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "missing_custom_accent_color")]
    pub accent_color: String,
    pub window_background: String,
    pub titlebar_background: String,
    pub sidebar_background: String,
    pub content_background: String,
    pub card_background: String,
    pub border_color: String,
    pub primary_text_color: String,
    pub secondary_text_color: String,
    pub nav_text_color: String,
    pub selected_nav_background: String,
    pub base_font_size_px: u8,
    pub radius_px: u8,
    #[serde(flatten)]
    pub background: ThemeBackgroundConfig,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeBackgroundConfig {
    pub window_opacity: u8,
    pub blur_px: u8,
    pub shadow_percent: u8,
    #[serde(default)]
    pub wallpaper_data_url: String,
    pub wallpaper_opacity: u8,
    #[serde(default)]
    pub platform_overrides: PlatformThemeOverrides,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyAppearanceConfig {
    #[serde(default = "default_builtin_theme")]
    theme: String,
    #[serde(default = "default_accent_color")]
    accent_color: String,
    #[serde(default)]
    custom_themes: Vec<LegacyCustomThemeConfig>,
}

impl Default for LegacyAppearanceConfig {
    fn default() -> Self {
        Self {
            theme: default_builtin_theme(),
            accent_color: default_accent_color(),
            custom_themes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyCustomThemeConfig {
    id: String,
    name: String,
    window_color: String,
    panel_color: String,
    field_color: String,
    text_color: String,
    muted_color: String,
    accent_color: String,
    selection_color: String,
    border_color: String,
    window_opacity: u8,
    blur_px: u8,
    shadow_percent: u8,
    #[serde(default)]
    wallpaper_data_url: String,
    wallpaper_opacity: u8,
    radius_px: u8,
    #[serde(rename = "fontFamily")]
    _font_family: String,
    font_size_px: u8,
    launcher_width_px: u16,
    result_density: String,
    max_results: u8,
    icon_size_px: u8,
    show_source_badge: bool,
    #[serde(default)]
    platform_overrides: PlatformThemeOverrides,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformThemeOverrides {
    pub enabled: bool,
    pub windows_blur_px: u8,
    pub windows_opacity: u8,
    pub macos_blur_px: u8,
    pub macos_opacity: u8,
}

impl Default for PlatformThemeOverrides {
    fn default() -> Self {
        Self {
            enabled: false,
            windows_blur_px: 18,
            windows_opacity: 94,
            macos_blur_px: 18,
            macos_opacity: 94,
        }
    }
}

const fn default_show_source_badge() -> bool {
    true
}

fn default_builtin_theme() -> String {
    "midnight".into()
}

fn default_accent_color() -> String {
    "#8a78ff".into()
}

fn missing_custom_accent_color() -> String {
    MISSING_CUSTOM_ACCENT_COLOR.into()
}

impl Default for LauncherThemeConfig {
    fn default() -> Self {
        Self {
            theme: default_builtin_theme(),
            accent_color: default_accent_color(),
            custom_themes: Vec::new(),
        }
    }
}

impl Default for SettingsThemeConfig {
    fn default() -> Self {
        Self {
            theme: default_builtin_theme(),
            accent_color: default_accent_color(),
            custom_themes: Vec::new(),
        }
    }
}

impl LauncherThemeConfig {
    fn fill_missing_custom_accents(&mut self) {
        for theme in &mut self.custom_themes {
            if theme.accent_color == MISSING_CUSTOM_ACCENT_COLOR {
                theme.accent_color.clone_from(&self.accent_color);
            }
        }
    }

    pub fn active_custom_theme(&self) -> Option<&LauncherCustomThemeConfig> {
        let id = self.theme.strip_prefix("custom:")?;
        self.custom_themes
            .iter()
            .find(|theme| theme.id.eq_ignore_ascii_case(id))
    }

    pub fn max_results(&self) -> usize {
        self.active_custom_theme()
            .map(|theme| usize::from(theme.max_results))
            .unwrap_or(8)
    }

    pub fn launcher_width(&self) -> f64 {
        self.active_custom_theme()
            .map(|theme| f64::from(theme.window_width_px))
            .unwrap_or(720.0)
    }

    fn from_legacy(legacy: &LegacyAppearanceConfig) -> Self {
        Self {
            theme: legacy.theme.clone(),
            accent_color: legacy_active_accent_color(legacy),
            custom_themes: legacy
                .custom_themes
                .iter()
                .map(LauncherCustomThemeConfig::from_legacy)
                .collect(),
        }
    }
}

impl SettingsThemeConfig {
    fn fill_missing_custom_accents(&mut self) {
        for theme in &mut self.custom_themes {
            if theme.accent_color == MISSING_CUSTOM_ACCENT_COLOR {
                theme.accent_color.clone_from(&self.accent_color);
            }
        }
    }

    fn from_legacy(legacy: &LegacyAppearanceConfig) -> Self {
        Self {
            theme: legacy.theme.clone(),
            accent_color: legacy_active_accent_color(legacy),
            custom_themes: legacy
                .custom_themes
                .iter()
                .map(SettingsCustomThemeConfig::from_legacy)
                .collect(),
        }
    }
}

fn legacy_active_accent_color(legacy: &LegacyAppearanceConfig) -> String {
    let Some(selected) = legacy.theme.strip_prefix("custom:") else {
        return legacy.accent_color.clone();
    };
    legacy
        .custom_themes
        .iter()
        .find(|theme| theme.id.eq_ignore_ascii_case(selected))
        .map(|theme| theme.accent_color.clone())
        .unwrap_or_else(|| legacy.accent_color.clone())
}

impl ThemeBackgroundConfig {
    fn from_legacy(theme: &LegacyCustomThemeConfig) -> Self {
        Self {
            window_opacity: theme.window_opacity,
            blur_px: theme.blur_px,
            shadow_percent: theme.shadow_percent,
            wallpaper_data_url: theme.wallpaper_data_url.clone(),
            wallpaper_opacity: theme.wallpaper_opacity,
            platform_overrides: theme.platform_overrides.clone(),
        }
    }
}

impl LauncherCustomThemeConfig {
    fn from_legacy(theme: &LegacyCustomThemeConfig) -> Self {
        let row_height = match theme.result_density.as_str() {
            "compact" => 48,
            "loose" => 68,
            _ => 58,
        };
        Self {
            id: theme.id.clone(),
            name: theme.name.clone(),
            accent_color: theme.accent_color.clone(),
            window_background: theme.window_color.clone(),
            window_border: theme.border_color.clone(),
            window_border_width_px: 1,
            window_width_px: theme.launcher_width_px,
            window_radius_px: theme.radius_px,
            search_background: theme.field_color.clone(),
            search_border: theme.border_color.clone(),
            search_border_width_px: 1,
            search_border_style: "solid".into(),
            search_width_px: theme.launcher_width_px,
            search_text_color: theme.text_color.clone(),
            search_font_size_px: 20,
            // Results were transparent in the unified v5 skin. Reusing the
            // outer window colour preserves that visual after the scope split.
            normal_row_background: theme.window_color.clone(),
            normal_primary_color: theme.text_color.clone(),
            normal_secondary_color: theme.muted_color.clone(),
            normal_primary_font_size_px: theme.font_size_px,
            normal_secondary_font_size_px: theme.font_size_px.saturating_sub(2).max(10),
            normal_row_height_px: row_height,
            selected_row_background: theme.selection_color.clone(),
            selected_primary_color: theme.text_color.clone(),
            selected_secondary_color: theme.muted_color.clone(),
            selected_primary_font_size_px: theme.font_size_px,
            selected_secondary_font_size_px: theme.font_size_px.saturating_sub(2).max(10),
            icon_size_px: theme.icon_size_px,
            show_search_icon: true,
            // The v5 launcher always rendered the Suo button on the right.
            // Preserve it during migration; users may hide it in the new skin.
            show_logo: true,
            show_source_badge: theme.show_source_badge,
            max_results: theme.max_results,
            background: ThemeBackgroundConfig::from_legacy(theme),
        }
    }
}

impl SettingsCustomThemeConfig {
    fn from_legacy(theme: &LegacyCustomThemeConfig) -> Self {
        Self {
            id: theme.id.clone(),
            name: theme.name.clone(),
            accent_color: theme.accent_color.clone(),
            window_background: theme.window_color.clone(),
            titlebar_background: theme.panel_color.clone(),
            sidebar_background: theme.field_color.clone(),
            content_background: theme.window_color.clone(),
            card_background: theme.panel_color.clone(),
            border_color: theme.border_color.clone(),
            primary_text_color: theme.text_color.clone(),
            secondary_text_color: theme.muted_color.clone(),
            nav_text_color: theme.text_color.clone(),
            selected_nav_background: theme.selection_color.clone(),
            base_font_size_px: theme.font_size_px,
            radius_px: theme.radius_px,
            background: ThemeBackgroundConfig::from_legacy(theme),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigView {
    pub config: AppConfig,
    pub config_file_path: String,
    pub config_directory: String,
    pub default_config_file_path: String,
    pub default_config_directory: String,
    pub using_default_config_location: bool,
    pub config_location_needs_reset: bool,
    pub translation_api_key_configured: bool,
    pub credential_store_error: Option<String>,
    pub config_load_warning: Option<String>,
    pub needs_legacy_preferences_migration: bool,
    pub config_read_only: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ConfigLocationPointer {
    version: u32,
    config_directory: Option<PathBuf>,
}

pub struct ConfigState {
    path: RwLock<PathBuf>,
    default_path: PathBuf,
    location_path: PathBuf,
    config: RwLock<AppConfig>,
    load_warning: RwLock<Option<String>>,
    needs_legacy_preferences_migration: RwLock<bool>,
    save_lock: Mutex<()>,
    incompatible_newer_version: Option<u64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            save_settings_manually: true,
            launcher: LauncherConfig {
                global_hotkey: hotkey::default_shortcut(),
                close_on_blur: true,
                keep_last_input: false,
                compact_when_empty: false,
                show_dock_icon: default_show_dock_icon(),
                empty_query_debounce_ms: default_empty_query_debounce_ms(),
                non_empty_query_debounce_ms: default_non_empty_query_debounce_ms(),
                window_width_px: None,
                window_height_px: default_launcher_height_px(),
                horizontal_offset_px: 0,
                vertical_offset_px: 0,
            },
            translation: TranslationConfig {
                enabled: true,
                keyword: "fy".into(),
                description: "中英文自动识别翻译；支持 fy:语言代码 临时指定目标语言。".into(),
                aliases: Vec::new(),
                region: String::new(),
                default_target_language: "zh-Hans".into(),
                chinese_target_language: "en".into(),
            },
            script_commands: vec![ScriptCommandConfig {
                id: "timestamp-example".into(),
                name: "时间戳转换".into(),
                keyword: "ts".into(),
                description: "将毫秒时间戳转换为日期时间；第二个参数可传 +8 等时区偏移。".into(),
                icon_data_url: String::new(),
                input_hint: String::new(),
                aliases: Vec::new(),
                enabled: true,
                runtime: ScriptRuntime::Python,
                script_path: "examples/timestamp.py".into(),
                immediate: true,
                debounce_ms: default_script_debounce_ms(),
                timeout_ms: 3_000,
            }],
            web_searches: vec![WebSearchConfig {
                id: "google".into(),
                name: "Google".into(),
                keyword: "google".into(),
                description: "使用默认浏览器在 Google 中搜索输入内容。".into(),
                icon_data_url: String::new(),
                input_hint: String::new(),
                aliases: Vec::new(),
                enabled: true,
                url_template: "https://www.google.com.hk/search?q={query}".into(),
            }],
            launcher_theme: LauncherThemeConfig::default(),
            settings_theme: SettingsThemeConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn launcher_width(&self) -> f64 {
        self.launcher
            .window_width_px
            .map(f64::from)
            .unwrap_or_else(|| self.launcher_theme.launcher_width())
    }

    pub fn launcher_height(&self) -> f64 {
        f64::from(self.launcher.window_height_px)
    }
}

impl ConfigState {
    pub fn load(app: &AppHandle) -> Self {
        let config_dir = app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        let default_path = config_dir.join(CONFIG_FILE_NAME);
        let location_path = config_dir.join(CONFIG_LOCATION_FILE_NAME);
        let (path, location_warning) = resolve_config_path(&default_path, &location_path);
        let needs_legacy_preferences_migration =
            !path.exists() && !path.with_extension("json.bak").exists();
        let incompatible_newer_version = newer_config_version(&path);
        let (config, config_warning) = if let Some(version) = incompatible_newer_version {
            (
                AppConfig::default(),
                Some(format!(
                    "配置来自更新版本 v{version}，当前 Suo v{CONFIG_VERSION} 仅以只读方式启动"
                )),
            )
        } else {
            load_config(&path)
        };
        let load_warning = join_warnings(location_warning, config_warning);
        Self {
            path: RwLock::new(path),
            default_path,
            location_path,
            config: RwLock::new(config),
            load_warning: RwLock::new(load_warning),
            needs_legacy_preferences_migration: RwLock::new(needs_legacy_preferences_migration),
            save_lock: Mutex::new(()),
            incompatible_newer_version,
        }
    }

    pub fn snapshot(&self) -> AppConfig {
        self.config
            .read()
            .map(|value| value.clone())
            .unwrap_or_default()
    }

    fn replace(&self, config: AppConfig) -> Result<(AppConfig, AppConfig), String> {
        let _save_guard = self
            .save_lock
            .lock()
            .map_err(|_| "配置保存锁暂时不可用".to_string())?;
        let path = self.config_path()?;
        if let Some(version) = self
            .incompatible_newer_version
            .or_else(|| newer_config_version(&path))
        {
            return Err(format!(
                "配置来自更新版本 v{version}，当前版本禁止覆盖，请升级 Suo"
            ));
        }
        let config = normalize_and_validate(config)?;
        let previous = self
            .config
            .read()
            .map_err(|_| "配置状态暂时不可用".to_string())?
            .clone();
        persist_config(&path, &config)?;
        let mut current = self
            .config
            .write()
            .map_err(|_| "配置状态暂时不可用".to_string())?;
        *current = config.clone();
        if let Ok(mut warning) = self.load_warning.write() {
            *warning = None;
        }
        if let Ok(mut migration) = self.needs_legacy_preferences_migration.write() {
            *migration = false;
        }
        Ok((previous, config))
    }

    fn config_path(&self) -> Result<PathBuf, String> {
        self.path
            .read()
            .map(|path| path.clone())
            .map_err(|_| "配置路径状态暂时不可用".to_string())
    }

    fn relocate(&self, directory: &Path) -> Result<(), String> {
        let _save_guard = self
            .save_lock
            .lock()
            .map_err(|_| "配置保存锁暂时不可用".to_string())?;
        if self.incompatible_newer_version.is_some() {
            return Err("当前配置来自更新版本，不能调整配置位置".into());
        }
        if !directory.is_absolute() {
            return Err("配置目录必须是绝对路径".into());
        }
        if !directory.is_dir() {
            return Err(format!(
                "配置目录不存在或不是文件夹：{}",
                directory.display()
            ));
        }

        let target = directory.join(CONFIG_FILE_NAME);
        let using_default = paths_equal(&target, &self.default_path);
        let mut current_path = self
            .path
            .write()
            .map_err(|_| "配置路径状态暂时不可用".to_string())?;
        if paths_equal(&current_path, &target) {
            if using_default && config_location_needs_reset(&self.location_path) {
                persist_config_location(&self.location_path, None)?;
                if let Ok(mut warning) = self.load_warning.write() {
                    *warning = None;
                }
            }
            return Ok(());
        }
        if let Some(version) = newer_config_version(&current_path) {
            return Err(format!(
                "当前配置已由更新版本 v{version} 写入，不能调整配置位置"
            ));
        }

        let target_backup = target.with_extension("json.bak");
        if !using_default && (target.exists() || target_backup.exists()) {
            return Err(format!(
                "目标文件夹已包含 {} 或其备份；为避免覆盖，请选择空文件夹",
                CONFIG_FILE_NAME
            ));
        }

        let config = self.snapshot();
        persist_config(&target, &config)?;
        if let Err(error) = read_config_file(&target) {
            if !using_default {
                let _ = fs::remove_file(&target);
            }
            return Err(format!("目标配置写入后校验失败，仍使用原位置：{error}"));
        }
        let pointer_directory = (!using_default).then_some(directory);
        if let Err(error) = persist_config_location(&self.location_path, pointer_directory) {
            if !using_default {
                let _ = fs::remove_file(&target);
                let _ = fs::remove_file(target.with_extension("json.tmp"));
            }
            return Err(format!("配置已复制到目标目录，但无法切换位置：{error}"));
        }
        let (resolved, pointer_warning) =
            resolve_config_path(&self.default_path, &self.location_path);
        if pointer_warning.is_some() || !paths_equal(&resolved, &target) {
            let previous_directory = (!paths_equal(&current_path, &self.default_path))
                .then(|| current_path.parent())
                .flatten();
            let rollback = persist_config_location(&self.location_path, previous_directory);
            if !using_default {
                let _ = fs::remove_file(&target);
            }
            let rollback_detail = rollback
                .err()
                .map(|error| format!("；恢复原位置指针也失败：{error}"))
                .unwrap_or_default();
            return Err(format!(
                "配置位置指针校验失败，仍使用原位置{rollback_detail}"
            ));
        }

        *current_path = target;
        if let Ok(mut warning) = self.load_warning.write() {
            *warning = None;
        }
        if let Ok(mut migration) = self.needs_legacy_preferences_migration.write() {
            *migration = false;
        }
        Ok(())
    }

    fn view(&self) -> AppConfigView {
        let (translation_api_key_configured, credential_store_error) =
            match read_translation_api_key() {
                Ok(value) => (value.is_some(), None),
                Err(error) => (false, Some(error)),
            };
        let config_path = self
            .config_path()
            .unwrap_or_else(|_| self.default_path.clone());
        let config_directory = config_path.parent().unwrap_or(Path::new("."));
        let default_config_directory = self.default_path.parent().unwrap_or(Path::new("."));
        AppConfigView {
            config: self.snapshot(),
            config_file_path: config_path.to_string_lossy().into_owned(),
            config_directory: config_directory.to_string_lossy().into_owned(),
            default_config_file_path: self.default_path.to_string_lossy().into_owned(),
            default_config_directory: default_config_directory.to_string_lossy().into_owned(),
            using_default_config_location: paths_equal(&config_path, &self.default_path),
            config_location_needs_reset: config_location_needs_reset(&self.location_path),
            translation_api_key_configured,
            credential_store_error,
            config_load_warning: self
                .load_warning
                .read()
                .ok()
                .and_then(|warning| warning.clone()),
            needs_legacy_preferences_migration: self
                .needs_legacy_preferences_migration
                .read()
                .map(|value| *value)
                .unwrap_or(false),
            config_read_only: self.incompatible_newer_version.is_some(),
        }
    }
}

fn resolve_config_path(default_path: &Path, location_path: &Path) -> (PathBuf, Option<String>) {
    if !location_path.exists() {
        return (default_path.to_path_buf(), None);
    }
    let pointer = match read_config_location_pointer(location_path) {
        Ok(pointer) => pointer,
        Err(error) => {
            return (
                default_path.to_path_buf(),
                Some(format!("{error}；已使用默认配置位置")),
            );
        }
    };
    if pointer.version != CONFIG_LOCATION_VERSION {
        return (
            default_path.to_path_buf(),
            Some(format!(
                "不支持的配置位置指针版本 v{}；已使用默认配置位置",
                pointer.version
            )),
        );
    }
    let Some(directory) = pointer.config_directory else {
        return (default_path.to_path_buf(), None);
    };
    if !directory.is_absolute() {
        return (
            default_path.to_path_buf(),
            Some("自定义配置目录不是绝对路径；已使用默认配置位置".into()),
        );
    }
    let candidate = directory.join(CONFIG_FILE_NAME);
    if paths_equal(&candidate, default_path) {
        return (default_path.to_path_buf(), None);
    }
    if candidate.exists() || candidate.with_extension("json.bak").exists() {
        return (candidate, None);
    }
    (
        default_path.to_path_buf(),
        Some(format!(
            "自定义配置 {} 不可用；已临时使用默认配置位置",
            candidate.display()
        )),
    )
}

fn read_config_location_pointer(location_path: &Path) -> Result<ConfigLocationPointer, String> {
    let content = fs::read_to_string(location_path)
        .map_err(|error| format!("无法读取配置位置指针：{error}"))?;
    serde_json::from_str::<ConfigLocationPointer>(&content)
        .map_err(|error| format!("配置位置指针格式无效：{error}"))
}

fn config_location_needs_reset(location_path: &Path) -> bool {
    if !location_path.exists() {
        return false;
    }
    match read_config_location_pointer(location_path) {
        Ok(pointer) => {
            pointer.version != CONFIG_LOCATION_VERSION || pointer.config_directory.is_some()
        }
        Err(_) => true,
    }
}

fn persist_config_location(
    location_path: &Path,
    config_directory: Option<&Path>,
) -> Result<(), String> {
    if let Some(parent) = location_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建默认配置目录：{error}"))?;
    }
    let pointer = ConfigLocationPointer {
        version: CONFIG_LOCATION_VERSION,
        config_directory: config_directory.map(Path::to_path_buf),
    };
    let data = serde_json::to_string_pretty(&pointer)
        .map_err(|error| format!("无法序列化配置位置：{error}"))?;
    let temporary = location_path.with_extension("json.tmp");
    let mut file = fs::File::create(&temporary)
        .map_err(|error| format!("无法创建配置位置临时文件：{error}"))?;
    file.write_all(data.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("无法写入配置位置临时文件：{error}"))?;
    let result = replace_config_file(&temporary, location_path);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn join_warnings(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}；{second}")),
        (Some(warning), None) | (None, Some(warning)) => Some(warning),
        (None, None) => None,
    }
}

#[cfg(target_os = "windows")]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(target_os = "windows"))]
fn paths_equal(left: &Path, right: &Path) -> bool {
    left == right
}

fn config_file_version(path: &Path) -> Option<u64> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str::<serde_json::Value>(&content)
        .ok()?
        .get("version")?
        .as_u64()
}

fn newer_config_version(path: &Path) -> Option<u64> {
    [path.to_path_buf(), path.with_extension("json.bak")]
        .into_iter()
        .filter_map(|candidate| config_file_version(&candidate))
        .filter(|version| *version > u64::from(CONFIG_VERSION))
        .max()
}

fn load_config(path: &Path) -> (AppConfig, Option<String>) {
    let backup = path.with_extension("json.bak");
    if !path.exists() {
        if backup.exists() {
            return match read_config_file(&backup) {
                Ok(config) => (config, Some("主配置缺失，已临时恢复上次备份".into())),
                Err(error) => (
                    AppConfig::default(),
                    Some(format!("主配置缺失且备份无效，已使用默认值：{error}")),
                ),
            };
        }
        return (AppConfig::default(), None);
    }
    match read_config_file(path) {
        Ok(config) => (config, None),
        Err(primary_error) => match read_config_file(&backup) {
            Ok(config) => (
                config,
                Some(format!("主配置无效，已临时恢复上次备份：{primary_error}")),
            ),
            Err(_) => (
                AppConfig::default(),
                Some(format!("配置无效，已使用默认值：{primary_error}")),
            ),
        },
    }
}

fn read_config_file(path: &Path) -> Result<AppConfig, String> {
    let content = fs::read_to_string(path).map_err(|error| format!("无法读取：{error}"))?;
    serde_json::from_str::<AppConfig>(&content)
        .map_err(|error| format!("JSON 格式无效：{error}"))
        .and_then(normalize_and_validate)
}

fn persist_config(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建配置目录：{error}"))?;
    }
    if let Some(version) = newer_config_version(path) {
        return Err(format!(
            "磁盘配置已由更新版本 v{version} 写入，当前版本拒绝覆盖"
        ));
    }
    let data =
        serde_json::to_string_pretty(config).map_err(|error| format!("无法序列化配置：{error}"))?;
    let temporary = path.with_extension("json.tmp");
    let mut file =
        fs::File::create(&temporary).map_err(|error| format!("无法创建临时配置：{error}"))?;
    file.write_all(data.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("无法写入临时配置：{error}"))?;
    if path.exists() && read_config_file(path).is_ok() {
        fs::copy(path, path.with_extension("json.bak"))
            .map_err(|error| format!("无法备份现有配置：{error}"))?;
    }
    // The single-instance plugin is registered before ConfigState::load, so
    // supported Suo versions cannot concurrently write this app config path.
    // Recheck here to catch a newer file placed on disk during this save. An
    // arbitrary external writer would not honor an advisory lock on macOS,
    // so the version guard remains the cross-platform compatibility boundary.
    if let Some(version) = newer_config_version(path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "磁盘配置已由更新版本 v{version} 写入，当前版本拒绝覆盖"
        ));
    }
    let result = replace_config_file(&temporary, path);
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(target_os = "windows")]
fn replace_config_file(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    // SAFETY: both UTF-16 buffers are NUL-terminated and live for this call.
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|error| format!("无法原子替换配置：{error}"))
    }
}

#[cfg(not(target_os = "windows"))]
fn replace_config_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination).map_err(|error| format!("无法原子替换配置：{error}"))
}

fn normalize_and_validate(config: AppConfig) -> Result<AppConfig, String> {
    let mut config = migrate_config(config)?;
    if config.script_commands.len() > MAX_COMMANDS || config.web_searches.len() > MAX_COMMANDS {
        return Err(format!("脚本命令和网络搜索分别最多允许 {MAX_COMMANDS} 项"));
    }

    config.launcher.global_hotkey = hotkey::normalize_shortcut(&config.launcher.global_hotkey)?;
    validate_launcher(&config.launcher)?;
    normalize_translation(&mut config.translation)?;
    for command in &mut config.script_commands {
        normalize_script(command)?;
    }
    for search in &mut config.web_searches {
        normalize_web_search(search)?;
    }
    normalize_launcher_theme(&mut config.launcher_theme)?;
    normalize_settings_theme(&mut config.settings_theme)?;
    validate_keyword_namespace(&config)?;
    validate_unique_ids(&config)?;
    Ok(config)
}

fn migrate_config(mut config: AppConfig) -> Result<AppConfig, String> {
    if config.version > CONFIG_VERSION {
        return Err(format!(
            "配置来自更新版本（v{}），当前 Suo 仅支持 v{CONFIG_VERSION}，已拒绝覆盖",
            config.version
        ));
    }

    match config.version {
        // v2 adds optional descriptions, v3 adds the empty-query compact mode,
        // v4 adds per-script debounce, v5 adds custom themes, v6 splits the
        // legacy appearance payload into launcherTheme/settingsTheme, v7 adds
        // the configurable manual/instant settings save mode, and v8 adds
        // independent empty/non-empty query debounce settings, v9 adds the
        // launcher's cross-platform initial size and position fine tuning, and
        // v10 adds the macOS Dock visibility preference, v11 makes the global
        // launcher shortcut configurable, and v12 adds optional per-command
        // icons and empty-argument hints.
        // Older versions preserve their former behavior through serde defaults.
        0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 => config.version = CONFIG_VERSION,
        12 => {}
        version => return Err(format!("不支持的配置版本 v{version}")),
    }
    Ok(config)
}

fn validate_launcher(config: &LauncherConfig) -> Result<(), String> {
    for (label, value) in [
        ("空输入防抖", config.empty_query_debounce_ms),
        ("非空输入防抖", config.non_empty_query_debounce_ms),
    ] {
        if !(MIN_QUERY_DEBOUNCE_MS..=MAX_QUERY_DEBOUNCE_MS).contains(&value) {
            return Err(format!(
                "{label}必须在 {MIN_QUERY_DEBOUNCE_MS}–{MAX_QUERY_DEBOUNCE_MS} ms 之间"
            ));
        }
    }
    if let Some(width) = config.window_width_px {
        if !(MIN_LAUNCHER_WIDTH_PX..=MAX_LAUNCHER_WIDTH_PX).contains(&width) {
            return Err(format!(
                "启动器宽度必须在 {MIN_LAUNCHER_WIDTH_PX}–{MAX_LAUNCHER_WIDTH_PX} px 之间"
            ));
        }
    }
    if !(MIN_LAUNCHER_HEIGHT_PX..=MAX_LAUNCHER_HEIGHT_PX).contains(&config.window_height_px) {
        return Err(format!(
            "启动器高度必须在 {MIN_LAUNCHER_HEIGHT_PX}–{MAX_LAUNCHER_HEIGHT_PX} px 之间"
        ));
    }
    if !(MIN_LAUNCHER_HORIZONTAL_OFFSET_PX..=MAX_LAUNCHER_HORIZONTAL_OFFSET_PX)
        .contains(&config.horizontal_offset_px)
    {
        return Err(format!(
            "启动器水平偏移必须在 {MIN_LAUNCHER_HORIZONTAL_OFFSET_PX}–{MAX_LAUNCHER_HORIZONTAL_OFFSET_PX} px 之间"
        ));
    }
    if !(MIN_LAUNCHER_VERTICAL_OFFSET_PX..=MAX_LAUNCHER_VERTICAL_OFFSET_PX)
        .contains(&config.vertical_offset_px)
    {
        return Err(format!(
            "启动器垂直偏移必须在 {MIN_LAUNCHER_VERTICAL_OFFSET_PX}–{MAX_LAUNCHER_VERTICAL_OFFSET_PX} px 之间"
        ));
    }
    Ok(())
}

fn validate_unique_ids(config: &AppConfig) -> Result<(), String> {
    let mut script_ids = HashMap::<String, &str>::new();
    for command in &config.script_commands {
        if let Some(existing) = script_ids.insert(command.id.to_lowercase(), &command.name) {
            return Err(format!(
                "脚本 ID {} 同时属于 {} 和 {}",
                command.id, existing, command.name
            ));
        }
    }
    let mut web_ids = HashMap::<String, &str>::new();
    for search in &config.web_searches {
        if let Some(existing) = web_ids.insert(search.id.to_lowercase(), &search.name) {
            return Err(format!(
                "网络搜索 ID {} 同时属于 {} 和 {}",
                search.id, existing, search.name
            ));
        }
    }
    Ok(())
}

fn normalize_translation(config: &mut TranslationConfig) -> Result<(), String> {
    config.keyword = normalize_keyword(&config.keyword, "翻译命令")?;
    config.description = optional_trimmed(&config.description, "翻译服务说明", 200)?;
    config.aliases = normalize_aliases(&config.aliases, "翻译命令")?;
    config.region = config.region.trim().to_string();
    config.default_target_language =
        required_trimmed(&config.default_target_language, "默认目标语言", 24)?;
    config.chinese_target_language =
        required_trimmed(&config.chinese_target_language, "中文目标语言", 24)?;
    Ok(())
}

fn normalize_script(command: &mut ScriptCommandConfig) -> Result<(), String> {
    command.id = required_trimmed(&command.id, "脚本 ID", 80)?;
    command.name = required_trimmed(&command.name, "脚本名称", 80)?;
    command.keyword = normalize_keyword(&command.keyword, "脚本命令")?;
    command.description = optional_trimmed(&command.description, "脚本说明", 200)?;
    command.input_hint = optional_trimmed(&command.input_hint, "脚本空参数提示", 160)?;
    command.icon_data_url = command.icon_data_url.trim().to_string();
    validate_command_icon_data_url(&command.icon_data_url, &format!("脚本 {}", command.name))?;
    command.aliases = normalize_aliases(&command.aliases, "脚本命令")?;
    command.script_path = required_trimmed(&command.script_path, "脚本路径", 1_024)?;
    if command.script_path.starts_with("http://") || command.script_path.starts_with("https://") {
        return Err(format!("脚本 {} 不允许使用远程 URL", command.keyword));
    }
    if !(MIN_SCRIPT_DEBOUNCE_MS..=MAX_SCRIPT_DEBOUNCE_MS).contains(&command.debounce_ms) {
        return Err(format!(
            "脚本 {} 的执行延迟必须在 {MIN_SCRIPT_DEBOUNCE_MS}–{MAX_SCRIPT_DEBOUNCE_MS} ms 之间",
            command.keyword
        ));
    }
    if !(100..=60_000).contains(&command.timeout_ms) {
        return Err(format!(
            "脚本 {} 的超时必须在 100–60000 ms 之间",
            command.keyword
        ));
    }
    Ok(())
}

fn normalize_web_search(search: &mut WebSearchConfig) -> Result<(), String> {
    search.id = required_trimmed(&search.id, "网络搜索 ID", 80)?;
    search.name = required_trimmed(&search.name, "网络搜索名称", 80)?;
    search.keyword = normalize_keyword(&search.keyword, "网络搜索命令")?;
    search.description = optional_trimmed(&search.description, "网络搜索说明", 200)?;
    search.input_hint = optional_trimmed(&search.input_hint, "网络搜索空参数提示", 160)?;
    search.icon_data_url = search.icon_data_url.trim().to_string();
    validate_command_icon_data_url(&search.icon_data_url, &format!("网络搜索 {}", search.name))?;
    search.aliases = normalize_aliases(&search.aliases, "网络搜索命令")?;
    search.url_template = required_trimmed(&search.url_template, "网络搜索 URL 模板", 2_048)?;
    let sample = web_search::sample_url(&search.url_template)
        .map_err(|error| format!("网络搜索 {} 的 URL 无效：{error}", search.keyword))?;
    let parsed = tauri::Url::parse(&sample)
        .map_err(|error| format!("网络搜索 {} 的 URL 无效：{error}", search.keyword))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("网络搜索 {} 只允许 HTTP/HTTPS URL", search.keyword));
    }
    Ok(())
}

trait ThemeEntry {
    fn id(&self) -> &str;
    fn set_id(&mut self, id: String);
    fn name(&self) -> &str;
    fn set_name(&mut self, name: String);
}

impl ThemeEntry for LauncherCustomThemeConfig {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

impl ThemeEntry for SettingsCustomThemeConfig {
    fn id(&self) -> &str {
        &self.id
    }
    fn set_id(&mut self, id: String) {
        self.id = id;
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
}

fn normalize_theme_scope<T, F>(
    theme: &mut String,
    accent_color: &str,
    custom_themes: &mut [T],
    scope_label: &str,
    mut validate_theme: F,
) -> Result<(), String>
where
    T: ThemeEntry,
    F: FnMut(&T) -> Result<(), String>,
{
    *theme = required_trimmed(theme, &format!("{scope_label}当前主题"), 80)?;
    validate_hex_color(accent_color, &format!("{scope_label}强调色"))?;
    if custom_themes.len() > MAX_CUSTOM_THEMES {
        return Err(format!(
            "{scope_label}自定义皮肤最多允许 {MAX_CUSTOM_THEMES} 个"
        ));
    }

    let mut ids = HashMap::<String, String>::new();
    for custom_theme in custom_themes.iter_mut() {
        let id = required_trimmed(custom_theme.id(), "皮肤 ID", 64)?;
        if !id
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
        {
            return Err(format!("皮肤 {id} 的 ID 只能包含字母、数字、- 和 _"));
        }
        custom_theme.set_id(id);
        let name = required_trimmed(custom_theme.name(), "皮肤名称", 40)?;
        custom_theme.set_name(name);
        if let Some(existing) = ids.insert(
            custom_theme.id().to_ascii_lowercase(),
            custom_theme.name().to_string(),
        ) {
            return Err(format!(
                "{scope_label}皮肤 ID {} 同时属于 {} 和 {}",
                custom_theme.id(),
                existing,
                custom_theme.name()
            ));
        }
        validate_theme(custom_theme)?;
    }

    if !matches!(theme.as_str(), "midnight" | "paper" | "forest") {
        let selected = theme
            .strip_prefix("custom:")
            .ok_or_else(|| format!("{scope_label}主题必须是内置主题或 custom:<id>"))?;
        let canonical_id = custom_themes
            .iter()
            .find(|custom_theme| custom_theme.id().eq_ignore_ascii_case(selected))
            .map(|custom_theme| custom_theme.id().to_string())
            .ok_or_else(|| format!("{scope_label}当前自定义皮肤不存在：{selected}"))?;
        *theme = format!("custom:{canonical_id}");
    }
    Ok(())
}

fn normalize_launcher_theme(theme: &mut LauncherThemeConfig) -> Result<(), String> {
    normalize_theme_scope(
        &mut theme.theme,
        &theme.accent_color,
        &mut theme.custom_themes,
        "搜索皮肤",
        validate_launcher_custom_theme,
    )
}

fn normalize_settings_theme(theme: &mut SettingsThemeConfig) -> Result<(), String> {
    normalize_theme_scope(
        &mut theme.theme,
        &theme.accent_color,
        &mut theme.custom_themes,
        "设置皮肤",
        validate_settings_custom_theme,
    )
}

fn validate_launcher_custom_theme(theme: &LauncherCustomThemeConfig) -> Result<(), String> {
    for (value, label) in [
        (&theme.accent_color, "强调色"),
        (&theme.window_background, "窗口背景"),
        (&theme.window_border, "窗口边框"),
        (&theme.search_background, "搜索框背景"),
        (&theme.search_border, "搜索框边框"),
        (&theme.search_text_color, "搜索文字"),
        (&theme.normal_row_background, "普通结果背景"),
        (&theme.normal_primary_color, "普通主文字"),
        (&theme.normal_secondary_color, "普通次文字"),
        (&theme.selected_row_background, "选中结果背景"),
        (&theme.selected_primary_color, "选中主文字"),
        (&theme.selected_secondary_color, "选中次文字"),
    ] {
        validate_hex_color(value, &format!("搜索皮肤 {} 的{label}", theme.name))?;
    }
    validate_range(
        theme.window_border_width_px,
        0,
        4,
        &theme.name,
        "窗口边框宽度",
    )?;
    if !(620..=900).contains(&theme.window_width_px) {
        return Err(format!(
            "搜索皮肤 {} 的窗口宽度必须在 620–900 px 之间",
            theme.name
        ));
    }
    validate_range(theme.window_radius_px, 0, 28, &theme.name, "窗口圆角")?;
    validate_range(
        theme.search_border_width_px,
        0,
        4,
        &theme.name,
        "搜索框边框宽度",
    )?;
    if !matches!(
        theme.search_border_style.as_str(),
        "solid" | "dashed" | "dotted" | "double" | "none"
    ) {
        return Err(format!("搜索皮肤 {} 的搜索框边框样式无效", theme.name));
    }
    if !(320..=900).contains(&theme.search_width_px)
        || theme.search_width_px > theme.window_width_px
    {
        return Err(format!(
            "搜索皮肤 {} 的搜索框宽度必须在 320–窗口宽度 px 之间",
            theme.name
        ));
    }
    validate_range(
        theme.search_font_size_px,
        12,
        24,
        &theme.name,
        "搜索文字大小",
    )?;
    validate_range(
        theme.normal_primary_font_size_px,
        12,
        20,
        &theme.name,
        "普通主文字大小",
    )?;
    validate_range(
        theme.normal_secondary_font_size_px,
        10,
        18,
        &theme.name,
        "普通次文字大小",
    )?;
    validate_range(
        theme.normal_row_height_px,
        44,
        84,
        &theme.name,
        "普通结果行高",
    )?;
    validate_range(
        theme.selected_primary_font_size_px,
        12,
        20,
        &theme.name,
        "选中主文字大小",
    )?;
    validate_range(
        theme.selected_secondary_font_size_px,
        10,
        18,
        &theme.name,
        "选中次文字大小",
    )?;
    validate_range(theme.icon_size_px, 16, 64, &theme.name, "图标尺寸")?;
    if !matches!(theme.max_results, 6 | 8 | 10 | 12) {
        return Err(format!(
            "搜索皮肤 {} 最多只能显示 6、8、10 或 12 项结果",
            theme.name
        ));
    }
    validate_background(&theme.background, &theme.name)
}

fn validate_settings_custom_theme(theme: &SettingsCustomThemeConfig) -> Result<(), String> {
    for (value, label) in [
        (&theme.accent_color, "强调色"),
        (&theme.window_background, "窗口背景"),
        (&theme.titlebar_background, "标题栏背景"),
        (&theme.sidebar_background, "侧栏背景"),
        (&theme.content_background, "内容背景"),
        (&theme.card_background, "卡片背景"),
        (&theme.border_color, "边框"),
        (&theme.primary_text_color, "主文字"),
        (&theme.secondary_text_color, "次文字"),
        (&theme.nav_text_color, "导航文字"),
        (&theme.selected_nav_background, "选中导航背景"),
    ] {
        validate_hex_color(value, &format!("设置皮肤 {} 的{label}", theme.name))?;
    }
    validate_range(theme.base_font_size_px, 12, 20, &theme.name, "基础字体大小")?;
    validate_range(theme.radius_px, 0, 28, &theme.name, "圆角")?;
    validate_background(&theme.background, &theme.name)
}

#[allow(dead_code)]
const LAUNCHER_THEME_BUNDLE_SCHEMA: &str = "suo-launcher-theme-v1";
#[allow(dead_code)]
const SETTINGS_THEME_BUNDLE_SCHEMA: &str = "suo-settings-theme-v1";

#[allow(dead_code)]
const LAUNCHER_THEME_BUNDLE_FIELDS: &[&str] = &[
    "name",
    "accentColor",
    "windowBackground",
    "windowBorder",
    "windowBorderWidthPx",
    "windowWidthPx",
    "windowRadiusPx",
    "searchBackground",
    "searchBorder",
    "searchBorderWidthPx",
    "searchBorderStyle",
    "searchWidthPx",
    "searchTextColor",
    "searchFontSizePx",
    "normalRowBackground",
    "normalPrimaryColor",
    "normalSecondaryColor",
    "normalPrimaryFontSizePx",
    "normalSecondaryFontSizePx",
    "normalRowHeightPx",
    "selectedRowBackground",
    "selectedPrimaryColor",
    "selectedSecondaryColor",
    "selectedPrimaryFontSizePx",
    "selectedSecondaryFontSizePx",
    "iconSizePx",
    "showSearchIcon",
    "showLogo",
    "showSourceBadge",
    "maxResults",
    "windowOpacity",
    "blurPx",
    "shadowPercent",
    "wallpaperDataUrl",
    "wallpaperOpacity",
    "platformOverrides",
];

#[allow(dead_code)]
const SETTINGS_THEME_BUNDLE_FIELDS: &[&str] = &[
    "name",
    "accentColor",
    "windowBackground",
    "titlebarBackground",
    "sidebarBackground",
    "contentBackground",
    "cardBackground",
    "borderColor",
    "primaryTextColor",
    "secondaryTextColor",
    "navTextColor",
    "selectedNavBackground",
    "baseFontSizePx",
    "radiusPx",
    "windowOpacity",
    "blurPx",
    "shadowPercent",
    "wallpaperDataUrl",
    "wallpaperOpacity",
    "platformOverrides",
];

#[allow(dead_code)]
const PLATFORM_OVERRIDE_FIELDS: &[&str] = &[
    "enabled",
    "windowsBlurPx",
    "windowsOpacity",
    "macosBlurPx",
    "macosOpacity",
];

/// Parses one strict launcher-scope bundle. This deliberately does not accept
/// the retired `suo-theme-v1` schema or a settings bundle.
#[allow(dead_code)]
pub fn parse_launcher_theme_bundle(value: &str) -> Result<LauncherCustomThemeConfig, String> {
    let mut payload = parse_theme_bundle_payload(
        value,
        LAUNCHER_THEME_BUNDLE_SCHEMA,
        LAUNCHER_THEME_BUNDLE_FIELDS,
    )?;
    payload.insert(
        "id".into(),
        serde_json::Value::String("imported-launcher-theme".into()),
    );
    let theme =
        serde_json::from_value::<LauncherCustomThemeConfig>(serde_json::Value::Object(payload))
            .map_err(|error| format!("搜索皮肤导入字段无效：{error}"))?;
    validate_launcher_custom_theme(&theme)?;
    Ok(theme)
}

/// Parses one strict settings-scope bundle. A launcher bundle is not a valid
/// settings import even when every overlapping token happens to be valid.
#[allow(dead_code)]
pub fn parse_settings_theme_bundle(value: &str) -> Result<SettingsCustomThemeConfig, String> {
    let mut payload = parse_theme_bundle_payload(
        value,
        SETTINGS_THEME_BUNDLE_SCHEMA,
        SETTINGS_THEME_BUNDLE_FIELDS,
    )?;
    payload.insert(
        "id".into(),
        serde_json::Value::String("imported-settings-theme".into()),
    );
    let theme =
        serde_json::from_value::<SettingsCustomThemeConfig>(serde_json::Value::Object(payload))
            .map_err(|error| format!("设置皮肤导入字段无效：{error}"))?;
    validate_settings_custom_theme(&theme)?;
    Ok(theme)
}

#[allow(dead_code)]
pub fn build_launcher_theme_bundle(
    theme: &LauncherCustomThemeConfig,
) -> Result<serde_json::Value, String> {
    validate_launcher_custom_theme(theme)?;
    build_theme_bundle(LAUNCHER_THEME_BUNDLE_SCHEMA, theme)
}

#[allow(dead_code)]
pub fn build_settings_theme_bundle(
    theme: &SettingsCustomThemeConfig,
) -> Result<serde_json::Value, String> {
    validate_settings_custom_theme(theme)?;
    build_theme_bundle(SETTINGS_THEME_BUNDLE_SCHEMA, theme)
}

#[allow(dead_code)]
fn parse_theme_bundle_payload(
    value: &str,
    expected_schema: &str,
    expected_theme_fields: &[&str],
) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let value = serde_json::from_str::<serde_json::Value>(value)
        .map_err(|error| format!("皮肤导入 JSON 无效：{error}"))?;
    let root = value
        .as_object()
        .ok_or_else(|| "皮肤导入必须是 JSON 对象".to_string())?;
    validate_exact_object_fields(root, &["schema", "version", "theme"], "皮肤导入")?;
    if root.get("schema").and_then(serde_json::Value::as_str) != Some(expected_schema) {
        return Err(format!("皮肤导入 scope 不匹配；只接受 {expected_schema}"));
    }
    if root.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err("皮肤导入版本必须是 v1".into());
    }
    let theme = root
        .get("theme")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "皮肤导入 theme 必须是对象".to_string())?;
    validate_exact_object_fields(theme, expected_theme_fields, "皮肤导入 theme")?;
    let overrides = theme
        .get("platformOverrides")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "皮肤导入 platformOverrides 必须是对象".to_string())?;
    validate_exact_object_fields(
        overrides,
        PLATFORM_OVERRIDE_FIELDS,
        "皮肤导入 platformOverrides",
    )?;
    Ok(theme.clone())
}

#[allow(dead_code)]
fn validate_exact_object_fields(
    value: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
    label: &str,
) -> Result<(), String> {
    if value.len() != expected.len()
        || expected.iter().any(|field| !value.contains_key(*field))
        || value
            .keys()
            .any(|field| !expected.contains(&field.as_str()))
    {
        return Err(format!("{label} 字段必须完整且不能包含未知字段"));
    }
    Ok(())
}

#[allow(dead_code)]
fn build_theme_bundle<T: Serialize>(schema: &str, theme: &T) -> Result<serde_json::Value, String> {
    let mut theme = serde_json::to_value(theme)
        .map_err(|error| format!("无法序列化皮肤：{error}"))?
        .as_object()
        .cloned()
        .ok_or_else(|| "皮肤必须是对象".to_string())?;
    theme.remove("id");
    Ok(serde_json::json!({
        "schema": schema,
        "version": 1,
        "theme": theme,
    }))
}

fn validate_background(background: &ThemeBackgroundConfig, name: &str) -> Result<(), String> {
    validate_range(background.window_opacity, 70, 100, name, "窗口透明度")?;
    validate_range(background.blur_px, 0, 40, name, "背景模糊")?;
    validate_range(background.shadow_percent, 0, 80, name, "阴影强度")?;
    validate_range(background.wallpaper_opacity, 0, 60, name, "背景图强度")?;
    validate_range(
        background.platform_overrides.windows_blur_px,
        0,
        40,
        name,
        "Windows 模糊",
    )?;
    validate_range(
        background.platform_overrides.windows_opacity,
        70,
        100,
        name,
        "Windows 透明度",
    )?;
    validate_range(
        background.platform_overrides.macos_blur_px,
        0,
        40,
        name,
        "macOS 模糊",
    )?;
    validate_range(
        background.platform_overrides.macos_opacity,
        70,
        100,
        name,
        "macOS 透明度",
    )?;
    validate_wallpaper_data_url(&background.wallpaper_data_url, name)
}

fn validate_hex_color(value: &str, label: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err(format!("{label}必须是 #RRGGBB 格式"));
    }
    Ok(())
}

fn validate_range<T>(
    value: T,
    minimum: T,
    maximum: T,
    name: &str,
    label: &str,
) -> Result<(), String>
where
    T: PartialOrd + std::fmt::Display,
{
    if value < minimum || value > maximum {
        return Err(format!(
            "皮肤 {name} 的{label}必须在 {minimum}–{maximum} 之间"
        ));
    }
    Ok(())
}

fn validate_wallpaper_data_url(value: &str, name: &str) -> Result<(), String> {
    validate_image_data_url(
        value,
        &format!("皮肤 {name} 的背景图"),
        MAX_THEME_WALLPAPER_BYTES,
        "1.5 MB",
        MAX_THEME_WALLPAPER_DIMENSION,
        MAX_THEME_WALLPAPER_PIXELS,
        MAX_THEME_WALLPAPER_ALLOCATION_BYTES,
    )
}

fn validate_command_icon_data_url(value: &str, owner: &str) -> Result<(), String> {
    validate_image_data_url(
        value,
        &format!("{owner} 的图标"),
        MAX_COMMAND_ICON_BYTES,
        "256 KB",
        MAX_COMMAND_ICON_DIMENSION,
        MAX_COMMAND_ICON_PIXELS,
        MAX_COMMAND_ICON_ALLOCATION_BYTES,
    )
}

fn validate_image_data_url(
    value: &str,
    label: &str,
    max_bytes: usize,
    max_size_label: &str,
    max_dimension: u32,
    max_pixels: u64,
    max_allocation_bytes: u64,
) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    let (mime_type, payload) = [
        ("image/png", "data:image/png;base64,"),
        ("image/jpeg", "data:image/jpeg;base64,"),
        ("image/webp", "data:image/webp;base64,"),
    ]
    .iter()
    .find_map(|(mime_type, prefix)| {
        value
            .strip_prefix(prefix)
            .map(|payload| (*mime_type, payload))
    })
    .ok_or_else(|| format!("{label}只允许 PNG、JPEG 或 WebP"))?;
    if payload.is_empty() || payload.len() % 4 != 0 {
        return Err(format!("{label}数据无效"));
    }
    let padding = if payload.ends_with("==") {
        2
    } else if payload.ends_with('=') {
        1
    } else {
        0
    };
    let content_len = payload.len() - padding;
    if !payload[..content_len]
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
        || !payload[content_len..].bytes().all(|byte| byte == b'=')
    {
        return Err(format!("{label}数据无效"));
    }
    let last_sextet = payload[..content_len]
        .bytes()
        .next_back()
        .and_then(base64_sextet)
        .ok_or_else(|| format!("{label}数据无效"))?;
    if (padding == 2 && last_sextet & 0x0f != 0) || (padding == 1 && last_sextet & 0x03 != 0) {
        return Err(format!("{label}数据无效"));
    }
    let decoded_bytes = payload
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| bytes.checked_sub(padding))
        .ok_or_else(|| format!("{label}数据无效"))?;
    if decoded_bytes > max_bytes {
        return Err(format!("{label}不能超过 {max_size_label}"));
    }
    let decoded =
        decode_base64(payload, decoded_bytes).ok_or_else(|| format!("{label}数据无效"))?;
    let is_complete_image = match mime_type {
        "image/png" => validate_png(&decoded),
        "image/jpeg" => validate_jpeg(&decoded),
        "image/webp" => validate_webp(&decoded),
        _ => false,
    };
    if !is_complete_image
        || !decode_image_with_limits(
            &decoded,
            mime_type,
            max_dimension,
            max_pixels,
            max_allocation_bytes,
        )
    {
        return Err(format!(
            "{label}不是完整且尺寸合规的 PNG、JPEG 或 WebP 文件"
        ));
    }
    Ok(())
}

/// Decodes a payload which has already passed the strict alphabet, padding and
/// canonical-tail checks above. Keeping this decoder local avoids accepting a
/// browser/runtime-specific forgiving Base64 variant.
fn decode_base64(payload: &str, expected_len: usize) -> Option<Vec<u8>> {
    let mut result = Vec::with_capacity(expected_len);
    for group in payload.as_bytes().chunks(4) {
        if group.len() != 4 {
            return None;
        }
        let first = base64_sextet(group[0])?;
        let second = base64_sextet(group[1])?;
        result.push((first << 2) | (second >> 4));
        if group[2] == b'=' {
            continue;
        }
        let third = base64_sextet(group[2])?;
        result.push((second << 4) | (third >> 2));
        if group[3] == b'=' {
            continue;
        }
        let fourth = base64_sextet(group[3])?;
        result.push((third << 6) | fourth);
    }
    (result.len() == expected_len).then_some(result)
}

fn base64_sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Container checks above reject malformed framing before the decoder runs.
/// Decode with explicit image and allocation limits so a tiny compressed input
/// cannot expand into an unbounded allocation.
fn decode_image_with_limits(
    bytes: &[u8],
    mime_type: &str,
    max_dimension: u32,
    max_pixels: u64,
    max_allocation_bytes: u64,
) -> bool {
    let format = match mime_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/webp" => ImageFormat::WebP,
        _ => return false,
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(max_dimension);
    limits.max_image_height = Some(max_dimension);
    limits.max_alloc = Some(max_allocation_bytes);

    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let Ok(image) = reader.decode() else {
        return false;
    };
    let (width, height) = image.dimensions();
    width > 0
        && height > 0
        && width <= max_dimension
        && height <= max_dimension
        && u64::from(width) * u64::from(height) <= max_pixels
}

fn validate_png(bytes: &[u8]) -> bool {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if !bytes.starts_with(SIGNATURE) {
        return false;
    }

    let mut position = SIGNATURE.len();
    let mut saw_ihdr = false;
    let mut saw_plte = false;
    let mut saw_idat = false;
    let mut left_idat = false;
    let mut indexed_colour = false;

    while position < bytes.len() {
        let Some(header_end) = position.checked_add(8) else {
            return false;
        };
        if header_end > bytes.len() {
            return false;
        }
        let length = usize::try_from(u32::from_be_bytes(
            bytes[position..position + 4].try_into().unwrap_or_default(),
        ))
        .ok();
        let Some(length) = length else {
            return false;
        };
        let data_start = header_end;
        let Some(data_end) = data_start.checked_add(length) else {
            return false;
        };
        let Some(chunk_end) = data_end.checked_add(4) else {
            return false;
        };
        if chunk_end > bytes.len() {
            return false;
        }
        let kind = &bytes[position + 4..header_end];
        let data = &bytes[data_start..data_end];
        let expected_crc =
            u32::from_be_bytes(bytes[data_end..chunk_end].try_into().unwrap_or_default());
        if png_crc32(&bytes[position + 4..data_end]) != expected_crc {
            return false;
        }

        if !saw_ihdr {
            if kind != b"IHDR" || !validate_png_ihdr(data) {
                return false;
            }
            indexed_colour = data[9] == 3;
            saw_ihdr = true;
        } else if kind == b"IHDR" {
            return false;
        } else if kind == b"PLTE" {
            if saw_idat || data.is_empty() || data.len() % 3 != 0 || data.len() > 768 {
                return false;
            }
            saw_plte = true;
        } else if kind == b"IDAT" {
            if left_idat || data.is_empty() {
                return false;
            }
            saw_idat = true;
        } else if kind == b"IEND" {
            return data.is_empty()
                && saw_idat
                && (!indexed_colour || saw_plte)
                && chunk_end == bytes.len();
        } else if saw_idat {
            left_idat = true;
        }
        position = chunk_end;
    }
    false
}

fn validate_png_ihdr(data: &[u8]) -> bool {
    if data.len() != 13 {
        return false;
    }
    let width = u32::from_be_bytes(data[0..4].try_into().unwrap_or_default());
    let height = u32::from_be_bytes(data[4..8].try_into().unwrap_or_default());
    if width == 0 || height == 0 || data[10] != 0 || data[11] != 0 || !matches!(data[12], 0 | 1) {
        return false;
    }
    matches!(
        (data[8], data[9]),
        (1 | 2 | 4 | 8 | 16, 0) | (8 | 16, 2 | 4 | 6) | (1 | 2 | 4 | 8, 3)
    )
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 0 {
                crc >> 1
            } else {
                (crc >> 1) ^ 0xedb8_8320
            };
        }
    }
    !crc
}

fn validate_jpeg(bytes: &[u8]) -> bool {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return false;
    }
    let mut position = 2;
    let mut saw_frame = false;
    let mut saw_scan = false;

    while position < bytes.len() {
        let Some((marker, marker_end)) = jpeg_marker(bytes, position) else {
            return false;
        };
        position = marker_end;
        match marker {
            0xd9 => return saw_frame && saw_scan && position == bytes.len(),
            0xd8 => return false,
            0x01 | 0xd0..=0xd7 => continue,
            _ => {}
        }

        let Some(length_end) = position.checked_add(2) else {
            return false;
        };
        if length_end > bytes.len() {
            return false;
        }
        let length = usize::from(u16::from_be_bytes(
            bytes[position..length_end].try_into().unwrap_or_default(),
        ));
        if length < 2 {
            return false;
        }
        let Some(segment_end) = position.checked_add(length) else {
            return false;
        };
        if segment_end > bytes.len() {
            return false;
        }
        let segment = &bytes[length_end..segment_end];
        if is_jpeg_frame_marker(marker) {
            if !validate_jpeg_frame(segment) {
                return false;
            }
            saw_frame = true;
        }
        if marker == 0xda {
            if !saw_frame || !validate_jpeg_scan_header(segment) {
                return false;
            }
            saw_scan = true;
            match jpeg_scan_end(bytes, segment_end) {
                Some(JpegScanEnd::End) => return true,
                Some(JpegScanEnd::NextMarker(next)) => position = next,
                None => return false,
            }
        } else {
            position = segment_end;
        }
    }
    false
}

fn jpeg_marker(bytes: &[u8], position: usize) -> Option<(u8, usize)> {
    if bytes.get(position) != Some(&0xff) {
        return None;
    }
    let mut marker_end = position + 1;
    while bytes.get(marker_end) == Some(&0xff) {
        marker_end += 1;
    }
    let marker = *bytes.get(marker_end)?;
    (marker != 0).then_some((marker, marker_end + 1))
}

enum JpegScanEnd {
    End,
    NextMarker(usize),
}

fn jpeg_scan_end(bytes: &[u8], mut position: usize) -> Option<JpegScanEnd> {
    while position < bytes.len() {
        if bytes[position] != 0xff {
            position += 1;
            continue;
        }
        let marker_start = position;
        position += 1;
        while bytes.get(position) == Some(&0xff) {
            position += 1;
        }
        let marker = *bytes.get(position)?;
        if marker == 0 {
            position += 1;
            continue;
        }
        if matches!(marker, 0xd0..=0xd7) {
            position += 1;
            continue;
        }
        if marker == 0xd9 {
            return (position + 1 == bytes.len()).then_some(JpegScanEnd::End);
        }
        return Some(JpegScanEnd::NextMarker(marker_start));
    }
    None
}

fn is_jpeg_frame_marker(marker: u8) -> bool {
    matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf)
}

fn validate_jpeg_frame(segment: &[u8]) -> bool {
    if segment.len() < 6 {
        return false;
    }
    let components = usize::from(segment[5]);
    let height = u16::from_be_bytes([segment[1], segment[2]]);
    let width = u16::from_be_bytes([segment[3], segment[4]]);
    components > 0 && height > 0 && width > 0 && segment.len() == 6 + components.saturating_mul(3)
}

fn validate_jpeg_scan_header(segment: &[u8]) -> bool {
    let Some(components) = segment.first().copied().map(usize::from) else {
        return false;
    };
    components > 0 && segment.len() == 4 + components.saturating_mul(2)
}

fn validate_webp(bytes: &[u8]) -> bool {
    if bytes.len() < 20 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return false;
    }
    let declared = usize::try_from(u32::from_le_bytes(
        bytes[4..8].try_into().unwrap_or_default(),
    ))
    .ok();
    let Some(declared) = declared else {
        return false;
    };
    if declared.checked_add(8) != Some(bytes.len()) {
        return false;
    }
    validate_webp_chunks(&bytes[12..])
}

fn validate_webp_chunks(bytes: &[u8]) -> bool {
    let mut position = 0;
    let mut saw_image = false;
    while position < bytes.len() {
        let Some(header_end) = position.checked_add(8) else {
            return false;
        };
        if header_end > bytes.len() {
            return false;
        }
        let length = usize::try_from(u32::from_le_bytes(
            bytes[position + 4..header_end]
                .try_into()
                .unwrap_or_default(),
        ))
        .ok();
        let Some(length) = length else {
            return false;
        };
        let Some(data_end) = header_end.checked_add(length) else {
            return false;
        };
        let Some(next) = data_end.checked_add(length % 2) else {
            return false;
        };
        if next > bytes.len() {
            return false;
        }
        let kind = &bytes[position..position + 4];
        let data = &bytes[header_end..data_end];
        let valid_chunk = match kind {
            b"VP8 " => validate_webp_vp8(data),
            b"VP8L" => validate_webp_vp8l(data),
            b"VP8X" => validate_webp_vp8x(data),
            b"ANMF" => data.len() >= 16 && validate_webp_chunks(&data[16..]),
            _ => true,
        };
        if !valid_chunk {
            return false;
        }
        if matches!(kind, b"VP8 " | b"VP8L" | b"ANMF") {
            saw_image = true;
        }
        position = next;
    }
    saw_image
}

fn validate_webp_vp8(data: &[u8]) -> bool {
    if data.len() < 10 || data[0] & 1 != 0 || data[3..6] != [0x9d, 0x01, 0x2a] {
        return false;
    }
    let width = u16::from_le_bytes([data[6], data[7]]) & 0x3fff;
    let height = u16::from_le_bytes([data[8], data[9]]) & 0x3fff;
    width > 0 && height > 0
}

fn validate_webp_vp8l(data: &[u8]) -> bool {
    if data.len() < 5 || data[0] != 0x2f {
        return false;
    }
    let dimensions = u32::from_le_bytes(data[1..5].try_into().unwrap_or_default());
    let width = (dimensions & 0x3fff) + 1;
    let height = ((dimensions >> 14) & 0x3fff) + 1;
    width > 0 && height > 0 && dimensions >> 29 == 0
}

fn validate_webp_vp8x(data: &[u8]) -> bool {
    if data.len() != 10 || data[0] & 0x01 != 0 || data[1..4] != [0, 0, 0] {
        return false;
    }
    let width = u32::from(data[4]) | (u32::from(data[5]) << 8) | (u32::from(data[6]) << 16);
    let height = u32::from(data[7]) | (u32::from(data[8]) << 8) | (u32::from(data[9]) << 16);
    width <= 0x00ff_ffff && height <= 0x00ff_ffff
}

fn validate_keyword_namespace(config: &AppConfig) -> Result<(), String> {
    let mut seen = HashMap::<String, String>::new();
    for reserved in ["f", "setting", "settings", "设置"] {
        seen.insert(reserved.into(), "系统命令".into());
    }
    register_keywords(
        &mut seen,
        "翻译命令",
        &config.translation.keyword,
        &config.translation.aliases,
    )?;
    for command in &config.script_commands {
        register_keywords(&mut seen, &command.name, &command.keyword, &command.aliases)?;
    }
    for search in &config.web_searches {
        register_keywords(&mut seen, &search.name, &search.keyword, &search.aliases)?;
    }
    Ok(())
}

fn register_keywords(
    seen: &mut HashMap<String, String>,
    owner: &str,
    keyword: &str,
    aliases: &[String],
) -> Result<(), String> {
    for value in std::iter::once(keyword).chain(aliases.iter().map(String::as_str)) {
        if let Some(existing) = seen.insert(value.to_lowercase(), owner.to_string()) {
            return Err(format!("命令关键字 {value} 同时属于 {existing} 和 {owner}"));
        }
    }
    Ok(())
}

fn normalize_aliases(values: &[String], owner: &str) -> Result<Vec<String>, String> {
    values
        .iter()
        .map(|value| normalize_keyword(value, owner))
        .collect()
}

fn normalize_keyword(value: &str, label: &str) -> Result<String, String> {
    let value = required_trimmed(value, label, 32)?.to_lowercase();
    if value.chars().any(char::is_whitespace) || value.contains(':') {
        return Err(format!("{label}不能包含空格或冒号"));
    }
    Ok(value)
}

fn required_trimmed(value: &str, label: &str, max: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{label}不能为空"));
    }
    if value.chars().count() > max {
        return Err(format!("{label}不能超过 {max} 个字符"));
    }
    Ok(value.to_string())
}

fn optional_trimmed(value: &str, label: &str, max: usize) -> Result<String, String> {
    let value = value.trim();
    // HTML maxlength uses UTF-16 code units. Match it here so emoji and
    // combining characters have the same boundary in the UI and backend.
    if value.encode_utf16().count() > max {
        return Err(format!("{label}不能超过 {max} 个字符"));
    }
    Ok(value.to_string())
}

fn credential_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(CREDENTIAL_SERVICE, TRANSLATOR_CREDENTIAL)
        .map_err(|error| format!("无法访问系统凭据库：{error}"))
}

pub fn read_translation_api_key() -> Result<Option<String>, String> {
    match credential_entry()?.get_password() {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("无法读取翻译密钥：{error}")),
    }
}

#[tauri::command]
pub fn get_app_config(state: State<'_, Arc<ConfigState>>) -> AppConfigView {
    state.view()
}

#[tauri::command]
pub fn open_config_directory(state: State<'_, Arc<ConfigState>>) -> Result<(), String> {
    let path = state.config_path()?;
    let directory = path
        .parent()
        .ok_or_else(|| "配置文件没有可打开的父目录".to_string())?;
    fs::create_dir_all(directory).map_err(|error| format!("无法创建配置目录：{error}"))?;
    open::that_detached(directory)
        .map_err(|error| format!("无法打开配置目录 {}：{error}", directory.display()))
}

#[tauri::command]
pub fn change_config_directory(
    state: State<'_, Arc<ConfigState>>,
    directory: String,
) -> Result<AppConfigView, String> {
    let directory = directory.trim();
    if directory.is_empty() {
        return Err("配置目录不能为空".into());
    }
    state.relocate(Path::new(directory))?;
    Ok(state.view())
}

#[tauri::command]
pub fn save_app_config(
    app: AppHandle,
    state: State<'_, Arc<ConfigState>>,
    launcher: State<'_, Arc<LauncherState>>,
    config: AppConfig,
) -> Result<AppConfigView, String> {
    // Register the requested shortcut before persisting it. A conflict must
    // leave both the on-disk configuration and the working shortcut intact.
    let normalized = normalize_and_validate(config)?;
    let current = state.snapshot();
    let shortcut_change = hotkey::RegisteredShortcutChange::apply(
        &app,
        &current.launcher.global_hotkey,
        &normalized.launcher.global_hotkey,
    )?;
    let shortcut_changed = shortcut_change.is_some();
    let (previous, config) = match state.replace(normalized) {
        Ok(result) => result,
        Err(save_error) => {
            if let Some(change) = shortcut_change {
                if let Err(rollback_error) = change.rollback(&app) {
                    return Err(format!("{save_error}；{rollback_error}"));
                }
            }
            return Err(save_error);
        }
    };
    let providers_changed = provider_settings_changed(&previous, &config);
    dock::preference_changed(&app, config.launcher.show_dock_icon)?;
    launcher.update_preferences(
        config.launcher.close_on_blur,
        config.launcher.keep_last_input,
    );
    if shortcut_changed {
        let label = hotkey::shortcut_label(&config.launcher.global_hotkey)
            .unwrap_or_else(|_| config.launcher.global_hotkey.clone());
        launcher.set_hotkey_status(format!("{label} 已就绪"));
    }
    if providers_changed {
        launcher.invalidate_provider_results();
    }
    app.emit("app-config-updated", &config)
        .map_err(|error| format!("配置已保存，但无法通知窗口：{error}"))?;
    if providers_changed {
        app.emit("provider-config-updated", ())
            .map_err(|error| format!("配置已保存，但无法刷新搜索结果：{error}"))?;
    }
    Ok(state.view())
}

fn provider_settings_changed(previous: &AppConfig, next: &AppConfig) -> bool {
    previous.translation != next.translation
        || previous.script_commands != next.script_commands
        || previous.web_searches != next.web_searches
}

#[tauri::command]
pub fn set_translation_api_key(
    app: AppHandle,
    state: State<'_, Arc<ConfigState>>,
    launcher: State<'_, Arc<LauncherState>>,
    api_key: String,
) -> Result<AppConfigView, String> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("翻译 API 密钥不能为空".into());
    }
    credential_entry()?
        .set_password(api_key)
        .map_err(|error| format!("无法保存翻译密钥：{error}"))?;
    launcher.invalidate_provider_results();
    app.emit("provider-config-updated", ())
        .map_err(|error| format!("密钥已保存，但无法刷新翻译结果：{error}"))?;
    Ok(state.view())
}

#[tauri::command]
pub fn clear_translation_api_key(
    app: AppHandle,
    state: State<'_, Arc<ConfigState>>,
    launcher: State<'_, Arc<LauncherState>>,
) -> Result<AppConfigView, String> {
    match credential_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {
            launcher.invalidate_provider_results();
            app.emit("provider-config-updated", ())
                .map_err(|error| format!("密钥已删除，但无法刷新翻译结果：{error}"))?;
            Ok(state.view())
        }
        Err(error) => Err(format!("无法删除翻译密钥：{error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_config_path(label: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("suo-config-{label}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&directory).expect("create temporary config directory");
        directory.join("config.json")
    }

    fn remove_temporary_config(path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }

    fn config_state_at(path: &Path) -> ConfigState {
        ConfigState {
            path: RwLock::new(path.to_path_buf()),
            default_path: path.to_path_buf(),
            location_path: path
                .parent()
                .expect("temporary config parent")
                .join(CONFIG_LOCATION_FILE_NAME),
            config: RwLock::new(AppConfig::default()),
            load_warning: RwLock::new(None),
            needs_legacy_preferences_migration: RwLock::new(false),
            save_lock: Mutex::new(()),
            incompatible_newer_version: None,
        }
    }

    fn theme_background() -> ThemeBackgroundConfig {
        ThemeBackgroundConfig {
            window_opacity: 94,
            blur_px: 18,
            shadow_percent: 45,
            wallpaper_data_url: String::new(),
            wallpaper_opacity: 18,
            platform_overrides: PlatformThemeOverrides::default(),
        }
    }

    fn launcher_custom_theme(id: &str) -> LauncherCustomThemeConfig {
        LauncherCustomThemeConfig {
            id: id.into(),
            name: "测试皮肤".into(),
            accent_color: "#8a78ff".into(),
            window_background: "#0b1222".into(),
            window_border: "#343d5a".into(),
            window_border_width_px: 1,
            window_width_px: 720,
            window_radius_px: 18,
            search_background: "#161f39".into(),
            search_border: "#343d5a".into(),
            search_border_width_px: 1,
            search_border_style: "solid".into(),
            search_width_px: 720,
            search_text_color: "#f5f7ff".into(),
            search_font_size_px: 20,
            normal_row_background: "#101a30".into(),
            normal_primary_color: "#f5f7ff".into(),
            normal_secondary_color: "#91a0c7".into(),
            normal_primary_font_size_px: 14,
            normal_secondary_font_size_px: 12,
            normal_row_height_px: 58,
            selected_row_background: "#302b63".into(),
            selected_primary_color: "#f5f7ff".into(),
            selected_secondary_color: "#91a0c7".into(),
            selected_primary_font_size_px: 14,
            selected_secondary_font_size_px: 12,
            icon_size_px: 36,
            show_search_icon: true,
            show_logo: false,
            show_source_badge: true,
            max_results: 8,
            background: theme_background(),
        }
    }

    fn settings_custom_theme(id: &str) -> SettingsCustomThemeConfig {
        SettingsCustomThemeConfig {
            id: id.into(),
            name: "设置皮肤".into(),
            accent_color: "#8a78ff".into(),
            window_background: "#0b1222".into(),
            titlebar_background: "#101a30".into(),
            sidebar_background: "#161f39".into(),
            content_background: "#0b1222".into(),
            card_background: "#101a30".into(),
            border_color: "#343d5a".into(),
            primary_text_color: "#f5f7ff".into(),
            secondary_text_color: "#91a0c7".into(),
            nav_text_color: "#f5f7ff".into(),
            selected_nav_background: "#302b63".into(),
            base_font_size_px: 14,
            radius_px: 18,
            background: theme_background(),
        }
    }

    fn legacy_config_value(version: u32) -> serde_json::Value {
        let mut config = serde_json::to_value(AppConfig::default()).expect("serialize defaults");
        let object = config.as_object_mut().expect("config object");
        object.remove("saveSettingsManually");
        object.remove("launcherTheme");
        object.remove("settingsTheme");
        object.insert("version".into(), serde_json::json!(version));
        object.insert(
            "appearance".into(),
            serde_json::json!({
                "theme": "midnight",
                "accentColor": "#8a78ff",
                "customThemes": [],
            }),
        );
        let launcher = config["launcher"].as_object_mut().expect("launcher object");
        launcher.remove("globalHotkey");
        launcher.remove("emptyQueryDebounceMs");
        launcher.remove("nonEmptyQueryDebounceMs");
        launcher.remove("showDockIcon");
        launcher.remove("windowWidthPx");
        launcher.remove("windowHeightPx");
        launcher.remove("horizontalOffsetPx");
        launcher.remove("verticalOffsetPx");
        config
    }

    fn legacy_custom_theme_value(id: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": "旧版皮肤",
            "windowColor": "#0b1222",
            "panelColor": "#101a30",
            "fieldColor": "#161f39",
            "textColor": "#f5f7ff",
            "mutedColor": "#91a0c7",
            "accentColor": "#8a78ff",
            "selectionColor": "#302b63",
            "borderColor": "#343d5a",
            "windowOpacity": 94,
            "blurPx": 18,
            "shadowPercent": 45,
            "wallpaperDataUrl": "",
            "wallpaperOpacity": 18,
            "radiusPx": 18,
            "fontFamily": "system",
            "fontSizePx": 14,
            "launcherWidthPx": 720,
            "resultDensity": "comfortable",
            "maxResults": 8,
            "iconSizePx": 36,
            "showSourceBadge": true,
            "platformOverrides": {
                "enabled": false,
                "windowsBlurPx": 18,
                "windowsOpacity": 94,
                "macosBlurPx": 18,
                "macosOpacity": 94,
            },
        })
    }

    fn base64_encode(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::with_capacity(4 * ((bytes.len() + 2) / 3));
        for chunk in bytes.chunks(3) {
            let a = chunk[0];
            let b = *chunk.get(1).unwrap_or(&0);
            let c = *chunk.get(2).unwrap_or(&0);
            result.push(TABLE[(a >> 2) as usize] as char);
            result.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
            result.push(if chunk.len() > 1 {
                TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
            } else {
                '='
            });
            result.push(if chunk.len() > 2 {
                TABLE[(c & 0x3f) as usize] as char
            } else {
                '='
            });
        }
        result
    }

    fn wallpaper_data_url(mime_type: &str, bytes: &[u8]) -> String {
        format!("data:{mime_type};base64,{}", base64_encode(bytes))
    }

    fn append_png_chunk(bytes: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        bytes.extend_from_slice(
            &u32::try_from(data.len())
                .expect("PNG chunk length")
                .to_be_bytes(),
        );
        let crc_start = bytes.len();
        bytes.extend_from_slice(kind);
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(&png_crc32(&bytes[crc_start..]).to_be_bytes());
    }

    fn valid_png_bytes(total_len: usize) -> Vec<u8> {
        // A real 1×1 RGBA PNG. For the size-limit test, an ancillary tEXt
        // chunk makes the file exact-sized without changing the image payload.
        const BASE_LEN: usize = 73;
        assert!(total_len == BASE_LEN || total_len >= BASE_LEN + 14);
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        append_png_chunk(
            &mut bytes,
            b"IHDR",
            &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0],
        );
        if total_len > BASE_LEN {
            let mut text = vec![b'a'; total_len - BASE_LEN - 12];
            text[0] = b'x';
            text[1] = 0;
            append_png_chunk(&mut bytes, b"tEXt", &text);
        }
        // zlib stream for the scanline [filter=0, R=0, G=0, B=0, A=255].
        append_png_chunk(
            &mut bytes,
            b"IDAT",
            &[
                0x78, 0x01, 0x01, 0x05, 0x00, 0xfa, 0xff, 0, 0, 0, 0, 0xff, 1, 4, 1, 0,
            ],
        );
        append_png_chunk(&mut bytes, b"IEND", &[]);
        assert_eq!(bytes.len(), total_len);
        bytes
    }

    fn encoded_image_bytes(format: ImageFormat) -> Vec<u8> {
        let image = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([24, 48, 96, 255]),
        ));
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, format)
            .expect("encode real test image");
        output.into_inner()
    }

    fn encoded_rgba16_png_bytes() -> Vec<u8> {
        let image = image::DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
            32,
            32,
            image::Rgba([u16::MAX, 0, 32_768, u16::MAX]),
        ));
        let mut output = Cursor::new(Vec::new());
        image
            .write_to(&mut output, ImageFormat::Png)
            .expect("encode real 16-bit PNG test image");
        output.into_inner()
    }

    fn valid_jpeg_bytes() -> Vec<u8> {
        encoded_image_bytes(ImageFormat::Jpeg)
    }

    fn valid_webp_bytes() -> Vec<u8> {
        encoded_image_bytes(ImageFormat::WebP)
    }

    fn corrupt_png_idat_with_valid_crc(bytes: &mut [u8]) {
        let mut position = 8;
        while position + 12 <= bytes.len() {
            let length = usize::try_from(u32::from_be_bytes(
                bytes[position..position + 4]
                    .try_into()
                    .expect("PNG chunk length"),
            ))
            .expect("PNG chunk length fits usize");
            let data_start = position + 8;
            let data_end = data_start + length;
            let chunk_end = data_end + 4;
            assert!(chunk_end <= bytes.len(), "complete PNG fixture");
            if &bytes[position + 4..data_start] == b"IDAT" {
                // Break the zlib header, then recompute the PNG chunk CRC so
                // the container remains structurally valid.
                bytes[data_start] ^= 0x01;
                let crc = png_crc32(&bytes[position + 4..data_end]).to_be_bytes();
                bytes[data_end..chunk_end].copy_from_slice(&crc);
                return;
            }
            position = chunk_end;
        }
        panic!("PNG fixture must contain IDAT");
    }

    fn jpeg_scan_positions(bytes: &[u8]) -> (usize, usize) {
        let mut position = 2;
        loop {
            let (marker, marker_end) = jpeg_marker(bytes, position).expect("JPEG marker");
            position = marker_end;
            if marker == 0xda {
                let length = usize::from(u16::from_be_bytes(
                    bytes[position..position + 2]
                        .try_into()
                        .expect("JPEG scan length"),
                ));
                return (position + 2, position + length);
            }
            if marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
                continue;
            }
            let length = usize::from(u16::from_be_bytes(
                bytes[position..position + 2]
                    .try_into()
                    .expect("JPEG segment length"),
            ));
            position += length;
        }
    }

    fn structurally_valid_invalid_webp_bytes() -> Vec<u8> {
        // A VP8 container/header with an empty first partition. The RIFF and
        // VP8 framing are internally consistent, but no decoder can form an
        // image from it.
        vec![
            b'R', b'I', b'F', b'F', 22, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b' ',
            10, 0, 0, 0, 0, 0, 0, 0x9d, 0x01, 0x2a, 1, 0, 1, 0,
        ]
    }

    #[test]
    fn defaults_are_valid() {
        let config =
            normalize_and_validate(AppConfig::default()).expect("default config should be valid");
        assert!(config.save_settings_manually);
        assert_eq!(
            config.launcher.global_hotkey,
            hotkey::normalize_shortcut(&hotkey::default_shortcut()).unwrap()
        );
        assert!(config.launcher.show_dock_icon);
        assert_eq!(
            config.launcher.empty_query_debounce_ms,
            default_empty_query_debounce_ms()
        );
        assert_eq!(
            config.launcher.non_empty_query_debounce_ms,
            default_non_empty_query_debounce_ms()
        );
        assert_eq!(config.launcher.window_width_px, None);
        assert_eq!(config.launcher_width(), 720.0);
        assert_eq!(
            config.launcher.window_height_px,
            default_launcher_height_px()
        );
        assert_eq!(config.launcher.horizontal_offset_px, 0);
        assert_eq!(config.launcher.vertical_offset_px, 0);
    }

    #[test]
    fn validates_general_query_debounce_range() {
        let mut maximum = AppConfig::default();
        maximum.launcher.empty_query_debounce_ms = MAX_QUERY_DEBOUNCE_MS;
        maximum.launcher.non_empty_query_debounce_ms = MAX_QUERY_DEBOUNCE_MS;
        assert!(normalize_and_validate(maximum).is_ok());

        let mut empty_too_slow = AppConfig::default();
        empty_too_slow.launcher.empty_query_debounce_ms = MAX_QUERY_DEBOUNCE_MS + 1;
        assert!(normalize_and_validate(empty_too_slow).is_err());

        let mut non_empty_too_slow = AppConfig::default();
        non_empty_too_slow.launcher.non_empty_query_debounce_ms = MAX_QUERY_DEBOUNCE_MS + 1;
        assert!(normalize_and_validate(non_empty_too_slow).is_err());
    }

    #[test]
    fn validates_and_normalizes_global_hotkey() {
        let mut valid = AppConfig::default();
        valid.launcher.global_hotkey = "Ctrl + Shift + K".into();
        let normalized = normalize_and_validate(valid).expect("valid configurable shortcut");
        assert_eq!(normalized.launcher.global_hotkey, "shift+control+KeyK");

        let mut missing_modifier = AppConfig::default();
        missing_modifier.launcher.global_hotkey = "KeyK".into();
        assert!(normalize_and_validate(missing_modifier).is_err());

        let mut unsupported_key = AppConfig::default();
        unsupported_key.launcher.global_hotkey = "Ctrl+Unidentified".into();
        assert!(normalize_and_validate(unsupported_key).is_err());
    }

    #[test]
    fn validates_launcher_size_and_position_ranges() {
        let mut boundaries = AppConfig::default();
        boundaries.launcher.window_width_px = Some(MIN_LAUNCHER_WIDTH_PX);
        boundaries.launcher.window_height_px = MAX_LAUNCHER_HEIGHT_PX;
        boundaries.launcher.horizontal_offset_px = MIN_LAUNCHER_HORIZONTAL_OFFSET_PX;
        boundaries.launcher.vertical_offset_px = MAX_LAUNCHER_VERTICAL_OFFSET_PX;
        assert!(normalize_and_validate(boundaries).is_ok());

        let mut width_too_small = AppConfig::default();
        width_too_small.launcher.window_width_px = Some(MIN_LAUNCHER_WIDTH_PX - 1);
        assert!(normalize_and_validate(width_too_small).is_err());

        let mut width_too_large = AppConfig::default();
        width_too_large.launcher.window_width_px = Some(MAX_LAUNCHER_WIDTH_PX + 1);
        assert!(normalize_and_validate(width_too_large).is_err());

        let mut height_too_small = AppConfig::default();
        height_too_small.launcher.window_height_px = MIN_LAUNCHER_HEIGHT_PX - 1;
        assert!(normalize_and_validate(height_too_small).is_err());

        let mut horizontal_too_large = AppConfig::default();
        horizontal_too_large.launcher.horizontal_offset_px = MAX_LAUNCHER_HORIZONTAL_OFFSET_PX + 1;
        assert!(normalize_and_validate(horizontal_too_large).is_err());

        let mut vertical_too_small = AppConfig::default();
        vertical_too_small.launcher.vertical_offset_px = MIN_LAUNCHER_VERTICAL_OFFSET_PX - 1;
        assert!(normalize_and_validate(vertical_too_small).is_err());
    }

    #[test]
    fn launcher_only_changes_keep_provider_results_valid() {
        let original = AppConfig::default();
        let mut launcher_change = original.clone();
        launcher_change.launcher.compact_when_empty = true;
        launcher_change.launcher.non_empty_query_debounce_ms = 80;
        assert!(!provider_settings_changed(&original, &launcher_change));

        let mut provider_change = original.clone();
        provider_change.web_searches[0].description = "更新后的说明".into();
        assert!(provider_settings_changed(&original, &provider_change));

        let mut enabled_change = original.clone();
        enabled_change.script_commands[0].enabled = false;
        assert!(provider_settings_changed(&original, &enabled_change));
    }

    #[test]
    fn rejects_duplicate_command_keywords_case_insensitively() {
        let mut config = AppConfig::default();
        config.web_searches[0].keyword = "TS".into();
        assert!(normalize_and_validate(config).is_err());
    }

    #[test]
    fn validates_supported_web_search_templates() {
        let mut positional = AppConfig::default();
        positional.web_searches[0].url_template =
            "https://example.com/?q={query0}&v={query1}".into();
        assert!(normalize_and_validate(positional).is_ok());

        let mut direct = AppConfig::default();
        direct.web_searches[0].url_template = "https://example.com/direct".into();
        assert!(normalize_and_validate(direct).is_ok());

        let mut legacy_position = AppConfig::default();
        legacy_position.web_searches[0].url_template = "https://example.com/?q={0}".into();
        assert!(normalize_and_validate(legacy_position).is_err());
    }

    #[test]
    fn rejects_duplicate_ids_and_newer_config_versions() {
        let mut duplicate = AppConfig::default();
        let mut second = duplicate.script_commands[0].clone();
        second.keyword = "other".into();
        duplicate.script_commands.push(second);
        assert!(normalize_and_validate(duplicate).is_err());

        let mut newer = AppConfig::default();
        newer.version = CONFIG_VERSION + 1;
        assert!(normalize_and_validate(newer).is_err());
    }

    #[test]
    fn migrates_v1_without_descriptions() {
        let mut legacy = legacy_config_value(1);
        legacy["translation"]
            .as_object_mut()
            .expect("translation object")
            .remove("description");
        legacy["scriptCommands"][0]
            .as_object_mut()
            .expect("script object")
            .remove("description");
        legacy["webSearches"][0]
            .as_object_mut()
            .expect("web object")
            .remove("description");
        legacy["launcher"]
            .as_object_mut()
            .expect("launcher object")
            .remove("compactWhenEmpty");
        legacy["scriptCommands"][0]
            .as_object_mut()
            .expect("script object")
            .remove("debounceMs");

        let config = serde_json::from_value::<AppConfig>(legacy).expect("deserialize legacy v1");
        let migrated = normalize_and_validate(config).expect("migrate legacy v1");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert!(!migrated.launcher.compact_when_empty);
        assert_eq!(
            migrated.script_commands[0].debounce_ms,
            default_script_debounce_ms()
        );
        assert!(migrated.translation.description.is_empty());
        assert!(migrated.script_commands[0].description.is_empty());
        assert!(migrated.web_searches[0].description.is_empty());
    }

    #[test]
    fn migrates_v2_without_compact_or_debounce_settings() {
        let mut legacy = legacy_config_value(2);
        legacy["launcher"]
            .as_object_mut()
            .expect("launcher object")
            .remove("compactWhenEmpty");
        legacy["scriptCommands"][0]
            .as_object_mut()
            .expect("script object")
            .remove("debounceMs");

        let config = serde_json::from_value::<AppConfig>(legacy).expect("deserialize legacy v2");
        let migrated = normalize_and_validate(config).expect("migrate legacy v2");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert!(!migrated.launcher.compact_when_empty);
        assert_eq!(
            migrated.script_commands[0].debounce_ms,
            default_script_debounce_ms()
        );
    }

    #[test]
    fn migrates_v3_without_script_debounce_setting() {
        let mut legacy = legacy_config_value(3);
        legacy["scriptCommands"][0]
            .as_object_mut()
            .expect("script object")
            .remove("debounceMs");

        let config = serde_json::from_value::<AppConfig>(legacy).expect("deserialize legacy v3");
        let migrated = normalize_and_validate(config).expect("migrate legacy v3");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(
            migrated.script_commands[0].debounce_ms,
            default_script_debounce_ms()
        );
    }

    #[test]
    fn migrates_v7_without_general_query_debounce_settings() {
        let mut previous = serde_json::to_value(AppConfig::default()).expect("serialize v7");
        previous["version"] = serde_json::json!(7);
        let launcher = previous["launcher"]
            .as_object_mut()
            .expect("launcher object");
        launcher.remove("emptyQueryDebounceMs");
        launcher.remove("nonEmptyQueryDebounceMs");
        launcher.remove("globalHotkey");
        launcher.remove("showDockIcon");
        launcher.remove("windowWidthPx");
        launcher.remove("windowHeightPx");
        launcher.remove("horizontalOffsetPx");
        launcher.remove("verticalOffsetPx");

        let migrated = normalize_and_validate(
            serde_json::from_value::<AppConfig>(previous).expect("deserialize v7"),
        )
        .expect("migrate v7 debounce defaults");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(
            migrated.launcher.empty_query_debounce_ms,
            default_empty_query_debounce_ms()
        );
        assert_eq!(
            migrated.launcher.non_empty_query_debounce_ms,
            default_non_empty_query_debounce_ms()
        );
    }

    #[test]
    fn migrates_v8_without_launcher_size_or_position_settings() {
        let mut previous = serde_json::to_value(AppConfig::default()).expect("serialize v8");
        previous["version"] = serde_json::json!(8);
        let launcher = previous["launcher"]
            .as_object_mut()
            .expect("launcher object");
        launcher.remove("windowWidthPx");
        launcher.remove("windowHeightPx");
        launcher.remove("horizontalOffsetPx");
        launcher.remove("verticalOffsetPx");
        launcher.remove("globalHotkey");
        launcher.remove("showDockIcon");

        let migrated = normalize_and_validate(
            serde_json::from_value::<AppConfig>(previous).expect("deserialize v8"),
        )
        .expect("migrate v8 launcher geometry defaults");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(migrated.launcher.window_width_px, None);
        assert_eq!(migrated.launcher_width(), 720.0);
        assert_eq!(
            migrated.launcher.window_height_px,
            default_launcher_height_px()
        );
        assert_eq!(migrated.launcher.horizontal_offset_px, 0);
        assert_eq!(migrated.launcher.vertical_offset_px, 0);
    }

    #[test]
    fn migrates_v9_with_visible_dock_icon_default() {
        let mut previous = serde_json::to_value(AppConfig::default()).expect("serialize v9");
        previous["version"] = serde_json::json!(9);
        previous["launcher"]
            .as_object_mut()
            .expect("launcher object")
            .remove("globalHotkey");
        previous["launcher"]
            .as_object_mut()
            .expect("launcher object")
            .remove("showDockIcon");

        let migrated = normalize_and_validate(
            serde_json::from_value::<AppConfig>(previous).expect("deserialize v9"),
        )
        .expect("migrate v9 Dock visibility default");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert!(migrated.launcher.show_dock_icon);
    }

    #[test]
    fn migrates_v10_with_platform_shortcut_default() {
        let mut previous = serde_json::to_value(AppConfig::default()).expect("serialize v10");
        previous["version"] = serde_json::json!(10);
        previous["launcher"]
            .as_object_mut()
            .expect("launcher object")
            .remove("globalHotkey");

        let migrated = normalize_and_validate(
            serde_json::from_value::<AppConfig>(previous).expect("deserialize v10"),
        )
        .expect("migrate v10 shortcut default");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(
            migrated.launcher.global_hotkey,
            hotkey::normalize_shortcut(&hotkey::default_shortcut()).unwrap()
        );
    }

    #[test]
    fn migrates_v11_without_command_icons_or_input_hints() {
        let mut previous = serde_json::to_value(AppConfig::default()).expect("serialize v11");
        previous["version"] = serde_json::json!(11);
        for key in ["iconDataUrl", "inputHint"] {
            previous["scriptCommands"][0]
                .as_object_mut()
                .expect("script object")
                .remove(key);
            previous["webSearches"][0]
                .as_object_mut()
                .expect("web search object")
                .remove(key);
        }

        let migrated = normalize_and_validate(
            serde_json::from_value::<AppConfig>(previous).expect("deserialize v11"),
        )
        .expect("migrate v11 command presentation defaults");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert!(migrated.script_commands[0].icon_data_url.is_empty());
        assert!(migrated.script_commands[0].input_hint.is_empty());
        assert!(migrated.web_searches[0].icon_data_url.is_empty());
        assert!(migrated.web_searches[0].input_hint.is_empty());
    }

    #[test]
    fn dock_visibility_round_trips_on_every_platform() {
        for visible in [false, true] {
            let mut config = AppConfig::default();
            config.launcher.show_dock_icon = visible;
            let encoded = serde_json::to_string(&config).expect("serialize Dock preference");
            let decoded: AppConfig =
                serde_json::from_str(&encoded).expect("deserialize Dock preference");
            let normalized =
                normalize_and_validate(decoded).expect("validate round-tripped Dock preference");

            assert_eq!(normalized.launcher.show_dock_icon, visible);
        }
    }

    #[test]
    fn migrates_v4_without_custom_themes() {
        let mut legacy = legacy_config_value(4);
        legacy["appearance"]
            .as_object_mut()
            .expect("appearance object")
            .remove("customThemes");

        let config = serde_json::from_value::<AppConfig>(legacy).expect("deserialize legacy v4");
        let migrated = normalize_and_validate(config).expect("migrate legacy v4");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert!(migrated.launcher_theme.custom_themes.is_empty());
        assert!(migrated.settings_theme.custom_themes.is_empty());
        assert_eq!(migrated.launcher_theme.theme, "midnight");
        assert_eq!(migrated.settings_theme.theme, "midnight");
    }

    #[test]
    fn migrates_v5_appearance_into_two_independent_scopes() {
        let mut legacy = legacy_config_value(5);
        let appearance = legacy["appearance"]
            .as_object_mut()
            .expect("legacy appearance object");
        appearance.insert("theme".into(), serde_json::json!("custom:nebula"));
        appearance.insert(
            "customThemes".into(),
            serde_json::json!([legacy_custom_theme_value("nebula")]),
        );
        appearance["customThemes"][0]["accentColor"] = serde_json::json!("#ef4f9a");

        let migrated = normalize_and_validate(
            serde_json::from_value::<AppConfig>(legacy).expect("deserialize v5 appearance"),
        )
        .expect("migrate v5 appearance");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert_eq!(migrated.launcher_theme.theme, "custom:nebula");
        assert_eq!(migrated.settings_theme.theme, "custom:nebula");
        assert_eq!(migrated.launcher_theme.custom_themes.len(), 1);
        assert_eq!(migrated.settings_theme.custom_themes.len(), 1);
        assert_eq!(migrated.launcher_theme.accent_color, "#ef4f9a");
        assert_eq!(migrated.settings_theme.accent_color, "#ef4f9a");
        assert_eq!(
            migrated.launcher_theme.custom_themes[0].accent_color,
            "#ef4f9a"
        );
        assert_eq!(
            migrated.settings_theme.custom_themes[0].accent_color,
            "#ef4f9a"
        );

        let mut edited = migrated.clone();
        edited.launcher_theme.custom_themes[0].name = "仅搜索皮肤".into();
        assert_eq!(edited.settings_theme.custom_themes[0].name, "旧版皮肤");
    }

    #[test]
    fn repairs_intermediate_v6_custom_themes_without_accent() {
        let mut config = AppConfig::default();
        config.launcher_theme.theme = "custom:launcher-v6".into();
        config.launcher_theme.accent_color = "#4361ee".into();
        config
            .launcher_theme
            .custom_themes
            .push(launcher_custom_theme("launcher-v6"));
        config.settings_theme.theme = "custom:settings-v6".into();
        config.settings_theme.accent_color = "#2a9d8f".into();
        config
            .settings_theme
            .custom_themes
            .push(settings_custom_theme("settings-v6"));

        let mut intermediate = serde_json::to_value(config).expect("serialize v6 config");
        intermediate["version"] = serde_json::json!(6);
        intermediate
            .as_object_mut()
            .expect("v6 config object")
            .remove("saveSettingsManually");
        intermediate["launcherTheme"]["customThemes"][0]
            .as_object_mut()
            .expect("launcher custom theme")
            .remove("accentColor");
        intermediate["settingsTheme"]["customThemes"][0]
            .as_object_mut()
            .expect("settings custom theme")
            .remove("accentColor");

        let repaired = normalize_and_validate(
            serde_json::from_value::<AppConfig>(intermediate)
                .expect("deserialize intermediate v6 config"),
        )
        .expect("repair intermediate v6 custom accents");
        assert_eq!(
            repaired.launcher_theme.custom_themes[0].accent_color,
            "#4361ee"
        );
        assert_eq!(
            repaired.settings_theme.custom_themes[0].accent_color,
            "#2a9d8f"
        );
        assert!(repaired.save_settings_manually);
        assert_eq!(repaired.version, CONFIG_VERSION);
    }

    #[test]
    fn rejects_incomplete_v6_theme_scopes_instead_of_falling_back_to_appearance() {
        let mut malformed = legacy_config_value(6);
        malformed["appearance"] = serde_json::json!({
            "theme": "midnight",
            "accentColor": "#8a78ff",
            "customThemes": []
        });
        assert!(serde_json::from_value::<AppConfig>(malformed).is_err());

        let mut incomplete = serde_json::to_value(AppConfig::default()).expect("serialize v6");
        incomplete["version"] = serde_json::json!(6);
        incomplete
            .as_object_mut()
            .expect("v6 config object")
            .remove("saveSettingsManually");
        incomplete
            .as_object_mut()
            .expect("v6 config object")
            .remove("settingsTheme");
        assert!(serde_json::from_value::<AppConfig>(incomplete).is_err());
    }

    #[test]
    fn rejects_v7_and_newer_without_save_mode() {
        for version in [7, CONFIG_VERSION] {
            let mut current =
                serde_json::to_value(AppConfig::default()).expect("serialize current config");
            current["version"] = serde_json::json!(version);
            current
                .as_object_mut()
                .expect("config object")
                .remove("saveSettingsManually");
            assert!(serde_json::from_value::<AppConfig>(current).is_err());
        }
    }

    #[test]
    fn validates_scope_custom_theme_selection_and_ranges() {
        let mut valid = AppConfig::default();
        valid.launcher_theme.theme = "custom:nebula".into();
        valid
            .launcher_theme
            .custom_themes
            .push(launcher_custom_theme("nebula"));
        valid.settings_theme.theme = "custom:calm".into();
        valid
            .settings_theme
            .custom_themes
            .push(settings_custom_theme("calm"));
        let normalized = normalize_and_validate(valid).expect("valid custom theme");
        assert_eq!(normalized.launcher_theme.max_results(), 8);
        assert_eq!(normalized.launcher_theme.launcher_width(), 720.0);
        assert_eq!(normalized.settings_theme.theme, "custom:calm");

        let mut differently_cased = AppConfig::default();
        differently_cased.launcher_theme.theme = "custom:NEBULA".into();
        differently_cased
            .launcher_theme
            .custom_themes
            .push(launcher_custom_theme("nebula"));
        let normalized = normalize_and_validate(differently_cased)
            .expect("custom theme selection should be canonicalized");
        assert_eq!(normalized.launcher_theme.theme, "custom:nebula");

        let mut missing = AppConfig::default();
        missing.launcher_theme.theme = "custom:missing".into();
        assert!(normalize_and_validate(missing).is_err());

        let mut invalid_color = AppConfig::default();
        let mut theme = launcher_custom_theme("bad-color");
        theme.normal_primary_color = "red".into();
        invalid_color.launcher_theme.custom_themes.push(theme);
        assert!(normalize_and_validate(invalid_color).is_err());

        let mut invalid_width = AppConfig::default();
        let mut theme = launcher_custom_theme("bad-width");
        theme.window_width_px = 901;
        invalid_width.launcher_theme.custom_themes.push(theme);
        assert!(normalize_and_validate(invalid_width).is_err());

        let mut borderless = AppConfig::default();
        let mut theme = launcher_custom_theme("borderless");
        theme.search_border_style = "none".into();
        theme.search_border_width_px = 0;
        borderless.launcher_theme.custom_themes.push(theme);
        assert!(normalize_and_validate(borderless).is_ok());

        let mut oversized_search_text = AppConfig::default();
        let mut theme = launcher_custom_theme("oversized-search-text");
        theme.search_font_size_px = 25;
        oversized_search_text
            .launcher_theme
            .custom_themes
            .push(theme);
        assert!(normalize_and_validate(oversized_search_text).is_err());

        let mut remote_wallpaper = AppConfig::default();
        let mut theme = settings_custom_theme("bad-wallpaper");
        theme.background.wallpaper_data_url = "https://example.com/background.png".into();
        remote_wallpaper.settings_theme.custom_themes.push(theme);
        assert!(normalize_and_validate(remote_wallpaper).is_err());
    }

    #[test]
    fn scopes_allow_same_id_without_cross_scope_validation_or_mutation() {
        let mut config = AppConfig::default();
        config.launcher_theme.theme = "custom:shared".into();
        config.settings_theme.theme = "custom:shared".into();
        config
            .launcher_theme
            .custom_themes
            .push(launcher_custom_theme("shared"));
        config
            .settings_theme
            .custom_themes
            .push(settings_custom_theme("shared"));
        assert!(normalize_and_validate(config).is_ok());
    }

    #[test]
    fn rejects_wrong_scope_and_illegal_theme_bundle_tokens() {
        let launcher = launcher_custom_theme("bundle");
        let launcher_bundle =
            build_launcher_theme_bundle(&launcher).expect("build launcher bundle");
        assert_eq!(launcher_bundle["theme"]["accentColor"], "#8a78ff");
        let serialized =
            serde_json::to_string(&launcher_bundle).expect("serialize launcher bundle");
        assert!(parse_launcher_theme_bundle(&serialized).is_ok());
        assert!(parse_settings_theme_bundle(&serialized).is_err());

        let settings_bundle = build_settings_theme_bundle(&settings_custom_theme("bundle"))
            .expect("build settings bundle");
        assert_eq!(settings_bundle["theme"]["accentColor"], "#8a78ff");
        let serialized_settings =
            serde_json::to_string(&settings_bundle).expect("serialize settings bundle");
        assert!(parse_settings_theme_bundle(&serialized_settings).is_ok());
        assert!(parse_launcher_theme_bundle(&serialized_settings).is_err());

        let mut invalid_style = launcher_bundle.clone();
        invalid_style["theme"]["searchBorderStyle"] = serde_json::json!("groove");
        assert!(parse_launcher_theme_bundle(
            &serde_json::to_string(&invalid_style).expect("serialize illegal style")
        )
        .is_err());

        let mut invalid_accent = launcher_bundle.clone();
        invalid_accent["theme"]["accentColor"] = serde_json::json!("red");
        assert!(parse_launcher_theme_bundle(
            &serde_json::to_string(&invalid_accent).expect("serialize illegal accent")
        )
        .is_err());

        let mut missing_accent = launcher_bundle.clone();
        missing_accent["theme"]
            .as_object_mut()
            .expect("launcher bundle theme")
            .remove("accentColor");
        assert!(parse_launcher_theme_bundle(
            &serde_json::to_string(&missing_accent).expect("serialize missing accent")
        )
        .is_err());

        let mut unknown_token = launcher_bundle;
        unknown_token["theme"]["unexpected"] = serde_json::json!(true);
        assert!(parse_launcher_theme_bundle(
            &serde_json::to_string(&unknown_token).expect("serialize unknown token")
        )
        .is_err());

        let legacy_schema = serde_json::json!({
            "schema": "suo-theme-v1",
            "version": 1,
            "theme": {}
        });
        assert!(parse_launcher_theme_bundle(
            &serde_json::to_string(&legacy_schema).expect("serialize legacy schema")
        )
        .is_err());
    }

    #[test]
    fn enforces_wallpaper_decoded_byte_limit_and_base64_padding() {
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/png", &valid_png_bytes(MAX_THEME_WALLPAPER_BYTES)),
            "边界皮肤"
        )
        .is_ok());
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/png", &valid_png_bytes(MAX_THEME_WALLPAPER_BYTES + 1),),
            "超限皮肤"
        )
        .is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,A=AA", "错误填充").is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,AAA", "错误长度").is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,AB==", "非规范填充").is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,AAB=", "非规范尾位").is_err());
        assert!(validate_wallpaper_data_url(
            "data:image/png;base64,iVBORw0KGgp=",
            "前端同样拒绝的非规范尾位"
        )
        .is_err());
    }

    #[test]
    fn accepts_complete_structured_wallpapers_for_each_allowed_format() {
        for (mime_type, bytes) in [
            ("image/png", encoded_image_bytes(ImageFormat::Png)),
            ("image/jpeg", valid_jpeg_bytes()),
            ("image/webp", valid_webp_bytes()),
        ] {
            assert!(
                validate_wallpaper_data_url(&wallpaper_data_url(mime_type, &bytes), "完整图片")
                    .is_ok(),
                "{mime_type} should be accepted"
            );
        }
    }

    #[test]
    fn accepts_rgba16_png_with_budget_for_the_maximum_supported_canvas() {
        const RGBA16_BYTES_PER_PIXEL: u64 = 8;
        const DECODER_SCRATCH_MARGIN: u64 = 32 * 1024 * 1024;
        assert!(
            MAX_THEME_WALLPAPER_ALLOCATION_BYTES
                >= MAX_THEME_WALLPAPER_PIXELS * RGBA16_BYTES_PER_PIXEL + DECODER_SCRATCH_MARGIN
        );
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/png", &encoded_rgba16_png_bytes()),
            "16 位 PNG 皮肤"
        )
        .is_ok());
    }

    #[test]
    fn rejects_structurally_valid_but_undecodable_wallpapers() {
        let mut bad_png = valid_png_bytes(73);
        corrupt_png_idat_with_valid_crc(&mut bad_png);
        assert!(
            validate_png(&bad_png),
            "CRC-correct container remains valid"
        );
        assert!(
            validate_wallpaper_data_url(&wallpaper_data_url("image/png", &bad_png), "坏 IDAT")
                .is_err()
        );

        let mut bad_jpeg = valid_jpeg_bytes();
        let (scan_header, _) = jpeg_scan_positions(&bad_jpeg);
        // Keep the SOS framing and entropy stream intact, but reference an
        // undefined frame component. The container parser only sees a valid
        // one-component scan; the JPEG decoder must reject this selector.
        bad_jpeg[scan_header + 1] = 0;
        assert!(validate_jpeg(&bad_jpeg), "marker framing remains valid");
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/jpeg", &bad_jpeg),
            "坏 JPEG scan"
        )
        .is_err());

        let bad_webp = structurally_valid_invalid_webp_bytes();
        assert!(
            validate_webp(&bad_webp),
            "RIFF and VP8 framing remain valid"
        );
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/webp", &bad_webp),
            "坏 VP8 bitstream"
        )
        .is_err());
    }

    #[test]
    fn rejects_signature_only_truncated_and_forged_wallpapers() {
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/png", b"\x89PNG\r\n\x1a\n"),
            "只有 PNG 签名"
        )
        .is_err());

        let mut truncated_png = valid_png_bytes(73);
        truncated_png.truncate(truncated_png.len() - 12);
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/png", &truncated_png),
            "截断 PNG"
        )
        .is_err());

        let mut forged_png_length = valid_png_bytes(73);
        forged_png_length[8..12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/png", &forged_png_length),
            "伪造 PNG 长度"
        )
        .is_err());

        let mut truncated_jpeg = valid_jpeg_bytes();
        truncated_jpeg.pop();
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/jpeg", &truncated_jpeg),
            "截断 JPEG"
        )
        .is_err());

        let mut forged_webp_length = valid_webp_bytes();
        forged_webp_length[4..8].copy_from_slice(&23u32.to_le_bytes());
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url("image/webp", &forged_webp_length),
            "伪造 WebP 长度"
        )
        .is_err());
    }

    #[test]
    fn validates_script_debounce_range() {
        let mut minimum = AppConfig::default();
        minimum.script_commands[0].debounce_ms = MIN_SCRIPT_DEBOUNCE_MS;
        assert!(normalize_and_validate(minimum).is_ok());

        let mut too_short = AppConfig::default();
        too_short.script_commands[0].debounce_ms = MIN_SCRIPT_DEBOUNCE_MS - 1;
        assert!(normalize_and_validate(too_short).is_err());

        let mut too_long = AppConfig::default();
        too_long.script_commands[0].debounce_ms = MAX_SCRIPT_DEBOUNCE_MS + 1;
        assert!(normalize_and_validate(too_long).is_err());
    }

    #[test]
    fn validates_command_icons_and_normalizes_input_hints() {
        let icon = wallpaper_data_url("image/png", &valid_png_bytes(73));
        let mut valid = AppConfig::default();
        valid.script_commands[0].icon_data_url = icon.clone();
        valid.script_commands[0].input_hint = "  输入时间戳和可选时区  ".into();
        valid.web_searches[0].icon_data_url = icon;
        valid.web_searches[0].input_hint = "  输入搜索内容  ".into();
        let normalized = normalize_and_validate(valid).expect("valid command presentation");
        assert_eq!(
            normalized.script_commands[0].input_hint,
            "输入时间戳和可选时区"
        );
        assert_eq!(normalized.web_searches[0].input_hint, "输入搜索内容");

        let mut oversized = AppConfig::default();
        oversized.web_searches[0].icon_data_url =
            wallpaper_data_url("image/png", &valid_png_bytes(MAX_COMMAND_ICON_BYTES + 1));
        assert!(normalize_and_validate(oversized).is_err());

        let mut invalid = AppConfig::default();
        invalid.script_commands[0].icon_data_url = "https://example.com/icon.png".into();
        assert!(normalize_and_validate(invalid).is_err());

        let mut hint_too_long = AppConfig::default();
        hint_too_long.web_searches[0].input_hint = "字".repeat(161);
        assert!(normalize_and_validate(hint_too_long).is_err());
    }

    #[test]
    fn trims_and_limits_descriptions() {
        let mut config = AppConfig::default();
        config.script_commands[0].description = "  示例说明  ".into();
        let normalized = normalize_and_validate(config).expect("normalize description");
        assert_eq!(normalized.script_commands[0].description, "示例说明");

        let mut too_long = AppConfig::default();
        too_long.web_searches[0].description = "字".repeat(201);
        assert!(normalize_and_validate(too_long).is_err());

        let mut emoji_boundary = AppConfig::default();
        emoji_boundary.translation.description = "😀".repeat(100);
        assert!(normalize_and_validate(emoji_boundary).is_ok());

        let mut emoji_too_long = AppConfig::default();
        emoji_too_long.translation.description = format!("{}a", "😀".repeat(100));
        assert!(normalize_and_validate(emoji_too_long).is_err());

        let mut combining_boundary = AppConfig::default();
        combining_boundary.script_commands[0].description = "e\u{301}".repeat(100);
        assert!(normalize_and_validate(combining_boundary).is_ok());

        let mut combining_too_long = AppConfig::default();
        combining_too_long.script_commands[0].description = format!("{}a", "e\u{301}".repeat(100));
        assert!(normalize_and_validate(combining_too_long).is_err());
    }

    #[test]
    fn detects_newer_versions_in_backup_candidates() {
        let backup_only = temporary_config_path("newer-backup-only");
        let newer_version = u64::from(CONFIG_VERSION) + 1;
        fs::write(
            backup_only.with_extension("json.bak"),
            format!(r#"{{"version":{newer_version}}}"#),
        )
        .expect("write newer backup");
        assert_eq!(newer_config_version(&backup_only), Some(newer_version));
        remove_temporary_config(&backup_only);

        let broken_primary = temporary_config_path("broken-primary-newer-backup");
        fs::write(&broken_primary, "not json").expect("write broken primary");
        let even_newer_version = u64::from(CONFIG_VERSION) + 2;
        fs::write(
            broken_primary.with_extension("json.bak"),
            format!(r#"{{"version":{even_newer_version}}}"#),
        )
        .expect("write newer backup");
        assert_eq!(
            newer_config_version(&broken_primary),
            Some(even_newer_version)
        );
        remove_temporary_config(&broken_primary);
    }

    #[test]
    fn refuses_newer_disk_config_written_after_startup() {
        let path = temporary_config_path("newer-after-startup");
        let state = config_state_at(&path);
        let newer_version = u64::from(CONFIG_VERSION) + 1;
        fs::write(
            &path,
            format!(r#"{{"version":{newer_version},"futureField":"preserve me"}}"#),
        )
        .expect("write newer config");

        let error = state
            .replace(AppConfig::default())
            .expect_err("newer on-disk config must be protected");
        assert!(error.contains(&format!("v{newer_version}")));
        assert_eq!(config_file_version(&path), Some(newer_version));
        let content = fs::read_to_string(&path).expect("read protected config");
        assert!(content.contains("futureField"));
        remove_temporary_config(&path);
    }

    #[test]
    fn config_location_pointer_resolves_only_available_absolute_configs() {
        let default_path = temporary_config_path("location-pointer");
        let default_directory = default_path.parent().expect("default directory");
        let location_path = default_directory.join(CONFIG_LOCATION_FILE_NAME);
        let custom_directory = default_directory.join("custom");
        fs::create_dir_all(&custom_directory).expect("create custom directory");
        let custom_path = custom_directory.join(CONFIG_FILE_NAME);
        persist_config(&custom_path, &AppConfig::default()).expect("write custom config");

        let (resolved_default, warning) = resolve_config_path(&default_path, &location_path);
        assert_eq!(resolved_default, default_path);
        assert!(warning.is_none());

        persist_config_location(&location_path, Some(&custom_directory))
            .expect("write custom location pointer");
        let (resolved_custom, warning) = resolve_config_path(&default_path, &location_path);
        assert_eq!(resolved_custom, custom_path);
        assert!(warning.is_none());

        fs::remove_file(&custom_path).expect("remove custom config");
        let (fallback, warning) = resolve_config_path(&default_path, &location_path);
        assert_eq!(fallback, default_path);
        assert!(warning.is_some_and(|value| value.contains("已临时使用默认配置位置")));
        assert!(config_location_needs_reset(&location_path));

        let state = config_state_at(&default_path);
        state
            .relocate(default_directory)
            .expect("reset unavailable custom location to default");
        assert!(!config_location_needs_reset(&location_path));
        let (repaired, warning) = resolve_config_path(&default_path, &location_path);
        assert_eq!(repaired, default_path);
        assert!(warning.is_none());
        remove_temporary_config(&default_path);
    }

    #[test]
    fn relocating_config_is_transactional_and_keeps_recovery_copies() {
        let default_path = temporary_config_path("relocate");
        let default_directory = default_path.parent().expect("default directory");
        persist_config(&default_path, &AppConfig::default()).expect("write default config");
        let state = config_state_at(&default_path);
        let custom_directory = default_directory.join("custom");
        fs::create_dir_all(&custom_directory).expect("create custom directory");

        state
            .relocate(&custom_directory)
            .expect("relocate to empty custom directory");
        let custom_path = custom_directory.join(CONFIG_FILE_NAME);
        assert_eq!(state.config_path().unwrap(), custom_path);
        assert!(custom_path.is_file());
        assert!(
            default_path.is_file(),
            "old config remains as a recovery copy"
        );
        let (resolved, warning) = resolve_config_path(&default_path, &state.location_path);
        assert_eq!(resolved, custom_path);
        assert!(warning.is_none());

        let occupied_directory = default_directory.join("occupied");
        fs::create_dir_all(&occupied_directory).expect("create occupied directory");
        fs::write(
            occupied_directory.join(CONFIG_FILE_NAME),
            "do not overwrite",
        )
        .expect("write occupied target");
        let error = state
            .relocate(&occupied_directory)
            .expect_err("occupied custom directory must be rejected");
        assert!(error.contains("为避免覆盖"));
        assert_eq!(
            fs::read_to_string(occupied_directory.join(CONFIG_FILE_NAME)).unwrap(),
            "do not overwrite"
        );
        assert_eq!(state.config_path().unwrap(), custom_path);

        state
            .relocate(default_directory)
            .expect("restore default location");
        assert_eq!(state.config_path().unwrap(), default_path);
        let (resolved, warning) = resolve_config_path(&default_path, &state.location_path);
        assert_eq!(resolved, default_path);
        assert!(warning.is_none());
        assert!(
            custom_path.is_file(),
            "custom config remains as a recovery copy"
        );
        remove_temporary_config(&default_path);
    }
}
