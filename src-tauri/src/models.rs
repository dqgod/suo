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
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ResultAction {
    OpenPath {
        path: String,
    },
    OpenUrl {
        url: String,
    },
    CopyText {
        text: String,
    },
    RunScript {
        command_id: String,
        args: Vec<String>,
    },
    OpenSettings,
    None,
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
    pub action_epoch: u64,
    pub results: Vec<SearchResult>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelStatus {
    pub action_epoch: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub indexing: bool,
    pub indexed_file_count: usize,
}

#[cfg(test)]
mod tests {
    use super::ResultAction;

    #[test]
    fn script_action_uses_camel_case_at_the_webview_boundary() {
        let value = serde_json::to_value(ResultAction::RunScript {
            command_id: "demo".into(),
            args: vec!["one".into()],
        })
        .unwrap();
        assert_eq!(value["type"], "runScript");
        assert_eq!(value["commandId"], "demo");
    }
}
