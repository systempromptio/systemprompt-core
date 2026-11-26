//! Validation failures of the on-disk credential and tenant stores.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CloudStoreValidationError {
    #[error("API URL must be a valid URL")]
    InvalidApiUrl(#[source] url::ParseError),

    #[error("Tenant name cannot be empty")]
    EmptyTenantName,
}
