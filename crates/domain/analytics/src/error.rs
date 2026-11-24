//! Typed error boundary for the `systemprompt-analytics` crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::domain_error;

domain_error! {
    pub enum AnalyticsError {
        common: [repository, io, json],

        #[error("session owner: {0}")]
        SessionOwner(#[from] systemprompt_traits::AnalyticsProviderError),

        #[error("Invalid argument: {reason}")]
        InvalidArgument {
            field: &'static str,
            reason: String,
        },

        #[error("Session expired")]
        SessionExpired,
    }
}

impl From<sqlx::Error> for AnalyticsError {
    fn from(err: sqlx::Error) -> Self {
        Self::Repository(systemprompt_traits::RepositoryError::from(err))
    }
}

impl AnalyticsError {
    pub fn invalid_argument(field: &'static str, reason: impl Into<String>) -> Self {
        Self::InvalidArgument {
            field,
            reason: reason.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, AnalyticsError>;
