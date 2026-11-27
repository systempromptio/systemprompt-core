//! The line-numbered window the read tool shows of a text file: an offset and a
//! limit, long lines clipped, the output capped in bytes, and a closing line
//! saying where to continue.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::ReadLimits;
use crate::error::ToolError;
use crate::text::{BOM, clip_chars};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Window {
    pub(super) text: String,
    pub(super) first: usize,
    pub(super) last: usize,
    pub(super) total: usize,
    pub(super) more: bool,
    pub(super) clipped: usize,
}

pub(super) fn window(
    content: &str,
    offset: usize,
    limit: usize,
    limits: &ReadLimits,
) -> Result<Window, ToolError> {
    let content = content.strip_prefix(BOM).unwrap_or(content);
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    if total == 0 {
        let text = "(empty file)".to_owned();
        return Ok(Window {
            text,
            first: 0,
            last: 0,
            total: 0,
            more: false,
            clipped: 0,
        });
    }
    let offset = offset.max(1);
    if offset > total {
        return Err(ToolError::invalid(format!(
            "offset {offset} is past the end of the file, which has {total} lines"
        )));
    }
    let limit = limit.max(1);
    let mut text = String::new();
    let mut clipped = 0usize;
    let mut last = offset - 1;
    let mut byte_capped = false;
    for (index, line) in lines.iter().enumerate().skip(offset - 1).take(limit) {
        let number = index + 1;
        let (shown, cut) = clip_chars(line, limits.max_line_chars);
        let rendered = if cut {
            clipped += 1;
            format!("{number:>6}\t{shown}… [line clipped]\n")
        } else {
            format!("{number:>6}\t{shown}\n")
        };
        if !text.is_empty() && text.len() + rendered.len() > limits.max_bytes {
            byte_capped = true;
            break;
        }
        text.push_str(&rendered);
        last = number;
    }
    let more = last < total;
    if more {
        let reason = if byte_capped {
            "output size limit reached"
        } else {
            "line limit reached"
        };
        text.push_str(&format!(
            "\n(Showing lines {offset}-{last} of {total}; {reason}. Use offset={} to continue.)",
            last + 1
        ));
    } else {
        text.push_str(&format!("\n(End of file: {total} lines.)"));
    }
    Ok(Window {
        text,
        first: offset,
        last,
        total,
        more,
        clipped,
    })
}
