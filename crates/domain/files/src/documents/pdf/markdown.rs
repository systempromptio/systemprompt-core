//! The flat block list written as Markdown, for the formats that have no rich
//! text model: a sheet becomes one table per sheet and a deck a text outline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::Projection;

use super::blocks::{Block, Style, blocks_of};

pub const MARKDOWN_MIME_TYPE: &str = "text/markdown; charset=utf-8";

#[must_use]
pub fn markdown_file_name(name: &str) -> String {
    let stem = name
        .rsplit_once('.')
        .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty() && !ext.contains(' '))
        .map_or(name, |(stem, _)| stem);
    format!("{stem}.md")
}

fn cell(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}

fn table(rows: &[Vec<String>]) -> Option<String> {
    let width = rows.iter().map(Vec::len).max().filter(|width| *width > 0)?;
    let line = |row: &[String]| {
        let cells: Vec<String> = (0..width)
            .map(|at| row.get(at).map_or_else(String::new, |text| cell(text)))
            .collect();
        format!("| {} |", cells.join(" | "))
    };
    let mut lines = vec![line(rows.first()?), format!("|{}", " --- |".repeat(width))];
    lines.extend(rows.iter().skip(1).map(|row| line(row)));
    Some(lines.join("\n"))
}

fn render(block: &Block) -> Option<String> {
    match block {
        Block::Heading { level, text } => (!text.is_empty()).then(|| {
            let hashes = "#".repeat(usize::from((*level).clamp(1, 6)));
            format!("{hashes} {text}")
        }),
        Block::Paragraph { text, .. } if text.trim().is_empty() => None,
        Block::Paragraph {
            style: Style::Mono,
            text,
        } => Some(format!("```\n{text}\n```")),
        Block::Paragraph {
            style: Style::Italic,
            text,
        } => Some(format!("*{text}*")),
        Block::Paragraph {
            style: Style::Body,
            text,
        } => Some(text.clone()),
        Block::ListItem { depth, text } => Some(format!("{}- {text}", "  ".repeat(*depth))),
        Block::Table { rows } => table(rows),
        Block::PageBreak => Some("---".to_owned()),
    }
}

#[must_use]
pub fn outline_markdown(projection: &Projection) -> String {
    let parts: Vec<String> = blocks_of(projection).iter().filter_map(render).collect();
    let mut out = parts.join("\n\n");
    out.push('\n');
    out
}
