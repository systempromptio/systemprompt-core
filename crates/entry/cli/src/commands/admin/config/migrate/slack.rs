//! `authz.link_by_workspace_email:` → `authz.link.link_by_email:` in every
//! Slack app under `slack_apps:` in the services tree.
//!
//! The Slack authz block replaced its email-linking flag with a link policy
//! and the retired key is no longer accepted, so a services file that still
//! carries it does not load. This rewrites the key in place, line by line so
//! comments, layout and the operator's value survive, and leaves an `authz`
//! block that already declares `link:` untouched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const RETIRED_KEY: &str = "link_by_workspace_email:";
const CURRENT_KEY: &str = "link_by_email:";

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

#[derive(Default)]
struct Scope {
    slack_apps: Option<usize>,
    authz: Option<usize>,
}

impl Scope {
    fn enter(&mut self, trimmed: &str, indent: usize) {
        if self.authz.is_some_and(|a| indent <= a) {
            self.authz = None;
        }
        if self.slack_apps.is_some_and(|s| indent <= s) {
            self.slack_apps = None;
        }
        if trimmed == "slack_apps:" {
            self.slack_apps = Some(indent);
        } else if trimmed == "authz:" && self.slack_apps.is_some() {
            self.authz = Some(indent);
        }
    }

    fn in_authz(&self, indent: usize) -> bool {
        self.authz.is_some_and(|a| indent > a)
    }
}

fn authz_declares_link(lines: &[&str], start: usize, authz_indent: usize) -> bool {
    lines[start + 1..]
        .iter()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .take_while(|l| indent_of(l) > authz_indent)
        .any(|l| l.trim_start().starts_with("link:"))
}

pub fn rewrite_slack_link_key(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut scope = Scope::default();
    let mut skip_authz: Option<usize> = None;
    let mut changed = false;
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let indent = indent_of(line);
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            scope.enter(trimmed, indent);
            if trimmed == "authz:" && scope.authz == Some(indent) {
                skip_authz = authz_declares_link(&lines, index, indent).then_some(indent);
            }
        }
        match trimmed.strip_prefix(RETIRED_KEY) {
            Some(rest) if scope.in_authz(indent) && skip_authz.is_none() => {
                changed = true;
                let pad = &line[..indent];
                out.push(format!("{pad}link:"));
                out.push(format!("{pad}  {CURRENT_KEY}{rest}"));
            },
            _ => out.push((*line).to_owned()),
        }
    }
    if !changed {
        return None;
    }
    let mut joined = out.join("\n");
    if text.ends_with('\n') {
        joined.push('\n');
    }
    Some(joined)
}

fn yaml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.filter_map(Result::ok).map(|e| e.path()) {
        if path.is_dir() {
            yaml_files(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml")
        {
            out.push(path);
        }
    }
}

fn pending_rewrites(services: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut files = Vec::new();
    yaml_files(services, &mut files);
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        if let Some(rewritten) = rewrite_slack_link_key(&text) {
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
