use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, RwLock,
};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, State};

use crate::{
    catalog::{self, CatalogEntry},
    everything::{self, EverythingOutcome},
    i18n,
    models::{LauncherPreferences, ResultAction, SearchResponse, SearchResult},
    scripts,
};

static PENDING_SHOW: AtomicBool = AtomicBool::new(false);

pub struct LauncherState {
    applications: RwLock<Vec<CatalogEntry>>,
    files: RwLock<Vec<CatalogEntry>>,
    indexing: AtomicBool,
    hotkey_status: RwLock<String>,
    search_generation: AtomicU64,
    keep_visible_on_blur: AtomicBool,
    close_on_blur: AtomicBool,
    keep_last_input: AtomicBool,
}

impl LauncherState {
    pub fn new() -> Self {
        Self {
            applications: RwLock::new(catalog::discover_applications()),
            files: RwLock::new(Vec::new()),
            indexing: AtomicBool::new(false),
            hotkey_status: RwLock::new("正在注册默认快捷键".into()),
            search_generation: AtomicU64::new(0),
            keep_visible_on_blur: AtomicBool::new(false),
            close_on_blur: AtomicBool::new(true),
            keep_last_input: AtomicBool::new(false),
        }
    }

    pub fn start_file_index(state: Arc<Self>) {
        if state.indexing.swap(true, Ordering::SeqCst) {
            return;
        }
        std::thread::spawn(move || {
            let files = catalog::build_limited_file_index();
            if let Ok(mut index) = state.files.write() {
                *index = files;
            }
            state.indexing.store(false, Ordering::SeqCst);
        });
    }

    pub fn set_hotkey_status(&self, value: String) {
        if let Ok(mut status) = self.hotkey_status.write() {
            *status = value;
        }
    }

    fn hotkey_status(&self) -> String {
        self.hotkey_status
            .read()
            .map(|value| value.clone())
            .unwrap_or_else(|_| "热键状态不可用".into())
    }

    fn file_count(&self) -> usize {
        self.files.read().map(|files| files.len()).unwrap_or(0)
    }

    fn advance_search_generation(&self, generation: u64) {
        self.search_generation
            .fetch_max(generation, Ordering::SeqCst);
    }

    fn search_is_cancelled(&self, generation: u64) -> bool {
        self.search_generation.load(Ordering::SeqCst) != generation
    }

    pub fn keep_visible_on_next_blur(&self, keep_visible: bool) {
        self.keep_visible_on_blur
            .store(keep_visible, Ordering::SeqCst);
    }

    pub fn consume_keep_visible_on_blur(&self) -> bool {
        self.keep_visible_on_blur.swap(false, Ordering::SeqCst)
    }

    pub fn close_on_blur(&self) -> bool {
        self.close_on_blur.load(Ordering::SeqCst)
    }

    fn preferences(&self) -> LauncherPreferences {
        LauncherPreferences {
            close_on_blur: self.close_on_blur.load(Ordering::SeqCst),
            keep_last_input: self.keep_last_input.load(Ordering::SeqCst),
        }
    }

    fn update_preferences(&self, close_on_blur: bool, keep_last_input: bool) {
        self.close_on_blur.store(close_on_blur, Ordering::SeqCst);
        self.keep_last_input
            .store(keep_last_input, Ordering::SeqCst);
    }
}

#[tauri::command]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
pub async fn search_launcher(
    app: AppHandle,
    state: State<'_, Arc<LauncherState>>,
    query: String,
    generation: u64,
) -> Result<SearchResponse, String> {
    let state = state.inner().clone();
    state.advance_search_generation(generation);
    tauri::async_runtime::spawn_blocking(move || {
        search_launcher_blocking(app, state, query, generation)
    })
    .await
    .map_err(|error| format!("搜索任务异常结束：{error}"))
}

