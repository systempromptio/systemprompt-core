//! Per-user filtering contract applied to the catalogue before the bridge
//! manifest is signed. Implementations must be cheap to clone via `Arc` and
//! safe to call from the axum worker pool.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

use crate::candidate::MarketplaceCandidate;
use crate::error::MarketplaceFilterError;

#[async_trait]
pub trait MarketplaceFilter: Send + Sync + std::fmt::Debug {
    async fn filter(
        &self,
        user_id: &UserId,
        candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct AllowAllFilter;

#[async_trait]
impl MarketplaceFilter for AllowAllFilter {
    async fn filter(
        &self,
        _user_id: &UserId,
        candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError> {
        Ok(candidate)
    }
}
