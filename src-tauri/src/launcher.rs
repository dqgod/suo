use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State};

use crate::{
    arguments,
    catalog::{self, CatalogEntry},
    config::{AppConfig, ConfigState, ScriptCommandConfig, TranslationConfig, WebSearchConfig},
    file_search::{self, FileSearchOutcome},
    i18n,
    models::{CancelStatus, IndexStatus, ResultAction, ResultKind, SearchResponse, SearchResult},
    scripts, translator, web_search,
};

static PENDING_SHOW: AtomicBool = AtomicBool::new(false);
const DEFAULT_LAUNCHER_WIDTH: f64 = 720.0;
const LAUNCHER_FULL_HEIGHT: f64 = 520.0;
const LAUNCHER_COMPACT_HEIGHT: f64 = 74.0;

pub struct LauncherState {
    applications: RwLock<Vec<CatalogEntry>>,
    application_paths: HashMap<String, PathBuf>,
    files: RwLock<Vec<CatalogEntry>>,
    indexing: AtomicBool,
    hotkey_status: RwLock<String>,
    search_generation: AtomicU64,
    action_generation: AtomicU64,
    action_epoch: AtomicU64,
    action_gate: Mutex<()>,
    keep_visible_on_blur: AtomicBool,
    close_on_blur: AtomicBool,
    keep_last_input: AtomicBool,
}

