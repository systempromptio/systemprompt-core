//! Line-level edits of a YAML layer that keep comments and layout: find a key
//! block by path, remove or replace it, insert a child.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::Value;

pub(super) fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

pub(super) fn is_content(line: &str) -> bool {
    let trimmed = line.trim_start();
    !trimmed.is_empty() && !trimmed.starts_with('#')
}

pub(super) fn key_of(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('-') || !is_content(line) {
        return None;
    }
    trimmed.split_once(':').map(|(key, _)| key.trim())
}

pub(super) fn find_block(lines: &[String], path: &[&str]) -> Option<(usize, usize)> {
    let (mut lo, mut hi) = (0usize, lines.len());
    for (depth, key) in path.iter().enumerate() {
        let level = lines[lo..hi]
            .iter()
            .find(|l| is_content(l))
            .map(|l| indent_of(l))?;
        let index = (lo..hi).find(|&i| {
            is_content(&lines[i])
                && indent_of(&lines[i]) == level
                && key_of(&lines[i]) == Some(*key)
        })?;
        let block_end = (index + 1..hi)
            .find(|&i| is_content(&lines[i]) && indent_of(&lines[i]) <= level)
            .unwrap_or(hi);
        let block_end = trim_trailing_blank(lines, index + 1, block_end);
        if depth + 1 == path.len() {
            return Some((index, block_end));
        }
        lo = index + 1;
        hi = block_end;
        if lo >= hi {
            return None;
        }
    }
    None
}

pub(super) fn trim_trailing_blank(lines: &[String], start: usize, mut end: usize) -> usize {
    while end > start && !is_content(&lines[end - 1]) {
        end -= 1;
    }
    end
}

pub(super) fn inline_mapping(lines: &[String], path: &[&str]) -> bool {
    find_block(lines, path).is_some_and(|(index, _)| {
        lines[index]
            .split_once(':')
            .is_some_and(|(_, rest)| rest.trim_start().starts_with('{'))
    })
}

pub(super) fn remove_block(lines: &mut Vec<String>, path: &[&str]) -> bool {
    match find_block(lines, path) {
        Some((start, end)) => {
            lines.drain(start..end);
            true
        },
        None => false,
    }
}

pub(super) fn replace_block(
    lines: &mut Vec<String>,
    path: &[&str],
    replacement: &[String],
) -> bool {
    let Some((start, end)) = find_block(lines, path) else {
        return false;
    };
    let pad = " ".repeat(indent_of(&lines[start]));
    lines.splice(
        start..end,
        replacement.iter().map(|line| format!("{pad}{line}")),
    );
    true
}

pub(super) fn first_stated<'p>(lines: &[String], paths: &[&'p [&'p str]]) -> Option<&'p [&'p str]> {
    paths
        .iter()
        .filter_map(|path| find_block(lines, path).map(|(start, _)| (start, *path)))
        .min_by_key(|(start, _)| *start)
        .map(|(_, path)| path)
}

pub(super) fn child_indent(lines: &[String], parent: usize) -> usize {
    let parent_indent = indent_of(&lines[parent]);
    lines
        .iter()
        .skip(parent + 1)
        .find(|l| is_content(l))
        .map(|l| indent_of(l))
        .filter(|indent| *indent > parent_indent)
        .unwrap_or(parent_indent + 2)
}

pub(super) fn insert_child(
    lines: &mut Vec<String>,
    parent_path: &[&str],
    child: &[String],
) -> bool {
    let Some((parent, _)) = find_block(lines, parent_path) else {
        return false;
    };
    let pad = " ".repeat(child_indent(lines, parent));
    for (offset, line) in child.iter().enumerate() {
        lines.insert(parent + 1 + offset, format!("{pad}{line}"));
    }
    true
}

pub(super) fn yaml_scalar(text: &str) -> String {
    serde_yaml::to_string(&Value::String(text.to_owned()))
        .map_or_else(|_| format!("{text:?}"), |s| s.trim_end().to_owned())
}

pub(super) fn prune_if_empty(lines: &mut Vec<String>, path: &[&str]) -> bool {
    let Some((start, end)) = find_block(lines, path) else {
        return false;
    };
    let bare = lines[start]
        .split_once(':')
        .is_some_and(|(_, rest)| rest.trim().is_empty() || rest.trim_start().starts_with('#'));
    let has_child = lines[start + 1..end].iter().any(|l| is_content(l));
    if bare && !has_child {
        lines.drain(start..end);
        return true;
    }
    false
}
