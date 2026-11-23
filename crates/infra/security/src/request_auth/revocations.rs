//! The revocation check of a kernel that holds no revocation store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::AccessTokenId;
use systemprompt_models::auth::TokenRevocations;
use systemprompt_models::execution::context::ContextExtractionError;

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("this kernel holds no token revocation store")]
struct NoRevocationStore;

/// Refuses every token id: without a store, "not revoked" cannot be known.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableRevocations;

#[async_trait]
impl TokenRevocations for UnavailableRevocations {
    async fn ensure_not_revoked(&self, _jti: &AccessTokenId) -> Result<(), ContextExtractionError> {
        Err(ContextExtractionError::DatabaseError {
            context: "JTI revocation lookup unavailable".to_owned(),
            source: Box::new(NoRevocationStore),
        })
    }
}