impl LauncherState {
    pub fn new() -> Self {
        let applications = catalog::discover_applications();
        let application_paths = applications
            .iter()
            .map(|entry| {
                (
                    format!("app:{}", entry.path.to_string_lossy()),
                    entry.path.clone(),
                )
            })
            .collect();
        Self {
            applications: RwLock::new(applications),
            application_paths,
            files: RwLock::new(Vec::new()),
            indexing: AtomicBool::new(false),
            hotkey_status: RwLock::new("正在注册默认快捷键".into()),
            search_generation: AtomicU64::new(0),
            action_generation: AtomicU64::new(0),
            action_epoch: AtomicU64::new(0),
            action_gate: Mutex::new(()),
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

    pub fn application_path_for_result_id(&self, result_id: &str) -> Option<PathBuf> {
        self.application_paths.get(result_id).cloned()
    }

    fn search_is_cancelled(&self, generation: u64) -> bool {
        self.search_generation.load(Ordering::SeqCst) != generation
    }

    fn begin_search(&self, generation: u64) -> u64 {
        let Ok(_gate) = self.action_gate.lock() else {
            return self.action_epoch.load(Ordering::SeqCst);
        };
        if generation > self.search_generation.load(Ordering::SeqCst) {
            self.search_generation.store(generation, Ordering::SeqCst);
            self.cancel_actions();
            self.action_epoch.fetch_add(1, Ordering::SeqCst);
        }
        self.action_epoch.load(Ordering::SeqCst)
    }

    fn begin_action(&self, expected_epoch: u64) -> Result<u64, String> {
        let _gate = self
            .action_gate
            .lock()
            .map_err(|_| "脚本执行锁暂时不可用".to_string())?;
        if self.action_epoch.load(Ordering::SeqCst) != expected_epoch {
            return Err("查询已取消，未启动脚本".into());
        }
        Ok(self.action_generation.fetch_add(1, Ordering::SeqCst) + 1)
    }

    fn ensure_action_epoch(&self, expected_epoch: u64) -> Result<(), String> {
        let _gate = self
            .action_gate
            .lock()
            .map_err(|_| "操作授权锁暂时不可用".to_string())?;
        if self.action_epoch.load(Ordering::SeqCst) != expected_epoch {
            return Err("结果已失效，请重新搜索".into());
        }
        Ok(())
    }

    fn cancel_actions(&self) {
        self.action_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn action_is_cancelled(&self, generation: u64) -> bool {
        self.action_generation.load(Ordering::SeqCst) != generation
    }

    fn cancel_search_and_actions(&self, generation: u64) -> u64 {
        if let Ok(_gate) = self.action_gate.lock() {
            if generation > self.search_generation.load(Ordering::SeqCst) {
                self.search_generation.store(generation, Ordering::SeqCst);
                self.cancel_actions();
                self.action_epoch.fetch_add(1, Ordering::SeqCst);
            }
        }
        self.action_epoch.load(Ordering::SeqCst)
    }

    fn action_epoch(&self) -> u64 {
        self.action_epoch.load(Ordering::SeqCst)
    }

    pub fn invalidate_provider_results(&self) {
        if let Ok(_gate) = self.action_gate.lock() {
            let generation = self.search_generation.load(Ordering::SeqCst);
            self.search_generation
                .store(generation.saturating_add(1), Ordering::SeqCst);
            self.cancel_actions();
            self.action_epoch.fetch_add(1, Ordering::SeqCst);
        }
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

    pub fn update_preferences(&self, close_on_blur: bool, keep_last_input: bool) {
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
    config: State<'_, Arc<ConfigState>>,
    query: String,
    generation: u64,
) -> Result<SearchResponse, String> {
    let state = state.inner().clone();
    let config = config.snapshot();
    state.begin_search(generation);
    tauri::async_runtime::spawn_blocking(move || {
        search_launcher_blocking(app, state, config, query, generation)
    })
    .await
    .map_err(|error| format!("搜索任务异常结束：{error}"))
}

fn search_launcher_blocking(
    app: AppHandle,
    state: Arc<LauncherState>,
    config: AppConfig,
    query: String,
    generation: u64,
) -> SearchResponse {
    let query = query.trim().to_string();
    if state.search_is_cancelled(generation) {
        return cancelled_response(&state, query);
    }
    let mut provider = "本地应用 + 限定目录索引".to_string();
    let max_results = config.launcher_theme.max_results();
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
                catalog_results(
                    &apps,
                    "",
                    ResultKind::App,
                    "应用",
                    max_results,
                    500,
                    true,
                    || state.search_is_cancelled(generation),
                )
            })
            .unwrap_or_default()
    } else if is_settings_query(&query) {
        provider = i18n::SETTINGS_PROVIDER.into();
        provider_detail = i18n::SETTINGS_PROVIDER_DETAIL.into();
        vec![SearchResult {
            id: "settings:open".into(),
            title: i18n::SETTINGS_RESULT_TITLE.into(),
            subtitle: i18n::SETTINGS_RESULT_SUBTITLE.into(),
            kind: ResultKind::Settings,
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
            kind: ResultKind::Calculator,
            badge: "计算".into(),
            score: 2_000,
            action: ResultAction::CopyText { text: value },
        }]
    } else if let Some((translation, arguments, explicit_target)) =
        translation_command(&config.translation, &query)
    {
        provider = "微软翻译".into();
        provider_detail = "输入停顿 50 ms 后翻译；结果可直接复制".into();
        if arguments.is_empty() {
            vec![hint_result(
                &format!("{}[:目标语言] <文本>", translation.keyword),
                "例如：fy hello 或 fy:ja hello",
            )]
        } else {
            let target = explicit_target
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| translator::target_language(translation, arguments));
            match translator::translate(translation, arguments, &target, || {
                state.search_is_cancelled(generation)
            }) {
                Ok(output) => vec![SearchResult {
                    id: format!("translate:{target}:{arguments}"),
                    title: output.clone(),
                    subtitle: format!("微软翻译 · → {target} · {arguments}"),
                    kind: ResultKind::Translation,
                    badge: "翻译".into(),
                    score: 2_050,
                    action: ResultAction::CopyText { text: output },
                }],
                Err(error) => vec![SearchResult {
                    id: "translate:error".into(),
                    title: error.clone(),
                    subtitle: if error.contains("尚未配置") {
                        "按 Enter 打开设置并配置翻译服务".into()
                    } else {
                        "检查网络、区域或微软翻译配置".into()
                    },
                    kind: ResultKind::Error,
                    badge: "翻译".into(),
                    score: 2_050,
                    action: if error.contains("尚未配置") {
                        ResultAction::OpenSettings
                    } else {
                        ResultAction::None
                    },
                }],
            }
        }
    } else if let Some((command, arguments)) = script_command(&config, &query) {
        provider = format!("脚本命令 · {}", command.keyword);
        provider_detail = if command.immediate {
            format!("参数数组模式 · 输入停顿 {} ms 后执行", command.debounce_ms)
        } else {
            "参数数组模式 · 按 Enter 执行".into()
        };
        match arguments::parse(arguments) {
            Err(error) => vec![error_result(
                format!("script:{}:args-error", command.id),
                error,
                "请检查参数引号",
            )],
            Ok(args) if command.immediate => {
                match scripts::run_configured(&app, command, &args, || {
                    state.search_is_cancelled(generation)
                }) {
                    Ok(output) => vec![script_output_result(command, arguments, output)],
                    Err(error) => vec![error_result(
                        format!("script:{}:error", command.id),
                        error,
                        "检查参数、脚本路径或解释器",
                    )],
                }
            }
            Ok(args) => vec![SearchResult {
                id: format!("script:{}:{arguments}", command.id),
                title: format!("运行 {}", command.name),
                subtitle: format!("{} {}", command.script_path, arguments),
                kind: ResultKind::Script,
                badge: "按 Enter".into(),
                score: 2_000,
                action: ResultAction::RunScript {
                    command_id: command.id.clone(),
                    args,
                },
            }],
        }
    } else if let Some((search, arguments)) = web_search_command(&config, &query) {
        provider = format!("自定义网络搜索 · {}", search.name);
        provider_detail = "按 Enter 使用系统默认浏览器打开".into();
        if arguments.is_empty() {
            vec![hint_result(
                &format!("{} <关键词>", search.keyword),
                "请输入要搜索的内容",
            )]
        } else {
            match web_search::expand_url(&search.url_template, arguments) {
                Ok(url) => vec![SearchResult {
                    id: format!("web:{}:{arguments}", search.id),
                    title: format!("{} 搜索：{arguments}", search.name),
                    subtitle: url.clone(),
                    kind: ResultKind::Web,
                    badge: "网络".into(),
                    score: 2_000,
                    action: ResultAction::OpenUrl { url },
                }],
                Err(error) => vec![error_result(
                    format!("web:{}:args-error", search.id),
                    error,
                    "请补充参数或检查引号",
                )],
            }
        }
    } else if let Some(arguments) = command_arguments(&query, "f") {
        if arguments.is_empty() {
            provider = "全盘文件搜索".into();
            provider_detail = file_search::provider_hint().into();
            vec![hint_result("f <文件名或路径>", "输入关键词开始搜索")]
        } else {
            match file_search::search(&app, arguments, max_results, || {
                state.search_is_cancelled(generation)
            }) {
                FileSearchOutcome::Available {
                    provider: source,
                    detail,
                    entries,
                } => {
                    provider = source.into();
                    provider_detail = detail.into();
                    catalog_results(
                        &entries,
                        arguments,
                        ResultKind::File,
                        source,
                        max_results,
                        900,
                        false,
                        || state.search_is_cancelled(generation),
                    )
                }
                FileSearchOutcome::Unavailable(reason) => {
                    if state.file_count() == 0 {
                        LauncherState::start_file_index(state.clone());
                    }
                    provider = "Suo 限定目录索引".into();
                    provider_detail = format!("系统索引不可用：{reason}");
                    state
                        .files
                        .read()
                        .map(|files| {
                            catalog_results(
                                &files,
                                arguments,
                                ResultKind::File,
                                "文件",
                                max_results,
                                700,
                                false,
                                || state.search_is_cancelled(generation),
                            )
                        })
                        .unwrap_or_default()
                }
                FileSearchOutcome::Cancelled => {
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
                catalog_results(
                    &apps,
                    &query,
                    ResultKind::App,
                    "应用",
                    max_results,
                    800,
                    true,
                    || state.search_is_cancelled(generation),
                )
            })
            .unwrap_or_default();
        if let Ok(files) = state.files.read() {
            combined.extend(catalog_results(
                &files,
                &query,
                ResultKind::File,
                "文件",
                max_results,
                500,
                false,
                || state.search_is_cancelled(generation),
            ));
        }
        combined.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then(left.title.cmp(&right.title))
        });
        combined.truncate(max_results);
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
        action_epoch: state.action_epoch(),
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
        action_epoch: state.action_epoch(),
        results: Vec::new(),
    }
}

