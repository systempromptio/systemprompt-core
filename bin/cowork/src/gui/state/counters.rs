use std::path::Path;

pub(super) fn read_index_count(path: &Path) -> Option<usize> {
    let bytes = std::fs::read(path).ok()?;
    let entries: Vec<serde::de::IgnoredAny> = serde_json::from_slice(&bytes).ok()?;
    Some(entries.len())
}

pub(super) fn count_plugin_dirs(root: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
            n += 1;
        }
    }
    Some(n)
}

pub(super) fn count_malformed_plugin_dirs(root: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with('.') {
            continue;
        }
        if !entry.file_type().ok().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if !entry
            .path()
            .join("claude-plugin")
            .join("plugin.json")
            .is_file()
        {
            n += 1;
        }
    }
    Some(n)
}
