//! `ManagedRevisionOwnership` over the owner-scoped revision table, so a
//! domain persisting a revision reference can refuse a foreign id without
//! depending on this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{ManagedResourceId, ResourceRevisionId, UserId};
use systemprompt_traits::{ManagedRevisionOwnership, ManagedSkillResolverError};

use super::{ManagedError, ManagedRepository};

#[async_trait::async_trait]
impl ManagedRevisionOwnership for ManagedRepository {
    async fn revision_resource(
        &self,
        owner: &UserId,
        revision: &ResourceRevisionId,
    ) -> Result<Option<ManagedResourceId>, ManagedSkillResolverError> {
        match Self::revision_resource(self, owner, revision).await {
            Ok(resource) => Ok(Some(resource)),
            Err(ManagedError::Unavailable) => Ok(None),
            Err(error) => Err(ManagedSkillResolverError::Unavailable(error.to_string())),
        }
    }
}