fn search_launcher_blocking(
    app: AppHandle,
    state: Arc<LauncherState>,
    query: String,
    generation: u64,
) -> SearchResponse {
    let query = query.trim().to_string();
    if state.search_is_cancelled(generation) {
        return cancelled_response(&state, query);
    }
    let mut provider = "本地应用 + 限定目录索引".to_string();
    let mut provider_detail = if state.indexing.load(Ordering::SeqCst) {
        "正在后台建立限定目录索引".to_string()
    } else {
        format!("已索引 {} 个文件", state.file_count())
    };

    let mut results = if query.is_empty() {
        state
            .applications
            .read()
            .map(|apps| {
                catalog_results(&apps, "", "app", "应用", 8, 500, || {
                    state.search_is_cancelled(generation)
                })
            })
            .unwrap_or_default()
    } else if is_settings_query(&query) {
        provider = i18n::SETTINGS_PROVIDER.into();
        provider_detail = i18n::SETTINGS_PROVIDER_DETAIL.into();
        vec![SearchResult {
            id: "settings:open".into(),
            title: i18n::SETTINGS_RESULT_TITLE.into(),
            subtitle: i18n::SETTINGS_RESULT_SUBTITLE.into(),
            kind: "settings".into(),
            badge: i18n::SETTINGS_BADGE.into(),
            score: 2_100,
            action: ResultAction::OpenSettings,
        }]
    } else if let Some(value) = calculate(&query) {
        provider = "计算器".into();
        provider_detail = "本地计算，不访问网络".into();
        vec![SearchResult {
            id: format!("calculator:{query}"),
            title: value.clone(),
            subtitle: query.clone(),
            kind: "calculator".into(),
            badge: "计算".into(),
            score: 2_000,
            action: ResultAction::CopyText { text: value },
        }]
    } else if let Some(arguments) = command_arguments(&query, "ts") {
        provider = "脚本命令 · ts".into();
        provider_detail = "Python · 参数数组模式 · 立即执行".into();
        let args = arguments
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        match scripts::run_timestamp(&app, &args, || state.search_is_cancelled(generation)) {
            Ok(output) => vec![SearchResult {
                id: format!("script:ts:{arguments}"),
                title: output.clone(),
                subtitle: format!("timestamp.py {arguments}"),
                kind: "script".into(),
                badge: "脚本".into(),
                score: 2_000,
                action: ResultAction::CopyText { text: output },
            }],
            Err(error) => vec![SearchResult {
                id: "script:ts:error".into(),
                title: error,
                subtitle: "检查参数或 Python 解释器".into(),
                kind: "error".into(),
                badge: "错误".into(),
                score: 2_000,
                action: ResultAction::None,
            }],
        }
    } else if let Some(arguments) = command_arguments(&query, "google") {
        provider = "自定义网络搜索".into();
        provider_detail = "按 Enter 使用系统默认浏览器打开".into();
        if arguments.is_empty() {
            vec![hint_result("google <关键词>", "请输入要搜索的内容")]
        } else {
            let url = format!(
                "https://www.google.com.hk/search?q={}",
                urlencoding::encode(arguments)
            );
            vec![SearchResult {
                id: format!("web:google:{arguments}"),
                title: format!("Google 搜索：{arguments}"),
                subtitle: url.clone(),
                kind: "web".into(),
                badge: "网络".into(),
                score: 2_000,
                action: ResultAction::OpenUrl { url },
            }]
        }
    } else if let Some(arguments) = command_arguments(&query, "f") {
        if arguments.is_empty() {
            provider = "全盘文件搜索".into();
            provider_detail = "优先 Everything，失败时回退限定目录索引".into();
            vec![hint_result("f <文件名或路径>", "输入关键词开始搜索")]
        } else {
            match everything::search(&app, arguments, 12, || {
                state.search_is_cancelled(generation)
            }) {
                EverythingOutcome::Available(entries) => {
                    provider = "Everything".into();
                    provider_detail = "已连接 Everything IPC".into();
                    catalog_results(&entries, arguments, "file", "Everything", 12, 900, || {
                        state.search_is_cancelled(generation)
                    })
                }
                EverythingOutcome::Unavailable(reason) => {
                    provider = "Suo 限定目录索引".into();
                    provider_detail = format!("Everything 不可用：{reason}");
                    state
                        .files
                        .read()
                        .map(|files| {
                            catalog_results(&files, arguments, "file", "文件", 12, 700, || {
                                state.search_is_cancelled(generation)
                            })
                        })
                        .unwrap_or_default()
                }
                EverythingOutcome::Cancelled => {
                    provider = "搜索已取消".into();
                    provider_detail = "正在处理更新的查询".into();
                    Vec::new()
                }
            }
        }
    } else {
        let mut combined = state
            .applications
            .read()
            .map(|apps| {
                catalog_results(&apps, &query, "app", "应用", 8, 800, || {
                    state.search_is_cancelled(generation)
                })
            })
            .unwrap_or_default();
        if let Ok(files) = state.files.read() {
            combined.extend(catalog_results(
                &files,
                &query,
                "file",
                "文件",
                8,
                500,
                || state.search_is_cancelled(generation),
            ));
        }
        combined.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then(left.title.cmp(&right.title))
        });
        combined.truncate(8);
        combined
    };

    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(left.title.cmp(&right.title))
    });

    SearchResponse {
        query,
        provider,
        provider_detail,
        hotkey_status: state.hotkey_status(),
        indexing: state.indexing.load(Ordering::SeqCst),
        indexed_file_count: state.file_count(),
        results,
    }
}

