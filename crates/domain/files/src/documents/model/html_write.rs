//! The `RichText` model written as HTML body markup.
//!
//! Consecutive list items are grouped into `ul`/`ol` lists nested by level;
//! `Raw` blocks are written back verbatim.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::inline::{escape_html, render_html};
use super::layout::CalloutTone;
use super::rich_text::{Block, BlockKind, RichText};

#[must_use]
pub fn materialise(model: &RichText) -> String {
    let mut out = String::new();
    let mut lists: Vec<bool> = Vec::new();
    for block in &model.blocks {
        let target = match block.kind {
            BlockKind::ListItem { level, ordered, .. } => Some((usize::from(level) + 1, ordered)),
            _ => None,
        };
        let depth = target.map_or(0, |(depth, _)| depth);
        while lists.len() > depth
            || target.is_some_and(|(d, o)| lists.len() == d && lists.last() != Some(&o))
        {
            let ordered = lists.pop().unwrap_or(false);
            out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
        }
        if let Some((depth, ordered)) = target {
            while lists.len() < depth {
                lists.push(ordered);
                out.push_str(if ordered { "<ol>\n" } else { "<ul>\n" });
            }
        }
        write_block(&mut out, block);
    }
    while let Some(ordered) = lists.pop() {
        out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
    }
    out.truncate(out.trim_end_matches('\n').len());
    out
}

fn write_block(out: &mut String, block: &Block) {
    let inline = render_html(&block.text, &block.marks);
    let line = match &block.kind {
        BlockKind::Paragraph => format!("<p>{inline}</p>"),
        BlockKind::Heading { level } => format!("<h{level}>{inline}</h{level}>"),
        BlockKind::ListItem { .. } => format!("<li>{inline}</li>"),
        BlockKind::Quote => format!("<blockquote>{inline}</blockquote>"),
        BlockKind::Code { .. } => format!("<pre>{}</pre>", escape_html(&block.text)),
        BlockKind::Table { rows, .. } => table_html(rows),
        BlockKind::Image { blob, alt, .. } => {
            format!(
                "<img src=\"{}\" alt=\"{}\">",
                escape_html(blob),
                escape_html(alt)
            )
        },
        BlockKind::Callout { tone } => {
            format!(
                "<aside class=\"callout callout-{}\">{inline}</aside>",
                tone_name(*tone)
            )
        },
        BlockKind::Divider => "<hr>".to_owned(),
        BlockKind::PageBreak | BlockKind::SectionBreak { .. } => {
            "<hr style=\"page-break-after:always\">".to_owned()
        },
        BlockKind::Toc { .. } => "<nav class=\"toc\"></nav>".to_owned(),
        BlockKind::Raw { source } => source.clone(),
    };
    out.push_str(&line);
    out.push('\n');
}

fn table_html(rows: &[Vec<RichText>]) -> String {
    let mut out = String::from("<table>");
    for row in rows {
        out.push_str("<tr>");
        for cell in row {
            let body: String = cell
                .blocks
                .iter()
                .map(|b| render_html(&b.text, &b.marks))
                .collect();
            out.push_str(&format!("<td>{body}</td>"));
        }
        out.push_str("</tr>");
    }
    out.push_str("</table>");
    out
}

const fn tone_name(tone: CalloutTone) -> &'static str {
    match tone {
        CalloutTone::Info => "info",
        CalloutTone::Success => "success",
        CalloutTone::Warning => "warning",
        CalloutTone::Danger => "danger",
        CalloutTone::Note => "note",
    }
}
