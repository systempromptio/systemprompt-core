//! Pure-unit coverage for the entry-local error envelope, the proxy error
//! taxonomy, the context-extraction error mapping, and the static fallback
//! path classifier.
//!
//! Every `From<DomainError>` arm of `ApiHttpError` is exercised so the
//! variant-to-status mapping cannot silently drift, and `ProxyError` /
//! `ContextExtractionError` are walked through their public status classifiers.

use axum::body::Body;
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;
use systemprompt_api::error::ApiHttpError;
use systemprompt_api::services::middleware::context::middleware::test_api::extraction_error_to_api_error;
use systemprompt_api::services::proxy::ProxyError;
use systemprompt_api::services::static_content::test_api::{get_api_suggestions, is_api_path};
use systemprompt_marketplace::MarketplaceError;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_users::UserError;

fn status_of(err: ApiHttpError) -> StatusCode {
    err.into_response().status()
}

#[test]
fn api_http_error_constructors_map_to_expected_status() {
    assert_eq!(
        status_of(ApiHttpError::not_found("x")),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(ApiHttpError::bad_request("x")),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        status_of(ApiHttpError::unauthorized("x")),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status_of(ApiHttpError::forbidden("x")),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        status_of(ApiHttpError::internal_error("x")),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn api_http_error_into_inner_round_trips() {
    let err = ApiHttpError::not_found("gone");
    let api = err.into_inner();
    assert_eq!(api.into_response().status(), StatusCode::NOT_FOUND);
}

#[test]
fn marketplace_error_variants_classify() {
    assert_eq!(
        status_of(MarketplaceError::NotFound("m".to_owned().into()).into()),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(MarketplaceError::NoDefault.into()),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(MarketplaceError::Validation("v".to_owned()).into()),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        status_of(MarketplaceError::Signing("s".to_owned()).into()),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn user_error_variants_classify() {
    assert_eq!(
        status_of(UserError::NotFound("u".to_owned().into()).into()),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(UserError::EmailAlreadyExists("e".to_owned()).into()),
        StatusCode::CONFLICT
    );
    assert_eq!(
        status_of(UserError::Validation("v".to_owned()).into()),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        status_of(UserError::InvalidRole("r".to_owned()).into()),
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn context_extraction_error_into_apihttperror_classifies() {
    let cases = [
        (
            ContextExtractionError::MissingAuthHeader,
            StatusCode::UNAUTHORIZED,
        ),
        (
            ContextExtractionError::InvalidToken("x".to_owned()),
            StatusCode::UNAUTHORIZED,
        ),
        (ContextExtractionError::Revoked, StatusCode::UNAUTHORIZED),
        (
            ContextExtractionError::MissingSessionId,
            StatusCode::UNAUTHORIZED,
        ),
        (
            ContextExtractionError::MissingUserId,
            StatusCode::UNAUTHORIZED,
        ),
        (
            ContextExtractionError::MissingContextId,
            StatusCode::BAD_REQUEST,
        ),
        (
            ContextExtractionError::InvalidUserId("x".to_owned()),
            StatusCode::BAD_REQUEST,
        ),
        (
            ContextExtractionError::ForbiddenHeader {
                header: "h".to_owned(),
                reason: "r".to_owned(),
            },
            StatusCode::FORBIDDEN,
        ),
        (
            ContextExtractionError::UserNotFound("u".to_owned()),
            StatusCode::NOT_FOUND,
        ),
        (
            ContextExtractionError::DatabaseError {
                message: "db".to_owned(),
            },
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ];
    for (err, expected) in cases {
        let http: ApiHttpError = err.into();
        assert_eq!(status_of(http), expected);
    }
}

#[test]
fn extraction_error_to_api_error_covers_all_variants() {
    let variants = [
        ContextExtractionError::MissingAuthHeader,
        ContextExtractionError::InvalidToken("t".to_owned()),
        ContextExtractionError::Revoked,
        ContextExtractionError::UserNotFound("u".to_owned()),
        ContextExtractionError::MissingSessionId,
        ContextExtractionError::MissingUserId,
        ContextExtractionError::MissingContextId,
        ContextExtractionError::MissingHeader("x-foo".to_owned()),
        ContextExtractionError::InvalidHeaderValue {
            header: "h".to_owned(),
            reason: "r".to_owned(),
        },
        ContextExtractionError::InvalidUserId("bad".to_owned()),
        ContextExtractionError::DatabaseError {
            message: "db".to_owned(),
        },
        ContextExtractionError::ForbiddenHeader {
            header: "h".to_owned(),
            reason: "r".to_owned(),
        },
    ];
    for variant in variants {
        let status = extraction_error_to_api_error(&variant)
            .into_response()
            .status();
        assert!(status.is_client_error() || status.is_server_error());
    }
}

#[test]
fn proxy_error_status_codes() {
    assert_eq!(
        ProxyError::ServiceNotFound {
            service: "s".to_owned()
        }
        .to_status_code(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        ProxyError::ServiceNotRunning {
            service: "s".to_owned(),
            status: "crashed".to_owned()
        }
        .to_status_code(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        ProxyError::Timeout {
            service: "s".to_owned()
        }
        .to_status_code(),
        StatusCode::GATEWAY_TIMEOUT
    );
    assert_eq!(
        ProxyError::InvalidResponse {
            service: "s".to_owned(),
            reason: "r".to_owned()
        }
        .to_status_code(),
        StatusCode::BAD_GATEWAY
    );
    assert_eq!(
        ProxyError::UrlConstructionFailed {
            service: "s".to_owned(),
            reason: "r".to_owned()
        }
        .to_status_code(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        ProxyError::InvalidMethod {
            reason: "r".to_owned()
        }
        .to_status_code(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        ProxyError::AuthenticationRequired {
            service: "s".to_owned()
        }
        .to_status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        ProxyError::MissingContext {
            message: "m".to_owned()
        }
        .to_status_code(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        ProxyError::Forbidden {
            service: "s".to_owned()
        }
        .to_status_code(),
        StatusCode::FORBIDDEN
    );
}

#[test]
fn proxy_error_auth_challenge_preserves_inner_status() {
    let inner: Response<Body> = (StatusCode::UNAUTHORIZED, "challenge").into_response();
    let err = ProxyError::AuthChallenge(Box::new(inner));
    assert_eq!(err.to_status_code(), StatusCode::UNAUTHORIZED);

    let status: StatusCode = err.into();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[test]
fn proxy_error_into_response_maps_status_class() {
    let not_found = ProxyError::ServiceNotFound {
        service: "svc".to_owned(),
    };
    assert_eq!(not_found.into_response().status(), StatusCode::NOT_FOUND);

    let unavailable = ProxyError::ServiceNotRunning {
        service: "svc".to_owned(),
        status: "stopped".to_owned(),
    };
    assert_eq!(
        unavailable.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let bad_req = ProxyError::InvalidMethod {
        reason: "nope".to_owned(),
    };
    assert_eq!(bad_req.into_response().status(), StatusCode::BAD_REQUEST);

    let challenge: Response<Body> = (StatusCode::FORBIDDEN, "c").into_response();
    let ch = ProxyError::AuthChallenge(Box::new(challenge));
    assert_eq!(ch.into_response().status(), StatusCode::FORBIDDEN);
}

#[test]
fn fallback_is_api_path_classifies() {
    for api in [
        "/api/v1/foo",
        "/.well-known/oauth-authorization-server",
        "/server/health",
        "/mcp/registry",
        "/health",
        "/v1/models",
        "/auth/login",
        "/oauth/token",
    ] {
        assert!(is_api_path(api), "{api} should be an API path");
    }
    for site in ["/", "/about", "/blog/my-post", "/pricing"] {
        assert!(!is_api_path(site), "{site} should not be an API path");
    }
}

#[test]
fn fallback_api_suggestions_branch_by_prefix() {
    assert!(!get_api_suggestions("/api/v1/nope").is_empty());
    assert!(!get_api_suggestions("/.well-known/nope").is_empty());
    assert!(!get_api_suggestions("/health/deep").is_empty());
    assert!(!get_api_suggestions("/openapi.json").is_empty());
    assert!(!get_api_suggestions("/something-else").is_empty());
}
