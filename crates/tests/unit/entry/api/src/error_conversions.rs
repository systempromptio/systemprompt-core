use std::path::PathBuf;

use axum::response::IntoResponse;
use http::StatusCode;
use systemprompt_api::error::ApiHttpError;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_loader::BundleError;
use systemprompt_marketplace::MarketplaceError;
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_traits::RepositoryError;

fn code_of(err: impl Into<ApiHttpError>) -> ApiError {
    err.into().into_inner()
}

fn marketplace_code(err: MarketplaceError) -> ApiError {
    ApiError::from(err)
}

fn repo_not_found() -> RepositoryError {
    RepositoryError::NotFound {
        entity: "widget",
        key: Some("w1".to_string()),
    }
}

fn boxed() -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::other("cause"))
}

#[test]
fn repository_error_classifies_by_variant() {
    assert_eq!(code_of(repo_not_found()).code, ErrorCode::NotFound);
    let conflict = RepositoryError::Conflict {
        entity: "widget",
        key: "w1".to_string(),
        reason: "taken".to_string(),
    };
    assert_eq!(code_of(conflict).code, ErrorCode::ConflictError);
    let invalid = RepositoryError::InvalidArgument {
        field: "name",
        reason: "empty".to_string(),
    };
    assert_eq!(code_of(invalid).code, ErrorCode::BadRequest);
}

#[test]
fn marketplace_error_maps_every_class() {
    assert_eq!(
        marketplace_code(MarketplaceError::NotFound(MarketplaceId::new("m"))).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        marketplace_code(MarketplaceError::NoDefault).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Managed(ManagedError::Unavailable)).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Managed(ManagedError::Invalid("x".into()))).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Managed(ManagedError::InvalidInput {
            context: "ctx",
            source: boxed(),
        }))
        .code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Managed(ManagedError::Conflict(
            "x".into()
        )))
        .code,
        ErrorCode::ConflictError
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Managed(ManagedError::Repository(
            repo_not_found()
        )))
        .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Managed(ManagedError::Integrity)).code,
        ErrorCode::InternalError
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Catalog("x".into())).code,
        ErrorCode::InternalError
    );
    assert_eq!(
        marketplace_code(MarketplaceError::Import {
            path: "p".into(),
            message: "m".into(),
        })
        .code,
        ErrorCode::InternalError
    );
}

#[test]
fn context_extraction_error_maps_every_class() {
    let cases = [
        (
            ContextExtractionError::InvalidToken(boxed()),
            ErrorCode::Unauthorized,
        ),
        (
            ContextExtractionError::MissingHeader("x".into()),
            ErrorCode::Unauthorized,
        ),
        (
            ContextExtractionError::MissingAuthHeader,
            ErrorCode::Unauthorized,
        ),
        (ContextExtractionError::Revoked, ErrorCode::Unauthorized),
        (
            ContextExtractionError::MissingSessionId,
            ErrorCode::Unauthorized,
        ),
        (
            ContextExtractionError::MissingUserId,
            ErrorCode::Unauthorized,
        ),
        (
            ContextExtractionError::MissingContextId,
            ErrorCode::BadRequest,
        ),
        (
            ContextExtractionError::InvalidHeaderValue {
                header: "h".into(),
                reason: "r".into(),
            },
            ErrorCode::BadRequest,
        ),
        (
            ContextExtractionError::InvalidUserId("u".into()),
            ErrorCode::BadRequest,
        ),
        (
            ContextExtractionError::ForbiddenHeader {
                header: "h".into(),
                reason: "r".into(),
            },
            ErrorCode::Forbidden,
        ),
        (
            ContextExtractionError::UserNotFound("u".into()),
            ErrorCode::NotFound,
        ),
        (
            ContextExtractionError::DatabaseError {
                context: "c".into(),
                source: boxed(),
            },
            ErrorCode::InternalError,
        ),
    ];
    for (err, expected) in cases {
        let label = err.to_string();
        assert_eq!(code_of(err).code, expected, "{label}");
    }
}

#[test]
fn bundle_error_maps_every_class() {
    assert_eq!(
        code_of(BundleError::Auth {
            source_name: "s".into(),
        })
        .code,
        ErrorCode::Forbidden
    );
    assert_eq!(
        code_of(BundleError::Ownership {
            id: "i".into(),
            kind: "k".into(),
            first: "a".into(),
            second: "b".into(),
        })
        .code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(BundleError::Policy {
            detail: "d".into(),
            cause: None,
        })
        .code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(BundleError::Fetch {
            source_name: "s".into(),
            detail: "d".into(),
            cause: None,
        })
        .code,
        ErrorCode::InternalError
    );
    assert_eq!(
        code_of(BundleError::Extract {
            path: PathBuf::from("/x"),
            detail: "d".into(),
            cause: None,
        })
        .code,
        ErrorCode::InternalError
    );
    assert_eq!(
        code_of(BundleError::SourceMissing { name: "s".into() }).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(BundleError::TooLarge { bytes: 10 }).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(BundleError::Io(std::io::Error::other("io"))).code,
        ErrorCode::InternalError
    );
}

#[test]
fn api_http_error_responds_with_mapped_status() {
    let response = ApiHttpError::from(repo_not_found()).into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = ApiHttpError::from(BundleError::Io(std::io::Error::other("io"))).into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn scope_binding_error_maps_dimension_membership_and_lookup() {
    use systemprompt_governance::authz::{AuthzError, ScopeBindingError};
    use systemprompt_identifiers::ScopeDimension;
    use systemprompt_traits::authz::SubjectAttributeError;
    let dimension = || ScopeDimension::try_new("team").expect("valid dimension");
    assert_eq!(
        code_of(ScopeBindingError::UnknownDimension(dimension())).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(ScopeBindingError::NotAMember {
            dimension: dimension(),
            value: "red".into(),
        })
        .code,
        ErrorCode::Forbidden
    );
    assert_eq!(
        code_of(ScopeBindingError::Lookup(SubjectAttributeError::Lookup(
            Box::new(AuthzError::InvalidRuleType("x".into()))
        )))
        .code,
        ErrorCode::InternalError
    );
}

#[test]
fn bootstrap_and_config_errors_are_internal_without_detail() {
    use systemprompt_config::{ProfileBootstrapError, SecretsBootstrapError};
    use systemprompt_loader::ConfigLoadError;
    use systemprompt_models::errors::GlobalConfigError;
    let cases = [
        code_of(ProfileBootstrapError::PathNotSet),
        code_of(SecretsBootstrapError::NoSecretsConfigured),
        code_of(GlobalConfigError::NotInitialized),
        code_of(ConfigLoadError::ProfileBootstrap(
            ProfileBootstrapError::PathNotSet,
        )),
    ];
    for api in cases {
        assert_eq!(api.code, ErrorCode::InternalError);
        assert!(!api.message.contains("initialized"), "{}", api.message);
    }
}