fn cancelled_response(state: &LauncherState, query: String) -> SearchResponse {
    SearchResponse {
        query,
        provider: "搜索已取消".into(),
        provider_detail: "正在处理更新的查询".into(),
        hotkey_status: state.hotkey_status(),
        indexing: state.indexing.load(Ordering::SeqCst),
        indexed_file_count: state.file_count(),
        results: Vec::new(),
    }
}

#[tauri::command]
pub fn activate_result(
    app: AppHandle,
    state: State<'_, Arc<LauncherState>>,
    action: ResultAction,
    keep_open: bool,
) -> Result<(), String> {
    let may_move_focus = matches!(
        &action,
        ResultAction::OpenPath { .. } | ResultAction::OpenUrl { .. } | ResultAction::OpenSettings
    );
    state.keep_visible_on_next_blur(keep_open && may_move_focus);

    let result = match action {
        ResultAction::OpenPath { path } => {
            let path = std::path::PathBuf::from(path);
            if !path.exists() {
                return Err("目标已不存在".into());
            }
            open::that(path).map_err(|error| error.to_string())
        }
        ResultAction::OpenUrl { url } => {
            let parsed = tauri::Url::parse(&url).map_err(|error| error.to_string())?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err("只允许打开 HTTP/HTTPS 地址".into());
            }
            open::that(parsed.as_str()).map_err(|error| error.to_string())
        }
        ResultAction::OpenSettings => open_settings(app),
        ResultAction::CopyText { .. } | ResultAction::None => Ok(()),
    };
    if result.is_err() {
        state.keep_visible_on_next_blur(false);
    }
    result
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("settings")
        .ok_or_else(|| "找不到设置窗口".to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.center().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_launcher_preferences(state: State<'_, Arc<LauncherState>>) -> LauncherPreferences {
    state.preferences()
}

#[tauri::command]
pub fn update_launcher_preferences(
    state: State<'_, Arc<LauncherState>>,
    close_on_blur: bool,
    keep_last_input: bool,
) -> LauncherPreferences {
    state.update_preferences(close_on_blur, keep_last_input);
    state.preferences()
}

#[tauri::command]
pub fn cancel_search(state: State<'_, Arc<LauncherState>>, generation: u64) {
    state.advance_search_generation(generation);
}

#[tauri::command]
pub fn rebuild_file_index(state: State<'_, Arc<LauncherState>>) -> Result<(), String> {
    LauncherState::start_file_index(state.inner().clone());
    Ok(())
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|error| error.to_string())?;
        let _ = window.emit("launcher-hidden", ());
    }
    Ok(())
}

