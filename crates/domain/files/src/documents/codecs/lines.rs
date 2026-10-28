//! Line-addressed documents shared by the Markdown and text codecs: one
//! `line[N]` node per line under a `body` root, and the edits applied to them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentFormat, EditOp, NodeKind, Projection, ProjectionNode, hash8,
    required_hash,
};

use super::anchors::{str_replace_once, verify_anchor};
use super::markdown_lowering::{lower, plain_lines};
use super::pending::{intent_of_line, line_marker};

pub(crate) const BODY_ID: &str = "body";

#[derive(Debug, Clone)]
struct Slot {
    text: Option<String>,
    after: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LineDoc {
    ending: &'static str,
    trailing: bool,
    head: Vec<String>,
    slots: Vec<Slot>,
    rewritten: bool,
}

impl LineDoc {
    pub(crate) fn parse(text: &str) -> Self {
        let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let (body, trailing) = text
            .strip_suffix(ending)
            .map_or((text, false), |body| (body, true));
        let slots = if text.is_empty() {
            Vec::new()
        } else {
            body.split(ending)
                .map(|line| Slot {
                    text: Some(line.to_owned()),
                    after: Vec::new(),
                })
                .collect()
        };
        Self {
            ending,
            trailing,
            head: Vec::new(),
            slots,
            rewritten: false,
        }
    }

    pub(crate) fn render(&self) -> String {
        let lines: Vec<&str> = self
            .head
            .iter()
            .map(String::as_str)
            .chain(self.slots.iter().flat_map(|slot| {
                slot.text
                    .as_deref()
                    .into_iter()
                    .chain(slot.after.iter().map(String::as_str))
            }))
            .collect();
        let mut out = lines.join(self.ending);
        if self.trailing && !lines.is_empty() {
            out.push_str(self.ending);
        }
        out
    }

