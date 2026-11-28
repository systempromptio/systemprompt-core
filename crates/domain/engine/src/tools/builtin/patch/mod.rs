//! The patch tool: several file changes in one envelope, checked in full before
//! anything is written.
//!
//! The envelope:
//!
//! ```text
//! %% begin patch
//! %% add path/to/new_file.rs
//! +every line of the new file, each prefixed with +
//! %% update path/to/existing.rs
//! %% rename-to path/to/renamed.rs        (optional, directly after update)
//! @@ fn some_anchor()                    (optional text that appears above the change)
//!  an unchanged line, prefixed with a space
//! -a removed line
//! +an added line
//! %% end of file                         (optional: the hunk above sits at the end of the file)
//! %% delete path/to/old_file.rs
//! %% end patch
//! ```
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod apply;
mod parse;
mod plan;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use std::path::PathBuf;

use self::parse::{FileOp, parse};
use self::plan::{Change, Planned, plan};
use crate::tools::contract::{Tool, ToolContext, ToolOutput};
use crate::tools::diff::{counts, unified};
use crate::tools::error::ToolError;
use crate::tools::files::{ask_edit, remove_file, with_notes, write_file};
use crate::tools::path::{guard_external, resolve};
use crate::tools::repair::parse_input;

/// Applies multi-file patches.
#[derive(Debug, Clone, Copy, Default)]
pub struct PatchTool;

#[derive(Debug, Deserialize)]
struct Input {
    patch: String,
}

#[async_trait]
impl Tool for PatchTool {
    fn name(&self) -> &str {
        crate::tools::names::PATCH
    }

    fn description(&self) -> String {
        "Apply a set of file changes in one call: create, change, rename and delete files together. Prefer it over \
         several edit calls when a change spans files or several places in one file. Nothing is written unless \
         every section applies.\n\n\
         Format:\n\
         %% begin patch\n\
         %% add path/to/new_file\n\
         +each line of the new file, prefixed with +\n\
         %% update path/to/file\n\
         %% rename-to path/to/new_name      (optional, directly after update)\n\
         @@ text of a line above the change  (optional anchor, e.g. the enclosing function signature)\n\
         \u{20}unchanged line (prefixed with one space)\n\
         -removed line\n\
         +added line\n\
         %% end of file                      (optional, after a hunk that must sit at the end of the file)\n\
         %% delete path/to/obsolete_file\n\
         %% end patch\n\n\
         Each `@@` starts a hunk. Give two or three unchanged lines around each change so it can be located, \
         copied exactly from the file; hunks for one file must be in file order. Paths are relative to the working \
         directory. Line endings and byte-order marks of updated files are kept."
            .to_owned()
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "patch": {"type": "string", "description": "The full patch text, from %% begin patch to %% end patch."}
            },
            "required": ["patch"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        let ops = parse(&input.patch).map_err(|err| ToolError::malformed("", err))?;
        let paths = op_paths(&ops, ctx);
        let _locked = ctx.files.lock(paths.iter().map(PathBuf::as_path)).await;
        let plans = plan(ops, ctx).await?;

        for planned in &plans {
            guard_external(ctx, &planned.path, "Patch").await?;
            if let Some((target, _)) = &planned.target {
                guard_external(ctx, target, "Patch").await?;
            }
        }
        let diff: String = plans
            .iter()
            .map(|planned| unified(planned.shown_after(), &planned.old, &planned.new))
            .collect();
        let mut touched: Vec<String> = Vec::new();
        for planned in &plans {
            touched.push(planned.shown.clone());
            if let Some((_, shown)) = &planned.target {
                touched.push(shown.clone());
            }
        }
        let files_word = if plans.len() == 1 { "file" } else { "files" };
        let title = format!("Apply patch to {} {files_word}", plans.len());
        ask_edit(ctx, touched, title, &diff).await?;
        ctx.ensure_live()?;

        let (notes, files) = write_plans(ctx, &plans).await?;
        let summary = format!(
            "Applied patch:\n{}",
            plans
                .iter()
                .map(Planned::summary)
                .collect::<Vec<_>>()
                .join("\n")
        );
        let title = format!("{} {files_word}", plans.len());
        Ok(ToolOutput::new(title, with_notes(summary, &notes))
            .with_metadata(json!({ "diff": diff, "files": files, "diagnostics": notes })))
    }
}

fn op_paths(ops: &[FileOp], ctx: &ToolContext) -> Vec<PathBuf> {
    ops.iter()
        .flat_map(|op| match op {
            FileOp::Add { path, .. } | FileOp::Delete { path } => vec![path.as_str()],
            FileOp::Update {
                path, rename_to, ..
            } => std::iter::once(path.as_str())
                .chain(rename_to.as_deref())
                .collect(),
        })
        .map(|path| resolve(&ctx.cwd, path))
        .collect()
}

// JSON: one metadata entry per changed file, stored with the call and rendered
// by the shell.
async fn write_plans(
    ctx: &ToolContext,
    plans: &[Planned],
) -> Result<(Vec<String>, Vec<Value>), ToolError> {
    let mut notes: Vec<String> = Vec::new();
    let mut files: Vec<Value> = Vec::new();
    let mut landed: Vec<&str> = Vec::new();
    for planned in plans {
        let note = match (&planned.change, &planned.target) {
            (Change::Deleted, _) => remove_file(ctx, &planned.path)
                .await
                .map_err(|err| half_applied(&landed, &planned.shown, err))?,
            (_, Some((target, shown))) => {
                let note = write_file(ctx, target, &planned.bytes)
                    .await
                    .map_err(|err| half_applied(&landed, shown, err))?;
                landed.push(shown);
                remove_file(ctx, &planned.path)
                    .await
                    .map_err(|err| half_applied(&landed, &planned.shown, err))?;
                note
            },
            (_, None) => write_file(ctx, &planned.path, &planned.bytes)
                .await
                .map_err(|err| half_applied(&landed, &planned.shown, err))?,
        };
        if planned.target.is_none() {
            landed.push(&planned.shown);
        }
        notes.extend(note);
        let (additions, deletions) = counts(&planned.old, &planned.new);
        let change = match planned.change {
            Change::Added => "add",
            Change::Modified => "update",
            Change::Deleted => "delete",
        };
        files.push(json!({
            "path": planned.shown_after(),
            "from": planned.target.as_ref().map(|_| planned.shown.clone()),
            "change": change,
            "additions": additions,
            "deletions": deletions,
        }));
    }
    Ok((notes, files))
}

fn half_applied(landed: &[&str], failed: &str, error: ToolError) -> ToolError {
    let done = if landed.is_empty() {
        "nothing had been changed yet".to_owned()
    } else {
        format!("already changed: {}", landed.join(", "))
    };
    ToolError::caused(
        format!(
            "the patch stopped part-way at {failed} ({error}); {done}. Re-read these files before \
             retrying"
        ),
        error,
    )
}
