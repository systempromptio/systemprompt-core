//! Node positions of one apply batch.
//!
//! Every target in a batch names a slide or paragraph as the deck stood
//! before the batch. Inserts and removals shift the positions after them,
//! so each op's node id is moved by the structural edits that came before
//! it, and a second insert after the same node lands after the first. A
//! Markdown insert shifts by every paragraph it wrote.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, DocumentFormat, EditOp, NodeTarget, Projection, content_hash, required_hash,
};

use super::apply::{Target, apply_one, target};
use super::{nodes, projection};
use crate::documents::codecs::zip_package::Package;

#[derive(Debug, Clone, Copy)]
enum Shift {
    Insert(usize),
    Remove(usize),
}

type Shape = (usize, usize);

#[derive(Debug, Default)]
struct DeckShifts {
    slides: Vec<Shift>,
    slide_tails: HashMap<usize, usize>,
    paras: HashMap<Shape, Vec<Shift>>,
    para_tails: HashMap<(usize, usize, usize), usize>,
}

fn shifted(shifts: &[Shift], index: usize) -> Option<usize> {
    shifts.iter().try_fold(index, |index, shift| match *shift {
        Shift::Insert(at) if index >= at => Some(index + 1),
        Shift::Remove(at) if index == at => None,
        Shift::Remove(at) if index > at => Some(index - 1),
        _ => Some(index),
    })
}

fn moved_tails<K>(tails: &mut HashMap<K, usize>, mut keep: impl FnMut(&K) -> bool, shift: Shift) {
    tails.retain(|key, tail| {
        if !keep(key) {
            return true;
        }
        shifted(&[shift], *tail).is_some_and(|next| {
            *tail = next;
            true
        })
    });
}

impl DeckShifts {
    fn slide(&self, slide: usize) -> Option<usize> {
        shifted(&self.slides, slide)
    }

    fn para(&self, slide: usize, shape: usize, para: usize) -> Option<usize> {
        self.paras
            .get(&(slide, shape))
            .map_or(Some(para), |shifts| shifted(shifts, para))
    }

    fn rebase(&self, node: &DocumentNodeId, anchor: bool) -> Result<DocumentNodeId, CodecError> {
        let gone = || CodecError::UnknownNode { node: node.clone() };
        Ok(match target(node)? {
            Target::Slide(slide) => {
                let current = match self.slide_tails.get(&slide).filter(|_| anchor) {
                    Some(tail) => *tail,
                    None => self.slide(slide).ok_or_else(gone)?,
                };
                DocumentNodeId::new(format!("slide[{current}]"))
            },
            Target::Paragraph { slide, shape, para } => {
                let current_slide = self.slide(slide).ok_or_else(gone)?;
                let current = match self
                    .para_tails
                    .get(&(slide, shape, para))
                    .filter(|_| anchor)
                {
                    Some(tail) => *tail,
                    None => self.para(slide, shape, para).ok_or_else(gone)?,
                };
                DocumentNodeId::new(format!(
                    "slide[{current_slide}]/shape[{shape}]/p[{current}]"
                ))
            },
            Target::Other => node.clone(),
        })
    }

    fn slide_shift(&mut self, shift: Shift) {
        moved_tails(&mut self.slide_tails, |_| true, shift);
        self.slides.push(shift);
    }

    fn para_shift(&mut self, (slide, shape): Shape, shift: Shift) {
        moved_tails(
            &mut self.para_tails,
            |(s, h, _)| (*s, *h) == (slide, shape),
            shift,
        );
        self.paras.entry((slide, shape)).or_default().push(shift);
    }

    fn apply(&mut self, package: &mut Package<'_>, op: &EditOp) -> Result<(), CodecError> {
        let Some(node) = op.touches() else {
            apply_one(package, op)?;
            return Ok(());
        };
        let anchor = matches!(op, EditOp::InsertAfter { .. } | EditOp::InsertSlide { .. });
        let current = self.rebase(node, anchor)?;
        let applied = apply_one(package, &with_node(op, current.clone()))?;
        match (op, target(node)?, target(&current)?) {
            (
                EditOp::InsertAfter { .. },
                Target::Paragraph { slide, shape, para },
                Target::Paragraph { para: at, .. },
            ) => {
                for offset in 1..=applied.inserted {
                    self.para_shift((slide, shape), Shift::Insert(at + offset));
                }
                if applied.inserted > 0 {
                    self.para_tails
                        .insert((slide, shape, para), at + applied.inserted);
                }
            },
            (EditOp::InsertSlide { .. }, Target::Slide(slide), Target::Slide(at)) => {
                self.slide_shift(Shift::Insert(at + 1));
                self.slide_tails.insert(slide, at + 1);
            },
            (EditOp::Delete { .. }, Target::Slide(_), Target::Slide(at)) => {
                self.slide_shift(Shift::Remove(at));
            },
            (
                EditOp::Delete { .. },
                Target::Paragraph { slide, shape, .. },
                Target::Paragraph { para: at, .. },
            ) if applied.removed => {
                self.para_shift((slide, shape), Shift::Remove(at));
            },
            _ => {},
        }
        Ok(())
    }
}

fn with_node(op: &EditOp, node: DocumentNodeId) -> EditOp {
    match op.clone() {
        EditOp::ReplaceText {
            expected_hash,
            text,
            content_as,
            ..
        } => EditOp::ReplaceText {
            node: NodeTarget::Id(node),
            expected_hash,
            text,
            content_as,
        },
        EditOp::InsertAfter {
            kind,
            text,
            content_as,
            ..
        } => EditOp::InsertAfter {
            node: NodeTarget::Id(node),
            kind,
            text,
            content_as,
        },
        EditOp::Delete { expected_hash, .. } => EditOp::Delete {
            node: NodeTarget::Id(node),
            expected_hash,
        },
        EditOp::InsertSlide {
            layout,
            title,
            body,
            ..
        } => EditOp::InsertSlide {
            after: Some(node),
            layout,
            title,
            body,
        },
        other => other,
    }
}

fn verify(current: &Projection, ops: &[EditOp]) -> Result<(), CodecError> {
    for op in ops {
        let (node, expected) = match op {
            EditOp::ReplaceText {
                node,
                expected_hash,
                ..
            }
            | EditOp::Delete {
                node,
                expected_hash,
            } => (
                node.resolved()?,
                Some(required_hash(expected_hash.as_ref())?),
            ),
            EditOp::InsertAfter { node, .. } => (node.resolved()?, None),
            EditOp::InsertSlide {
                after: Some(node), ..
            } => (node, None),
            _ => continue,
        };
        let found = current
            .find(node)
            .ok_or_else(|| CodecError::UnknownNode { node: node.clone() })?;
        if let Some(expected) = expected.filter(|expected| *expected != found.hash8) {
            return Err(CodecError::StaleAnchor {
                node: node.clone(),
                expected: expected.to_owned(),
                actual: found.hash8.clone(),
                current_text: found.text.clone().unwrap_or_default(),
            });
        }
    }
    Ok(())
}

pub(super) fn apply(bytes: &[u8], ops: &[EditOp]) -> Result<Vec<u8>, CodecError> {
    let mut package = Package::open(bytes)?;
    let current = Projection {
        format: DocumentFormat::Pptx,
        root: nodes::nodes(&projection::load(&mut package)?),
        content_sha256: content_hash(bytes),
    };
    ops.iter().try_for_each(EditOp::check_resolved)?;
    verify(&current, ops)?;
    let mut shifts = DeckShifts::default();
    for op in ops {
        shifts.apply(&mut package, op)?;
    }
    package.finish()
}
