//! The handles a [`DocumentsRuntime`] is bound into its cell as.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_files::documents::DocumentsHandles;

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
