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
