use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{launcher::LauncherState, web_search};

const CONFIG_VERSION: u32 = 5;
const CREDENTIAL_SERVICE: &str = "io.github.dqgod.suo";
const TRANSLATOR_CREDENTIAL: &str = "microsoft-translator-api-key";
const MAX_COMMANDS: usize = 50;
const MAX_CUSTOM_THEMES: usize = 12;
const MAX_THEME_WALLPAPER_BYTES: usize = 1_572_864;
const MIN_SCRIPT_DEBOUNCE_MS: u64 = 20;
const MAX_SCRIPT_DEBOUNCE_MS: u64 = 60_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub launcher: LauncherConfig,
    pub translation: TranslationConfig,
    pub script_commands: Vec<ScriptCommandConfig>,
    pub web_searches: Vec<WebSearchConfig>,
    pub appearance: AppearanceConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherConfig {
    pub close_on_blur: bool,
    pub keep_last_input: bool,
    #[serde(default)]
    pub compact_when_empty: bool,
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
    pub aliases: Vec<String>,
    pub enabled: bool,
    pub url_template: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceConfig {
    pub theme: String,
    pub accent_color: String,
    #[serde(default)]
    pub custom_themes: Vec<CustomThemeConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomThemeConfig {
    pub id: String,
    pub name: String,
    pub window_color: String,
    pub panel_color: String,
    pub field_color: String,
    pub text_color: String,
    pub muted_color: String,
    pub accent_color: String,
    pub selection_color: String,
    pub border_color: String,
    pub window_opacity: u8,
    pub blur_px: u8,
    pub shadow_percent: u8,
    #[serde(default)]
    pub wallpaper_data_url: String,
    pub wallpaper_opacity: u8,
    pub radius_px: u8,
    pub font_family: String,
    pub font_size_px: u8,
    pub launcher_width_px: u16,
    pub result_density: String,
    pub max_results: u8,
    pub icon_size_px: u8,
    pub show_source_badge: bool,
    #[serde(default)]
    pub platform_overrides: PlatformThemeOverrides,
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

impl AppearanceConfig {
    pub fn active_custom_theme(&self) -> Option<&CustomThemeConfig> {
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
            .map(|theme| f64::from(theme.launcher_width_px))
            .unwrap_or(720.0)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfigView {
    pub config: AppConfig,
    pub translation_api_key_configured: bool,
    pub credential_store_error: Option<String>,
    pub config_load_warning: Option<String>,
    pub needs_legacy_preferences_migration: bool,
    pub config_read_only: bool,
}

pub struct ConfigState {
    path: PathBuf,
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
            launcher: LauncherConfig {
                close_on_blur: true,
                keep_last_input: false,
                compact_when_empty: false,
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
                aliases: Vec::new(),
                enabled: true,
                url_template: "https://www.google.com.hk/search?q={query}".into(),
            }],
            appearance: AppearanceConfig {
                theme: "midnight".into(),
                accent_color: "#8a78ff".into(),
                custom_themes: Vec::new(),
            },
        }
    }
}

impl ConfigState {
    pub fn load(app: &AppHandle) -> Self {
        let config_dir = app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        let path = config_dir.join("config.json");
        let needs_legacy_preferences_migration =
            !path.exists() && !path.with_extension("json.bak").exists();
        let incompatible_newer_version = newer_config_version(&path);
        let (config, load_warning) = if let Some(version) = incompatible_newer_version {
            (
                AppConfig::default(),
                Some(format!(
                    "配置来自更新版本 v{version}，当前 Suo v{CONFIG_VERSION} 仅以只读方式启动"
                )),
            )
        } else {
            load_config(&path)
        };
        Self {
            path,
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
        if let Some(version) = self
            .incompatible_newer_version
            .or_else(|| newer_config_version(&self.path))
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
        persist_config(&self.path, &config)?;
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

    fn view(&self) -> AppConfigView {
        let (translation_api_key_configured, credential_store_error) =
            match read_translation_api_key() {
                Ok(value) => (value.is_some(), None),
                Err(error) => (false, Some(error)),
            };
        AppConfigView {
            config: self.snapshot(),
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
    use std::io::Write;
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

    normalize_translation(&mut config.translation)?;
    for command in &mut config.script_commands {
        normalize_script(command)?;
    }
    for search in &mut config.web_searches {
        normalize_web_search(search)?;
    }
    normalize_appearance(&mut config.appearance)?;
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
        // v4 adds per-script debounce, and v5 adds custom themes. Serde defaults
        // keep older configurations safe during migration.
        0 | 1 | 2 | 3 | 4 => config.version = 5,
        5 => {}
        version => return Err(format!("不支持的配置版本 v{version}")),
    }
    Ok(config)
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

fn normalize_appearance(appearance: &mut AppearanceConfig) -> Result<(), String> {
    appearance.theme = required_trimmed(&appearance.theme, "当前主题", 80)?;
    validate_hex_color(&appearance.accent_color, "强调色")?;
    if appearance.custom_themes.len() > MAX_CUSTOM_THEMES {
        return Err(format!("自定义皮肤最多允许 {MAX_CUSTOM_THEMES} 个"));
    }

    let mut ids = HashMap::<String, String>::new();
    for theme in &mut appearance.custom_themes {
        theme.id = required_trimmed(&theme.id, "皮肤 ID", 64)?;
        if !theme
            .id
            .bytes()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
        {
            return Err(format!(
                "皮肤 {} 的 ID 只能包含字母、数字、- 和 _",
                theme.id
            ));
        }
        theme.name = required_trimmed(&theme.name, "皮肤名称", 40)?;
        if let Some(existing) = ids.insert(theme.id.to_ascii_lowercase(), theme.name.clone()) {
            return Err(format!(
                "皮肤 ID {} 同时属于 {} 和 {}",
                theme.id, existing, theme.name
            ));
        }
        validate_custom_theme(theme)?;
    }

    if !matches!(appearance.theme.as_str(), "midnight" | "paper" | "forest") {
        let selected = appearance
            .theme
            .strip_prefix("custom:")
            .ok_or_else(|| "主题必须是内置主题或 custom:<id>".to_string())?;
        let canonical_id = appearance
            .custom_themes
            .iter()
            .find(|theme| theme.id.eq_ignore_ascii_case(selected))
            .map(|theme| theme.id.clone())
            .ok_or_else(|| format!("当前自定义皮肤不存在：{selected}"))?;
        appearance.theme = format!("custom:{canonical_id}");
    }
    Ok(())
}

fn validate_custom_theme(theme: &CustomThemeConfig) -> Result<(), String> {
    for (value, label) in [
        (&theme.window_color, "窗口背景"),
        (&theme.panel_color, "卡片与面板"),
        (&theme.field_color, "输入框"),
        (&theme.text_color, "主文字"),
        (&theme.muted_color, "次要文字"),
        (&theme.accent_color, "强调色"),
        (&theme.selection_color, "选中项背景"),
        (&theme.border_color, "边框"),
    ] {
        validate_hex_color(value, &format!("皮肤 {} 的{label}", theme.name))?;
    }

    validate_range(theme.window_opacity, 70, 100, &theme.name, "窗口透明度")?;
    validate_range(theme.blur_px, 0, 40, &theme.name, "背景模糊")?;
    validate_range(theme.shadow_percent, 0, 80, &theme.name, "阴影强度")?;
    validate_range(theme.wallpaper_opacity, 0, 60, &theme.name, "背景图强度")?;
    validate_range(theme.radius_px, 0, 28, &theme.name, "圆角")?;
    validate_range(theme.font_size_px, 12, 18, &theme.name, "字体大小")?;
    if !(620..=900).contains(&theme.launcher_width_px) {
        return Err(format!(
            "皮肤 {} 的启动器宽度必须在 620–900 px 之间",
            theme.name
        ));
    }
    if !matches!(theme.font_family.as_str(), "system" | "cjk" | "mono") {
        return Err(format!("皮肤 {} 的字体族无效", theme.name));
    }
    if !matches!(
        theme.result_density.as_str(),
        "compact" | "comfortable" | "loose"
    ) {
        return Err(format!("皮肤 {} 的结果密度无效", theme.name));
    }
    if !matches!(theme.max_results, 6 | 8 | 10 | 12) {
        return Err(format!(
            "皮肤 {} 最多只能显示 6、8、10 或 12 项结果",
            theme.name
        ));
    }
    validate_range(theme.icon_size_px, 28, 48, &theme.name, "图标尺寸")?;
    validate_range(
        theme.platform_overrides.windows_blur_px,
        0,
        40,
        &theme.name,
        "Windows 模糊",
    )?;
    validate_range(
        theme.platform_overrides.windows_opacity,
        70,
        100,
        &theme.name,
        "Windows 透明度",
    )?;
    validate_range(
        theme.platform_overrides.macos_blur_px,
        0,
        40,
        &theme.name,
        "macOS 模糊",
    )?;
    validate_range(
        theme.platform_overrides.macos_opacity,
        70,
        100,
        &theme.name,
        "macOS 透明度",
    )?;
    validate_wallpaper_data_url(&theme.wallpaper_data_url, &theme.name)
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
    if value.is_empty() {
        return Ok(());
    }
    let payload = [
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
        "data:image/webp;base64,",
    ]
    .iter()
    .find_map(|prefix| value.strip_prefix(prefix))
    .ok_or_else(|| format!("皮肤 {name} 的背景图只允许 PNG、JPEG 或 WebP"))?;
    if payload.is_empty() || payload.len() % 4 != 0 {
        return Err(format!("皮肤 {name} 的背景图数据无效"));
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
        return Err(format!("皮肤 {name} 的背景图数据无效"));
    }
    let last_sextet = payload[..content_len]
        .bytes()
        .next_back()
        .and_then(base64_sextet)
        .ok_or_else(|| format!("皮肤 {name} 的背景图数据无效"))?;
    if (padding == 2 && last_sextet & 0x0f != 0) || (padding == 1 && last_sextet & 0x03 != 0) {
        return Err(format!("皮肤 {name} 的背景图数据无效"));
    }
    let decoded_bytes = payload
        .len()
        .checked_div(4)
        .and_then(|groups| groups.checked_mul(3))
        .and_then(|bytes| bytes.checked_sub(padding))
        .ok_or_else(|| format!("皮肤 {name} 的背景图数据无效"))?;
    if decoded_bytes > MAX_THEME_WALLPAPER_BYTES {
        return Err(format!("皮肤 {name} 的背景图不能超过 1.5 MB"));
    }
    Ok(())
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
pub fn save_app_config(
    app: AppHandle,
    state: State<'_, Arc<ConfigState>>,
    launcher: State<'_, Arc<LauncherState>>,
    config: AppConfig,
) -> Result<AppConfigView, String> {
    let (previous, config) = state.replace(config)?;
    let providers_changed = provider_settings_changed(&previous, &config);
    launcher.update_preferences(
        config.launcher.close_on_blur,
        config.launcher.keep_last_input,
    );
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

    fn custom_theme(id: &str) -> CustomThemeConfig {
        CustomThemeConfig {
            id: id.into(),
            name: "测试皮肤".into(),
            window_color: "#0b1222".into(),
            panel_color: "#101a30".into(),
            field_color: "#161f39".into(),
            text_color: "#f5f7ff".into(),
            muted_color: "#91a0c7".into(),
            accent_color: "#8a78ff".into(),
            selection_color: "#302b63".into(),
            border_color: "#343d5a".into(),
            window_opacity: 94,
            blur_px: 18,
            shadow_percent: 45,
            wallpaper_data_url: String::new(),
            wallpaper_opacity: 18,
            radius_px: 18,
            font_family: "system".into(),
            font_size_px: 14,
            launcher_width_px: 720,
            result_density: "comfortable".into(),
            max_results: 8,
            icon_size_px: 36,
            show_source_badge: true,
            platform_overrides: PlatformThemeOverrides::default(),
        }
    }

    fn wallpaper_data_url(decoded_bytes: usize) -> String {
        let encoded_len = 4 * ((decoded_bytes + 2) / 3);
        let padding = (3 - decoded_bytes % 3) % 3;
        let mut payload = "A".repeat(encoded_len - padding);
        payload.push_str(&"=".repeat(padding));
        format!("data:image/png;base64,{payload}")
    }

    #[test]
    fn defaults_are_valid() {
        normalize_and_validate(AppConfig::default()).expect("default config should be valid");
    }

    #[test]
    fn launcher_only_changes_keep_provider_results_valid() {
        let original = AppConfig::default();
        let mut launcher_change = original.clone();
        launcher_change.launcher.compact_when_empty = true;
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
    fn validates_supported_web_search_placeholders() {
        let mut positional = AppConfig::default();
        positional.web_searches[0].url_template =
            "https://example.com/?q={query0}&v={query1}".into();
        assert!(normalize_and_validate(positional).is_ok());

        let mut missing = AppConfig::default();
        missing.web_searches[0].url_template = "https://example.com".into();
        assert!(normalize_and_validate(missing).is_err());

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
        let mut legacy = serde_json::to_value(AppConfig::default()).expect("serialize defaults");
        legacy["version"] = serde_json::json!(1);
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
        let mut legacy = serde_json::to_value(AppConfig::default()).expect("serialize defaults");
        legacy["version"] = serde_json::json!(2);
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
        let mut legacy = serde_json::to_value(AppConfig::default()).expect("serialize defaults");
        legacy["version"] = serde_json::json!(3);
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
    fn migrates_v4_without_custom_themes() {
        let mut legacy = serde_json::to_value(AppConfig::default()).expect("serialize defaults");
        legacy["version"] = serde_json::json!(4);
        legacy["appearance"]
            .as_object_mut()
            .expect("appearance object")
            .remove("customThemes");

        let config = serde_json::from_value::<AppConfig>(legacy).expect("deserialize legacy v4");
        let migrated = normalize_and_validate(config).expect("migrate legacy v4");
        assert_eq!(migrated.version, CONFIG_VERSION);
        assert!(migrated.appearance.custom_themes.is_empty());
        assert_eq!(migrated.appearance.theme, "midnight");
    }

    #[test]
    fn validates_custom_theme_selection_and_ranges() {
        let mut valid = AppConfig::default();
        valid.appearance.theme = "custom:nebula".into();
        valid.appearance.custom_themes.push(custom_theme("nebula"));
        let normalized = normalize_and_validate(valid).expect("valid custom theme");
        assert_eq!(normalized.appearance.max_results(), 8);
        assert_eq!(normalized.appearance.launcher_width(), 720.0);

        let mut differently_cased = AppConfig::default();
        differently_cased.appearance.theme = "custom:NEBULA".into();
        differently_cased
            .appearance
            .custom_themes
            .push(custom_theme("nebula"));
        let normalized = normalize_and_validate(differently_cased)
            .expect("custom theme selection should be canonicalized");
        assert_eq!(normalized.appearance.theme, "custom:nebula");

        let mut missing = AppConfig::default();
        missing.appearance.theme = "custom:missing".into();
        assert!(normalize_and_validate(missing).is_err());

        let mut invalid_color = AppConfig::default();
        let mut theme = custom_theme("bad-color");
        theme.text_color = "red".into();
        invalid_color.appearance.custom_themes.push(theme);
        assert!(normalize_and_validate(invalid_color).is_err());

        let mut invalid_width = AppConfig::default();
        let mut theme = custom_theme("bad-width");
        theme.launcher_width_px = 901;
        invalid_width.appearance.custom_themes.push(theme);
        assert!(normalize_and_validate(invalid_width).is_err());

        let mut remote_wallpaper = AppConfig::default();
        let mut theme = custom_theme("bad-wallpaper");
        theme.wallpaper_data_url = "https://example.com/background.png".into();
        remote_wallpaper.appearance.custom_themes.push(theme);
        assert!(normalize_and_validate(remote_wallpaper).is_err());
    }

    #[test]
    fn enforces_wallpaper_decoded_byte_limit_and_base64_padding() {
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url(MAX_THEME_WALLPAPER_BYTES),
            "边界皮肤"
        )
        .is_ok());
        assert!(validate_wallpaper_data_url(
            &wallpaper_data_url(MAX_THEME_WALLPAPER_BYTES + 1),
            "超限皮肤"
        )
        .is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,A=AA", "错误填充").is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,AAA", "错误长度").is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,AB==", "非规范填充").is_err());
        assert!(validate_wallpaper_data_url("data:image/png;base64,AAB=", "非规范尾位").is_err());
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
        let state = ConfigState {
            path: path.clone(),
            config: RwLock::new(AppConfig::default()),
            load_warning: RwLock::new(None),
            needs_legacy_preferences_migration: RwLock::new(false),
            save_lock: Mutex::new(()),
            incompatible_newer_version: None,
        };
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
}
