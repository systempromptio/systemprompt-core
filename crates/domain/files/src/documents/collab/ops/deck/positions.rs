//! The ids a deck batch wrote, named by their positions in the deck the
//! whole batch leaves behind.
//!
//! An op names what it wrote positionally (`slide[N]/shape[S]/p[P]`, or
//! `slide[N]`) as the deck stands when it runs; [`Written::note`] turns
//! each into the stable ids of that slide, shape and paragraph at once, and
//! [`Written::finish`] walks the final deck and names each again at its
//! final position, so a later op in the batch that inserts or deletes
//! before it does not leave a stale position. A noted node the batch later
//! deleted is dropped.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use loro::LoroDoc;

use super::live::{Node, child_list, node};
use crate::documents::engine::loro::deck::slides;
use crate::documents::engine::loro::rich::{block_map, string_of};

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Slide(String),
    Paragraph(String, String, String),
}

#[derive(Default)]
pub(super) struct Written(Vec<Key>);

fn slide_id(doc: &LoroDoc, slide: usize) -> Option<String> {
    block_map(&slides(doc), slide).and_then(|map| string_of(&map, "id"))
}

fn shape_ids(doc: &LoroDoc, slide: usize, shape: usize) -> Option<(String, Vec<String>)> {
    let map = block_map(&slides(doc), slide)?;
    let shape = block_map(&child_list(&map, "shapes")?, shape)?;
    let paras = child_list(&shape, "paragraphs")?;
    let ids = (0..paras.len())
        .map(|p| {
            block_map(&paras, p)
                .and_then(|m| string_of(&m, "id"))
                .unwrap_or_default()
        })
        .collect();
    Some((string_of(&shape, "id")?, ids))
}

impl Written {
    pub(super) fn note(&mut self, doc: &LoroDoc, positional: &[String]) {
        for id in positional {
            let key = match node(id) {
                Ok(Node::Slide(slide)) => slide_id(doc, slide).map(Key::Slide),
                Ok(Node::Paragraph(slide, shape, para)) => slide_id(doc, slide)
                    .zip(shape_ids(doc, slide, shape))
                    .and_then(|(slide, (shape, paras))| {
                        paras
                            .get(para)
                            .map(|p| Key::Paragraph(slide, shape, p.clone()))
                    }),
                Err(_) => None,
            };
            if let Some(key) = key
                && !self.0.contains(&key)
            {
                self.0.push(key);
            }
        }
    }

    pub(super) fn finish(self, doc: &LoroDoc) -> Vec<String> {
        let mut at: HashMap<Key, String> = HashMap::new();
        let list = slides(doc);
        for s in 0..list.len() {
            let Some(slide) = slide_id(doc, s) else {
                continue;
            };
            at.insert(Key::Slide(slide.clone()), format!("slide[{}]", s + 1));
            let shapes = block_map(&list, s)
                .and_then(|map| child_list(&map, "shapes"))
                .map_or(0, |shapes| shapes.len());
            for h in 0..shapes {
                let Some((shape, paras)) = shape_ids(doc, s, h) else {
                    continue;
                };
                for (p, para) in paras.into_iter().enumerate() {
                    at.insert(
                        Key::Paragraph(slide.clone(), shape.clone(), para),
                        format!("slide[{}]/shape[{h}]/p[{p}]", s + 1),
                    );
                }
            }
        }
        self.0
            .into_iter()
            .filter_map(|key| at.remove(&key))
            .collect()
    }
}
