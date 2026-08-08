use tauri::AppHandle;

use crate::catalog::CatalogEntry;

pub enum FileSearchOutcome {
    Available {
        provider: &'static str,
        detail: &'static str,
        entries: Vec<CatalogEntry>,
    },
    Unavailable(String),
    Cancelled,
}

pub fn provider_hint() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "优先 Everything，失败时回退限定目录索引"
    }
    #[cfg(target_os = "macos")]
    {
        "优先 Spotlight，失败时回退限定目录索引"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "使用限定目录索引"
    }
}

pub fn search<F>(
    app: &AppHandle,
    query: &str,
    max_results: usize,
    is_cancelled: F,
) -> FileSearchOutcome
where
    F: Fn() -> bool,
{
    #[cfg(target_os = "windows")]
    {
        return match crate::everything::search(app, query, max_results, is_cancelled) {
            crate::everything::EverythingOutcome::Available(entries) => {
                FileSearchOutcome::Available {
                    provider: "Everything",
                    detail: "已连接 Everything IPC",
                    entries,
                }
            }
            crate::everything::EverythingOutcome::Unavailable(reason) => {
                FileSearchOutcome::Unavailable(reason)
            }
            crate::everything::EverythingOutcome::Cancelled => FileSearchOutcome::Cancelled,
        };
    }

    #[cfg(target_os = "macos")]
    {
        return match crate::spotlight::search(query, max_results, is_cancelled) {
            crate::spotlight::SpotlightOutcome::Available(entries) => {
                FileSearchOutcome::Available {
                    provider: "Spotlight",
                    detail: "已使用 macOS Spotlight 文件名索引",
                    entries,
                }
            }
            crate::spotlight::SpotlightOutcome::Unavailable(reason) => {
                FileSearchOutcome::Unavailable(reason)
            }
            crate::spotlight::SpotlightOutcome::Cancelled => FileSearchOutcome::Cancelled,
        };
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, query, max_results, is_cancelled);
        FileSearchOutcome::Unavailable("当前平台没有系统文件搜索适配器".into())
    }
}
