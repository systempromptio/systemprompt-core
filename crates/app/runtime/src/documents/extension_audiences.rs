//! The extension audience resolvers the document subject directory expands
//! dimension subjects through. The directory is built before the context
//! that extension factories need exists, so the set is bound once, by the
//! builder, after notification seam discovery; until then, and in a build
//! without the `notifications` feature, it is empty and a dimension names
//! nobody.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use systemprompt_extension::NotificationAudienceResolver;

/// A late-bound, per-context list of extension audience resolvers.
#[derive(Clone, Default)]
pub struct ExtensionAudiences(Arc<OnceLock<Vec<Arc<dyn NotificationAudienceResolver>>>>);

impl ExtensionAudiences {
    pub fn bind(&self, resolvers: Vec<Arc<dyn NotificationAudienceResolver>>) {
        if self.0.set(resolvers).is_err() {
            tracing::warn!("Extension audience resolvers are already bound to this context");
        }
    }

    #[must_use]
    pub fn resolvers(&self) -> &[Arc<dyn NotificationAudienceResolver>] {
        self.0.get().map_or(&[], Vec::as_slice)
    }
}

impl std::fmt::Debug for ExtensionAudiences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionAudiences")
            .field("bound", &self.0.get().map(Vec::len))
            .finish()
    }
}
