//! The secrets an install member may read: the names it declared
//! (`Contributions::secret`), recorded at admission, read through
//! [`Resolved::secrets`].
//!
//! The kernel inserts one [`SecretDeclarations`] sealed, after admission has
//! refused every declared name a [`SecretReferences`] names (the platform's
//! pinned keys, the profile's licence secret, every secret ref of the
//! services config and every core member's contributed references). A
//! provide fn, factory or slot build scoped to a member reads, through
//! [`Resolved::secrets`], a [`DeclaredSecrets`] over exactly that member's
//! declared names; the references are consulted again on every read. A
//! command's or route set's resolution reads none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_identifiers::{ExtensionId, SecretName};
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_traits::{DeclaredSecrets, SecretReferences, SecretStore};

use super::resolved::Resolved;

/// Every admitted member's declared secret names, over the live store and the
/// secret references; inserted sealed by the kernel only.
pub struct SecretDeclarations {
    store: Option<Arc<dyn SecretStore>>,
    references: Arc<dyn SecretReferences>,
    declared: HashMap<ExtensionId, Arc<[SecretName]>>,
}

impl SecretDeclarations {
    #[must_use]
    pub fn new(
        store: Option<Arc<dyn SecretStore>>,
        references: Arc<dyn SecretReferences>,
        declared: HashMap<ExtensionId, Arc<[SecretName]>>,
    ) -> Self {
        Self {
            store,
            references,
            declared,
        }
    }

    fn for_member(&self, owner: &ExtensionId) -> Result<DeclaredSecrets, MissingDependency> {
        let store = self
            .store
            .clone()
            .ok_or_else(MissingDependency::of::<Arc<dyn SecretStore>>)?;
        let declared = self
            .declared
            .get(owner)
            .cloned()
            .unwrap_or_else(|| Arc::from(Vec::new()));
        Ok(DeclaredSecrets::over(
            store,
            declared,
            Arc::clone(&self.references),
        ))
    }
}

impl std::fmt::Debug for SecretDeclarations {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretDeclarations")
            .field("declared", &self.declared)
            .finish_non_exhaustive()
    }
}

impl Resolved<'_> {
    pub fn secrets(&self) -> Result<DeclaredSecrets, MissingDependency> {
        let (owner, state) = self
            .secret_reader()
            .ok_or_else(MissingDependency::of::<DeclaredSecrets>)?;
        state
            .find::<Arc<SecretDeclarations>>()
            .ok_or_else(MissingDependency::of::<DeclaredSecrets>)?
            .for_member(owner)
    }
}
