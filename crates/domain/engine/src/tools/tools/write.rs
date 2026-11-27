//! The write tool: creates or replaces a whole file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::tools::contract::{Tool, ToolContext, ToolOutput};
use crate::tools::diff::{counts, unified};
use crate::tools::error::ToolError;
use crate::tools::files::{ask_edit, read_existing, with_notes, write_file};
use crate::tools::path::{display_path, guard_external, resolve};
use crate::tools::repair::parse_input;

/// Writes whole files.
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteTool;

#[derive(Debug, Deserialize)]
struct Input {
    path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        crate::tools::names::WRITE
    }

    fn description(&self) -> String {
        "Create a file, or replace an existing file's entire contents, with the text you give. Parent directories \
         are created as needed. For changes to part of an existing file use edit or patch instead: they are cheaper \
         and keep the rest of the file intact. If the file already exists, read it first so you do not discard \
         content you have not seen. Do not create documentation or example files unless asked."
            .to_owned()
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File to write, absolute or relative to the working directory."},
                "content": {"type": "string", "description": "The complete new contents of the file."}
            },
            "required": ["path", "content"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        let path = resolve(&ctx.cwd, &input.path);
        guard_external(ctx, &path, "Write").await?;
        let shown = display_path(&path, &ctx.worktree);
        if tokio::fs::metadata(&path)
            .await
            .is_ok_and(|meta| meta.is_dir())
        {
            return Err(ToolError::invalid(format!("{shown} is a directory")));
        }
        let existing = read_existing(&path).await?;
        let created = existing.is_none();
        let old_text = existing
            .as_deref()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
            .unwrap_or_default();
        let diff = unified(&shown, &old_text, &input.content);
        let verb = if created { "Create" } else { "Overwrite" };
        ask_edit(ctx, vec![shown.clone()], format!("{verb} {shown}"), &diff).await?;
        ctx.ensure_live()?;

        let notes: Vec<String> = write_file(ctx, &path, input.content.as_bytes())
            .await?
            .into_iter()
            .collect();
        let (additions, deletions) = counts(&old_text, &input.content);
        let lines = input.content.lines().count();
        let summary = if created {
            format!("Created {shown} ({lines} lines).")
        } else {
            format!("Overwrote {shown} ({lines} lines, +{additions} -{deletions}).")
        };
        Ok(
            ToolOutput::new(shown.clone(), with_notes(summary, &notes)).with_metadata(json!({
                "path": shown,
                "created": created,
                "diff": diff,
                "additions": additions,
                "deletions": deletions,
                "diagnostics": notes.first(),
            })),
        )
    }
}
