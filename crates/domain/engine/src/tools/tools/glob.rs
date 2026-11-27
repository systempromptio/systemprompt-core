//! The glob tool: files by name pattern, honouring ignore files, newest first.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use globset::GlobBuilder;
use ignore::WalkBuilder;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::tools::config::SearchLimits;
use crate::tools::contract::{PermissionRequest, Tool, ToolContext, ToolOutput};
use crate::tools::error::ToolError;
use crate::tools::path::{display_path, guard_external, resolve, slash};
use crate::tools::repair::parse_input;

/// Finds files by glob pattern.
#[derive(Debug, Clone, Copy)]
pub struct GlobTool {
    limits: SearchLimits,
}

impl GlobTool {
    pub const fn new(limits: SearchLimits) -> Self {
        Self { limits }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    pattern: String,
    path: Option<String>,
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        crate::tools::names::GLOB
    }

    fn description(&self) -> String {
        format!(
            "Find files whose path matches a glob pattern, such as `**/*.rs`, `src/**/mod.rs` or `*.{{ts,tsx}}`. A \
             pattern without a slash matches file names at any depth. Files excluded by .gitignore and similar \
             ignore files are skipped. Results are paths sorted with the most recently modified first, at most {} \
             of them. Use grep to search file contents instead.",
            self.limits.glob_results
        )
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Glob pattern relative to the search directory."},
                "path": {"type": "string", "description": "Directory to search. Defaults to the working directory."}
            },
            "required": ["pattern"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        let root = input
            .path
            .as_deref()
            .map_or_else(|| ctx.cwd.clone(), |path| resolve(&ctx.cwd, path));
        if !tokio::fs::metadata(&root)
            .await
            .is_ok_and(|meta| meta.is_dir())
        {
            return Err(ToolError::invalid(format!(
                "{} is not a directory",
                slash(&root)
            )));
        }
        guard_external(ctx, &root, "Search").await?;
        ctx.ask(PermissionRequest {
            permission: "glob".to_owned(),
            patterns: vec![input.pattern.clone()],
            always: vec!["*".to_owned()],
            title: format!("Find {}", input.pattern),
            metadata: json!({ "pattern": input.pattern, "path": slash(&root) }),
        })
        .await?;

        let pattern = normalize_pattern(&input.pattern, &root);
        let cancel = ctx.cancel.clone();
        let walk_root = root.clone();
        let mut found = tokio::task::spawn_blocking(move || walk(&walk_root, &pattern, &cancel))
            .await
            .map_err(|err| ToolError::caused("the file search stopped unexpectedly", err))??;
        found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let total = found.len();
        found.truncate(self.limits.glob_results);

        let shown: Vec<String> = found
            .iter()
            .map(|(path, _)| display_path(path, &ctx.worktree))
            .collect();
        let mut output = if shown.is_empty() {
            "No files found.".to_owned()
        } else {
            shown.join("\n")
        };
        if total > shown.len() {
            output.push_str(&format!(
                "\n\n({total} files matched; showing the {} most recently modified. Use a more specific pattern or path.)",
                shown.len()
            ));
        }
        Ok(ToolOutput::new(input.pattern.clone(), output)
            .with_metadata(json!({ "count": total, "truncated": total > shown.len() })))
    }
}

fn normalize_pattern(pattern: &str, root: &Path) -> String {
    let unified = pattern.trim().replace('\\', "/");
    let root_text = slash(root);
    let prefix = format!("{}/", root_text.trim_end_matches('/'));
    let mut text = unified.strip_prefix(&prefix).unwrap_or(&unified);
    while let Some(rest) = text.strip_prefix("./") {
        text = rest;
    }
    if text.contains('/') {
        text.to_owned()
    } else {
        format!("**/{text}")
    }
}

fn walk(
    root: &Path,
    pattern: &str,
    cancel: &CancellationToken,
) -> Result<Vec<(PathBuf, SystemTime)>, ToolError> {
    let matcher = GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map_err(|err| ToolError::malformed("invalid glob pattern", err))?
        .compile_matcher();
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();
    let mut found = Vec::new();
    for entry in walker {
        if cancel.is_cancelled() {
            return Err(ToolError::Aborted);
        }
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        if matcher.is_match(slash(relative)) {
            let modified = entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            found.push((entry.path().to_path_buf(), modified));
        }
    }
    Ok(found)
}
