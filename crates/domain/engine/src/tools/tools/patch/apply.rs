//! Applying the hunks of one update to a file body: each hunk is located after
//! the previous one (or anywhere, failing that), through its anchor and then
//! its old lines, matched with growing tolerance for whitespace; kept lines
//! keep the file's own text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::parse::{Hunk, HunkLine, PatchError};
use crate::matching::squash;

fn replace_range(lines: &mut Vec<String>, start: usize, len: usize, with: Vec<String>) {
    let tail = lines.split_off(start + len);
    lines.truncate(start);
    lines.extend(with);
    lines.extend(tail);
}

fn find_anchor(lines: &[String], from: usize, anchor: &str) -> Option<usize> {
    let anchor = anchor.trim();
    let scan = |test: &dyn Fn(&str) -> bool| {
        lines
            .iter()
            .enumerate()
            .skip(from)
            .find(|(_, line)| test(line))
            .map(|(at, _)| at)
    };
    scan(&|line: &str| line.trim() == anchor).or_else(|| scan(&|line: &str| line.contains(anchor)))
}

fn find_block(lines: &[String], old: &[&str], from: usize) -> Option<usize> {
    if old.len() > lines.len() {
        return None;
    }
    let ladder: [fn(&str, &str) -> bool; 4] = [
        |have, want| have == want,
        |have, want| have.trim_end() == want.trim_end(),
        |have, want| have.trim() == want.trim(),
        |have, want| squash(have) == squash(want),
    ];
    let last_start = lines.len() - old.len();
    for same in ladder {
        for start in from..=last_start {
            if old
                .iter()
                .enumerate()
                .all(|(offset, want)| same(&lines[start + offset], want))
            {
                return Some(start);
            }
        }
    }
    None
}

fn block_at(lines: &[String], old: &[&str], start: usize) -> bool {
    old.iter().enumerate().all(|(offset, want)| {
        lines
            .get(start + offset)
            .is_some_and(|have| squash(have) == squash(want))
    })
}

pub(super) fn apply_hunks(body: &str, hunks: &[Hunk], path: &str) -> Result<String, PatchError> {
    let trailing = body.ends_with('\n') || body.is_empty();
    let mut lines: Vec<String> = if body.is_empty() {
        Vec::new()
    } else {
        body.strip_suffix('\n')
            .unwrap_or(body)
            .split('\n')
            .map(str::to_owned)
            .collect()
    };
    let mut cursor = 0usize;
    for (index, hunk) in hunks.iter().enumerate() {
        let number = index + 1;
        if let Some(anchor) = &hunk.anchor {
            let at = find_anchor(&lines, cursor, anchor)
                .or_else(|| find_anchor(&lines, 0, anchor))
                .ok_or_else(|| PatchError::AnchorNotFound {
                    path: path.to_owned(),
                    hunk: number,
                    anchor: anchor.clone(),
                })?;
            cursor = at + 1;
        }
        cursor = apply_one(&mut lines, cursor, hunk, path, number)?;
    }
    let mut out = lines.join("\n");
    if trailing && !lines.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

fn apply_one(
    lines: &mut Vec<String>,
    cursor: usize,
    hunk: &Hunk,
    path: &str,
    number: usize,
) -> Result<usize, PatchError> {
    let old: Vec<&str> = hunk
        .lines
        .iter()
        .filter_map(|line| match line {
            HunkLine::Keep(text) | HunkLine::Remove(text) => Some(text.as_str()),
            HunkLine::Add(_) => None,
        })
        .collect();
    if old.is_empty() {
        let added: Vec<String> = hunk
            .lines
            .iter()
            .filter_map(|line| match line {
                HunkLine::Add(text) => Some(text.clone()),
                HunkLine::Keep(_) | HunkLine::Remove(_) => None,
            })
            .collect();
        let at = if hunk.anchor.is_some() && !hunk.at_end {
            cursor
        } else {
            lines.len()
        };
        let count = added.len();
        replace_range(lines, at, 0, added);
        return Ok(at + count);
    }
    let found = if hunk.at_end {
        lines
            .len()
            .checked_sub(old.len())
            .filter(|start| block_at(lines, &old, *start))
    } else {
        find_block(lines, &old, cursor).or_else(|| find_block(lines, &old, 0))
    };
    let start = found.ok_or_else(|| PatchError::HunkNotFound {
        path: path.to_owned(),
        hunk: number,
        expected: old
            .iter()
            .take(8)
            .map(|line| format!("  {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
    })?;
    let mut replacement = Vec::with_capacity(hunk.lines.len());
    let mut taken = start;
    for line in &hunk.lines {
        match line {
            HunkLine::Keep(_) => {
                replacement.push(lines[taken].clone());
                taken += 1;
            },
            HunkLine::Remove(_) => taken += 1,
            HunkLine::Add(text) => replacement.push(text.clone()),
        }
    }
    let count = replacement.len();
    replace_range(lines, start, old.len(), replacement);
    Ok(start + count)
}
