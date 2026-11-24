//! `From` impls mapping domain and repository errors onto [`ApiHttpError`],
//! keeping the variant-to-HTTP-status mapping in one place so non-OAuth
//! handlers use `?`. `RepositoryError` already classifies into [`ApiError`] in
//! `systemprompt-models`; that impl is reused here. The umbrella domain errors
//! are classified by variant so that, e.g., a repository failure surfaces as
//! 500 while a missing entity surfaces as 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_config::{ProfileBootstrapError, SecretsBootstrapError};
use systemprompt_governance::authz::{AuthzError, ScopeBindingError, SubjectAttributeError};
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_loader::{BundleError, ConfigLoadError};
use systemprompt_models::api::ApiError;
use systemprompt_models::errors::GlobalConfigError;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth_issuance::DriveSyncError;
use systemprompt_provider_contracts::MissingDependency;
use systemprompt_traits::RepositoryError;

use super::ApiHttpError;

impl From<IdValidationError> for ApiHttpError {
    fn from(err: IdValidationError) -> Self {
        Self(Box::new(ApiError::from(err)))
    }
}

impl From<MissingDependency> for ApiHttpError {
    fn from(err: MissingDependency) -> Self {
        Self(Box::new(ApiError::internal(
            "Host dependency unavailable",
            err,
        )))
    }
}

impl From<RepositoryError> for ApiHttpError {
    fn from(err: RepositoryError) -> Self {
        Self(Box::new(ApiError::from(err)))
    }
}

impl From<AuthzError> for ApiHttpError {
    fn from(err: AuthzError) -> Self {
        Self(Box::new(ApiError::internal(
            "Authorization lookup failed",
            err,
        )))
    }
}

impl From<SubjectAttributeError> for ApiHttpError {
    fn from(err: SubjectAttributeError) -> Self {
        Self(Box::new(ApiError::internal(
            "Authorization lookup failed",
            err,
        )))
    }
}

impl From<ScopeBindingError> for ApiHttpError {
    fn from(err: ScopeBindingError) -> Self {
        Self(Box::new(ApiError::from(err)))
    }
}

impl From<ConfigLoadError> for ApiHttpError {
    fn from(err: ConfigLoadError) -> Self {
        Self(Box::new(ApiError::internal(
            "Services configuration unavailable",
            err,
        )))
    }
}

impl From<ProfileBootstrapError> for ApiHttpError {
    fn from(err: ProfileBootstrapError) -> Self {
        Self(Box::new(ApiError::internal("Profile not ready", err)))
    }
}

impl From<SecretsBootstrapError> for ApiHttpError {
    fn from(err: SecretsBootstrapError) -> Self {
        Self(Box::new(ApiError::internal("Secrets not ready", err)))
    }
}

impl From<GlobalConfigError> for ApiHttpError {
    fn from(err: GlobalConfigError) -> Self {
        Self(Box::new(ApiError::internal("Configuration not ready", err)))
    }
}

impl From<ContextExtractionError> for ApiHttpError {
    fn from(err: ContextExtractionError) -> Self {
        Self(Box::new(ApiError::from(err)))
    }
}

impl From<BundleError> for ApiHttpError {
    fn from(err: BundleError) -> Self {
        let api = match err {
            e @ BundleError::Auth { .. } => ApiError::forbidden(e.to_string()),
            e @ (BundleError::Verify(_)
            | BundleError::Ownership { .. }
            | BundleError::FileCollision { .. }
            | BundleError::Policy { .. }
            | BundleError::TooLarge { .. }) => ApiError::bad_request(e.to_string()),
            e @ BundleError::SourceMissing { .. } => ApiError::not_found(e.to_string()),
            e @ BundleError::NoSharedComposition { .. } => ApiError::conflict(e.to_string()),
            e @ (BundleError::Fetch { .. } | BundleError::Extract { .. } | BundleError::Io(_)) => {
                ApiError::internal("Services bundle operation failed", e)
            },
        };
        Self(Box::new(api))
    }
}

impl From<DriveSyncError> for ApiHttpError {
    fn from(error: DriveSyncError) -> Self {
        match error {
            DriveSyncError::NotDeviceToken | DriveSyncError::NoUserPermission => {
                Self::forbidden(error.to_string())
            },
            DriveSyncError::Expired => Self::unauthorized(error.to_string()),
            DriveSyncError::Mint(source) => {
                Self::internal("drive sync token was not minted", source)
            },
        }
    }
}
