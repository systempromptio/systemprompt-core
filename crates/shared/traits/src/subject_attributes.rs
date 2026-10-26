//! [`SubjectAttributes`]: the values a user holds per extension-declared
//! subject dimension (department, cost centre, clearance), as a handle an
//! extension can read without naming the governance crate.
//!
//! Held as `Arc<dyn SubjectAttributes>`, hence `#[async_trait]`: native `async
//! fn` in traits is not `dyn`-compatible.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::UserId;

use crate::BoxedSource;

#[derive(Debug, thiserror::Error)]
pub enum SubjectAttributesError {
    #[error("no subject dimension '{0}' is registered")]
    UnknownDimension(String),
    #[error("subject dimension '{dimension}' could not be read")]
    Provider {
        dimension: String,
        #[source]
        source: BoxedSource,
    },
}

#[async_trait]
pub trait SubjectAttributes: Send + Sync {
    async fn values_for(
        &self,
        user: &UserId,
        dimension: &str,
    ) -> Result<Vec<String>, SubjectAttributesError>;

    async fn resolve_all(
        &self,
        user: &UserId,
    ) -> Result<Vec<(String, Vec<String>)>, SubjectAttributesError>;
}
