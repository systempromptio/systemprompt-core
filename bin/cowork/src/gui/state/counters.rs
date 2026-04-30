use crate::config::paths;
use std::path::Path;

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
        let path = entry.path();
        let well_formed = path.join(".claude-plugin").join("plugin.json").is_file()
            || path.join("claude-plugin").join("plugin.json").is_file();
        if !well_formed && name != paths::SYNTHETIC_PLUGIN_NAME {
            n += 1;
        }
    }
    Some(n)
}

pub(super) fn count_dir_children(dir: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
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

pub(super) fn count_md_files(dir: &Path) -> Option<usize> {
    let mut n = 0usize;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if !entry.file_type().ok().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            n += 1;
        }
    }
    Some(n)
}