pub fn toggle_launcher(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        let _ = window.emit("launcher-hidden", ());
        return;
    }

    if let Err(error) = show_launcher(app) {
        eprintln!("无法显示 Suo 主窗口：{error}");
    }
}

pub fn request_show_launcher(app: &AppHandle) {
    // 先记录请求再尝试消费，避免与应用 setup 创建主窗口的时序交错而丢失唤醒。
    PENDING_SHOW.store(true, Ordering::SeqCst);
    show_pending_launcher(app);
}

pub fn show_pending_launcher(app: &AppHandle) {
    if app.get_webview_window("main").is_none() || !PENDING_SHOW.swap(false, Ordering::SeqCst) {
        return;
    }

    if let Err(error) = show_launcher(app) {
        PENDING_SHOW.store(true, Ordering::SeqCst);
        eprintln!("无法响应第二实例的窗口唤醒请求：{error}");
    }
}

pub fn show_launcher(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;

    window.unminimize().map_err(|error| error.to_string())?;
    let _ = position_launcher(app);
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    let _ = window.emit("launcher-shown", ());
    Ok(())
}

pub fn position_launcher(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    let cursor = app.cursor_position().map_err(|error| error.to_string())?;
    let monitor = app
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(|error| error.to_string())?
        .or(app.primary_monitor().map_err(|error| error.to_string())?)
        .ok_or_else(|| "找不到显示器".to_string())?;
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let window_size = window.outer_size().map_err(|error| error.to_string())?;

    let x = monitor_position.x + (monitor_size.width.saturating_sub(window_size.width) / 2) as i32;
    let y =
        monitor_position.y + (monitor_size.height.saturating_sub(window_size.height) / 4) as i32;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

fn catalog_results<F>(
    entries: &[CatalogEntry],
    query: &str,
    kind: &str,
    badge: &str,
    limit: usize,
    boost: i32,
    is_cancelled: F,
) -> Vec<SearchResult>
where
    F: Fn() -> bool,
{
    let normalized_query = query.to_lowercase();
    let mut best: Vec<(i32, &CatalogEntry)> = Vec::with_capacity(limit);

    for entry in entries {
        if is_cancelled() {
            break;
        }
        let score = if normalized_query.is_empty() {
            1
        } else {
            let Some(score) = match_score_normalized(
                &entry.normalized_name,
                &entry.normalized_path,
                &normalized_query,
            ) else {
                continue;
            };
            score
        } + boost;

        let position = best
            .iter()
            .position(|(existing_score, existing)| {
                score > *existing_score || (score == *existing_score && entry.name < existing.name)
            })
            .unwrap_or(best.len());
        if position < limit {
            best.insert(position, (score, entry));
            if best.len() > limit {
                best.pop();
            }
        } else if best.len() < limit {
            best.push((score, entry));
        }
    }

    best.into_iter()
        .map(|(score, entry)| {
            let path = entry.path.to_string_lossy().into_owned();
            SearchResult {
                id: format!("{kind}:{}", entry.normalized_path),
                title: entry.name.clone(),
                subtitle: path.clone(),
                kind: kind.into(),
                badge: badge.into(),
                score,
                action: ResultAction::OpenPath { path },
            }
        })
        .collect()
}

#[cfg(test)]
fn match_score(name: &str, path: &str, query: &str) -> Option<i32> {
    match_score_normalized(
        &name.to_lowercase(),
        &path.to_lowercase(),
        &query.to_lowercase(),
    )
}

fn match_score_normalized(name: &str, path: &str, query: &str) -> Option<i32> {
    if name == query {
        return Some(1_000);
    }
    if name.starts_with(&query) {
        return Some(900 - query.len() as i32);
    }
    if let Some(position) = name.find(&query) {
        return Some(760 - position as i32);
    }
    if let Some(position) = path.find(&query) {
        return Some(620 - position.min(200) as i32);
    }
    if is_subsequence(&name, &query) {
        return Some(420 - (name.len().saturating_sub(query.len())).min(200) as i32);
    }
    None
}

fn is_subsequence(value: &str, query: &str) -> bool {
    let mut query_chars = query.chars();
    let mut current = query_chars.next();
    for character in value.chars() {
        if current == Some(character) {
            current = query_chars.next();
            if current.is_none() {
                return true;
            }
        }
    }
    current.is_none()
}

fn calculate(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty()
        || !trimmed.chars().all(|character| {
            character.is_ascii_digit()
                || character.is_ascii_whitespace()
                || matches!(
                    character,
                    '.' | '+' | '-' | '*' | '/' | '%' | '(' | ')' | '^'
                )
        })
        || !trimmed
            .chars()
            .any(|character| "+-*/%^".contains(character))
    {
        return None;
    }

    let value = crate::calculator::evaluate(trimmed)?;
    if !value.is_finite() {
        return None;
    }
    if value.fract().abs() < f64::EPSILON {
        Some(format!("{value:.0}"))
    } else {
        Some(
            format!("{value:.10}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string(),
        )
    }
}

fn command_arguments<'a>(query: &'a str, command: &str) -> Option<&'a str> {
    let mut parts = query.splitn(2, char::is_whitespace);
    let keyword = parts.next()?;
    keyword
        .eq_ignore_ascii_case(command)
        .then(|| parts.next().unwrap_or("").trim())
}

