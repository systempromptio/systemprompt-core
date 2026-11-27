//! The grep tool: a regular expression over file contents, in-process,
//! honouring ignore files.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod search;

use async_trait::async_trait;
use globset::GlobBuilder;
use regex::RegexBuilder;
use serde::Deserialize;
use serde_json::{Value, json};

use self::search::{Found, Include, Query, search};
use crate::tools::config::SearchLimits;
use crate::tools::contract::{PermissionRequest, Tool, ToolContext, ToolOutput};
use crate::tools::error::ToolError;
use crate::tools::path::{display_path, guard_external, resolve, slash};
use crate::tools::repair::parse_input;

/// Searches file contents with a regular expression.
#[derive(Debug, Clone, Copy)]
pub struct GrepTool {
    limits: SearchLimits,
}

impl GrepTool {
    pub const fn new(limits: SearchLimits) -> Self {
        Self { limits }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    pattern: String,
    path: Option<String>,
    include: Option<String>,
    context: Option<usize>,
    #[serde(default)]
    ignore_case: bool,
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        crate::tools::names::GREP
    }

    fn description(&self) -> String {
        format!(
            "Search file contents with a regular expression (Rust regex syntax: `fn\\s+\\w+`, `TODO|FIXME`; escape \
             literal braces and dots). Searches the working directory, or `path` (a directory or one file), skipping \
             ignored and binary files. Narrow it with `include`, a glob on file names or paths such as `*.rs` or \
             `src/**/*.{{ts,tsx}}`. `context` adds that many lines around each match. Results are grouped by file, \
             most recently modified first, with line numbers, and stop after {} matches. Use glob to find files by \
             name.",
            self.limits.grep_matches
        )
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {"type": "string", "description": "Regular expression to search for."},
                "path": {"type": "string", "description": "Directory or file to search. Defaults to the working directory."},
                "include": {"type": "string", "description": "Glob limiting which files are searched, e.g. *.rs."},
                "context": {"type": "integer", "minimum": 0, "maximum": 10, "description": "Lines of context around each match."},
                "ignore_case": {"type": "boolean", "description": "Match case-insensitively."}
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
        if tokio::fs::metadata(&root).await.is_err() {
            return Err(ToolError::invalid(format!(
                "{} does not exist",
                slash(&root)
            )));
        }
        guard_external(ctx, &root, "Search").await?;
        ctx.ask(PermissionRequest {
            permission: "grep".to_owned(),
            patterns: vec![input.pattern.clone()],
            always: vec!["*".to_owned()],
            title: format!("Search for {}", input.pattern),
            metadata: json!({
                "pattern": input.pattern,
                "path": slash(&root),
                "include": input.include,
            }),
        })
        .await?;

        let regex = RegexBuilder::new(&input.pattern)
            .case_insensitive(input.ignore_case)
            .size_limit(32 * 1024 * 1024)
            .build()
            .map_err(|err| ToolError::malformed("invalid regular expression", err))?;
        let include = include_matcher(input.include.as_deref())?;
        let context = input.context.unwrap_or(0).min(10);
        let query = Query {
            regex,
            include,
            context,
            limits: self.limits,
        };
        let cancel = ctx.cancel.clone();
        let search_root = root.clone();
        let mut found = tokio::task::spawn_blocking(move || search(&search_root, &query, &cancel))
            .await
            .map_err(|err| ToolError::caused("the content search stopped unexpectedly", err))??;
        found.files.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.path.cmp(&b.path))
        });

        let output = render(&found, ctx);
        Ok(
            ToolOutput::new(input.pattern.clone(), output).with_metadata(json!({
                "matches": found.matches,
                "files": found.files.len(),
                "truncated": found.capped,
            })),
        )
    }
}

fn include_matcher(include: Option<&str>) -> Result<Option<Include>, ToolError> {
    let Some(glob) = include.map(str::trim).filter(|glob| !glob.is_empty()) else {
        return Ok(None);
    };
    let text = glob.replace('\\', "/");
    let whole_path = text.contains('/');
    let matcher = GlobBuilder::new(&text)
        .literal_separator(whole_path)
        .build()
        .map_err(|err| ToolError::malformed("invalid include glob", err))?
        .compile_matcher();
    Ok(Some(Include {
        matcher,
        whole_path,
    }))
}

fn render(found: &Found, ctx: &ToolContext) -> String {
    if found.files.is_empty() {
        return "No matches found.".to_owned();
    }
    let mut out = format!(
        "{} match{} in {} file{}",
        found.matches,
        if found.matches == 1 { "" } else { "es" },
        found.files.len(),
        if found.files.len() == 1 { "" } else { "s" }
    );
    if found.capped {
        out.push_str(
            " (stopped at the match limit; narrow the pattern, path or include to see the rest)",
        );
    }
    out.push('\n');
    for file in &found.files {
        out.push('\n');
        out.push_str(&display_path(&file.path, &ctx.worktree));
        out.push('\n');
        let mut previous: Option<usize> = None;
        for (&index, &is_match) in &file.rows {
            if previous.is_some_and(|prev| index > prev + 1) {
                out.push_str("  --\n");
            }
            let marker = if is_match { ':' } else { '-' };
            let line = file.lines.get(index).map_or("", String::as_str);
            out.push_str(&format!("  {}{marker} {line}\n", index + 1));
            previous = Some(index);
        }
    }
    out
}
