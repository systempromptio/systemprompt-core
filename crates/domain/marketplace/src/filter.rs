use async_trait::async_trait;
use systemprompt_identifiers::UserId;

use crate::candidate::MarketplaceCandidate;
use crate::error::MarketplaceFilterError;

/// Decides which marketplace items a given user is permitted to see.
///
/// Implementations run inside the `/v1/bridge/manifest` handler before
/// the canonical view is signed. Returning fewer items shrinks the
/// signed manifest; returning the candidate unchanged is the
/// passthrough default. Implementations must be cheap to clone via
/// `Arc` and safe to call from the axum request worker pool.
///
/// Marked `#[async_trait]` because it is consumed as `Arc<dyn ...>` on
/// `AppContext`; see `instructions/prompt/rust.md` for the trait
/// dispatch rule.
#[async_trait]
pub trait MarketplaceFilter: Send + Sync + std::fmt::Debug {
    async fn filter(
        &self,
        user_id: &UserId,
        candidate: MarketplaceCandidate,
    ) -> Result<MarketplaceCandidate, MarketplaceFilterError>;
}

/// Passthrough filter that returns the candidate unchanged.
///
/// Used as the core default so the gateway works without any extension
/// providing an ACL backend.
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
