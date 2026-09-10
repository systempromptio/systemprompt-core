//! Path descriptions for the diagnostics dump: contents, size, readability
//! and the owner/DACL or mode of each entry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

pub(super) fn append_file(out: &mut Vec<String>, path: &Path) {
    out.push(format!("  {}", path.display()));
    match std::fs::read_to_string(path) {
        Ok(body) => {
            for line in body.lines() {
                out.push(format!("    {line}"));
            }
        },
        Err(e) => {
            out.push(format!("    <{e}>"));
        },
    }
    out.push(format!("    {}", describe_access(path)));
}

pub(super) fn append_path(out: &mut Vec<String>, label: &str, path: &Path) {
    let size = std::fs::metadata(path).map_or_else(
        |e| format!("<{e}>"),
        |m| {
            if m.is_dir() {
                "dir".to_owned()
            } else {
                format!("{} bytes", m.len())
            }
        },
    );
    let readable = if path.is_dir() {
        match std::fs::read_dir(path) {
            Ok(_) => " listable".to_owned(),
            Err(e) => format!(" UNLISTABLE: {e}"),
        }
    } else {
        match std::fs::File::open(path) {
            Ok(_) => " readable".to_owned(),
            Err(e) => format!(" UNREADABLE: {e}"),
        }
    };
    out.push(format!("  {label}: {}: {size}{readable}", path.display()));
    out.push(format!("    {}", describe_access(path)));
}

pub(super) fn append_dir(out: &mut Vec<String>, dir: &Path) {
    out.push(format!("  {}", dir.display()));
    out.push(format!("    {}", describe_access(dir)));
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            out.push(format!("    <{e}>"));
            return;
        },
    };
    let mut names: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    names.sort();
    for path in names {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        append_path(out, &name, &path);
    }
}

#[cfg(target_os = "windows")]
pub(super) fn describe_access(path: &Path) -> String {
    crate::windows_acl::describe(path).unwrap_or_else(|e| format!("acl: <{e}>"))
}

#[cfg(unix)]
pub(super) fn describe_access(path: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).map_or_else(
        |e| format!("mode: <{e}>"),
        |m| {
            format!(
                "mode {:o} uid {} gid {}",
                m.mode() & 0o7777,
                m.uid(),
                m.gid()
            )
        },
    )
}

#[cfg(not(any(unix, target_os = "windows")))]
pub(super) fn describe_access(_path: &Path) -> String {
    String::new()
}
