use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
};

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, State};

use crate::{
    arguments,
    catalog::{self, CatalogEntry},
    config::{
        AppConfig, ConfigState, ScriptCommandConfig, ScriptResultAction, TranslationConfig,
        WebSearchConfig,
    },
    dock,
    file_search::{self, FileSearchOutcome},
    focus, i18n,
    models::{CancelStatus, IndexStatus, ResultAction, ResultKind, SearchResponse, SearchResult},
    scripts, translator, web_search,
};

static PENDING_SHOW: AtomicBool = AtomicBool::new(false);
const DEFAULT_LAUNCHER_WIDTH: f64 = 720.0;
const DEFAULT_LAUNCHER_HEIGHT: f64 = 520.0;
const LAUNCHER_COMPACT_HEIGHT: f64 = 74.0;

#[derive(Debug)]
struct PendingScriptOutputAction {
    command_id: String,
    shell_command: String,
}

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
    pending_script_output_actions: Mutex<HashMap<String, PendingScriptOutputAction>>,
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
            pending_script_output_actions: Mutex::new(HashMap::new()),
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
            self.invalidate_actions_locked();
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

    fn register_script_output_action(
        &self,
        expected_epoch: u64,
        command_id: String,
        shell_command: String,
    ) -> Result<String, String> {
        let _gate = self
            .action_gate
            .lock()
            .map_err(|_| "操作授权锁暂时不可用".to_string())?;
        if self.action_epoch.load(Ordering::SeqCst) != expected_epoch {
            return Err("结果已失效，请重新运行脚本".into());
        }
        let action_id = uuid::Uuid::new_v4().to_string();
        self.pending_script_output_actions
            .lock()
            .map_err(|_| "脚本返回值授权暂时不可用".to_string())?
            .insert(
                action_id.clone(),
                PendingScriptOutputAction {
                    command_id,
                    shell_command,
                },
            );
        Ok(action_id)
    }

    fn begin_script_output_action(
        &self,
        expected_epoch: u64,
        action_id: &str,
    ) -> Result<(u64, PendingScriptOutputAction), String> {
        let _gate = self
            .action_gate
            .lock()
            .map_err(|_| "操作授权锁暂时不可用".to_string())?;
        if self.action_epoch.load(Ordering::SeqCst) != expected_epoch {
            return Err("结果已失效，请重新运行脚本".into());
        }
        let pending = self
            .pending_script_output_actions
            .lock()
            .map_err(|_| "脚本返回值授权暂时不可用".to_string())?
            .remove(action_id)
            .ok_or_else(|| "脚本返回值已经执行或失效，请重新运行脚本".to_string())?;
        let generation = self.action_generation.fetch_add(1, Ordering::SeqCst) + 1;
        Ok((generation, pending))
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

    fn invalidate_actions_locked(&self) {
        self.cancel_actions();
        if let Ok(mut actions) = self.pending_script_output_actions.lock() {
            actions.clear();
        }
        self.action_epoch.fetch_add(1, Ordering::SeqCst);
    }

    fn action_is_cancelled(&self, generation: u64) -> bool {
        self.action_generation.load(Ordering::SeqCst) != generation
    }

    fn cancel_search_and_actions(&self, generation: u64) -> u64 {
        if let Ok(_gate) = self.action_gate.lock() {
            if generation > self.search_generation.load(Ordering::SeqCst) {
                self.search_generation.store(generation, Ordering::SeqCst);
                self.invalidate_actions_locked();
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
            self.invalidate_actions_locked();
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
    let action_epoch = state.begin_search(generation);
    tauri::async_runtime::spawn_blocking(move || {
        search_launcher_blocking(app, state, config, query, generation, action_epoch)
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
    action_epoch: u64,
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
            icon_data_url: String::new(),
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
            icon_data_url: String::new(),
            badge: "计算".into(),
            score: 2_000,
            action: ResultAction::CopyText { text: value },
        }]
    } else if let Some((translation, arguments, explicit_target)) =
        translation_command(&config.translation, &query)
    {
        let provider_name = translation.provider.display_name();
        provider = provider_name.into();
        provider_detail = format!("{provider_name} · 输入停顿后翻译；结果可直接复制");
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
                    subtitle: format!("{provider_name} · → {target} · {arguments}"),
                    kind: ResultKind::Translation,
                    icon_data_url: String::new(),
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
                        "检查网络、目标语言或当前翻译提供方配置".into()
                    },
                    kind: ResultKind::Error,
                    icon_data_url: String::new(),
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
            Err(error) => vec![error_result_with_icon(
                format!("script:{}:args-error", command.id),
                error,
                "请检查参数引号",
                &command.icon_data_url,
            )],
            Ok(args) if command.immediate => {
                match scripts::run_configured(&app, command, &args, || {
                    state.search_is_cancelled(generation)
                }) {
                    Ok(output) => {
                        match script_output_result(&state, action_epoch, command, arguments, output)
                        {
                            Ok(result) => vec![result],
                            Err(error) => vec![error_result_with_icon(
                                format!("script:{}:output-error", command.id),
                                error,
                                "检查脚本返回值或重新运行脚本",
                                &command.icon_data_url,
                            )],
                        }
                    }
                    Err(error) => vec![error_result_with_icon(
                        format!("script:{}:error", command.id),
                        error,
                        "检查参数、脚本路径或解释器",
                        &command.icon_data_url,
                    )],
                }
            }
            Ok(args) => vec![SearchResult {
                id: format!("script:{}:{arguments}", command.id),
                title: format!("运行 {}", command.name),
                subtitle: script_action_subtitle(command, arguments),
                kind: ResultKind::Script,
                icon_data_url: command.icon_data_url.clone(),
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
        web_search_results(search, arguments)
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
        ResultAction::OpenPath { .. }
            | ResultAction::OpenUrl { .. }
            | ResultAction::RunScriptOutput { .. }
            | ResultAction::OpenSettings
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
                let output = scripts::run_configured(&script_app, &command, &args, || {
                    script_state.action_is_cancelled(generation)
                })?;
                script_output_result(
                    &script_state,
                    action_epoch,
                    &command,
                    &args.join(" "),
                    output,
                )
            })
            .await
            .map_err(|error| format!("脚本任务异常结束：{error}"))??;
            Ok(Some(output))
        }
        ResultAction::RunScriptOutput { action_id } => {
            let (generation, pending) =
                state.begin_script_output_action(action_epoch, &action_id)?;
            let Some(command) = config
                .snapshot()
                .script_commands
                .into_iter()
                .find(|command| {
                    command.id == pending.command_id
                        && command.enabled
                        && command.result_action == ScriptResultAction::ExecuteShell
                })
            else {
                return Err("脚本命令已被删除、禁用或不再允许执行返回值".into());
            };
            let shell_state = state.inner().clone();
            let shell_app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                scripts::run_result_shell(&shell_app, &command, &pending.shell_command, || {
                    shell_state.action_is_cancelled(generation)
                })
            })
            .await
            .map_err(|error| format!("Shell 任务异常结束：{error}"))??;
            Ok(None)
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
    let show_dock_icon = app
        .state::<Arc<ConfigState>>()
        .snapshot()
        .launcher
        .show_dock_icon;
    let window = app
        .get_webview_window("settings")
        .ok_or_else(|| "找不到设置窗口".to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.center().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    dock::settings_opened(&app, show_dock_icon)?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn hide_settings(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.hide().map_err(|error| error.to_string())?;
    }
    dock::settings_closed(&app)
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
    focus::forget_previous_application();
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
    let snapshot = config.snapshot();
    let target = configured_launcher_size(&snapshot, compact);
    let target_width = target.width;
    let target_height = target.height;
    if (current.height - target_height).abs() < 0.5 && (current.width - target_width).abs() < 0.5 {
        return Ok(());
    }
    window.set_size(target).map_err(|error| error.to_string())
}

pub fn prepare_launcher_window(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    // Apply persisted geometry while the native window is still hidden. This
    // prevents the first shortcut invocation from briefly showing Tauri's
    // default centered 720×520 window before the frontend loads its config.
    window
        .set_size(configured_launcher_size(
            config,
            config.launcher.compact_when_empty,
        ))
        .map_err(|error| error.to_string())?;
    position_launcher(app)
}

fn configured_launcher_size(config: &AppConfig, compact: bool) -> LogicalSize<f64> {
    LogicalSize::new(
        config.launcher_width(),
        if compact {
            LAUNCHER_COMPACT_HEIGHT
        } else {
            config.launcher_height()
        },
    )
}

pub fn toggle_launcher(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        let _ = window.emit("launcher-hidden", ());
        focus::restore_previous_application();
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

    if !window.is_visible().unwrap_or(false) {
        focus::capture_previous_application();
    }
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
    let work_area = monitor.work_area();
    // Keep the launcher's top edge stable in compact and full modes. Using
    // the current compact height here would make later invocations drift down.
    // Use the destination monitor's DPI because the hidden window may still
    // belong to a different monitor when this position is calculated.
    let (launcher_width, launcher_height, horizontal_offset, vertical_offset) = app
        .try_state::<Arc<ConfigState>>()
        .map(|config| {
            let config = config.snapshot();
            (
                config.launcher_width(),
                config.launcher_height(),
                config.launcher.horizontal_offset_px,
                config.launcher.vertical_offset_px,
            )
        })
        .unwrap_or((DEFAULT_LAUNCHER_WIDTH, DEFAULT_LAUNCHER_HEIGHT, 0, 0));
    let positioning_width = (launcher_width * monitor.scale_factor()).round() as u32;
    let positioning_height = (launcher_height * monitor.scale_factor()).round() as u32;
    let horizontal_offset = (f64::from(horizontal_offset) * monitor.scale_factor()).round() as i32;
    let vertical_offset = (f64::from(vertical_offset) * monitor.scale_factor()).round() as i32;
    let position = launcher_position(
        *monitor_position,
        *monitor_size,
        work_area.position,
        work_area.size,
        PhysicalSize::new(positioning_width, positioning_height),
        PhysicalPosition::new(horizontal_offset, vertical_offset),
    );
    window
        .set_position(position)
        .map_err(|error| error.to_string())
}

fn launcher_position(
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    work_area_position: PhysicalPosition<i32>,
    work_area_size: PhysicalSize<u32>,
    launcher_size: PhysicalSize<u32>,
    offset: PhysicalPosition<i32>,
) -> PhysicalPosition<i32> {
    // Preserve the pre-v9 default anchor, which used the full monitor bounds.
    // The work area is only used to keep the adjusted window clear of the
    // menu bar/taskbar/Dock and visible on screen.
    let base_x = i64::from(monitor_position.x)
        + i64::from(monitor_size.width.saturating_sub(launcher_size.width) / 2);
    let base_y = i64::from(monitor_position.y)
        + i64::from(monitor_size.height.saturating_sub(launcher_size.height) / 4);
    let free_width = work_area_size.width.saturating_sub(launcher_size.width);
    let free_height = work_area_size.height.saturating_sub(launcher_size.height);
    PhysicalPosition::new(
        clamped_axis_position(work_area_position.x, free_width, base_x, offset.x),
        clamped_axis_position(work_area_position.y, free_height, base_y, offset.y),
    )
}

fn clamped_axis_position(origin: i32, free_space: u32, base: i64, offset: i32) -> i32 {
    let minimum = i64::from(origin);
    let maximum = minimum + i64::from(free_space);
    (base + i64::from(offset)).clamp(minimum, maximum) as i32
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
                &entry.aliases,
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
                icon_data_url: String::new(),
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
        &[],
    )
}

