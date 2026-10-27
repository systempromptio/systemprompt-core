//! The per-format codec registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use systemprompt_provider_contracts::documents::{
    DocumentCodec, DocumentFormat, registered_codecs,
};

/// The codec chosen for each document format.
///
/// Built once at a composition root and shared; the highest-priority codec
/// for a format wins, and among equal priorities the one listed first.
#[derive(Debug, Clone, Default)]
pub struct CodecRegistry {
    by_format: BTreeMap<DocumentFormat, Arc<dyn DocumentCodec>>,
}

impl CodecRegistry {
    #[must_use]
    pub fn discover() -> Self {
        Self::from_codecs(registered_codecs())
    }

    #[must_use]
    pub fn from_codecs(codecs: impl IntoIterator<Item = Arc<dyn DocumentCodec>>) -> Self {
        let mut ordered: Vec<Arc<dyn DocumentCodec>> = codecs.into_iter().collect();
        ordered.sort_by_key(|codec| std::cmp::Reverse(codec.priority()));
        let mut by_format = BTreeMap::new();
        for codec in ordered {
            for format in codec.formats() {
                by_format
                    .entry(*format)
                    .or_insert_with(|| Arc::clone(&codec));
            }
        }
        Self { by_format }
    }

    #[must_use]
    pub fn for_format(&self, format: DocumentFormat) -> Option<Arc<dyn DocumentCodec>> {
        self.by_format.get(&format).cloned()
    }

    #[must_use]
    pub fn formats(&self) -> Vec<DocumentFormat> {
        self.by_format.keys().copied().collect()
    }
}
