use systemprompt_identifiers::MarketplaceId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarketplaceFilterError {
    #[error("acl backend unavailable: {0}")]
    Backend(String),

    #[error("user not found: {0}")]
    UnknownUser(String),

    #[error("policy evaluation failed: {0}")]
    Policy(String),
}

#[derive(Debug, Error)]
pub enum MarketplaceError {
    #[error("marketplace not found: {0}")]
    NotFound(MarketplaceId),

    #[error("no default marketplace configured")]
    NoDefault,

    #[error("marketplace validation failed: {0}")]
    Validation(String),

    #[error("catalogue load failed: {0}")]
    Catalog(String),

    #[error("manifest signing failed: {0}")]
    Signing(String),

    #[error(transparent)]
    Filter(#[from] MarketplaceFilterError),
}
