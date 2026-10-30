//! Lowering a [`DeckModel`] onto a deck's bytes.
//!
//! The target is aligned against the deck as read (slides and shapes by
//! id, paragraphs by text, see [`align`](super::align)) and the difference
//! becomes one batch of edit operations, which the ordinary apply path
//! splices into the slide parts; parts nothing touched keep their bytes.
//!
//! Lowering carries text and slide structure. Marks, paragraph kinds,
//! frames, slide order, and shapes added to or removed from an existing
//! slide are not written; a slide added in the target becomes a slide of its
//! layout with its title and body text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{
    CodecError, ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget, Projection, content_hash,
};

use super::align::{Step, align};
use super::deck::model_of;
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

/// Writes `target` onto `bytes`, returning `bytes` unchanged when the deck
/// already holds it.
pub fn lower_deck(bytes: &[u8], target: &DeckModel) -> Result<Vec<u8>, CodecError> {
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
    let base_ids: Vec<&str> = base.slides.iter().map(|s| s.id.as_str()).collect();
    let target_ids: Vec<&str> = target.slides.iter().map(|s| s.id.as_str()).collect();
    let mut anchor: Option<usize> = None;
    for step in align(&base_ids, &target_ids) {
        match step {
            Step::Keep { base: b, target: t } | Step::Change { base: b, target: t } => {
                lower_slide(&mut batch, b + 1, &base.slides[b], &target.slides[t])?;
                anchor = Some(b + 1);
            },
            Step::Remove { base: b } => batch.delete(slide_id(b + 1))?,
            Step::Add { target: t } => batch.ops.push(new_slide(anchor, &target.slides[t])),
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
    let base_ids: Vec<&str> = base.shapes.iter().map(|s| s.id.as_str()).collect();
    let target_ids: Vec<&str> = target.shapes.iter().map(|s| s.id.as_str()).collect();
    for step in align(&base_ids, &target_ids) {
        match step {
            Step::Keep { base: b, target: t } | Step::Change { base: b, target: t } => {
                lower_shape(batch, slide, b, &base.shapes[b], &target.shapes[t])?;
            },
            Step::Remove { .. } | Step::Add { .. } => {
                tracing::warn!(slide = %base.id, "a shape added or removed is not lowered to pptx");
            },
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

/// Paragraphs added before the first paragraph the deck kept have no
/// paragraph to follow, so the kept one takes the first added text and the
/// rest, then its own target text, are inserted after it.
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
