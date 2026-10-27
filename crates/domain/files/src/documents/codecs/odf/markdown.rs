//! Markdown blocks written as `OpenDocument` text markup.
//!
//! Headings become `text:h` at their outline level, paragraphs `text:p`,
//! runs of list items one `text:list` whose items carry their marker for
//! ordered lists and their depth as leading indentation, code blocks
//! `text:p` with line breaks, and links `text:a`. Bold, italic and code
//! spans keep their text without character styles, since those need
//! automatic styles this codec does not write. In a text document a table
//! becomes a `table:table`; a presentation frame holds no table, so there,
//! and for headings, the codec writes `text:p` paragraphs (one per table row,
//! cells separated by ` | `).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::DocumentFormat;

use super::package::{attr_escaped, text_markup};
use crate::documents::codecs::markdown_lowering::{Block, Span, lower, row_text};

fn spans_markup(spans: &[Span]) -> String {
    spans
        .iter()
        .map(|span| {
            span.link.as_ref().map_or_else(
                || text_markup(&span.text),
                |url| {
                    format!(
                        "<text:a xlink:type=\"simple\" xlink:href=\"{}\">{}</text:a>",
                        attr_escaped(url),
                        text_markup(&span.text)
                    )
                },
            )
        })
        .collect::<String>()
        .trim()
        .to_owned()
}

fn table_markup(rows: &[Vec<Vec<Span>>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let name = format!("Table{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let mut out = format!(
        "<table:table table:name=\"{name}\"><table:table-column \
         table:number-columns-repeated=\"{columns}\"/>"
    );
    for row in rows {
        out.push_str("<table:table-row>");
        for column in 0..columns {
            let cell = row
                .get(column)
                .map(|spans| spans_markup(spans))
                .unwrap_or_default();
            out.push_str(&format!(
                "<table:table-cell office:value-type=\"string\"><text:p>{cell}</text:p></table:table-cell>"
            ));
        }
        out.push_str("</table:table-row>");
    }
    out.push_str("</table:table>");
    out
}

pub(super) fn markdown_markup(format: DocumentFormat, markdown: &str) -> String {
    let flow = format == DocumentFormat::Odt;
    let mut out = String::new();
    let mut in_list = false;
    for block in lower(markdown) {
        let is_item = matches!(block, Block::ListItem { .. });
        if is_item != in_list {
            out.push_str(if is_item {
                "<text:list>"
            } else {
                "</text:list>"
            });
            in_list = is_item;
        }
        match &block {
            Block::Heading { level, spans } if flow => out.push_str(&format!(
                "<text:h text:outline-level=\"{}\">{}</text:h>",
                level.clamp(&1, &9),
                spans_markup(spans)
            )),
            Block::Heading { spans, .. } | Block::Paragraph(spans) => {
                out.push_str(&format!("<text:p>{}</text:p>", spans_markup(spans)));
            },
            Block::ListItem {
                depth,
                number,
                spans,
            } => {
                let marker = number.map(|n| format!("{n}. ")).unwrap_or_default();
                out.push_str(&format!(
                    "<text:list-item><text:p>{}{}</text:p></text:list-item>",
                    text_markup(&format!("{}{marker}", "  ".repeat(*depth))),
                    spans_markup(spans)
                ));
            },
            Block::Table(rows) if flow => out.push_str(&table_markup(rows)),
            Block::Table(rows) => {
                for row in rows {
                    out.push_str(&format!("<text:p>{}</text:p>", text_markup(&row_text(row))));
                }
            },
            Block::Code(code) => {
                out.push_str(&format!("<text:p>{}</text:p>", text_markup(code)));
            },
            Block::Pending(_) => {
                out.push_str(&format!("<text:p>{}</text:p>", text_markup(&block.plain())));
            },
        }
    }
    if in_list {
        out.push_str("</text:list>");
    }
    out
}
