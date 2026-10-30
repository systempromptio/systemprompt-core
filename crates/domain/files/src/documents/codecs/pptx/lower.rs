//! Lowering a [`DeckModel`] onto a deck's bytes.
//!
//! Paragraphs are aligned against the deck as read by text, see
//! [`align`](super::align), and the text difference becomes one batch of
//! edit operations, which the ordinary apply path
//! splices into the slide parts; parts nothing touched keep their bytes.
//!
//! Slides and shapes are paired by id, then an unpaired target entry takes
//! the earliest unpaired deck entry that fits it ([`pair`]). Marks and
//! paragraph kinds of existing paragraphs are not written.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget, Projection, content_hash,
};

use super::align::{Step, align, pair};
use super::deck::model_of;
use super::structure::restructure;
use super::{nodes, projection, rebase};
use crate::documents::codecs::zip_package::Package;
use crate::documents::engine::loro::deck::{DeckModel, DeckShape, DeckSlide, ShapeKind};
use crate::documents::model::{Block, BlockKind};

struct Batch<'p> {
    current: &'p Projection,
    ops: Vec<EditOp>,
}

impl Batch<'_> {
    fn hash(&self, id: &DocumentNodeId) -> Result<String, CodecError> {
        self.current
            .find(id)
            .map(|node| node.hash8.clone())
            .ok_or_else(|| CodecError::UnknownNode { node: id.clone() })
    }

    fn delete(&mut self, id: DocumentNodeId) -> Result<(), CodecError> {
        let expected_hash = Some(self.hash(&id)?);
        self.ops.push(EditOp::Delete {
            node: NodeTarget::Id(id),
            expected_hash,
        });
        Ok(())
    }

    fn replace(&mut self, id: DocumentNodeId, text: &str) -> Result<(), CodecError> {
        let expected_hash = Some(self.hash(&id)?);
        self.ops.push(EditOp::ReplaceText {
            node: NodeTarget::Id(id),
            expected_hash,
            text: text.to_owned(),
            content_as: ContentAs::Text,
        });
        Ok(())
    }

    fn insert_after(&mut self, id: DocumentNodeId, block: &Block) {
        let kind = match block.kind {
            BlockKind::ListItem { .. } => NodeKind::ListItem,
            _ => NodeKind::Paragraph,
        };
        self.ops.push(EditOp::InsertAfter {
            node: NodeTarget::Id(id),
            kind,
            text: block.text.clone(),
            content_as: ContentAs::Text,
        });
    }
}

pub fn lower_deck(bytes: &[u8], target: &DeckModel) -> Result<Vec<u8>, CodecError> {
    let first = lower_text(bytes, target)?;
    let second = lower_text(&first, target)?;
    restructure(&second, target)
}

pub(super) fn slide_pairs(base: &DeckModel, target: &DeckModel) -> Vec<Option<usize>> {
    pair(
        &base.slides,
        &target.slides,
        |slide| slide.id.as_str(),
        |_, slide| part_number(&slide.id).unwrap_or(u64::MAX),
        |_, _| true,
    )
}

pub(super) fn shape_pairs(base: &DeckSlide, target: &DeckSlide) -> Vec<Option<usize>> {
    pair(
        &base.shapes,
        &target.shapes,
        |shape| shape.id.as_str(),
        |at, _| at as u64,
        |base, target| {
            base.kind == target.kind || (base.kind.holds_text() && target.kind.holds_text())
        },
    )
}

fn part_number(part: &str) -> Option<u64> {
    part.strip_prefix("ppt/slides/slide")?
        .strip_suffix(".xml")?
        .parse()
        .ok()
}

