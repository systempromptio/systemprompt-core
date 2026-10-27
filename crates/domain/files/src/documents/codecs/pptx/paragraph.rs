//! `DrawingML` paragraph markup: plain-text paragraphs written with the
//! anchor's properties, and Markdown blocks lowered into paragraphs.
//!
//! Markdown headings become bold paragraphs, list items bulleted (`•`) or
//! auto-numbered paragraphs indented by depth, other blocks unbulleted
//! paragraphs. A text frame holds no table, so a Markdown table becomes one
//! paragraph per row with cells separated by ` | `. Links are underlined runs
//! followed by the URL in parentheses.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;
use systemprompt_provider_contracts::documents::CodecError;

use super::projection::Paragraph;
use crate::documents::codecs::markdown_lowering::{Block, Span, lower, row_text};
use crate::documents::codecs::xml_tree::{parse_xml, splice};

pub(super) fn paragraph_xml(para: &Paragraph, ppr: Option<&str>, text: &str) -> String {
    let mut out = String::from("<a:p>");
    out.push_str(ppr.unwrap_or_default());
    if text.is_empty() {
        out.push_str(para.end_rpr.as_deref().unwrap_or("<a:endParaRPr/>"));
        out.push_str("</a:p>");
        return out;
    }
    let rpr = para.rpr.as_deref().unwrap_or_default();
    for (line, segment) in text.split('\n').enumerate() {
        if line > 0 {
            out.push_str(&if rpr.is_empty() {
                "<a:br/>".to_owned()
            } else {
                format!("<a:br>{rpr}</a:br>")
            });
        }
        if !segment.is_empty() {
            out.push_str(&format!("<a:r>{rpr}<a:t>{}</a:t></a:r>", escape(segment)));
        }
    }
    out.push_str(para.end_rpr.as_deref().unwrap_or_default());
    out.push_str("</a:p>");
    out
}

pub(super) fn without_bullets(ppr: Option<&str>) -> Result<String, CodecError> {
    let Some(raw) = ppr else {
        return Ok("<a:pPr><a:buNone/></a:pPr>".to_owned());
    };
    let root = parse_xml("a:pPr", raw)?;
    if root.inner_start == root.end {
        let open = raw.trim_end().trim_end_matches("/>");
        return Ok(format!("{open}><a:buNone/></a:pPr>"));
    }
    let bullets = ["a:buNone", "a:buChar", "a:buAutoNum", "a:buBlip"];
    let mut edits: Vec<(usize, usize, String)> = root
        .children
        .iter()
        .filter(|child| bullets.contains(&child.name.as_str()))
        .map(|child| (child.start, child.end, String::new()))
        .collect();
    let at = edits.first().map_or_else(
        || {
            root.children
                .iter()
                .find(|child| matches!(child.name.as_str(), "a:tabLst" | "a:defRPr" | "a:extLst"))
                .map_or(root.inner_end, |child| child.start)
        },
        |edit| edit.0,
    );
    edits.push((at, at, "<a:buNone/>".to_owned()));
    Ok(splice(raw, edits))
}

fn span_runs(span: &Span, force_bold: bool) -> String {
    let mut attrs = String::new();
    if span.bold || force_bold {
        attrs.push_str(" b=\"1\"");
    }
    if span.italic {
        attrs.push_str(" i=\"1\"");
    }
    if span.link.is_some() {
        attrs.push_str(" u=\"sng\"");
    }
    let font = if span.code {
        "<a:latin typeface=\"Courier New\"/>"
    } else {
        ""
    };
    let rpr = format!("<a:rPr lang=\"en-US\"{attrs} dirty=\"0\">{font}</a:rPr>");
    let mut out = String::new();
    for (line, segment) in span.text.split('\n').enumerate() {
        if line > 0 {
            out.push_str("<a:br/>");
        }
        if !segment.is_empty() {
            out.push_str(&format!("<a:r>{rpr}<a:t>{}</a:t></a:r>", escape(segment)));
        }
    }
    if let Some(url) = span.link.as_ref().filter(|url| **url != span.text) {
        out.push_str(&format!(
            "<a:r><a:rPr lang=\"en-US\" dirty=\"0\"/><a:t>{}</a:t></a:r>",
            escape(format!(" ({url})").as_str())
        ));
    }
    out
}

fn runs(spans: &[Span], bold: bool) -> String {
    spans.iter().map(|span| span_runs(span, bold)).collect()
}

fn list_ppr(depth: usize, number: Option<u64>) -> String {
    let margin = 342_900 * (depth + 1);
    let bullet = number.map_or_else(
        || "<a:buFont typeface=\"Arial\"/><a:buChar char=\"\u{2022}\"/>".to_owned(),
        |n| format!("<a:buFont typeface=\"+mj-lt\"/><a:buAutoNum type=\"arabicPeriod\" startAt=\"{n}\"/>"),
    );
    format!("<a:pPr marL=\"{margin}\" lvl=\"{depth}\" indent=\"-342900\">{bullet}</a:pPr>")
}

pub(super) fn markdown_xml(anchor: &Paragraph, markdown: &str) -> Result<String, CodecError> {
    let plain = without_bullets(anchor.ppr.as_deref())?;
    let mut out = String::new();
    let mut push = |ppr: &str, body: String| {
        out.push_str(&format!(
            "<a:p>{ppr}{body}<a:endParaRPr lang=\"en-US\" dirty=\"0\"/></a:p>"
        ));
    };
    for block in lower(markdown) {
        match &block {
            Block::Heading { spans, .. } => push(&plain, runs(spans, true)),
            Block::Paragraph(spans) => push(&plain, runs(spans, false)),
            Block::ListItem {
                depth,
                number,
                spans,
            } => push(&list_ppr(*depth, *number), runs(spans, false)),
            Block::Table(rows) => {
                for row in rows {
                    let span = Span {
                        text: row_text(row),
                        ..Span::default()
                    };
                    push(&plain, span_runs(&span, false));
                }
            },
            Block::Code(code) => {
                let span = Span {
                    text: code.clone(),
                    code: true,
                    ..Span::default()
                };
                push(&plain, span_runs(&span, false));
            },
            Block::Pending(_) => {
                let span = Span {
                    text: block.plain(),
                    italic: true,
                    ..Span::default()
                };
                push(&plain, span_runs(&span, false));
            },
        }
    }
    Ok(out)
}