    pub(crate) fn line_nodes(&self) -> Vec<ProjectionNode> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.text.as_deref().map(|text| line_node(index + 1, text)))
            .collect()
    }

    fn normalise(&self, text: &str) -> String {
        let unix = text.replace("\r\n", "\n");
        if self.ending == "\n" {
            unix
        } else {
            unix.replace('\n', self.ending)
        }
    }

    fn split_inserted(text: &str) -> Vec<String> {
        text.replace("\r\n", "\n")
            .split('\n')
            .map(str::to_owned)
            .collect()
    }

    fn inserted(
        text: &str,
        content_as: ContentAs,
        format: DocumentFormat,
        lead: bool,
    ) -> Vec<String> {
        match (content_as, format) {
            (ContentAs::Text, _) => Self::split_inserted(text),
            (ContentAs::Markdown, DocumentFormat::Markdown) => {
                let body = Self::split_inserted(text.trim_matches('\n'));
                if lead {
                    std::iter::once(String::new()).chain(body).collect()
                } else {
                    body
                }
            },
            (ContentAs::Markdown, _) => plain_lines(&lower(text)),
        }
    }

    fn rewrite(&mut self, text: &str) {
        let keep_trailing = self.trailing;
        *self = Self::parse(&self.normalise(text));
        if self.slots.is_empty() {
            self.trailing = keep_trailing;
        }
        self.rewritten = true;
    }

    pub(crate) fn apply(
        &mut self,
        op: &EditOp,
        format: DocumentFormat,
        current: &Projection,
    ) -> Result<(), CodecError> {
        if self.rewritten && op.touches().is_some() {
            return Err(CodecError::Malformed {
                detail: "an anchored op cannot follow a whole-document op in the same batch",
            });
        }
        match op {
            EditOp::ReplaceText { .. } | EditOp::Delete { .. } | EditOp::InsertAfter { .. } => {
                self.apply_anchored(op, format, current)
            },
            EditOp::StrReplace { old_str, new_str } => {
                let replaced = str_replace_once(&self.render(), old_str, &self.normalise(new_str))?;
                self.rewrite(&replaced);
                Ok(())
            },
            EditOp::SetText { text } => {
                self.rewrite(text);
                Ok(())
            },
            EditOp::AppendMarkdown { markdown } => {
                let lead = !self.render().is_empty();
                let lines = Self::inserted(markdown, ContentAs::Markdown, format, lead);
                match self.slots.last_mut() {
                    Some(slot) => slot.after.extend(lines),
                    None => self.head.extend(lines),
                }
                Ok(())
            },
            EditOp::SetCells { .. }
            | EditOp::InsertSlide { .. }
            | EditOp::SetHtml { .. }
            | EditOp::Restore { .. }
            | EditOp::InsertFromRevision { .. } => Err(CodecError::UnsupportedOp {
                op: op.name(),
                format,
            }),
        }
    }

    fn apply_anchored(
        &mut self,
        op: &EditOp,
        format: DocumentFormat,
        current: &Projection,
    ) -> Result<(), CodecError> {
        match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                text,
                ..
            } => {
                let node = node.resolved()?;
                let mut lines = Self::split_inserted(text).into_iter();
                let slot = self.anchored_slot(
                    line_index(node, op, format, current)?,
                    node,
                    required_hash(expected_hash.as_ref())?,
                )?;
                slot.text = Some(lines.next().unwrap_or_default());
                let rest: Vec<String> = lines.collect();
                slot.after.splice(0..0, rest);
                Ok(())
            },
            EditOp::Delete {
                node,
                expected_hash,
            } => {
                let node = node.resolved()?;
                let slot = self.anchored_slot(
                    line_index(node, op, format, current)?,
                    node,
                    required_hash(expected_hash.as_ref())?,
                )?;
                slot.text = None;
                Ok(())
            },
            EditOp::InsertAfter {
                node,
                kind,
                text,
                content_as,
            } => {
                let node = node.resolved()?;
                let lines = if *kind == NodeKind::Pending {
                    vec![line_marker(format, text)]
                } else {
                    Self::inserted(text, *content_as, format, node.as_str() != BODY_ID)
                };
                if node.as_str() == BODY_ID {
                    self.head.extend(lines);
                    return Ok(());
                }
                let index = line_index(node, op, format, current)?;
                let slot = self
                    .slots
                    .get_mut(index)
                    .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
                slot.after.extend(lines);
                Ok(())
            },
            _ => Err(CodecError::Malformed {
                detail: "apply_anchored takes replace_text, delete or insert_after",
            }),
        }
    }

    fn anchored_slot(
        &mut self,
        index: usize,
        node: &DocumentNodeId,
        expected_hash: &str,
    ) -> Result<&mut Slot, CodecError> {
        let slot = self
            .slots
            .get_mut(index)
            .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
        let text = slot
            .text
            .as_deref()
            .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
        verify_anchor(&line_node(index + 1, text), expected_hash)?;
        Ok(slot)
    }
}

pub(crate) fn line_node(number: usize, text: &str) -> ProjectionNode {
    let (kind, shown) =
        intent_of_line(text).map_or((NodeKind::Text, text), |intent| (NodeKind::Pending, intent));
    ProjectionNode {
        id: DocumentNodeId::new(format!("line[{number}]")),
        kind,
        text: Some(shown.to_owned()),
        hash8: hash8(text),
        children: Vec::new(),
        attrs: std::collections::BTreeMap::new(),
    }
}

fn line_index(
    node: &DocumentNodeId,
    op: &EditOp,
    format: DocumentFormat,
    current: &Projection,
) -> Result<usize, CodecError> {
    let number = node
        .as_str()
        .strip_prefix("line[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|digits| digits.parse::<usize>().ok())
        .filter(|number| *number > 0);
    match number {
        Some(number) => Ok(number - 1),
        None if current.find(node).is_some() => Err(CodecError::UnsupportedOp {
            op: op.name(),
            format,
        }),
        None => Err(CodecError::UnknownNode { node: node.clone() }),
    }
}