fn match_score_normalized(
    name: &str,
    path: &str,
    query: &str,
    pinyin: Option<(&str, &str)>,
    aliases: &[catalog::CatalogAlias],
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
    for alias in aliases {
        if alias.normalized == query {
            return Some(880);
        }
        if alias.normalized.starts_with(query) {
            return Some(820 - query.len() as i32);
        }
        if let Some(position) = alias.normalized.find(query) {
            return Some(700 - position.min(200) as i32);
        }
    }
    if let Some(position) = path.find(&query) {
        return Some(620 - position.min(200) as i32);
    }
    if is_subsequence(&name, &query) {
        return Some(420 - (name.len().saturating_sub(query.len())).min(200) as i32);
    }
    for alias in aliases {
        if is_subsequence(&alias.normalized, query) {
            return Some(
                400 - (alias.normalized.len().saturating_sub(query.len())).min(200) as i32,
            );
        }
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
        for alias in aliases {
            if alias.pinyin == query {
                return Some(680);
            }
            if alias.pinyin.starts_with(query) {
                return Some(640 - query.len() as i32);
            }
            if let Some(position) = alias.pinyin.find(query) {
                return Some(580 - position.min(200) as i32);
            }
            if alias.pinyin_initials == query {
                return Some(540);
            }
            if alias.pinyin_initials.starts_with(query) {
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

fn web_search_results(search: &WebSearchConfig, arguments: &str) -> Vec<SearchResult> {
    let requires_arguments = web_search::requires_arguments(&search.url_template).unwrap_or(true);
    if arguments.is_empty() && requires_arguments {
        let input_hint = if search.input_hint.is_empty() {
            "请输入要搜索的内容"
        } else {
            &search.input_hint
        };
        return vec![hint_result_with_icon(
            &format!("{} <关键词>", search.keyword),
            input_hint,
            &search.icon_data_url,
        )];
    }
    match web_search::expand_url(&search.url_template, arguments) {
        Ok(url) => vec![SearchResult {
            id: format!("web:{}:{arguments}", search.id),
            title: if arguments.is_empty() {
                format!("打开 {}", search.name)
            } else {
                format!("{} 搜索：{arguments}", search.name)
            },
            subtitle: url.clone(),
            kind: ResultKind::Web,
            icon_data_url: search.icon_data_url.clone(),
            badge: "网络".into(),
            score: 2_000,
            action: ResultAction::OpenUrl { url },
        }],
        Err(error) => vec![error_result_with_icon(
            format!("web:{}:args-error", search.id),
            error,
            "请补充参数或检查引号",
            &search.icon_data_url,
        )],
    }
}

fn script_output_result(
    state: &LauncherState,
    action_epoch: u64,
    command: &ScriptCommandConfig,
    arguments: &str,
    output: String,
) -> Result<SearchResult, String> {
    let (action, badge, action_hint) = match command.result_action {
        ScriptResultAction::Copy => (
            ResultAction::CopyText {
                text: output.clone(),
            },
            "复制",
            "按 Enter 复制返回文本",
        ),
        ScriptResultAction::ExecuteShell => {
            let shell_command = scripts::validate_result_shell_command(&output)?;
            let action_id = state.register_script_output_action(
                action_epoch,
                command.id.clone(),
                shell_command,
            )?;
            (
                ResultAction::RunScriptOutput { action_id },
                "执行",
                if cfg!(target_os = "windows") {
                    "按 Enter 通过 PowerShell 执行"
                } else {
                    "按 Enter 通过 Bash 执行"
                },
            )
        }
    };
    let source = if arguments.is_empty() {
        command.script_path.clone()
    } else {
        format!("{} {arguments}", command.script_path)
    };
    Ok(SearchResult {
        id: format!("script:{}:{arguments}:output", command.id),
        title: if output.is_empty() {
            "脚本执行完成（无输出）".into()
        } else {
            output.clone()
        },
        subtitle: format!("{source} · {action_hint}"),
        kind: ResultKind::Script,
        icon_data_url: command.icon_data_url.clone(),
        badge: badge.into(),
        score: 2_000,
        action,
    })
}

fn script_action_subtitle(command: &ScriptCommandConfig, arguments: &str) -> String {
    if arguments.is_empty() {
        if command.input_hint.is_empty() {
            command.script_path.clone()
        } else {
            command.input_hint.clone()
        }
    } else {
        format!("{} {arguments}", command.script_path)
    }
}

fn error_result_with_icon(
    id: String,
    title: String,
    subtitle: &str,
    icon_data_url: &str,
) -> SearchResult {
    SearchResult {
        id,
        title,
        subtitle: subtitle.into(),
        kind: ResultKind::Error,
        icon_data_url: icon_data_url.into(),
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
    hint_result_with_icon(title, subtitle, "")
}

fn hint_result_with_icon(title: &str, subtitle: &str, icon_data_url: &str) -> SearchResult {
    SearchResult {
        id: format!("hint:{title}"),
        title: title.into(),
        subtitle: subtitle.into(),
        kind: ResultKind::Hint,
        icon_data_url: icon_data_url.into(),
        badge: "提示".into(),
        score: 1,
        action: ResultAction::None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{
        catalog::CatalogEntry,
        config::{AppConfig, ScriptResultAction, WebSearchConfig},
        models::{ResultAction, ResultKind},
    };

    use super::{
        calculate, catalog_results, command_arguments, configured_launcher_size, is_settings_query,
        launcher_position, match_score, script_action_subtitle, script_command,
        script_output_result, translation_command, web_search_command, web_search_results,
    };
    use tauri::{PhysicalPosition, PhysicalSize};

    #[test]
    fn launcher_position_preserves_the_current_default_and_clamps_offsets() {
        let default = launcher_position(
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1_440, 900),
            PhysicalPosition::new(0, 24),
            PhysicalSize::new(1_440, 876),
            PhysicalSize::new(720, 520),
            PhysicalPosition::new(0, 0),
        );
        assert_eq!((default.x, default.y), (360, 95));

        let upper_left = launcher_position(
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1_440, 900),
            PhysicalPosition::new(0, 24),
            PhysicalSize::new(1_440, 876),
            PhysicalSize::new(720, 520),
            PhysicalPosition::new(-1_000, -1_000),
        );
        assert_eq!((upper_left.x, upper_left.y), (0, 24));

        let lower_right = launcher_position(
            PhysicalPosition::new(-1_920, 0),
            PhysicalSize::new(1_920, 1_080),
            PhysicalPosition::new(-1_920, 0),
            PhysicalSize::new(1_920, 1_080),
            PhysicalSize::new(1_200, 720),
            PhysicalPosition::new(1_000, 1_000),
        );
        assert_eq!((lower_right.x, lower_right.y), (-1_200, 360));
    }

    #[test]
    fn startup_geometry_uses_persisted_size_before_the_first_show() {
        let mut config = AppConfig::default();
        config.launcher.window_width_px = Some(900);
        config.launcher.window_height_px = 640;

        let full = configured_launcher_size(&config, false);
        assert_eq!((full.width, full.height), (900.0, 640.0));
        let compact = configured_launcher_size(&config, true);
        assert_eq!((compact.width, compact.height), (900.0, 74.0));
    }

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
    fn localized_bundle_aliases_find_macos_application_names() {
        let mut wechat = CatalogEntry::from_application_path_with_type(
            PathBuf::from("/Applications/WeChat.app"),
            true,
        );
        wechat.add_application_aliases(["微信".into(), "weixin".into()]);
        let mut lark = CatalogEntry::from_application_path_with_type(
            PathBuf::from("/Applications/Lark.app"),
            true,
        );
        lark.add_application_aliases(["飞书".into(), "Feishu".into()]);
        let entries = vec![wechat, lark];

        for (query, expected) in [("微信", "WeChat"), ("weixin", "WeChat"), ("飞书", "Lark")] {
            let results = catalog_results(
                &entries,
                query,
                ResultKind::App,
                "应用",
                8,
                800,
                true,
                || false,
            );
            assert_eq!(
                results.first().map(|result| result.title.as_str()),
                Some(expected)
            );
        }
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
    fn direct_web_link_is_actionable_without_arguments() {
        let search = WebSearchConfig {
            id: "mydoc".into(),
            name: "我的文档".into(),
            keyword: "mydoc".into(),
            description: String::new(),
            icon_data_url: String::new(),
            input_hint: String::new(),
            aliases: Vec::new(),
            enabled: true,
            url_template: "https://bytedance.feishu.cn/drive/home/".into(),
        };

        let results = web_search_results(&search, "");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "打开 我的文档");
        match &results[0].action {
            ResultAction::OpenUrl { url } => {
                assert_eq!(url, "https://bytedance.feishu.cn/drive/home/")
            }
            action => panic!("expected direct URL action, got {action:?}"),
        }
    }

    #[test]
    fn configured_web_search_hint_and_icon_reach_results() {
        let mut search = AppConfig::default().web_searches.remove(0);
        search.input_hint = "输入站内搜索内容".into();
        search.icon_data_url = "data:image/png;base64,AAAA".into();

        let hint = web_search_results(&search, "");
        assert_eq!(hint[0].subtitle, "输入站内搜索内容");
        assert_eq!(hint[0].icon_data_url, search.icon_data_url);

        let actionable = web_search_results(&search, "codex");
        assert_eq!(actionable[0].kind, ResultKind::Web);
        assert_eq!(actionable[0].icon_data_url, search.icon_data_url);
    }

    #[test]
    fn configured_script_hint_only_replaces_empty_argument_subtitle() {
        let mut command = AppConfig::default().script_commands.remove(0);
        command.input_hint = "输入毫秒时间戳".into();
        assert_eq!(script_action_subtitle(&command, ""), "输入毫秒时间戳");
        assert_eq!(
            script_action_subtitle(&command, "123"),
            format!("{} 123", command.script_path)
        );
    }

    #[test]
    fn script_output_action_defaults_to_copy_and_shell_tokens_are_one_time() {
        let state = super::LauncherState::new();
        let epoch = state.begin_search(1);
        let mut command = AppConfig::default().script_commands.remove(0);

        let copied = script_output_result(&state, epoch, &command, "", "value".into()).unwrap();
        assert_eq!(copied.badge, "复制");
        assert!(matches!(
            copied.action,
            ResultAction::CopyText { ref text } if text == "value"
        ));

        command.result_action = ScriptResultAction::ExecuteShell;
        let executable =
            script_output_result(&state, epoch, &command, "~/", "open ~/".into()).unwrap();
        assert_eq!(executable.badge, "执行");
        let ResultAction::RunScriptOutput { action_id } = executable.action else {
            panic!("expected an opaque script output action");
        };
        let (_, pending) = state
            .begin_script_output_action(epoch, &action_id)
            .expect("first activation should consume the token");
        assert_eq!(pending.command_id, command.id);
        assert_eq!(pending.shell_command, "open ~/");
        assert!(state.begin_script_output_action(epoch, &action_id).is_err());
    }

    #[test]
    fn a_new_search_invalidates_pending_script_output_actions() {
        let state = super::LauncherState::new();
        let epoch = state.begin_search(1);
        let action_id = state
            .register_script_output_action(epoch, "demo".into(), "open ~/".into())
            .unwrap();

        let next_epoch = state.begin_search(2);
        assert_ne!(epoch, next_epoch);
        assert!(state
            .begin_script_output_action(next_epoch, &action_id)
            .is_err());
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
