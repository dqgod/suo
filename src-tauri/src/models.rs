use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: String,
    pub badge: String,
    pub score: i32,
    pub action: ResultAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ResultAction {
    OpenPath { path: String },
    OpenUrl { url: String },
    CopyText { text: String },
    OpenSettings,
    None,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherPreferences {
    pub close_on_blur: bool,
    pub keep_last_input: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub provider: String,
    pub provider_detail: String,
    pub hotkey_status: String,
    pub indexing: bool,
    pub indexed_file_count: usize,
    pub results: Vec<SearchResult>,
}
