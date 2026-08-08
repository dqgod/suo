use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::launcher::LauncherState;

const CONFIG_VERSION: u32 = 1;
const CREDENTIAL_SERVICE: &str = "io.github.dqgod.suo";
const TRANSLATOR_CREDENTIAL: &str = "microsoft-translator-api-key";
const MAX_COMMANDS: usize = 50;

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationConfig {
    pub enabled: bool,
    pub keyword: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub region: String,
    pub default_target_language: String,
    pub chinese_target_language: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCommandConfig {
    pub id: String,
    pub name: String,
    pub keyword: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub enabled: bool,
    pub runtime: ScriptRuntime,
    pub script_path: String,
    pub immediate: bool,
    pub timeout_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptRuntime {
    Python,
    PowerShell,
    Bash,
    Executable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSearchConfig {
    pub id: String,
    pub name: String,
    pub keyword: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub enabled: bool,
    pub url_template: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceConfig {
    pub theme: String,
    pub accent_color: String,
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
            },
            translation: TranslationConfig {
                enabled: true,
                keyword: "fy".into(),
                aliases: Vec::new(),
                region: String::new(),
                default_target_language: "zh-Hans".into(),
                chinese_target_language: "en".into(),
            },
            script_commands: vec![ScriptCommandConfig {
                id: "timestamp-example".into(),
                name: "时间戳转换".into(),
                keyword: "ts".into(),
                aliases: Vec::new(),
                enabled: true,
                runtime: ScriptRuntime::Python,
                script_path: "examples/timestamp.py".into(),
                immediate: true,
                timeout_ms: 3_000,
            }],
            web_searches: vec![WebSearchConfig {
                id: "google".into(),
                name: "Google".into(),
                keyword: "google".into(),
                aliases: Vec::new(),
                enabled: true,
                url_template: "https://www.google.com.hk/search?q={query}".into(),
            }],
            appearance: AppearanceConfig {
                theme: "midnight".into(),
                accent_color: "#8a78ff".into(),
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
        let incompatible_newer_version =
            config_file_version(&path).filter(|version| *version > u64::from(CONFIG_VERSION));
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

    fn replace(&self, config: AppConfig) -> Result<AppConfig, String> {
        let _save_guard = self
            .save_lock
            .lock()
            .map_err(|_| "配置保存锁暂时不可用".to_string())?;
        if let Some(version) = self.incompatible_newer_version {
            return Err(format!(
                "配置来自更新版本 v{version}，当前版本禁止覆盖，请升级 Suo"
            ));
        }
        let config = normalize_and_validate(config)?;
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
        Ok(config)
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

fn normalize_and_validate(mut config: AppConfig) -> Result<AppConfig, String> {
    if config.version > CONFIG_VERSION {
        return Err(format!(
            "配置来自更新版本（v{}），当前 Suo 仅支持 v{CONFIG_VERSION}，已拒绝覆盖",
            config.version
        ));
    }
    // v0 is the only legacy shape currently accepted; later migrations must
    // be explicit before increasing CONFIG_VERSION.
    config.version = CONFIG_VERSION;
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
    validate_appearance(&config.appearance)?;
    validate_keyword_namespace(&config)?;
    validate_unique_ids(&config)?;
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
    command.aliases = normalize_aliases(&command.aliases, "脚本命令")?;
    command.script_path = required_trimmed(&command.script_path, "脚本路径", 1_024)?;
    if command.script_path.starts_with("http://") || command.script_path.starts_with("https://") {
        return Err(format!("脚本 {} 不允许使用远程 URL", command.keyword));
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
    search.aliases = normalize_aliases(&search.aliases, "网络搜索命令")?;
    search.url_template = required_trimmed(&search.url_template, "网络搜索 URL 模板", 2_048)?;
    if !search.url_template.contains("{query}") {
        return Err(format!(
            "网络搜索 {} 的 URL 必须包含 {{query}}",
            search.keyword
        ));
    }
    let sample = search.url_template.replace("{query}", "test");
    let parsed = tauri::Url::parse(&sample)
        .map_err(|error| format!("网络搜索 {} 的 URL 无效：{error}", search.keyword))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("网络搜索 {} 只允许 HTTP/HTTPS URL", search.keyword));
    }
    Ok(())
}

fn validate_appearance(appearance: &AppearanceConfig) -> Result<(), String> {
    if !matches!(appearance.theme.as_str(), "midnight" | "paper" | "forest") {
        return Err("主题必须是 midnight、paper 或 forest".into());
    }
    let accent = appearance.accent_color.as_bytes();
    if accent.len() != 7 || accent[0] != b'#' || !accent[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err("强调色必须是 #RRGGBB 格式".into());
    }
    Ok(())
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
    let config = state.replace(config)?;
    launcher.update_preferences(
        config.launcher.close_on_blur,
        config.launcher.keep_last_input,
    );
    launcher.invalidate_provider_results();
    app.emit("app-config-updated", &config)
        .map_err(|error| format!("配置已保存，但无法通知窗口：{error}"))?;
    app.emit("provider-config-updated", ())
        .map_err(|error| format!("配置已保存，但无法刷新搜索结果：{error}"))?;
    Ok(state.view())
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

    #[test]
    fn defaults_are_valid() {
        normalize_and_validate(AppConfig::default()).expect("default config should be valid");
    }

    #[test]
    fn rejects_duplicate_command_keywords_case_insensitively() {
        let mut config = AppConfig::default();
        config.web_searches[0].keyword = "TS".into();
        assert!(normalize_and_validate(config).is_err());
    }

    #[test]
    fn rejects_web_template_without_query_placeholder() {
        let mut config = AppConfig::default();
        config.web_searches[0].url_template = "https://example.com".into();
        assert!(normalize_and_validate(config).is_err());
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
}
