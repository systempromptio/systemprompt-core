//! Marketplace error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::MarketplaceId;
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
