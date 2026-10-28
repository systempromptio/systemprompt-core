//! The walks of [`super::validate`] that reach every nested model: table
//! cells, section-break headers and footers, and the final section's
//! headers and footers, so raw markup and comment-level changes are caught
//! at any depth.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use super::rich_text::{BlockKind, MarkKind, RichText};
use super::validate::ModelRejection;

pub(super) fn check_raw(before: &RichText, next: &RichText) -> Result<(), ModelRejection> {
    let mut held = HashSet::new();
    collect_raw(before, &mut held);
    let mut now = HashSet::new();
    collect_raw(next, &mut now);
    match now.difference(&held).next() {
        Some((id, _)) => Err(ModelRejection::RawChanged((*id).to_owned())),
        None => Ok(()),
    }
}

fn collect_raw<'a>(model: &'a RichText, into: &mut HashSet<(&'a str, &'a str)>) {
    for block in &model.blocks {
        match &block.kind {
            BlockKind::Raw { source } => {
                into.insert((&block.id, source));
            },
            kind => {
                for nested in kind.nested() {
                    collect_raw(nested, into);
                }
            },
        }
    }
    for part in model.running() {
        collect_raw(part, into);
    }
}

pub(super) fn strip_comments(model: &RichText) -> RichText {
    let mut stripped = model.clone();
    strip_in_place(&mut stripped);
    for part in stripped.running_mut() {
        strip_in_place(part);
    }
    stripped
}

fn strip_in_place(model: &mut RichText) {
    for block in &mut model.blocks {
        block
            .marks
            .retain(|mark| !matches!(mark.kind, MarkKind::Comment { .. }));
        for nested in block.kind.nested_mut() {
            strip_in_place(nested);
        }
    }
}
