use std::path::PathBuf;

use axum::response::IntoResponse;
use http::StatusCode;
use systemprompt_api::error::ApiHttpError;
use systemprompt_content::ContentError;
use systemprompt_identifiers::{MarketplaceId, UserId};
use systemprompt_loader::BundleError;
use systemprompt_marketplace::MarketplaceError;
use systemprompt_marketplace::managed::ManagedError;
use systemprompt_models::api::{ApiError, ErrorCode};
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_oauth::OauthError;
use systemprompt_oauth::services::SessionCreationError;
use systemprompt_traits::RepositoryError;
use systemprompt_users::UserError;

fn code_of(err: impl Into<ApiHttpError>) -> ApiError {
    err.into().into_inner()
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
fn content_error_maps_not_found_bad_request_and_internal() {
    assert_eq!(
        code_of(ContentError::ContentNotFound("a".into())).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(ContentError::LinkNotFound("a".into())).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(ContentError::InvalidRequest("a".into())).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(ContentError::Validation("a".into())).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(ContentError::Repository(repo_not_found())).code,
        ErrorCode::NotFound
    );
    let internal = code_of(ContentError::Parse("secret detail".into()));
    assert_eq!(internal.code, ErrorCode::InternalError);
    assert!(!internal.message.contains("secret detail"));
}

#[test]
fn marketplace_error_maps_every_class() {
    assert_eq!(
        code_of(MarketplaceError::NotFound(MarketplaceId::new("m"))).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(MarketplaceError::NoDefault).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(MarketplaceError::Managed(ManagedError::Unavailable)).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(MarketplaceError::Managed(ManagedError::Invalid("x".into()))).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(MarketplaceError::Managed(ManagedError::InvalidInput {
            context: "ctx",
            source: boxed(),
        }))
        .code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(MarketplaceError::Managed(ManagedError::Conflict(
            "x".into()
        )))
        .code,
        ErrorCode::ConflictError
    );
    assert_eq!(
        code_of(MarketplaceError::Managed(ManagedError::Repository(
            repo_not_found()
        )))
        .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(MarketplaceError::Managed(ManagedError::Integrity)).code,
        ErrorCode::InternalError
    );
    assert_eq!(
        code_of(MarketplaceError::Catalog("x".into())).code,
        ErrorCode::InternalError
    );
    assert_eq!(
        code_of(MarketplaceError::Import {
            path: "p".into(),
            message: "m".into(),
        })
        .code,
        ErrorCode::InternalError
    );
}

#[test]
fn user_error_maps_every_class() {
    let id = || UserId::new("u-1");
    assert_eq!(code_of(UserError::NotFound(id())).code, ErrorCode::NotFound);
    assert_eq!(
        code_of(UserError::EmailAlreadyExists("a@b".into())).code,
        ErrorCode::ConflictError
    );
    assert_eq!(
        code_of(UserError::LegalHold(id())).code,
        ErrorCode::ConflictError
    );
    assert_eq!(
        code_of(UserError::NotArchived(id())).code,
        ErrorCode::ConflictError
    );
    assert_eq!(
        code_of(UserError::RestoreRefused {
            id: id(),
            window_days: 30,
        })
        .code,
        ErrorCode::ConflictError
    );
    assert_eq!(
        code_of(UserError::Validation("x".into())).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(UserError::InvalidStatus("x".into())).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(UserError::InvalidRole("x".into())).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(UserError::InvalidRoles(vec!["x".into()])).code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(UserError::ApiKeyLifetimeExceeded {
            requested: chrono::Utc::now(),
            max_days: 90,
        })
        .code,
        ErrorCode::BadRequest
    );
    assert_eq!(
        code_of(UserError::MergeUnavailable).code,
        ErrorCode::InternalError
    );
    assert_eq!(
        code_of(UserError::OwnerReassignment {
            domain: "content",
            source: repo_not_found(),
        })
        .code,
        ErrorCode::InternalError
    );
    assert_eq!(
        code_of(UserError::Repository(repo_not_found())).code,
        ErrorCode::NotFound
    );
}

#[test]
fn oauth_error_maps_by_kind() {
    let cases = [
        (OauthError::CodeNotFound("c".into()), ErrorCode::NotFound),
        (OauthError::TokenNotFound("t".into()), ErrorCode::NotFound),
        (OauthError::ClientNotFound("c".into()), ErrorCode::NotFound),
        (OauthError::UserNotFound("u".into()), ErrorCode::NotFound),
        (OauthError::Validation("v".into()), ErrorCode::BadRequest),
        (OauthError::ChallengeExpired, ErrorCode::BadRequest),
        (
            OauthError::WebAuthnVerificationFailed("w".into()),
            ErrorCode::BadRequest,
        ),
        (
            OauthError::UsernameTaken("n".into()),
            ErrorCode::ConflictError,
        ),
        (
            OauthError::EmailRegistered("e".into()),
            ErrorCode::ConflictError,
        ),
        (
            OauthError::InvalidClient("c".into()),
            ErrorCode::Unauthorized,
        ),
        (
            OauthError::AuthenticationUnavailable,
            ErrorCode::Unauthorized,
        ),
        (
            OauthError::InvalidGrant("g".into()),
            ErrorCode::Unauthorized,
        ),
        (OauthError::TokenMissingKid, ErrorCode::Unauthorized),
        (
            OauthError::Unauthorized("x".into()),
            ErrorCode::Unauthorized,
        ),
        (
            OauthError::InvalidTokenLifetime { seconds: -1 },
            ErrorCode::InternalError,
        ),
    ];
    for (err, expected) in cases {
        let label = err.to_string();
        assert_eq!(code_of(err).code, expected, "{label}");
    }
}

#[test]
fn oauth_client_failure_hides_detail() {
    let api = code_of(OauthError::InvalidClient("client secret xyz".into()));
    assert_eq!(api.message, "Client authentication failed");
}

#[test]
fn session_creation_error_maps_missing_user_to_404() {
    assert_eq!(
        code_of(SessionCreationError::UserNotFound {
            user_id: UserId::new("u"),
        })
        .code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(SessionCreationError::Internal(boxed())).code,
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
    let response = ApiHttpError::from(UserError::NotFound(UserId::new("u"))).into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = ApiHttpError::from(ContentError::Parse("p".into())).into_response();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn agent_error_maps_not_found_repository_and_internal() {
    use systemprompt_agent::AgentError;
    assert_eq!(
        code_of(AgentError::NotFound("a".into())).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(AgentError::Repository(repo_not_found())).code,
        ErrorCode::NotFound
    );
    assert_eq!(
        code_of(AgentError::EmptyCorsAllowlist).code,
        ErrorCode::InternalError
    );
}

#[test]
fn scope_binding_error_maps_dimension_membership_and_lookup() {
    use systemprompt_governance::authz::{AuthzError, ScopeBindingError};
    use systemprompt_identifiers::ScopeDimension;
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
        code_of(ScopeBindingError::Lookup(AuthzError::InvalidRuleType(
            "x".into()
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