#[tauri::command]
pub async fn activate_result(
    app: AppHandle,
    state: State<'_, Arc<LauncherState>>,
    config: State<'_, Arc<ConfigState>>,
    action: ResultAction,
    keep_open: bool,
    action_epoch: u64,
) -> Result<Option<SearchResult>, String> {
    let may_move_focus = matches!(
        &action,
        ResultAction::OpenPath { .. } | ResultAction::OpenUrl { .. } | ResultAction::OpenSettings
    );
    state.keep_visible_on_next_blur(keep_open && may_move_focus);

    let result = match action {
        ResultAction::OpenPath { path } => {
            state.ensure_action_epoch(action_epoch)?;
            let path = std::path::PathBuf::from(path);
            if !path.exists() {
                return Err("目标已不存在".into());
            }
            open::that(path)
                .map(|_| None)
                .map_err(|error| error.to_string())
        }
        ResultAction::OpenUrl { url } => {
            state.ensure_action_epoch(action_epoch)?;
            let parsed = tauri::Url::parse(&url).map_err(|error| error.to_string())?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err("只允许打开 HTTP/HTTPS 地址".into());
            }
            open::that(parsed.as_str())
                .map(|_| None)
                .map_err(|error| error.to_string())
        }
        ResultAction::RunScript { command_id, args } => {
            let Some(command) = config
                .snapshot()
                .script_commands
                .into_iter()
                .find(|command| command.id == command_id && command.enabled)
            else {
                return Err("脚本命令已被删除或禁用".into());
            };
            let script_state = state.inner().clone();
            let generation = script_state.begin_action(action_epoch)?;
            let script_app = app.clone();
            let output = tauri::async_runtime::spawn_blocking(move || {
                scripts::run_configured(&script_app, &command, &args, || {
                    script_state.action_is_cancelled(generation)
                })
                .map(|output| script_output_result(&command, &args.join(" "), output))
            })
            .await
            .map_err(|error| format!("脚本任务异常结束：{error}"))??;
            Ok(Some(output))
        }
        ResultAction::OpenSettings => {
            state.ensure_action_epoch(action_epoch)?;
            open_settings(app).map(|_| None)
        }
        ResultAction::CopyText { .. } | ResultAction::None => Ok(None),
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
pub fn cancel_search(state: State<'_, Arc<LauncherState>>, generation: u64) -> CancelStatus {
    CancelStatus {
        action_epoch: state.cancel_search_and_actions(generation),
    }
}

#[tauri::command]
pub fn rebuild_file_index(app: AppHandle, state: State<'_, Arc<LauncherState>>) -> IndexStatus {
    LauncherState::start_file_index(state.inner().clone());
    let _ = app.emit("file-index-started", ());
    index_status(state)
}

#[tauri::command]
pub fn get_index_status(state: State<'_, Arc<LauncherState>>) -> IndexStatus {
    index_status(state)
}

fn index_status(state: State<'_, Arc<LauncherState>>) -> IndexStatus {
    IndexStatus {
        indexing: state.indexing.load(Ordering::SeqCst),
        indexed_file_count: state.file_count(),
    }
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.hide().map_err(|error| error.to_string())?;
        let _ = window.emit("launcher-hidden", ());
    }
    Ok(())
}

