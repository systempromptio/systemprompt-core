//! Marketplace error types, and the one mapping of [`MarketplaceError`] onto
//! the API error model: a repository failure is classified by
//! `RepositoryError`, an unknown or unavailable marketplace or resource is
//! 404, invalid input 400, a conflict 409, and every other failure the fixed
//! 5xx body with its cause kept as the source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_identifiers::{
    LibraryArtifactId, MarketplaceId, MarketplaceRuleId, PluginId, SkillId,
};
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

    #[error("catalogue load failed: skill id '{declared}' does not match its directory name '{dir_name}'")]
    SkillIdMismatch { declared: SkillId, dir_name: String },

    #[error("catalogue load failed: rule id '{declared}' does not match its directory name '{dir_name}'")]
    RuleIdMismatch {
        declared: MarketplaceRuleId,
        dir_name: String,
    },

    #[error("catalogue load failed: rule '{rule}' names content file '{content_file}' which does not exist")]
    RuleContentMissing { rule: String, content_file: String },

    #[error("catalogue load failed: artifact '{artifact}': mcp_tools entry '{tool}' is not of the form mcp__<server>__<tool>")]
    ArtifactToolMalformed {
        artifact: LibraryArtifactId,
        tool: String,
    },

    #[error("catalogue load failed: artifact '{artifact}': mcp_tools entry '{tool}' names unknown mcp_server '{server}'")]
    ArtifactServerUnknown {
        artifact: LibraryArtifactId,
        tool: String,
        server: String,
    },

    #[error("catalogue load failed: artifact '{artifact}' is enabled but depends on disabled mcp_server '{server}' — enable the server or disable the artifact")]
    ArtifactServerDisabled {
        artifact: LibraryArtifactId,
        server: String,
    },

    #[error("catalogue load failed: plugin {plugin} missing from services config")]
    PluginUnconfigured { plugin: PluginId },

    #[error("catalogue load failed: managed {} {resource_key} lost its resource binding", .kind.as_str())]
    ManagedBindingLost {
        kind: crate::managed::ResourceKind,
        resource_key: String,
    },

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
            MarketplaceError::Managed(managed)
                if managed.is_invalid() || matches!(managed, ManagedError::InvalidInput { .. }) =>
            {
                Self::bad_request(MarketplaceError::Managed(managed).to_string())
            },
            MarketplaceError::Managed(managed) if managed.is_conflict() => {
                Self::conflict(MarketplaceError::Managed(managed).to_string())
            },
            e @ (MarketplaceError::SkillIdMismatch { .. }
            | MarketplaceError::RuleIdMismatch { .. }
            | MarketplaceError::RuleContentMissing { .. }
            | MarketplaceError::ArtifactToolMalformed { .. }
            | MarketplaceError::ArtifactServerUnknown { .. }
            | MarketplaceError::ArtifactServerDisabled { .. }
            | MarketplaceError::PluginUnconfigured { .. }
            | MarketplaceError::ManagedBindingLost { .. }
            | MarketplaceError::CatalogSource { .. }
            | MarketplaceError::Managed(_)
            | MarketplaceError::Import { .. }
            | MarketplaceError::ImportSource { .. }
            | MarketplaceError::Signing(_)
            | MarketplaceError::Filter(_)) => Self::internal("Marketplace operation failed", e),
        }
    }
}
