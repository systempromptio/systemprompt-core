//! Paragraph-level edits for the text and presentation codecs.
//!
//! Every op in a batch is resolved against the same read of `content.xml`,
//! so node ids and hashes are those the caller saw even after an earlier op
//! in the batch inserted or removed a paragraph. Each op becomes a byte-range
//! splice; inserts after the same node keep their op order and two ops that
//! touch overlapping ranges fail the whole batch.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentFormat, EditOp, NodeKind, required_hash,
};

use super::markdown::markdown_markup;
use super::package::{Splice, malformed, verify};
use super::projection::{Para, emptied, fresh, replaced};

#[derive(Debug, Clone)]
pub(super) struct Container {
    pub id: String,
    pub text: String,
    pub range: Range<usize>,
    pub accepts_insert_after: bool,
}

pub(super) struct FlowBatch<'d> {
    pub format: DocumentFormat,
    pub paras: &'d [Para],
    pub containers: &'d [Container],
    pub splices: Vec<Splice>,
    pub body_end: Option<usize>,
}

enum Target<'d> {
    Para(&'d Para),
    Container(&'d Container),
}

impl<'d> FlowBatch<'d> {
    pub(super) const fn new(
        format: DocumentFormat,
        paras: &'d [Para],
        containers: &'d [Container],
    ) -> Self {
        Self {
            format,
            paras,
            containers,
            splices: Vec::new(),
            body_end: None,
        }
    }

    const fn unsupported(&self, op: &EditOp) -> CodecError {
        CodecError::UnsupportedOp {
            op: op.name(),
            format: self.format,
        }
    }

    fn find(&self, node: &DocumentNodeId) -> Result<Target<'d>, CodecError> {
        let id = node.as_str();
        self.paras
            .iter()
            .find(|para| para.id == id)
            .map(Target::Para)
            .or_else(|| {
                self.containers
                    .iter()
                    .find(|container| container.id == id)
                    .map(Target::Container)
            })
            .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })
    }

    pub(super) fn push(&mut self, order: usize, range: Range<usize>, text: String) {
        self.splices.push(Splice { range, text, order });
    }

    pub(super) fn op(&mut self, order: usize, op: &EditOp) -> Result<(), CodecError> {
        match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                text,
                ..
            } => {
                let node = node.resolved()?;
                let Target::Para(para) = self.find(node)? else {
                    return Err(malformed("replace_text needs a paragraph node"));
                };
                verify(
                    node.as_str(),
                    &para.text,
                    required_hash(expected_hash.as_ref())?,
                )?;
                self.push(order, para.range.clone(), replaced(para, text));
            },
            EditOp::StrReplace { old_str, new_str } => {
                let para = self.str_match(old_str)?;
                let text = para.text.replacen(old_str.as_str(), new_str, 1);
                self.push(order, para.range.clone(), replaced(para, &text));
            },
            EditOp::InsertAfter {
                node,
                text,
                content_as: ContentAs::Markdown,
                ..
            } => {
                let at = self.markdown_at(node.resolved()?, op)?;
                self.push(order, at..at, markdown_markup(self.format, text));
            },
            EditOp::InsertAfter { .. } => self.insert_after(order, op)?,
            EditOp::AppendMarkdown { markdown } => {
                let at = self
                    .body_end
                    .or_else(|| self.paras.last().map(|para| para.range.end))
                    .ok_or_else(|| self.unsupported(op))?;
                self.push(order, at..at, markdown_markup(self.format, markdown));
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
            | EditOp::InsertFromRevision { .. }
            | EditOp::SetMarks(_)
            | EditOp::SetBlockType(_)
            | EditOp::InsertTable(_)
            | EditOp::TableInsertRow(_)
            | EditOp::TableInsertCol(_)
            | EditOp::TableDelete(_)
            | EditOp::InsertImage(_) => return Err(self.unsupported(op)),
        }
        Ok(())
    }

    fn str_match(&self, needle: &str) -> Result<&'d Para, CodecError> {
        if needle.is_empty() {
            return Err(CodecError::NoMatch);
        }
        let joined = self
            .paras
            .iter()
            .map(|para| para.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let total = joined.matches(needle).count();
        let hits: Vec<(&Para, usize)> = self
            .paras
            .iter()
            .map(|para| (para, para.text.matches(needle).count()))
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
            Target::Container(container) => {
                verify(node.as_str(), &container.text, expected)?;
                self.push(order, container.range.clone(), String::new());
            },
            Target::Para(para) => {
                verify(node.as_str(), &para.text, expected)?;
                match (&para.list_item, para.sole_in_list_item) {
                    (Some(item), true) => self.push(order, item.clone(), String::new()),
                    _ if para.last_in_container => {
                        self.push(order, para.range.clone(), emptied(para));
                    },
                    _ => self.push(order, para.range.clone(), String::new()),
                }
            },
        }
        Ok(())
    }

    fn markdown_at(&self, node: &DocumentNodeId, op: &EditOp) -> Result<usize, CodecError> {
        match self.find(node)? {
            Target::Para(para) => Ok(para
                .list_item
                .as_ref()
                .map_or(para.range.end, |item| item.end)),
            Target::Container(container) if container.accepts_insert_after => {
                Ok(container.range.end)
            },
            Target::Container(_) => Err(self.unsupported(op)),
        }
    }

    fn insert_after(&mut self, order: usize, op: &EditOp) -> Result<(), CodecError> {
        let EditOp::InsertAfter {
            node, kind, text, ..
        } = op
        else {
            return Err(self.unsupported(op));
        };
        let node = node.resolved()?;
        let kind = *kind;
        let (at, anchor) = match self.find(node)? {
            Target::Para(para) => {
                let at = match (&para.list_item, kind) {
                    (Some(item), NodeKind::ListItem) => item.end,
                    _ => para.range.end,
                };
                (at, Some(para))
            },
            Target::Container(container) if container.accepts_insert_after => {
                (container.range.end, None)
            },
            Target::Container(_) => return Err(self.unsupported(op)),
        };
        let mut markup = fresh(kind, anchor, text).ok_or_else(|| self.unsupported(op))?;
        if kind == NodeKind::ListItem && anchor.is_none_or(|para| para.list_item.is_none()) {
            markup = format!("<text:list>{markup}</text:list>");
        }
        self.push(order, at..at, markup);
        Ok(())
    }
}
