use serde::{Deserialize, Serialize};

/// The fixed result categories sent across the Tauri boundary.
///
/// Keeping this as an enum avoids accidentally treating an arbitrary path or
/// URL as a special icon-bearing result in the webview. `Directory` and
/// `File` deliberately remain separate: their icons are built into the UI,
/// while only `App` can request a native icon by opaque result id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ResultKind {
    App,
    File,
    Directory,
    Calculator,
    Script,
    Web,
    Translation,
    Settings,
    Hint,
    Error,
}

impl ResultKind {
    /// Prefixes keep result ids opaque and aligned with their serialized kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::File => "file",
            Self::Directory => "directory",
            Self::Calculator => "calculator",
            Self::Script => "script",
            Self::Web => "web",
            Self::Translation => "translation",
            Self::Settings => "settings",
            Self::Hint => "hint",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub kind: ResultKind,
    /// Optional validated local PNG/JPEG/WebP data URL for configured commands.
    pub icon_data_url: String,
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
    RunScriptOutput {
        action_id: String,
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

/// RGBA pixels for a native application icon.
///
/// The path that produced this value never crosses the webview boundary. The
/// icon command accepts only an opaque id for an application discovered by the
/// local catalog, and validates this payload again before serializing it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeAppIcon {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::{NativeAppIcon, ResultAction, ResultKind, SearchResult};

    #[test]
    fn script_action_uses_camel_case_at_the_webview_boundary() {
        let value = serde_json::to_value(ResultAction::RunScript {
            command_id: "demo".into(),
            args: vec!["one".into()],
        })
        .unwrap();
        assert_eq!(value["type"], "runScript");
        assert_eq!(value["commandId"], "demo");

        let value = serde_json::to_value(ResultAction::RunScriptOutput {
            action_id: "opaque-action".into(),
        })
        .unwrap();
        assert_eq!(value["type"], "runScriptOutput");
        assert_eq!(value["actionId"], "opaque-action");
    }

    #[test]
    fn result_kinds_and_native_icon_use_the_stable_webview_contract() {
        assert_eq!(
            serde_json::to_value(ResultKind::Directory).unwrap(),
            serde_json::Value::String("directory".into())
        );

        let value = serde_json::to_value(NativeAppIcon {
            width: 48,
            height: 48,
            pixels: vec![0; 48 * 48 * 4],
        })
        .unwrap();
        assert_eq!(value["width"], 48);
        assert_eq!(value["height"], 48);
        assert_eq!(value["pixels"].as_array().unwrap().len(), 48 * 48 * 4);

        let result = serde_json::to_value(SearchResult {
            id: "web:google:codex".into(),
            title: "Google 搜索：codex".into(),
            subtitle: "https://example.com".into(),
            kind: ResultKind::Web,
            icon_data_url: "data:image/png;base64,AAAA".into(),
            badge: "网络".into(),
            score: 2_000,
            action: ResultAction::None,
        })
        .unwrap();
        assert_eq!(
            result["iconDataUrl"],
            serde_json::Value::String("data:image/png;base64,AAAA".into())
        );
    }
}
