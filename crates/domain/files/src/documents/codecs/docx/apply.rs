//! Edit application.
//!
//! Every op in a batch is resolved against the same scan of the incoming
//! bytes — the revision the caller's projection was read from — so node ids
//! and hashes are those the caller saw, even after an earlier op in the batch
//! inserted or removed a paragraph. Each op becomes a byte-range splice;
//! splices are laid over the source from the end backwards, inserts after
//! the same node keep their op order, and two ops that rewrite overlapping
//! ranges fail the whole batch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::{BTreeSet, HashMap};

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentFormat, EditOp, hash8, required_hash,
};

use super::markup::{self, Splice};
use super::parts;
use super::parts::malformed;
use super::projection::table_text;
use super::scan::{self, Block, Cell, Doc, Para, Table};

pub(super) enum Located<'d> {
    Para(&'d Para, Option<&'d Cell>),
    Table(&'d Table),
}

fn locate<'d>(blocks: &'d [Block], cell: Option<&'d Cell>, id: &str) -> Option<Located<'d>> {
    blocks.iter().find_map(|block| match block {
        Block::Para(para) => (para.id == id).then_some(Located::Para(para.as_ref(), cell)),
        Block::Table(table) if table.id == id => Some(Located::Table(table)),
        Block::Table(table) => table
            .rows
            .iter()
            .flatten()
            .find_map(|c| locate(&c.blocks, Some(c), id)),
    })
}

fn verify(node: &DocumentNodeId, text: &str, expected: &str) -> Result<(), CodecError> {
    let actual = hash8(text);
    if actual == expected {
        return Ok(());
    }
    Err(CodecError::StaleAnchor {
        node: node.clone(),
        expected: expected.to_owned(),
        actual,
        current_text: text.to_owned(),
    })
}

pub(super) struct Batch<'d> {
    pub(super) bytes: &'d [u8],
    pub(super) doc: &'d Doc<'d>,
    splices: Vec<Splice>,
    pub(super) para_ids: BTreeSet<String>,
    cell_paras: HashMap<&'d str, usize>,
}

pub(super) fn apply(bytes: &[u8], ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    if ops.is_empty() {
        return Ok(bytes.to_vec());
    }
    ops.iter().try_for_each(EditOp::check_resolved)?;
    let xml = parts::read_document(bytes)?;
    let doc = scan::parse(&xml)?;
    let mut batch = Batch {
        bytes,
        doc: &doc,
        splices: Vec::new(),
        para_ids: doc
            .paragraphs()
            .iter()
            .filter_map(|p| p.w14_id.clone())
            .collect(),
        cell_paras: HashMap::new(),
    };
    for (order, op) in ops.iter().enumerate() {
        batch.op(order, op)?;
    }
    let edited = markup::apply_splices(&xml, batch.splices)?;
    scan::parse(&edited)?;
    parts::replace_part(bytes, parts::DOCUMENT, edited.as_bytes())
}

impl<'d> Batch<'d> {
    pub(super) fn find(&self, node: &DocumentNodeId) -> Result<Located<'d>, CodecError> {
        locate(&self.doc.blocks, None, node.as_str())
            .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })
    }

    fn paragraph(&self, node: &DocumentNodeId) -> Result<&'d Para, CodecError> {
        match self.find(node)? {
            Located::Para(para, _) => Ok(para),
            Located::Table(_) => Err(malformed(
                "the operation needs a paragraph node, not a table",
            )),
        }
    }

    pub(super) fn push(&mut self, order: usize, range: std::ops::Range<usize>, text: String) {
        self.splices.push(Splice { range, text, order });
    }

    fn op(&mut self, order: usize, op: &EditOp) -> Result<(), CodecError> {
        let src = self.doc.src;
        match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                text,
                ..
            } => {
                let node = node.resolved()?;
                let para = self.paragraph(node)?;
                verify(node, &para.text(), required_hash(expected_hash.as_ref())?)?;
                self.push(
                    order,
                    para.range.clone(),
                    markup::replace_paragraph(src, para, text),
                );
            },
            EditOp::StrReplace { old_str, new_str } => {
                let para = self.str_match(old_str)?;
                let text = para.text().replacen(old_str.as_str(), new_str, 1);
                self.push(
                    order,
                    para.range.clone(),
                    markup::replace_paragraph(src, para, &text),
                );
            },
            EditOp::InsertAfter {
                node,
                kind,
                text,
                content_as,
            } => match content_as {
                ContentAs::Text => self.insert_after(order, node.resolved()?, *kind, text)?,
                ContentAs::Markdown => {
                    let at = self.insert_at(node.resolved()?)?;
                    let xml = self.markdown_xml(text)?;
                    self.push(order, at..at, xml);
                },
            },
            EditOp::AppendMarkdown { markdown } => {
                let at = self.body_end()?;
                let xml = self.markdown_xml(markdown)?;
                self.push(order, at..at, xml);
            },
            EditOp::Delete {
                node,
                expected_hash,
            } => self.delete(
                order,
                node.resolved()?,
                required_hash(expected_hash.as_ref())?,
            )?,
            EditOp::SetCells { .. }
            | EditOp::InsertSlide { .. }
            | EditOp::SetHtml { .. }
            | EditOp::SetText { .. }
            | EditOp::Restore { .. }
            | EditOp::InsertFromRevision { .. } => {
                return Err(CodecError::UnsupportedOp {
                    op: op.name(),
                    format: DocumentFormat::Docx,
                });
            },
        }
        Ok(())
    }

    fn str_match(&self, needle: &str) -> Result<&'d Para, CodecError> {
        if needle.is_empty() {
            return Err(CodecError::NoMatch);
        }
        let plain = super::projection::blocks_text(&self.doc.blocks);
        let total = plain.matches(needle).count();
        let hits: Vec<(&Para, usize)> = self
            .doc
            .paragraphs()
            .into_iter()
            .map(|para| (para, para.text().matches(needle).count()))
            .filter(|(_, count)| *count > 0)
            .collect();
        match (total, hits.as_slice()) {
            (0, _) => Err(CodecError::NoMatch),
            (1, [(para, 1)]) => Ok(para),
            (matches, _) => Err(CodecError::AmbiguousMatch { matches }),
        }
    }

    fn delete(
        &mut self,
        order: usize,
        node: &DocumentNodeId,
        expected: &str,
    ) -> Result<(), CodecError> {
        match self.find(node)? {
            Located::Table(table) => {
                verify(node, &table_text(table), expected)?;
                self.push(order, table.range.clone(), String::new());
            },
            Located::Para(para, cell) => {
                verify(node, &para.text(), expected)?;
                let last_in_cell = cell.is_some_and(|cell| {
                    let remaining = self.cell_paras.entry(cell.id.as_str()).or_insert_with(|| {
                        cell.blocks
                            .iter()
                            .filter(|b| matches!(b, Block::Para(_)))
                            .count()
                    });
                    *remaining = remaining.saturating_sub(1);
                    *remaining == 0
                });
                // Why: OOXML requires every w:tc to end with a w:p; Word refuses to open a cell
                // without one.
                let text = if last_in_cell {
                    markup::empty_paragraph(self.doc.src, para)
                } else {
                    String::new()
                };
                self.push(order, para.range.clone(), text);
            },
        }
        Ok(())
    }
}
