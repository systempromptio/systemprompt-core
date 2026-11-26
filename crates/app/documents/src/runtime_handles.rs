//! The handles a [`DocumentsRuntime`] is bound into its cell as, and
//! [`bound_handles`], the one read of that cell from the kernel state
//! ([`resolved_handles`] reads it through a resolution).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::Resolved;
use systemprompt_files::documents::{
    DocumentsHandles, DocumentsHandlesSlot, DocumentsUnbound, bound,
};
use systemprompt_provider_contracts::Dependencies;

use crate::DocumentsRuntime;

impl DocumentsRuntime {
    #[must_use]
    pub fn handles(&self, documents_enabled: bool) -> DocumentsHandles {
        DocumentsHandles {
            access: Arc::clone(self.access()),
            repositories: Arc::clone(self.repositories()),
            issues: Arc::clone(self.issues()),
            service: documents_enabled.then(|| Arc::clone(self.service())),
            uploads: Arc::clone(self.uploads()),
            contexts: Arc::clone(self.contexts()),
            #[cfg(feature = "collab")]
            collab: Arc::clone(self.collab()),
        }
    }
}

pub fn bound_handles(state: &Dependencies) -> Result<Arc<DocumentsHandles>, DocumentsUnbound> {
    let slot = state
        .find::<DocumentsHandlesSlot>()
        .ok_or(DocumentsUnbound)?;
    bound(slot).cloned()
}

pub fn resolved_handles(r: &Resolved<'_>) -> Result<Arc<DocumentsHandles>, DocumentsUnbound> {
    let slot = r
        .optional::<DocumentsHandlesSlot>()
        .ok_or(DocumentsUnbound)?;
    bound(&slot).cloned()
}
