//! Marketplace error types, and the one mapping of [`MarketplaceError`] onto
//! the API error model: a repository failure is classified by
//! `RepositoryError`, an unknown or unavailable marketplace or resource is
//! 404, invalid input 400, a conflict 409, and every other failure the fixed
//! 5xx body with its cause kept as the source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::api::ApiError;
use systemprompt_security::ManifestSigningError;
use systemprompt_traits::BoxedSource;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarketplaceFilterError {
    #[error("acl backend unavailable: {0}")]
    Backend(#[source] BoxedSource),
}

#[derive(Debug, Error)]
pub enum MarketplaceError {
    #[error("marketplace not found: {0}")]
    NotFound(MarketplaceId),

    #[error("no default marketplace configured")]
    NoDefault,

    #[error("catalogue load failed: {0}")]
    Catalog(String),

    #[error("catalogue load failed: {context}: {source}")]
    CatalogSource {
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error("managed resource resolution failed: {0}")]
    Managed(#[from] crate::managed::ManagedError),

    #[error("manifest signing failed: {0}")]
    Signing(#[from] ManifestSigningError),

    #[error("import failed at {path}: {message}")]
    Import { path: String, message: String },

    #[error("import failed at {path}: {context}: {source}")]
    ImportSource {
        path: String,
        context: String,
        #[source]
        source: BoxedSource,
    },

    #[error(transparent)]
    Filter(#[from] MarketplaceFilterError),
}

impl MarketplaceError {
    pub fn catalog(context: impl Into<String>, source: impl Into<BoxedSource>) -> Self {
        Self::CatalogSource {
            context: context.into(),
            source: source.into(),
        }
    }

    pub fn import(path: &Path, context: impl Into<String>, source: impl Into<BoxedSource>) -> Self {
        Self::ImportSource {
            path: path.display().to_string(),
            context: context.into(),
            source: source.into(),
        }
    }
}

impl From<MarketplaceError> for ApiError {
    fn from(err: MarketplaceError) -> Self {
        use crate::managed::ManagedError;
        match err {
            MarketplaceError::Managed(ManagedError::Repository(inner)) => Self::from(inner),
            e @ (MarketplaceError::NotFound(_)
            | MarketplaceError::NoDefault
            | MarketplaceError::Managed(ManagedError::Unavailable)) => {
                Self::not_found(e.to_string())
            },
            e @ MarketplaceError::Managed(
                ManagedError::Invalid(_) | ManagedError::InvalidInput { .. },
            ) => Self::bad_request(e.to_string()),
            e @ MarketplaceError::Managed(ManagedError::Conflict(_)) => {
                Self::conflict(e.to_string())
            },
            e @ (MarketplaceError::Catalog(_)
            | MarketplaceError::CatalogSource { .. }
            | MarketplaceError::Managed(_)
            | MarketplaceError::Import { .. }
            | MarketplaceError::ImportSource { .. }
            | MarketplaceError::Signing(_)
            | MarketplaceError::Filter(_)) => Self::internal("Marketplace operation failed", e),
        }
    }
}
