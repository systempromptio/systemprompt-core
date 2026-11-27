//! Output truncation: what a tool returns is capped by lines and bytes, and the
//! full text is spilled to a file the model can read or search.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use crate::path::slash;
use crate::text::{ceil_boundary, floor_boundary};

/// The most a tool returns inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputLimits {
    pub max_lines: usize,
    pub max_bytes: usize,
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self {
            max_lines: 2000,
            max_bytes: 50 * 1024,
        }
    }
}

/// Which end of an over-long output is kept inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    Head,
    Tail,
}

/// Inline text after capping, and where the full text went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Truncated {
    pub text: String,
    #[expect(
        clippy::struct_field_names,
        reason = "mirrors the `truncated` key tools put in their output metadata; harness-engine and the tests read it"
    )]
    pub truncated: bool,
    pub spill: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Cut {
    kept: String,
    truncated: bool,
    kept_lines: usize,
    total_lines: usize,
    total_bytes: usize,
}

fn cut(text: &str, limits: OutputLimits, keep: Keep) -> Cut {
    let total_lines = text.lines().count();
    let total_bytes = text.len();
    if total_lines <= limits.max_lines && total_bytes <= limits.max_bytes {
        return Cut {
            kept: text.to_owned(),
            truncated: false,
            kept_lines: total_lines,
            total_lines,
            total_bytes,
        };
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut chosen: Vec<&str> = Vec::new();
    let mut bytes = 0usize;
    let ordered: Vec<&str> = match keep {
        Keep::Head => lines,
        Keep::Tail => lines.into_iter().rev().collect(),
    };
    for line in ordered {
        if chosen.len() >= limits.max_lines {
            break;
        }
        if bytes + line.len() > limits.max_bytes {
            if chosen.is_empty() {
                chosen.push(clip_line(line, limits.max_bytes, keep));
            }
            break;
        }
        bytes += line.len();
        chosen.push(line);
    }
    if keep == Keep::Tail {
        chosen.reverse();
    }
    let kept_lines = chosen.len();
    Cut {
        kept: chosen.concat(),
        truncated: true,
        kept_lines,
        total_lines,
        total_bytes,
    }
}

fn clip_line(line: &str, room: usize, keep: Keep) -> &str {
    match keep {
        Keep::Head => &line[..floor_boundary(line, room)],
        Keep::Tail => &line[ceil_boundary(line, line.len().saturating_sub(room))..],
    }
}

fn spill_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(96)
        .collect();
    if clean.is_empty() {
        format!("output-{}", uuid::Uuid::new_v4().simple())
    } else {
        clean
    }
}

pub async fn truncate_output(
    text: String,
    limits: OutputLimits,
    keep: Keep,
    data_dir: &Path,
    name: &str,
) -> Truncated {
    let piece = cut(&text, limits, keep);
    if !piece.truncated {
        return Truncated {
            text,
            truncated: false,
            spill: None,
        };
    }
    let dir = data_dir.join("tool-output");
    let file = dir.join(format!("{}.txt", spill_name(name)));
    let saved = match tokio::fs::create_dir_all(&dir).await {
        Ok(()) => tokio::fs::write(&file, text.as_bytes()).await,
        Err(err) => Err(err),
    };
    let omitted = piece.total_lines.saturating_sub(piece.kept_lines);
    let (where_to, spill) = match saved {
        Ok(()) => (
            format!(
                "The full output ({} bytes, {} lines) is saved at {}; read it with offset/limit or \
                 grep it instead of rerunning.",
                piece.total_bytes,
                piece.total_lines,
                slash(&file)
            ),
            Some(file),
        ),
        Err(err) => {
            tracing::warn!(error = %err, path = %file.display(), "could not spill truncated tool output");
            ("The full output could not be saved.".to_owned(), None)
        },
    };
    let text = match keep {
        Keep::Head => format!(
            "{}\n\n[Output truncated: {omitted} more lines not shown. {where_to}]",
            piece.kept.trim_end_matches('\n')
        ),
        Keep::Tail => format!(
            "[Output truncated: the first {omitted} lines are not shown. {where_to}]\n\n{}",
            piece.kept
        ),
    };
    Truncated {
        text,
        truncated: true,
        spill,
    }
}