fn lower_text(bytes: &[u8], target: &DeckModel) -> Result<Vec<u8>, CodecError> {
    let slides = projection::load(&mut Package::open(bytes)?)?;
    let base = model_of(&slides)?;
    let current = Projection {
        format: DocumentFormat::Pptx,
        root: nodes::nodes(&slides),
        content_sha256: content_hash(bytes),
    };
    let mut batch = Batch {
        current: &current,
        ops: Vec::new(),
    };
    let pairs = slide_pairs(&base, target);
    let mut kept = vec![false; base.slides.len()];
    for (t, paired) in pairs.iter().enumerate() {
        match *paired {
            Some(b) => {
                kept[b] = true;
                lower_slide(&mut batch, b + 1, &base.slides[b], &target.slides[t])?;
            },
            None => batch.ops.push(new_slide(None, &target.slides[t])),
        }
    }
    for (b, kept) in kept.iter().enumerate() {
        if !kept {
            batch.delete(slide_id(b + 1))?;
        }
    }
    if batch.ops.is_empty() {
        return Ok(bytes.to_vec());
    }
    rebase::apply(bytes, &batch.ops)
}

fn slide_id(slide: usize) -> DocumentNodeId {
    DocumentNodeId::new(format!("slide[{slide}]"))
}

fn para_id(slide: usize, shape: usize, para: usize) -> DocumentNodeId {
    DocumentNodeId::new(format!("slide[{slide}]/shape[{shape}]/p[{para}]"))
}

fn new_slide(anchor: Option<usize>, slide: &DeckSlide) -> EditOp {
    let texts = |kind: ShapeKind| {
        slide
            .shapes
            .iter()
            .filter(move |shape| shape.kind == kind)
            .flat_map(|shape| shape.paragraphs.iter().map(|block| block.text.clone()))
    };
    let title: Vec<String> = texts(ShapeKind::Title).collect();
    EditOp::InsertSlide {
        after: anchor.map(slide_id),
        layout: slide.layout.clone(),
        title: (!title.is_empty()).then(|| title.join("\n")),
        body: texts(ShapeKind::Body).collect(),
    }
}

fn lower_slide(
    batch: &mut Batch<'_>,
    slide: usize,
    base: &DeckSlide,
    target: &DeckSlide,
) -> Result<(), CodecError> {
    for (t, paired) in shape_pairs(base, target).into_iter().enumerate() {
        if let Some(b) = paired {
            lower_shape(batch, slide, b, &base.shapes[b], &target.shapes[t])?;
        }
    }
    Ok(())
}

fn lower_shape(
    batch: &mut Batch<'_>,
    slide: usize,
    shape: usize,
    base: &DeckShape,
    target: &DeckShape,
) -> Result<(), CodecError> {
    if !base.kind.holds_text() || base.paragraphs.is_empty() {
        return Ok(());
    }
    let base_texts: Vec<&str> = base.paragraphs.iter().map(|b| b.text.as_str()).collect();
    let target_texts: Vec<&str> = target.paragraphs.iter().map(|b| b.text.as_str()).collect();
    let mut anchor: Option<usize> = None;
    let mut front: Vec<&Block> = Vec::new();
    for step in align(&base_texts, &target_texts) {
        match step {
            Step::Keep { base: b, .. } => {
                flush_front(batch, slide, shape, b, &base.paragraphs[b], &mut front)?;
                anchor = Some(b);
            },
            Step::Change { base: b, target: t } => {
                let led = !front.is_empty();
                flush_front(batch, slide, shape, b, &target.paragraphs[t], &mut front)?;
                if !led && base_texts[b] != target_texts[t] {
                    batch.replace(para_id(slide, shape, b), target_texts[t])?;
                }
                anchor = Some(b);
            },
            Step::Remove { base: b } => batch.delete(para_id(slide, shape, b))?,
            Step::Add { target: t } => match anchor {
                Some(b) => batch.insert_after(para_id(slide, shape, b), &target.paragraphs[t]),
                None => front.push(&target.paragraphs[t]),
            },
        }
    }
    Ok(())
}

fn flush_front(
    batch: &mut Batch<'_>,
    slide: usize,
    shape: usize,
    at: usize,
    kept: &Block,
    front: &mut Vec<&Block>,
) -> Result<(), CodecError> {
    let Some((first, rest)) = front.split_first() else {
        return Ok(());
    };
    let id = para_id(slide, shape, at);
    batch.replace(id.clone(), &first.text)?;
    for block in rest {
        batch.insert_after(id.clone(), block);
    }
    batch.insert_after(id, kept);
    front.clear();
    Ok(())
}
