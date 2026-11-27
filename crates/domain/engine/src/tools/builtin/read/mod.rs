//! The read tool: a line-numbered window of a text file; images and PDFs come
//! back as attachments and a directory comes back as a listing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod listing;
mod window;

use std::path::Path;

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};

use self::listing::{list_dir, media_type, missing_message};
use self::window::window;
use crate::tools::config::ReadLimits;
use crate::tools::contract::{Attachment, PermissionRequest, Tool, ToolContext, ToolOutput};
use crate::tools::error::ToolError;
use crate::tools::path::{display_path, guard_external, resolve};
use crate::tools::repair::parse_input;
use crate::tools::text::{decode_utf16, looks_binary};

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Reads files and lists directories.
#[derive(Debug, Clone, Copy)]
pub struct ReadTool {
    limits: ReadLimits,
}

impl ReadTool {
    pub const fn new(limits: ReadLimits) -> Self {
        Self { limits }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    path: String,
    offset: Option<usize>,
    limit: Option<usize>,
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        crate::tools::names::READ
    }

    fn description(&self) -> String {
        format!(
            "Read a file from disk. Text comes back with line numbers (a number, a tab, then the line) so you can cite \
             and edit precisely; the number prefix is not part of the file. By default the first {lines} lines are \
             shown; pass `offset` (1-based line to start at) and `limit` (number of lines) to page through larger \
             files. Lines longer than {chars} characters are clipped. Images (PNG, JPEG, GIF, WebP, BMP) and PDFs are \
             returned as attachments you can look at. Reading a directory lists its entries. Relative paths resolve \
             against the working directory. Read a file before editing it, and prefer reading several likely files \
             in parallel over guessing.",
            lines = self.limits.default_lines,
            chars = self.limits.max_line_chars,
        )
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File or directory to read, absolute or relative to the working directory."},
                "offset": {"type": "integer", "minimum": 1, "description": "1-based line number to start from."},
                "limit": {"type": "integer", "minimum": 1, "description": "Maximum number of lines to return."}
            },
            "required": ["path"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        let path = resolve(&ctx.cwd, &input.path);
        guard_external(ctx, &path, "Read").await?;
        let shown = display_path(&path, &ctx.worktree);
        ctx.ask(PermissionRequest {
            permission: "read".to_owned(),
            patterns: vec![shown.clone()],
            always: vec![shown.clone()],
            title: format!("Read {shown}"),
            metadata: json!({ "path": shown }),
        })
        .await?;

        let meta = match tokio::fs::metadata(&path).await {
            Ok(meta) => meta,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(ToolError::invalid(missing_message(&path, &shown)));
            },
            Err(err) => return Err(ToolError::io(format!("could not open {shown}"), err)),
        };
        if meta.is_dir() {
            return list_dir(&path, &shown).await;
        }
        if meta.len() > MAX_FILE_BYTES {
            return Err(ToolError::invalid(format!(
                "{shown} is {} bytes, too large to read whole; use grep to find the part you need",
                meta.len()
            )));
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|err| ToolError::io(format!("could not read {shown}"), err))?;
        if let Some(mime) = media_type(&bytes) {
            return self.attach(&path, &shown, mime, &bytes);
        }
        self.text_view(&bytes, &shown, &input)
    }
}

impl ReadTool {
    fn attach(
        &self,
        path: &Path,
        shown: &str,
        mime: &str,
        bytes: &[u8],
    ) -> Result<ToolOutput, ToolError> {
        if bytes.len() > self.limits.max_attachment_bytes {
            return Err(ToolError::invalid(format!(
                "{shown} is a {mime} of {} bytes, over the {} byte attachment limit",
                bytes.len(),
                self.limits.max_attachment_bytes
            )));
        }
        let attachment = Attachment {
            mime: mime.to_owned(),
            filename: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        };
        let text = format!("Attached {shown} ({mime}, {} bytes).", bytes.len());
        Ok(ToolOutput::new(shown.to_owned(), text)
            .with_metadata(json!({ "path": shown, "mime": mime, "bytes": bytes.len() }))
            .with_attachment(attachment))
    }

    fn text_view(&self, bytes: &[u8], shown: &str, input: &Input) -> Result<ToolOutput, ToolError> {
        let text = match decode_utf16(bytes) {
            Some(text) => text,
            None if looks_binary(&bytes[..bytes.len().min(8192)]) => {
                return Err(ToolError::invalid(format!(
                    "{shown} looks like a binary file; it cannot be shown as text"
                )));
            },
            None => String::from_utf8_lossy(bytes).into_owned(),
        };
        let offset = input.offset.unwrap_or(1);
        let limit = input.limit.unwrap_or(self.limits.default_lines);
        let view = window(&text, offset, limit, &self.limits)?;
        Ok(
            ToolOutput::new(shown.to_owned(), view.text).with_metadata(json!({
                "path": shown,
                "first_line": view.first,
                "last_line": view.last,
                "total_lines": view.total,
                "more": view.more,
                "clipped_lines": view.clipped,
            })),
        )
    }
}