#[tauri::command]
pub fn set_launcher_compact(
    app: AppHandle,
    config: State<'_, Arc<ConfigState>>,
    compact: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    let scale_factor = window.scale_factor().map_err(|error| error.to_string())?;
    let current = window
        .inner_size()
        .map_err(|error| error.to_string())?
        .to_logical::<f64>(scale_factor);
    let target_height = if compact {
        LAUNCHER_COMPACT_HEIGHT
    } else {
        LAUNCHER_FULL_HEIGHT
    };
    let target_width = config.snapshot().launcher_theme.launcher_width();
    if (current.height - target_height).abs() < 0.5 && (current.width - target_width).abs() < 0.5 {
        return Ok(());
    }
    window
        .set_size(LogicalSize::new(target_width, target_height))
        .map_err(|error| error.to_string())
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
    // Keep the launcher's top edge stable in compact and full modes. Using
    // the current compact height here would make later invocations drift down.
    // Use the destination monitor's DPI because the hidden window may still
    // belong to a different monitor when this position is calculated.
    let launcher_width = app
        .try_state::<Arc<ConfigState>>()
        .map(|config| config.snapshot().launcher_theme.launcher_width())
        .unwrap_or(DEFAULT_LAUNCHER_WIDTH);
    let positioning_width = (launcher_width * monitor.scale_factor()).round() as u32;
    let positioning_height = (LAUNCHER_FULL_HEIGHT * monitor.scale_factor()).round() as u32;

    let x = monitor_position.x + (monitor_size.width.saturating_sub(positioning_width) / 2) as i32;
    let y =
        monitor_position.y + (monitor_size.height.saturating_sub(positioning_height) / 4) as i32;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

fn catalog_results<F>(
    entries: &[CatalogEntry],
    query: &str,
    kind: ResultKind,
    badge: &str,
    limit: usize,
    boost: i32,
    allow_pinyin: bool,
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
                allow_pinyin.then_some((&entry.pinyin_name, &entry.pinyin_initials)),
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
            let (result_kind, result_badge) = if kind == ResultKind::File && entry.is_directory {
                (ResultKind::Directory, "文件夹")
            } else {
                (kind, badge)
            };
            SearchResult {
                id: format!("{}:{path}", result_kind.as_str()),
                title: entry.name.clone(),
                subtitle: path.clone(),
                kind: result_kind,
                badge: result_badge.into(),
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
        None,
    )
}

fn match_score_normalized(
    name: &str,
    path: &str,
    query: &str,
    pinyin: Option<(&str, &str)>,
) -> Option<i32> {
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
    // Pinyin is intentionally only a fallback after native text/path matching.
    // Its best score remains below an exact file result after catalog boosts,
    // so an English/file exact match keeps its established priority.
    if query.is_ascii()
        && query
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        if let Some((full, initials)) = pinyin {
            if full == query {
                return Some(680);
            }
            if full.starts_with(query) {
                return Some(640 - query.len() as i32);
            }
            if let Some(position) = full.find(query) {
                return Some(580 - position.min(200) as i32);
            }
            if initials == query {
                return Some(540);
            }
            if initials.starts_with(query) {
                return Some(500 - query.len() as i32);
            }
        }
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

fn query_command_parts(query: &str) -> (&str, &str) {
    let mut parts = query.splitn(2, char::is_whitespace);
    let keyword = parts.next().unwrap_or("");
    let arguments = parts.next().unwrap_or("").trim();
    (keyword, arguments)
}

fn keyword_matches(keyword: &str, primary: &str, aliases: &[String]) -> bool {
    keyword.eq_ignore_ascii_case(primary)
        || aliases
            .iter()
            .any(|alias| keyword.eq_ignore_ascii_case(alias))
}

fn translation_command<'config, 'query>(
    config: &'config TranslationConfig,
    query: &'query str,
) -> Option<(&'config TranslationConfig, &'query str, Option<&'query str>)> {
    if !config.enabled {
        return None;
    }
    let (token, arguments) = query_command_parts(query);
    let (keyword, target) = token
        .split_once(':')
        .map_or((token, None), |(keyword, target)| (keyword, Some(target)));
    keyword_matches(keyword, &config.keyword, &config.aliases)
        .then_some((config, arguments, target))
}

fn script_command<'config, 'query>(
    config: &'config AppConfig,
    query: &'query str,
) -> Option<(&'config ScriptCommandConfig, &'query str)> {
    let (keyword, arguments) = query_command_parts(query);
    config.script_commands.iter().find_map(|command| {
        (command.enabled && keyword_matches(keyword, &command.keyword, &command.aliases))
            .then_some((command, arguments))
    })
}

fn web_search_command<'config, 'query>(
    config: &'config AppConfig,
    query: &'query str,
) -> Option<(&'config WebSearchConfig, &'query str)> {
    let (keyword, arguments) = query_command_parts(query);
    config.web_searches.iter().find_map(|search| {
        (search.enabled && keyword_matches(keyword, &search.keyword, &search.aliases))
            .then_some((search, arguments))
    })
}

fn script_output_result(
    command: &ScriptCommandConfig,
    arguments: &str,
    output: String,
) -> SearchResult {
    SearchResult {
        id: format!("script:{}:{arguments}:output", command.id),
        title: if output.is_empty() {
            "脚本执行完成（无输出）".into()
        } else {
            output.clone()
        },
        subtitle: format!("{} {}", command.script_path, arguments),
        kind: ResultKind::Script,
        badge: "脚本".into(),
        score: 2_000,
        action: ResultAction::CopyText { text: output },
    }
}

fn error_result(id: String, title: String, subtitle: &str) -> SearchResult {
    SearchResult {
        id,
        title,
        subtitle: subtitle.into(),
        kind: ResultKind::Error,
        badge: "错误".into(),
        score: 2_000,
        action: ResultAction::None,
    }
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
        kind: ResultKind::Hint,
        badge: "提示".into(),
        score: 1,
        action: ResultAction::None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{catalog::CatalogEntry, config::AppConfig, models::ResultKind};

    use super::{
        calculate, catalog_results, command_arguments, is_settings_query, match_score,
        script_command, translation_command, web_search_command,
    };

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
                CatalogEntry::from_path_with_type(
                    PathBuf::from(format!("C:/files/item-{index:03}.txt")),
                    false,
                )
            })
            .collect::<Vec<_>>();

        let results = catalog_results(
            &entries,
            "item",
            ResultKind::File,
            "文件",
            8,
            0,
            false,
            || false,
        );
        assert_eq!(results.len(), 8);
        assert_eq!(results[0].title, "item-000");
        assert_eq!(results[7].title, "item-007");
    }

    #[test]
    fn catalog_results_distinguish_folders_from_files() {
        let entries = vec![
            CatalogEntry::from_path_with_type(PathBuf::from("C:/files/folder"), true),
            CatalogEntry::from_path_with_type(PathBuf::from("C:/files/report.txt"), false),
        ];

        let results = catalog_results(
            &entries,
            "",
            ResultKind::File,
            "文件",
            8,
            0,
            false,
            || false,
        );
        assert_eq!(results[0].kind, ResultKind::Directory);
        assert_eq!(results[0].badge, "文件夹");
        assert_eq!(results[1].kind, ResultKind::File);
        assert_eq!(results[1].badge, "文件");
    }

    #[test]
    fn chinese_applications_match_full_pinyin_without_displacing_english_matches() {
        let entries = vec![
            CatalogEntry::from_application_path_with_type(PathBuf::from("C:/apps/微信.lnk"), false),
            CatalogEntry::from_application_path_with_type(
                PathBuf::from("C:/apps/Weixin.lnk"),
                false,
            ),
        ];

        let results = catalog_results(
            &entries,
            "weixin",
            ResultKind::App,
            "应用",
            8,
            800,
            true,
            || false,
        );

        assert_eq!(results[0].title, "Weixin");
        assert!(results.iter().any(|result| result.title == "微信"));
    }

    #[test]
    fn pinyin_catalog_matching_keeps_the_top_k_limit() {
        let entries = (0..100)
            .map(|index| {
                CatalogEntry::from_application_path_with_type(
                    PathBuf::from(format!("C:/apps/微信工具-{index:03}.lnk")),
                    false,
                )
            })
            .collect::<Vec<_>>();

        let results = catalog_results(
            &entries,
            "weixin",
            ResultKind::App,
            "应用",
            8,
            800,
            true,
            || false,
        );

        assert_eq!(results.len(), 8);
        assert_eq!(results[0].title, "微信工具-000");
        assert_eq!(results[7].title, "微信工具-007");
    }

    #[test]
    fn pinyin_catalog_matching_still_stops_on_cancellation() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let entries = (0..100)
            .map(|index| {
                CatalogEntry::from_application_path_with_type(
                    PathBuf::from(format!("C:/apps/微信工具-{index:03}.lnk")),
                    false,
                )
            })
            .collect::<Vec<_>>();
        let checks = AtomicUsize::new(0);

        let results = catalog_results(
            &entries,
            "weixin",
            ResultKind::App,
            "应用",
            8,
            800,
            true,
            || checks.fetch_add(1, Ordering::SeqCst) >= 5,
        );

        assert!(checks.load(Ordering::SeqCst) <= 6);
        assert!(results.len() <= 5);
    }

    #[test]
    fn pinyin_is_not_enabled_for_file_catalog_results() {
        let entries = vec![CatalogEntry::from_path_with_type(
            PathBuf::from("C:/files/微信.txt"),
            false,
        )];

        let results = catalog_results(
            &entries,
            "weixin",
            ResultKind::File,
            "文件",
            8,
            500,
            false,
            || false,
        );

        assert!(results.is_empty());
    }

    #[test]
    fn disabled_commands_and_services_do_not_match() {
        let mut config = AppConfig::default();
        config.translation.enabled = false;
        config.script_commands[0].enabled = false;
        config.web_searches[0].enabled = false;

        assert!(translation_command(&config.translation, "fy hello").is_none());
        assert!(script_command(&config, "ts 123456").is_none());
        assert!(web_search_command(&config, "google codex").is_none());
    }

    #[test]
    fn action_cancellation_is_independent_from_search_generation() {
        let state = super::LauncherState::new();
        let epoch = state.begin_search(42);
        let action = state.begin_action(epoch).unwrap();
        assert!(!state.action_is_cancelled(action));
        let next_epoch = state.cancel_search_and_actions(43);
        assert!(state.action_is_cancelled(action));
        assert_ne!(epoch, next_epoch);
        assert!(state.begin_action(epoch).is_err());
        assert!(state.begin_action(next_epoch).is_ok());
    }

    #[test]
    fn stale_cancellation_cannot_invalidate_a_newer_search() {
        let state = super::LauncherState::new();
        let epoch = state.begin_search(10);
        assert_eq!(state.cancel_search_and_actions(9), epoch);
        assert_eq!(state.action_epoch(), epoch);
    }
}
