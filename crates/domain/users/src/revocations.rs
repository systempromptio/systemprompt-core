//! The users member's token revocation cell.
//!
//! The request authenticator the users member provides checks every token id
//! through [`TokenRevocationsCell`]. The revocation store belongs to the
//! oauth member, which composes after this one, so the users member declares
//! the cell (`Contributions::late_cell`): the kernel seeds the read half
//! before any member provides and binds it to the sealed `DynTokenRevocations`
//! the oauth member provides once the composition has resolved. Only a
//! core-standing `oauth` member may provide that type. With no store bound
//! (oauth disabled or not admitted) every token that carries a token id is
//! refused: "not revoked" cannot be known.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::LateCell;
use systemprompt_identifiers::AccessTokenId;
use systemprompt_models::auth::TokenRevocations;
use systemprompt_models::execution::context::ContextExtractionError;
use systemprompt_models::{AlreadyBound, LateBinder, LateRead, late_pair};

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("no token revocation store is bound on this host")]
struct NoRevocationStore;

/// The read half of the revocation cell; see the module docs.
#[derive(Debug, Clone)]
pub struct TokenRevocationsCell(LateRead<dyn TokenRevocations>);

impl LateCell for TokenRevocationsCell {
    type Value = dyn TokenRevocations;
    type Binder = LateBinder<dyn TokenRevocations>;

    fn unbound() -> (Self, Self::Binder) {
        let (read, binder) = late_pair::<dyn TokenRevocations>();
        (Self(read), binder)
    }

    fn bind(binder: Self::Binder, value: Arc<Self::Value>) -> Result<(), AlreadyBound> {
        binder.bind(value)
    }
}

#[async_trait]
impl TokenRevocations for TokenRevocationsCell {
    async fn ensure_not_revoked(&self, jti: &AccessTokenId) -> Result<(), ContextExtractionError> {
        let Some(store) = self.0.get() else {
            return Err(ContextExtractionError::DatabaseError {
                context: "JTI revocation lookup unavailable".to_owned(),
                source: Box::new(NoRevocationStore),
            });
        };
        store.ensure_not_revoked(jti).await
    }
}