fn is_settings_query(query: &str) -> bool {
    query.eq_ignore_ascii_case("setting")
        || query.eq_ignore_ascii_case("settings")
        || query == "设置"
}

fn hint_result(title: &str, subtitle: &str) -> SearchResult {
    SearchResult {
        id: format!("hint:{title}"),
        title: title.into(),
        subtitle: subtitle.into(),
        kind: "hint".into(),
        badge: "提示".into(),
        score: 1,
        action: ResultAction::None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::catalog::CatalogEntry;

    use super::{calculate, catalog_results, command_arguments, is_settings_query, match_score};

    #[test]
    fn evaluates_basic_calculation() {
        assert_eq!(calculate("11+1"), Some("12".into()));
        assert_eq!(calculate("(5 + 3) * 2"), Some("16".into()));
        assert_eq!(calculate("hello"), None);
    }

    #[test]
    fn parses_commands_case_insensitively() {
        assert_eq!(
            command_arguments("TS 1786082576069", "ts"),
            Some("1786082576069")
        );
        assert_eq!(command_arguments("timestamp 1", "ts"), None);
        assert!(is_settings_query("setting"));
        assert!(is_settings_query("SETTINGS"));
        assert!(is_settings_query("设置"));
        assert!(!is_settings_query("setting extra"));
    }

    #[test]
    fn exact_and_prefix_matches_rank_above_path_matches() {
        let exact = match_score("Visual Studio Code", "C:/Code.lnk", "visual studio code").unwrap();
        let prefix = match_score("Visual Studio Code", "C:/Code.lnk", "visual").unwrap();
        let path = match_score("Code", "C:/Visual/Code.lnk", "visual").unwrap();
        assert!(exact > prefix);
        assert!(prefix > path);
    }

    #[test]
    fn catalog_search_only_materializes_top_results() {
        let entries = (0..100)
            .map(|index| {
                CatalogEntry::from_path(PathBuf::from(format!("C:/files/item-{index:03}.txt")))
            })
            .collect::<Vec<_>>();

        let results = catalog_results(&entries, "item", "file", "文件", 8, 0, || false);
        assert_eq!(results.len(), 8);
        assert_eq!(results[0].title, "item-000");
        assert_eq!(results[7].title, "item-007");
    }
}
