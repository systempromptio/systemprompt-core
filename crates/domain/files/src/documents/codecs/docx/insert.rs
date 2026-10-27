//! Paragraph inserts: the properties an inserted paragraph takes from its
//! anchor, the position an insert lands at, and fresh `w14:paraId`s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{CodecError, DocumentFormat, NodeKind};

use super::apply::{Batch, Located};
use super::parts::malformed;
use super::projection::heading_level;
use super::scan::{Block, Para};
use super::{markup, styles};

impl Batch<'_> {
    pub(super) fn insert_after(
        &mut self,
        order: usize,
        node: &DocumentNodeId,
        kind: NodeKind,
        text: &str,
    ) -> Result<(), CodecError> {
        let (at, anchor) = match self.find(node)? {
            Located::Para(para, _) => (para.range.end, Some(para)),
            Located::Table(table) => (table.range.end, None),
        };
        if kind == NodeKind::Pending {
            let paragraph = self.pending_paragraph(text);
            self.push(order, at..at, paragraph);
            return Ok(());
        }
        let props = self.insert_props(anchor, kind)?;
        let open = self.open_para();
        let paragraph = format!("{open}{props}{}</w:p>", markup::run(None, text));
        self.push(order, at..at, paragraph);
        Ok(())
    }

    fn insert_props(&self, anchor: Option<&Para>, kind: NodeKind) -> Result<String, CodecError> {
        let src = self.doc.src;
        let props = anchor.and_then(|para| para.ppr.as_ref());
        match kind {
            NodeKind::Heading { level } => {
                if let Some(style) = styles::heading_style(self.bytes, level)? {
                    return Ok(format!("<w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>"));
                }
                tracing::warn!(
                    level,
                    "docx has no heading style for level; copying anchor paragraph properties"
                );
                Ok(props.map_or_else(String::new, |p| src[p.range.clone()].to_owned()))
            },
            NodeKind::ListItem => {
                Ok(props.map_or_else(String::new, |p| src[p.range.clone()].to_owned()))
            },
            NodeKind::Paragraph => Ok(props.map_or_else(String::new, |p| {
                let heading = p.style.as_deref().and_then(heading_level).is_some();
                let cut: Vec<_> = p
                    .num_range
                    .iter()
                    .chain(p.style_range.iter().filter(|_| heading))
                    .cloned()
                    .collect();
                markup::ppr_without(src, &p.range, &cut)
            })),
            _ => Err(CodecError::UnsupportedOp {
                op: "insert_after",
                format: DocumentFormat::Docx,
            }),
        }
    }

    pub(super) fn open_para(&mut self) -> String {
        if self.doc.uses_para_ids() {
            format!("<w:p w14:paraId=\"{}\">", self.fresh_para_id())
        } else {
            "<w:p>".to_owned()
        }
    }

    pub(super) fn insert_at(&self, node: &DocumentNodeId) -> Result<usize, CodecError> {
        Ok(match self.find(node)? {
            Located::Para(para, _) => para.range.end,
            Located::Table(table) => table.range.end,
        })
    }

    pub(super) fn body_end(&self) -> Result<usize, CodecError> {
        let src = self.doc.src;
        let close = src
            .rfind("</w:body>")
            .ok_or_else(|| malformed("word/document.xml has no w:body end"))?;
        let after_blocks = self.doc.blocks.last().map_or_else(
            || src.find("<w:body").unwrap_or(0),
            |block| match block {
                Block::Para(para) => para.range.end,
                Block::Table(table) => table.range.end,
            },
        );
        Ok(src[after_blocks..close]
            .find("<w:sectPr")
            .map_or(close, |offset| after_blocks + offset))
    }

    fn fresh_para_id(&mut self) -> String {
        fresh_para_id(&mut self.para_ids)
    }
}

pub(super) fn fresh_para_id(taken: &mut BTreeSet<String>) -> String {
    loop {
        // Why: Word requires w14:paraId values below 0x80000000.
        let [a, b, c, d, ..] = *uuid::Uuid::new_v4().as_bytes();
        let value = u32::from_be_bytes([a, b, c, d]) & 0x7FFF_FFFF;
        let id = format!("{value:08X}");
        if value != 0 && taken.insert(id.clone()) {
            return id;
        }
    }
}
