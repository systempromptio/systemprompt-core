//! Config validation error collection types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::errors::GlobalConfigError;

pub fn validate_postgres_url(url: &str) -> Result<(), GlobalConfigError> {
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err(GlobalConfigError::InvalidPostgresUrl);
    }
    Ok(())
}
