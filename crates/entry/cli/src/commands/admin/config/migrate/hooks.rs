//! `hooks.evaluation:` → `hooks.judge:` in `plugins/*/config.yaml`.
//!
//! The plugin hooks block renamed `evaluation` to `judge` and the retired
//! spelling is no longer accepted, so a pre-0.73 plugin descriptor does not
//! load. This rewrites the key in place, line by line so comments and layout
//! survive, and leaves a descriptor that already says `judge` or declares
//! both untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const PLUGINS_DIR: &str = "plugins";
const CONFIG_FILE: &str = "config.yaml";
const RETIRED_KEY: &str = "evaluation:";
const CURRENT_KEY: &str = "judge:";

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

pub fn rewrite_hooks_key(text: &str) -> Option<String> {
    let mut hooks_indent: Option<usize> = None;
    let mut changed = false;
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let indent = indent_of(line);
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if hooks_indent.is_some_and(|h| indent <= h) {
                hooks_indent = None;
            }
            if trimmed == "hooks:" {
                hooks_indent = Some(indent);
            }
        }
        match (hooks_indent, trimmed.strip_prefix(RETIRED_KEY)) {
            (Some(h), Some(rest)) if indent > h => {
                changed = true;
                out.push(format!("{}{CURRENT_KEY}{rest}", &line[..indent]));
            },
            _ => out.push(line.to_owned()),
        }
    }
    if !changed || hooks_has_both(text) {
        return None;
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    Some(joined)
}

fn hooks_has_both(text: &str) -> bool {
    text.contains("judge:") && text.contains("evaluation:")
}

fn descriptors(services: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(services.join(PLUGINS_DIR)) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path().join(CONFIG_FILE))
        .filter(|p| p.is_file())
        .collect()
}

fn pending_rewrites(services: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    for path in descriptors(services) {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        if let Some(rewritten) = rewrite_hooks_key(&text) {
            out.push((path, rewritten));
        }
    }
    Ok(out)
}

pub fn pending(services: &Path) -> bool {
    pending_rewrites(services).is_ok_and(|p| !p.is_empty())
}

pub fn migrate(services: &Path) -> Result<Vec<PathBuf>> {
    let rewrites = pending_rewrites(services)?;
    for (path, body) in &rewrites {
        std::fs::write(path, body)
            .with_context(|| format!("Failed to write {}", path.display()))?;
    }
    Ok(rewrites.into_iter().map(|(path, _)| path).collect())
}
