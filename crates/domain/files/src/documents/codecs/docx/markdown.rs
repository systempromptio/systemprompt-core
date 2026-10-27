//! Markdown blocks written as `WordprocessingML`.
//!
//! Headings take the document's `Heading1`–`Heading9` style when
//! `word/styles.xml` defines it. List items take `ListParagraph` when it is
//! defined, are indented by depth and keep their marker (`- ` or `N. `) as
//! text: no numbering definition is added, so the list projects as
//! paragraphs. Tables are written as bordered `w:tbl`s, code blocks as
//! monospaced paragraphs, and links as underlined runs followed by the URL in
//! parentheses; the relationships part is not edited.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::apply::Batch;
use super::{markup, styles};
use crate::documents::codecs::markdown_lowering::{Block, Span, lower};

const BORDERS: &str = "<w:tblBorders><w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" \
    w:color=\"auto\"/><w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/><w:bottom \
    w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/><w:right w:val=\"single\" w:sz=\"4\" \
    w:space=\"0\" w:color=\"auto\"/><w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" \
    w:color=\"auto\"/><w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" \
    w:color=\"auto\"/></w:tblBorders>";

fn span_runs(span: &Span) -> String {
    let mut rpr = String::new();
    if span.code {
        rpr.push_str("<w:rFonts w:ascii=\"Courier New\" w:hAnsi=\"Courier New\"/>");
    }
    if span.bold {
        rpr.push_str("<w:b/>");
    }
    if span.italic {
        rpr.push_str("<w:i/>");
    }
    if span.link.is_some() {
        rpr.push_str("<w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/>");
    }
    let props = (!rpr.is_empty()).then(|| format!("<w:rPr>{rpr}</w:rPr>"));
    let mut out = markup::run(props.as_deref(), &span.text);
    if let Some(url) = span.link.as_ref().filter(|url| **url != span.text) {
        out.push_str(&markup::run(None, &format!(" ({url})")));
    }
    out
}

fn runs(spans: &[Span]) -> String {
    spans.iter().map(span_runs).collect()
}

impl Batch<'_> {
    pub(super) fn markdown_xml(&mut self, markdown: &str) -> Result<String, CodecError> {
        let mut out = String::new();
        for block in lower(markdown) {
            let xml = self.block_xml(&block)?;
            out.push_str(&xml);
        }
        Ok(out)
    }

    fn new_paragraph(&mut self, ppr: &str, runs: &str) -> String {
        let open = self.open_para();
        format!("{open}{ppr}{runs}</w:p>")
    }

    fn block_xml(&mut self, block: &Block) -> Result<String, CodecError> {
        Ok(match block {
            Block::Heading { level, spans } => {
                let ppr = styles::heading_style(self.bytes, *level)?
                    .map(|style| format!("<w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>"))
                    .unwrap_or_default();
                self.new_paragraph(&ppr, &runs(spans))
            },
            Block::Paragraph(spans) => self.new_paragraph("", &runs(spans)),
            Block::ListItem { depth, spans, .. } => {
                let style = if styles::has_style(self.bytes, "ListParagraph")? {
                    "<w:pStyle w:val=\"ListParagraph\"/>"
                } else {
                    ""
                };
                let indent = 720 * (depth + 1);
                let ppr = format!("<w:pPr>{style}<w:ind w:left=\"{indent}\"/></w:pPr>");
                let marker = block.marker();
                let body = format!("{}{}", markup::run(None, marker.trim_start()), runs(spans));
                self.new_paragraph(&ppr, &body)
            },
            Block::Code(code) => {
                let span = Span {
                    text: code.clone(),
                    code: true,
                    ..Span::default()
                };
                self.new_paragraph("", &span_runs(&span))
            },
            Block::Table(rows) => self.table_xml(rows),
        })
    }

    fn table_xml(&mut self, rows: &[Vec<Vec<Span>>]) -> String {
        let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut out = format!(
            "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/>{BORDERS}</w:tblPr><w:tblGrid>{}</w:tblGrid>",
            "<w:gridCol/>".repeat(columns)
        );
        for row in rows {
            out.push_str("<w:tr>");
            for column in 0..columns {
                let cell = row.get(column).map(|spans| runs(spans)).unwrap_or_default();
                let paragraph = self.new_paragraph("", &cell);
                out.push_str(&format!(
                    "<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr>{paragraph}</w:tc>"
                ));
            }
            out.push_str("</w:tr>");
        }
        out.push_str("</w:tbl>");
        out
    }
}
