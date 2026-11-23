//! The secrets a member may read: the names and prefixes it declared
//! (`Contributions::secret`, `Contributions::secret_prefix`) that the
//! operator granted it in the profile (`extensions.secrets`), read through
//! [`Resolved::secrets`].
//!
//! The kernel inserts one [`SecretDeclarations`] sealed, after admission has
//! refused every declaration no grant covers and every declared name a
//! [`SecretReferences`] names (the core secret names, every secret field of
//! the profile and the services config, and every core member's contributed
//! references). A provide fn, factory or slot build scoped to a member reads,
//! through [`Resolved::secrets`], a [`DeclaredSecrets`] over exactly that
//! member's declarations and grants; the grant and the references are
//! consulted again on every read. A command's or route set's resolution reads
//! none.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Arc;

use systemprompt_identifiers::ExtensionId;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_traits::{DeclaredSecrets, SecretPattern, SecretReferences, SecretStore};

use super::resolved::Resolved;

/// One admitted member's declared secret patterns and the patterns the
/// operator granted it.
#[derive(Debug, Clone)]
pub struct MemberSecrets {
    pub declared: Arc<[SecretPattern]>,
    pub granted: Arc<[SecretPattern]>,
}

/// Every admitted member's declared and granted secrets, over the live store
/// and the secret references; inserted sealed by the kernel only.
pub struct SecretDeclarations {
    store: Option<Arc<dyn SecretStore>>,
    references: Arc<dyn SecretReferences>,
    declared: HashMap<ExtensionId, MemberSecrets>,
}

impl SecretDeclarations {
    #[must_use]
    pub fn new(
        store: Option<Arc<dyn SecretStore>>,
        references: Arc<dyn SecretReferences>,
        declared: HashMap<ExtensionId, MemberSecrets>,
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
        let member = self
            .declared
            .get(owner)
            .cloned()
            .unwrap_or_else(|| MemberSecrets {
                declared: Arc::from(Vec::new()),
                granted: Arc::from(Vec::new()),
            });
        Ok(DeclaredSecrets::over(
            store,
            member.declared,
            member.granted,
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
