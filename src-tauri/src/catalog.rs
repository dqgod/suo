use std::{collections::HashSet, path::PathBuf};

#[cfg(target_os = "macos")]
use std::{fs, path::Path};

#[cfg(target_os = "windows")]
use std::env;

use pinyin::ToPinyin;
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Debug)]
pub struct CatalogAlias {
    pub normalized: String,
    pub pinyin: String,
    pub pinyin_initials: String,
}

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
    /// Bundle metadata and localized display names are searchable without
    /// changing the stable path-derived title shown in launcher results.
    pub aliases: Vec<CatalogAlias>,
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
            aliases: Vec::new(),
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub(crate) fn add_application_aliases<I>(&mut self, aliases: I)
    where
        I: IntoIterator<Item = String>,
    {
        let mut seen = HashSet::from([self.normalized_name.clone()]);
        for alias in aliases {
            let alias = alias.trim();
            if alias.is_empty() {
                continue;
            }
            let normalized = alias.to_lowercase();
            if !seen.insert(normalized.clone()) {
                continue;
            }
            let (pinyin, pinyin_initials) = pinyin_search_keys(alias);
            self.aliases.push(CatalogAlias {
                normalized,
                pinyin,
                pinyin_initials,
            });
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

    let mut entries = collect_entries(roots, 4, 10_000, true, true, |entry| {
        entry.file_type().is_dir()
            && entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("app"))
    });
    for entry in &mut entries {
        entry.add_application_aliases(macos_application_aliases(&entry.path));
    }
    entries
}

#[cfg(target_os = "macos")]
fn macos_application_aliases(application: &Path) -> Vec<String> {
    let contents = application.join("Contents");
    let mut aliases = Vec::new();
    if let Ok(value) = plist::Value::from_file(contents.join("Info.plist")) {
        if let Some(dictionary) = value.as_dictionary() {
            for key in ["CFBundleDisplayName", "CFBundleName", "CFBundleExecutable"] {
                if let Some(value) = dictionary.get(key).and_then(plist::Value::as_string) {
                    aliases.push(value.to_string());
                }
            }
            if let Some(url_types) = dictionary
                .get("CFBundleURLTypes")
                .and_then(plist::Value::as_array)
            {
                for url_type in url_types.iter().filter_map(plist::Value::as_dictionary) {
                    let Some(schemes) = url_type
                        .get("CFBundleURLSchemes")
                        .and_then(plist::Value::as_array)
                    else {
                        continue;
                    };
                    for scheme in schemes.iter().filter_map(plist::Value::as_string) {
                        if !matches!(
                            scheme.to_ascii_lowercase().as_str(),
                            "file" | "http" | "https" | "mailto"
                        ) {
                            aliases.push(scheme.to_string());
                        }
                    }
                }
            }
        }
    }

    let resources = contents.join("Resources");
    for locale in ["zh-Hans", "zh_CN", "zh-Hant", "zh_TW", "zh_HK"] {
        let path = resources.join(format!("{locale}.lproj/InfoPlist.strings"));
        if let Ok(value) = plist::Value::from_file(&path) {
            if let Some(dictionary) = value.as_dictionary() {
                for key in ["CFBundleDisplayName", "CFBundleName"] {
                    if let Some(value) = dictionary.get(key).and_then(plist::Value::as_string) {
                        aliases.push(value.to_string());
                    }
                }
            }
            continue;
        }
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Some(strings) = decode_strings_file(&bytes) else {
            continue;
        };
        for key in ["CFBundleDisplayName", "CFBundleName"] {
            if let Some(value) = localized_string_value(&strings, key) {
                aliases.push(value);
            }
        }
    }
    aliases
}

#[cfg(target_os = "macos")]
fn decode_strings_file(bytes: &[u8]) -> Option<String> {
    if let Some(data) = bytes.strip_prefix(&[0xff, 0xfe]) {
        let units = data
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16(&units).ok();
    }
    if let Some(data) = bytes.strip_prefix(&[0xfe, 0xff]) {
        let units = data
            .chunks_exact(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16(&units).ok();
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    String::from_utf8(bytes.to_vec()).ok()
}

#[cfg(target_os = "macos")]
fn localized_string_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with("//") || line.starts_with("/*") || line.starts_with('*') {
            return None;
        }
        let (candidate, value) = line.split_once('=')?;
        if candidate.trim().trim_matches('"') != key {
            return None;
        }
        let value = value.trim().trim_end_matches(';').trim();
        let value = value.strip_prefix('"')?.strip_suffix('"')?;
        Some(value.replace("\\\"", "\"").replace("\\\\", "\\"))
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

    #[test]
    fn application_aliases_keep_native_and_pinyin_search_keys() {
        let mut entry = CatalogEntry::from_application_path_with_type(
            PathBuf::from("/Applications/WeChat.app"),
            true,
        );
        entry.add_application_aliases(["微信".into(), "weixin".into(), "WeChat".into()]);

        assert!(entry.aliases.iter().any(|alias| alias.normalized == "微信"));
        assert!(entry
            .aliases
            .iter()
            .any(|alias| alias.normalized == "weixin"));
        assert!(entry
            .aliases
            .iter()
            .any(|alias| alias.pinyin == "weixin" && alias.pinyin_initials == "wx"));
        assert!(!entry
            .aliases
            .iter()
            .any(|alias| alias.normalized == "wechat"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reads_bundle_and_utf16_localized_application_aliases() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let application = std::env::temp_dir().join(format!(
            "suo-localized-app-{}-{nonce}.app",
            std::process::id()
        ));
        let contents = application.join("Contents");
        let localized = contents.join("Resources/zh-Hans.lproj");
        fs::create_dir_all(&localized).expect("create localized application bundle");
        fs::write(
            contents.join("Info.plist"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDisplayName</key><string>WeChat</string>
<key>CFBundleName</key><string>Weixin</string>
<key>CFBundleExecutable</key><string>WeChat</string>
<key>CFBundleURLTypes</key><array><dict><key>CFBundleURLSchemes</key><array><string>weixin</string><string>https</string></array></dict></array>
</dict></plist>"#,
        )
        .expect("write Info.plist");
        let localized_contents = "\u{feff}\"CFBundleDisplayName\" = \"微信\";\n";
        let utf16 = localized_contents
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        fs::write(localized.join("InfoPlist.strings"), utf16).expect("write localized strings");

        let aliases = macos_application_aliases(&application);
        assert!(aliases.iter().any(|value| value == "Weixin"));
        assert!(aliases.iter().any(|value| value == "weixin"));
        assert!(aliases.iter().any(|value| value == "微信"));
        assert!(!aliases.iter().any(|value| value == "https"));

        fs::remove_dir_all(application).expect("remove application bundle");
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires locally installed WeChat and Lark bundles"]
    fn installed_wechat_and_lark_expose_chinese_aliases() {
        let wechat = macos_application_aliases(Path::new("/Applications/WeChat.app"));
        assert!(wechat.iter().any(|value| value == "微信"));
        assert!(wechat
            .iter()
            .any(|value| value.eq_ignore_ascii_case("weixin")));

        let lark = macos_application_aliases(Path::new("/Applications/Lark.app"));
        assert!(lark.iter().any(|value| value == "飞书"));
        assert!(lark
            .iter()
            .any(|value| value.eq_ignore_ascii_case("feishu")));
    }
}
