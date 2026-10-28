//! Who placed each uploaded image in a live session.
//!
//! An update that adds an image reference records its editor as the
//! image's owner in the session; any later editor (someone who only types)
//! does not change it. A checkpoint asks the uploads service whether any
//! of the image's owners, or any editor of the session, uploaded it, so an
//! image A inserted survives a checkpoint B's typing triggers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use systemprompt_identifiers::UserId;

use crate::documents::model::{RichText, media_refs};

const MAX_EDITORS: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct ImageOwners {
    by_ref: HashMap<String, UserId>,
    editors: Vec<UserId>,
}

impl ImageOwners {
    /// Records `editor` as the owner of every image `after` holds that
    /// `before` did not, and as an editor of the session.
    pub fn record(&mut self, before: &RichText, after: &RichText, editor: &UserId) {
        let held = media_refs(before);
        for reference in media_refs(after) {
            if !held.contains(&reference) {
                self.by_ref
                    .entry(reference)
                    .or_insert_with(|| editor.clone());
            }
        }
        if !self.editors.contains(editor) && self.editors.len() < MAX_EDITORS {
            self.editors.push(editor.clone());
        }
    }

    /// Everyone whose upload may place `reference`: the editor who inserted
    /// it first, then every editor of the session.
    #[must_use]
    pub fn owners(&self, reference: &str) -> Vec<UserId> {
        let mut owners: Vec<UserId> = self.by_ref.get(reference).cloned().into_iter().collect();
        for editor in &self.editors {
            if !owners.contains(editor) {
                owners.push(editor.clone());
            }
        }
        owners
    }
}
