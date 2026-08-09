use std::{collections::HashSet, path::PathBuf};

#[cfg(target_os = "windows")]
use std::env;

use pinyin::ToPinyin;
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_directory: bool,
    pub normalized_name: String,
    pub normalized_path: String,
    /// Tone-free, separator-free transliteration used only for application
    /// lookup. Keeping this on the catalog entry makes query-time matching a
    /// cheap string comparison and works the same on Windows and macOS.
    pub pinyin_name: String,
    /// Initials are a secondary convenience match (for example, `wx` for
    /// `微信`). They deliberately score below full pinyin and native text.
    pub pinyin_initials: String,
}

impl CatalogEntry {
    pub fn from_path_with_type(path: PathBuf, is_directory: bool) -> Self {
        Self::from_path_with_pinyin(path, is_directory, false)
    }

    pub(crate) fn from_application_path_with_type(path: PathBuf, is_directory: bool) -> Self {
        Self::from_path_with_pinyin(path, is_directory, true)
    }

    fn from_path_with_pinyin(path: PathBuf, is_directory: bool, include_pinyin: bool) -> Self {
        let name = display_name(&path);
        let normalized_name = name.to_lowercase();
        let normalized_path = path.to_string_lossy().to_lowercase();
        let (pinyin_name, pinyin_initials) = include_pinyin
            .then(|| pinyin_search_keys(&name))
            .unwrap_or_default();
        Self {
            name,
            path,
            is_directory,
            normalized_name,
            normalized_path,
            pinyin_name,
            pinyin_initials,
        }
    }
}

/// Returns compact pinyin search keys for a display name.
///
/// `pinyin` uses its deterministic primary pronunciation for a character.
/// That covers normal application names without making search depend on a
/// platform IME. Non-Chinese characters are retained in the full key so mixed
/// names such as `微信 3.0` remain searchable as `weixin30`.
fn pinyin_search_keys(value: &str) -> (String, String) {
    let mut full = String::new();
    let mut initials = String::new();

    for character in value.chars() {
        if let Some(pinyin) = character.to_pinyin() {
            let plain = pinyin.plain();
            full.push_str(plain);
            if let Some(initial) = plain.chars().next() {
                initials.push(initial);
            }
        } else if character.is_ascii_alphanumeric() {
            full.push(character.to_ascii_lowercase());
        }
    }

    (full, initials)
}

pub fn discover_applications() -> Vec<CatalogEntry> {
    #[cfg(target_os = "windows")]
    {
        discover_windows_applications()
    }

    #[cfg(target_os = "macos")]
    {
        discover_macos_applications()
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "windows")]
fn discover_windows_applications() -> Vec<CatalogEntry> {
    let mut roots = Vec::new();
    if let Some(app_data) = env::var_os("APPDATA") {
        roots.push(PathBuf::from(app_data).join("Microsoft/Windows/Start Menu/Programs"));
    }
    if let Some(program_data) = env::var_os("PROGRAMDATA") {
        roots.push(PathBuf::from(program_data).join("Microsoft/Windows/Start Menu/Programs"));
    }

    collect_entries(roots, 10, 10_000, true, true, |entry| {
        matches!(
            entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("lnk" | "url" | "exe")
        )
    })
}

#[cfg(target_os = "macos")]
fn discover_macos_applications() -> Vec<CatalogEntry> {
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
    ];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }

    collect_entries(roots, 4, 10_000, true, true, |entry| {
        entry.file_type().is_dir()
            && entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("app"))
    })
}

pub fn build_limited_file_index() -> Vec<CatalogEntry> {
    let mut roots = Vec::new();
    for root in [
        dirs::desktop_dir(),
        dirs::document_dir(),
        dirs::download_dir(),
    ]
    .into_iter()
    .flatten()
    {
        if root.exists() && !roots.contains(&root) {
            roots.push(root);
        }
    }

    collect_entries(roots, 8, 50_000, false, false, |entry| {
        entry.depth() > 0 && (entry.file_type().is_file() || entry.file_type().is_dir())
    })
}

fn collect_entries<F>(
    roots: Vec<PathBuf>,
    max_depth: usize,
    max_entries: usize,
    skip_included_directories: bool,
    include_pinyin: bool,
    include: F,
) -> Vec<CatalogEntry>
where
    F: Fn(&DirEntry) -> bool,
{
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    for root in roots {
        let mut walker = WalkDir::new(root)
            .follow_links(false)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(should_descend);

        while let Some(entry) = walker.next() {
            if entries.len() >= max_entries {
                break;
            }
            let Ok(entry) = entry else {
                continue;
            };
            if !include(&entry) {
                continue;
            }

            let is_directory = entry.file_type().is_dir();
            let path = entry.into_path();
            let key = path.to_string_lossy().to_lowercase();
            if !seen.insert(key) {
                continue;
            }
            let entry = if include_pinyin {
                CatalogEntry::from_application_path_with_type(path, is_directory)
            } else {
                CatalogEntry::from_path_with_type(path, is_directory)
            };
            entries.push(entry);

            // Application bundles are directories. Once included, their
            // internal files are not useful launcher results.
            if is_directory && skip_included_directories {
                walker.skip_current_dir();
            }
        }
    }

    entries.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    entries
}

fn should_descend(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }

    let name = entry.file_name().to_string_lossy();
    !name.starts_with('.')
        && !matches!(
            name.to_ascii_lowercase().as_str(),
            "node_modules" | "target" | "$recycle.bin" | "system volume information"
        )
}

pub fn display_name(path: &std::path::Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn indexing_a_directory_does_not_skip_its_children() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("suo-catalog-tree-{}-{nonce}", std::process::id()));
        let nested = root.join("folder").join("nested");
        fs::create_dir_all(&nested).expect("create nested directory");
        fs::write(nested.join("report.txt"), b"report").expect("create nested file");

        let entries = collect_entries(vec![root.clone()], 8, 100, false, false, |entry| {
            entry.depth() > 0 && (entry.file_type().is_file() || entry.file_type().is_dir())
        });
        assert!(entries
            .iter()
            .any(|entry| entry.is_directory && entry.path.ends_with("folder")));
        assert!(entries
            .iter()
            .any(|entry| entry.is_directory && entry.path.ends_with("nested")));
        assert!(entries
            .iter()
            .any(|entry| !entry.is_directory && entry.path.ends_with("report.txt")));

        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn creates_full_pinyin_and_initial_keys_for_chinese_names() {
        let entry =
            CatalogEntry::from_application_path_with_type(PathBuf::from("C:/apps/微信.lnk"), false);

        assert_eq!(entry.pinyin_name, "weixin");
        assert_eq!(entry.pinyin_initials, "wx");
    }

    #[test]
    fn does_not_create_pinyin_keys_for_file_index_entries() {
        let entry = CatalogEntry::from_path_with_type(PathBuf::from("C:/files/微信.txt"), false);

        assert!(entry.pinyin_name.is_empty());
        assert!(entry.pinyin_initials.is_empty());
    }
}
