//! The permission questions a shell command asks before it runs:
//! `external_directory` for every directory outside the project it reaches,
//! then `shell` with one pattern per simple command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;

use super::ShellTool;
use super::tokenize::{Dialect, Segment, split_commands};
use crate::contract::{PermissionRequest, ToolContext};
use crate::error::ToolError;
use crate::path::{dir_pattern, is_within, resolve, slash};
use crate::text::title_line;

impl ShellTool {
    pub(super) async fn authorize(
        &self,
        ctx: &ToolContext,
        command: &str,
        cwd: &Path,
        description: Option<&str>,
    ) -> Result<(), ToolError> {
        let dialect = self.spec.kind.dialect();
        let segments = split_commands(command, dialect);

        let outside = outside_directories(&segments, dialect, cwd, ctx);
        if !outside.is_empty() {
            ctx.ask(PermissionRequest {
                permission: "external_directory".to_owned(),
                patterns: outside.clone(),
                always: outside.clone(),
                title: format!(
                    "Shell command reaches outside the project: {}",
                    outside.join(", ")
                ),
                metadata: json!({ "command": command, "directories": outside }),
            })
            .await?;
        }

        let (patterns, always) = approval_patterns(&segments, dialect, command);
        ctx.ask(PermissionRequest {
            permission: "shell".to_owned(),
            patterns,
            always,
            title: description.map_or_else(|| title_line(command, 120), str::to_owned),
            metadata: json!({
                "command": command,
                "description": description,
                "shell": self.spec.kind.label(),
                "workdir": slash(cwd),
            }),
        })
        .await
    }
}

fn approval_patterns(
    segments: &[Segment],
    dialect: Dialect,
    command: &str,
) -> (Vec<String>, Vec<String>) {
    let mut patterns: Vec<String> = Vec::new();
    let mut always: Vec<String> = Vec::new();
    for segment in segments {
        if segment.head(dialect).is_none() {
            continue;
        }
        let text = segment.text(dialect);
        let prefix = format!("{} *", segment.prefix(dialect));
        if !patterns.contains(&text) {
            patterns.push(text);
        }
        if !always.contains(&prefix) {
            always.push(prefix);
        }
    }
    if patterns.is_empty() {
        patterns.push(command.to_owned());
    }
    (patterns, always)
}

fn outside_directories(
    segments: &[Segment],
    dialect: Dialect,
    cwd: &Path,
    ctx: &ToolContext,
) -> Vec<String> {
    let mut outside: BTreeSet<String> = BTreeSet::new();
    if !is_within(cwd, &ctx.worktree) {
        outside.insert(dir_pattern(cwd));
    }
    for segment in segments {
        for word in segment.path_words(dialect) {
            if let Some(rest) = word.strip_prefix('~') {
                let rest = rest.replace('\\', "/");
                let dir = match rest.rfind('/') {
                    Some(at) if at > 0 => &rest[..at],
                    _ => "",
                };
                outside.insert(format!("~{dir}/*"));
                continue;
            }
            let path = resolve(cwd, &word);
            if !is_within(&path, &ctx.worktree) && !is_within(&path, &ctx.data_dir) {
                outside.insert(dir_pattern(&path));
            }
        }
    }
    outside.into_iter().collect()
}
