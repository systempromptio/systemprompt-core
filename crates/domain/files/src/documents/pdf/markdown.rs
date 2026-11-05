//! The flat block list written as Markdown, for the formats that have no rich
//! text model: a sheet becomes one table per sheet and a deck a text outline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::Projection;

use super::blocks::{Block, Style, blocks_of};
use super::runs::Run;

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

fn joined(runs: &[Run]) -> String {
    runs.iter().map(|run| run.text.as_str()).collect()
}

fn render(block: &Block) -> Option<String> {
    match block {
        Block::Heading { level, runs, .. } => {
            let text = joined(runs);
            (!text.is_empty()).then(|| {
                let hashes = "#".repeat(usize::from((*level).clamp(1, 6)));
                format!("{hashes} {text}")
            })
        },
        Block::Paragraph { style, runs, .. } => {
            let text = joined(runs);
            if text.trim().is_empty() {
                return None;
            }
            Some(match style {
                Style::Code | Style::Lines => format!("```\n{text}\n```"),
                _ if runs.iter().all(|run| run.italic) => format!("*{text}*"),
                _ => text,
            })
        },
        Block::ListItem { depth, runs, .. } => {
            Some(format!("{}- {}", "  ".repeat(*depth), joined(runs)))
        },
        Block::Table { rows, .. } => {
            let rows: Vec<Vec<String>> = rows
                .iter()
                .map(|row| row.iter().map(|cell| joined(&cell.runs)).collect())
                .collect();
            table(&rows)
        },
        Block::PageBreak => Some("---".to_owned()),
        Block::Divider | Block::Image { .. } => None,
    }
}

#[must_use]
pub fn outline_markdown(projection: &Projection) -> String {
    let parts: Vec<String> = blocks_of(projection).iter().filter_map(render).collect();
    let mut out = parts.join("\n\n");
    out.push('\n');
    out
}
