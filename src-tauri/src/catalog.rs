use std::{collections::HashSet, env, path::PathBuf};

use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Debug)]
pub struct CatalogEntry {
    pub name: String,
    pub path: PathBuf,
    pub normalized_name: String,
    pub normalized_path: String,
}

impl CatalogEntry {
    pub fn from_path(path: PathBuf) -> Self {
        let name = display_name(&path);
        let normalized_name = name.to_lowercase();
        let normalized_path = path.to_string_lossy().to_lowercase();
        Self {
            name,
            path,
            normalized_name,
            normalized_path,
        }
    }
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

    collect_entries(roots, 10, 10_000, |entry| {
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

    collect_entries(roots, 4, 10_000, |entry| {
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

    collect_entries(roots, 8, 50_000, |entry| entry.file_type().is_file())
}

fn collect_entries<F>(
    roots: Vec<PathBuf>,
    max_depth: usize,
    max_entries: usize,
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
            entries.push(CatalogEntry::from_path(path));

            // Application bundles are directories. Once included, their
            // internal files are not useful launcher results.
            if is_directory {
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
