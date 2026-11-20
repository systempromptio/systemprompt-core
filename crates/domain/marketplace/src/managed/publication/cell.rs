//! [`PublicationPolicyCell`]: the publication policy a provided
//! [`ManagedRepository`](super::ManagedRepository) reads, bound once after the
//! governance surfaces it is built from exist.
//!
//! The policy is assembled from the built marketplace seams and the subject
//! providers, which exist only once every member's provide fn has run. The
//! `managed_resources` member therefore provides its repository over an
//! unbound cell, and the host binds the cell with the assembled policy before
//! it serves anything. Until then every operation that reads the policy fails
//! with [`ManagedError::PolicyUnbound`]; a second bind is refused.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use systemprompt_models::{AlreadyBound, LateBound};

use super::PublicationPolicy;
use crate::managed::ManagedError;

/// The late-bound publication policy shared by a repository and its host.
#[derive(Clone, Default)]
pub struct PublicationPolicyCell(LateBound<PublicationPolicy>);

impl PublicationPolicyCell {
    #[must_use]
    pub fn bound(policy: PublicationPolicy) -> Self {
        Self(LateBound::bound(Arc::new(policy)))
    }

    pub fn bind(&self, policy: PublicationPolicy) -> Result<(), AlreadyBound> {
        self.0.bind(Arc::new(policy))
    }

    #[must_use]
    pub fn is_bound(&self) -> bool {
        self.0.is_bound()
    }

    pub(crate) fn policy(&self) -> Result<&PublicationPolicy, ManagedError> {
        self.0
            .get()
            .map(AsRef::as_ref)
            .ok_or(ManagedError::PolicyUnbound)
    }
}

impl fmt::Debug for PublicationPolicyCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PublicationPolicyCell")
            .field("bound", &self.is_bound())
            .finish()
    }
}
