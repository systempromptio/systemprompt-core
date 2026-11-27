//! The edit tool: replaces one exact piece of a file, forgiving whitespace
//! drift through the matching ladder, and keeping the file's line endings and
//! byte-order mark.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::contract::{Tool, ToolContext, ToolOutput};
use crate::diff::{counts, unified};
use crate::error::ToolError;
use crate::files::{ask_edit, read_existing, with_notes, write_file};
use crate::matching::{self, MatchError, Strategy};
use crate::path::{display_path, guard_external, resolve};
use crate::repair::parse_input;
use crate::text::{TextFile, normalize_newlines};

/// Replaces text inside a file.
#[derive(Debug, Clone, Copy, Default)]
pub struct EditTool;

#[derive(Debug, Deserialize)]
struct Input {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Debug)]
struct Change {
    file: TextFile,
    old_body: String,
    new_body: String,
    strategy: Strategy,
    replacements: usize,
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        crate::names::EDIT
    }

    fn description(&self) -> String {
        "Replace a piece of text in a file. `old_string` must identify exactly one place in the file: copy it from \
         what read showed you (without the line-number prefix), including indentation, and add neighbouring lines \
         if the snippet occurs more than once. Set `replace_all` to change every occurrence instead, which is handy \
         for renames. Small whitespace differences are tolerated when no exact match exists, but exact text is \
         always the safest. The file's line endings and byte-order mark are kept. To create a new file, pass an \
         empty `old_string` and the whole file as `new_string`. Read the file before editing it."
            .to_owned()
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File to edit, absolute or relative to the working directory."},
                "old_string": {"type": "string", "description": "The exact text to replace."},
                "new_string": {"type": "string", "description": "The text to put in its place."},
                "replace_all": {"type": "boolean", "description": "Replace every occurrence instead of requiring a unique match. Defaults to false."}
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        if input.old_string == input.new_string {
            return Err(ToolError::invalid(
                "old_string and new_string are identical; there is nothing to change",
            ));
        }
        let path = resolve(&ctx.cwd, &input.path);
        guard_external(ctx, &path, "Edit").await?;
        let shown = display_path(&path, &ctx.worktree);
        let existing = read_existing(&path).await?;
        let change = plan(existing.as_deref(), &input, &shown)?;

        let diff = unified(&shown, &change.old_body, &change.new_body);
        ask_edit(ctx, vec![shown.clone()], format!("Edit {shown}"), &diff).await?;
        ctx.ensure_live()?;
        let bytes = change.file.encode(&change.new_body);
        let notes: Vec<String> = write_file(ctx, &path, &bytes).await?.into_iter().collect();

        let (additions, deletions) = counts(&change.old_body, &change.new_body);
        let replacements = change.replacements;
        let mut summary = format!(
            "Edited {shown}: {replacements} replacement{} (+{additions} -{deletions}).",
            if replacements == 1 { "" } else { "s" }
        );
        if change.strategy != Strategy::Exact {
            summary.push_str(&format!(
                " old_string was matched {} because its whitespace differed from the file; re-read \
                 before the next edit.",
                change.strategy.as_str()
            ));
        }
        Ok(
            ToolOutput::new(shown.clone(), with_notes(summary, &notes)).with_metadata(json!({
                "path": shown,
                "diff": diff,
                "additions": additions,
                "deletions": deletions,
                "replacements": replacements,
                "strategy": change.strategy.as_str(),
                "diagnostics": notes.first(),
            })),
        )
    }
}

fn plan(existing: Option<&[u8]>, input: &Input, shown: &str) -> Result<Change, ToolError> {
    let Some(bytes) = existing else {
        if input.old_string.is_empty() {
            let file = TextFile::plain(&input.new_string);
            let new_body = file.body.clone();
            return Ok(Change {
                file,
                old_body: String::new(),
                new_body,
                strategy: Strategy::Exact,
                replacements: 1,
            });
        }
        return Err(ToolError::invalid(format!(
            "file not found: {shown}. To create it, pass an empty old_string or use the write tool."
        )));
    };
    let file = TextFile::decode(bytes).ok_or_else(|| {
        ToolError::invalid(format!("{shown} is not UTF-8 text and cannot be edited"))
    })?;
    if input.old_string.is_empty() {
        if !file.body.trim().is_empty() {
            return Err(ToolError::invalid(format!(
                "{shown} already has content; old_string must name the text to replace"
            )));
        }
        let old_body = file.body.clone();
        let new_body = normalize_newlines(&input.new_string);
        return Ok(Change {
            file,
            old_body,
            new_body,
            strategy: Strategy::Exact,
            replacements: 1,
        });
    }
    let old = normalize_newlines(&input.old_string);
    let new = normalize_newlines(&input.new_string);
    let done = matching::replace(&file.body, &old, &new, input.replace_all)
        .map_err(|err| explain(err, shown))?;
    let old_body = file.body.clone();
    Ok(Change {
        file,
        old_body,
        new_body: done.text,
        strategy: done.strategy,
        replacements: done.count,
    })
}

fn explain(err: MatchError, shown: &str) -> ToolError {
    match err {
        MatchError::EmptyNeedle => ToolError::invalid("old_string is empty"),
        MatchError::NotFound => ToolError::invalid(format!(
            "old_string was not found in {shown}. Read the file again and copy the exact text, \
             including indentation."
        )),
        MatchError::Ambiguous(count) => ToolError::invalid(format!(
            "old_string matches {count} places in {shown}. Include more surrounding lines so it is \
             unique, or set replace_all."
        )),
    }
}
