//! What the read tool answers besides a text window: the media types it
//! attaches, the suggestions for a missing file, and the listing of a
//! directory.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_json::json;

use crate::contract::ToolOutput;
use crate::error::ToolError;

const DIR_ENTRIES: usize = 500;

pub(super) const fn media_type(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        [b'B', b'M', _, _, _, _, 0, 0, 0, 0, ..] => Some("image/bmp"),
        [b'%', b'P', b'D', b'F', b'-', ..] => Some("application/pdf"),
        _ => None,
    }
}

pub(super) fn missing_message(path: &Path, shown: &str) -> String {
    let suggestions = similar_names(path);
    if suggestions.is_empty() {
        format!("file not found: {shown}")
    } else {
        format!(
            "file not found: {shown}. Did you mean: {}?",
            suggestions.join(", ")
        )
    }
}

fn similar_names(path: &Path) -> Vec<String> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Vec::new();
    };
    let wanted = name.to_string_lossy().to_lowercase();
    let wanted_stem = wanted.split('.').next().unwrap_or(&wanted).to_owned();
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|candidate| {
            let lower = candidate.to_lowercase();
            let stem = lower.split('.').next().unwrap_or(&lower);
            lower == wanted
                || (!wanted_stem.is_empty() && stem == wanted_stem)
                || (wanted.len() >= 3 && (lower.contains(&wanted) || wanted.contains(&lower)))
        })
        .map(|candidate| parent.join(candidate).to_string_lossy().replace('\\', "/"))
        .collect();
    found.sort();
    found.truncate(3);
    found
}

pub(super) async fn list_dir(path: &Path, shown: &str) -> Result<ToolOutput, ToolError> {
    let mut reader = tokio::fs::read_dir(path)
        .await
        .map_err(|err| ToolError::io(format!("could not list {shown}"), err))?;
    let mut entries: Vec<String> = Vec::new();
    while let Some(entry) = reader
        .next_entry()
        .await
        .map_err(|err| ToolError::io(format!("could not list {shown}"), err))?
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().await.is_ok_and(|kind| kind.is_dir());
        entries.push(if is_dir { format!("{name}/") } else { name });
    }
    entries.sort_by_key(|name| name.to_lowercase());
    let total = entries.len();
    let more = total > DIR_ENTRIES;
    entries.truncate(DIR_ENTRIES);
    let mut text = format!(
        "{shown} is a directory with {total} entries:\n{}",
        entries.join("\n")
    );
    if more {
        text.push_str(&format!(
            "\n… and {} more. Use glob to narrow the listing.",
            total - DIR_ENTRIES
        ));
    }
    text.push_str("\n\nRead a file inside it by its path, or use glob/grep to search it.");
    Ok(ToolOutput::new(shown.to_owned(), text)
        .with_metadata(json!({ "path": shown, "directory": true, "entries": total })))
}
