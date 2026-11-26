//! Pure-unit coverage for the entry-local error envelope, the proxy error
//! taxonomy, the context-extraction error mapping, and the static fallback
//! path classifier.
//!
//! Every `From<DomainError>` arm of `ApiHttpError` is exercised so the
//! variant-to-status mapping cannot silently drift, and `ProxyError` /
//! `ContextExtractionError` are walked through their public status classifiers.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::error::ApiHttpError;
use systemprompt_api::services::middleware::context::middleware::error::extraction_error_to_api_error;
use systemprompt_api::services::proxy::{ProxyError, ResponseBuildError};
use systemprompt_api::services::static_content::fallback::{get_api_suggestions, is_api_path};
use systemprompt_identifiers::{MarketplaceId, UserId};
use systemprompt_marketplace::MarketplaceError;
use systemprompt_models::execution::{ContextExtractionError, UserUnavailable};
use systemprompt_traits::RepositoryError;

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
        status_of(ApiHttpError::internal("x", "cause")),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn api_http_error_into_inner_round_trips() {
    let err = ApiHttpError::not_found("gone");
    let api = err.into_inner();
    assert_eq!(
        ApiHttpError::from(api).into_response().status(),
        StatusCode::NOT_FOUND
    );
}

fn marketplace_status(err: MarketplaceError) -> StatusCode {
    status_of(ApiHttpError::from(
        systemprompt_models::api::ApiError::from(err),
    ))
}

#[test]
fn marketplace_error_variants_classify() {
    assert_eq!(
        marketplace_status(MarketplaceError::NotFound(
            MarketplaceId::try_new("m").expect("a valid marketplace id")
        )),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        marketplace_status(MarketplaceError::NoDefault),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        marketplace_status(MarketplaceError::Signing(
            systemprompt_security::ManifestSigningError::KeyMissing
        )),
        StatusCode::INTERNAL_SERVER_ERROR
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
            ContextExtractionError::InvalidToken("x".into()),
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
            ContextExtractionError::ForbiddenHeader {
                header: "h".to_owned(),
                reason: "r".to_owned(),
            },
            StatusCode::FORBIDDEN,
        ),
        (
            ContextExtractionError::UserNotFound {
                user_id: UserId::new("u"),
                state: UserUnavailable::Missing,
            },
            StatusCode::NOT_FOUND,
        ),
        (
            ContextExtractionError::DatabaseError {
                context: "db".to_owned(),
                source: "db".into(),
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
        ContextExtractionError::InvalidToken("t".into()),
        ContextExtractionError::Revoked,
        ContextExtractionError::UserNotFound {
            user_id: UserId::new("u"),
            state: UserUnavailable::Missing,
        },
        ContextExtractionError::MissingSessionId,
        ContextExtractionError::MissingUserId,
        ContextExtractionError::MissingContextId,
        ContextExtractionError::MissingHeader {
            header: "x-foo".to_owned(),
        },
        ContextExtractionError::InvalidHeaderValue {
            header: "h".to_owned(),
            reason: "r".to_owned(),
        },
        ContextExtractionError::DatabaseError {
            context: "db".to_owned(),
            source: "db".into(),
        },
        ContextExtractionError::ForbiddenHeader {
            header: "h".to_owned(),
            reason: "r".to_owned(),
        },
    ];
    for variant in variants {
        let status = ApiHttpError::from(extraction_error_to_api_error(&variant))
            .into_response()
            .status();
        assert!(status.is_client_error() || status.is_server_error());
    }
}

fn invalid_method() -> ProxyError {
    ProxyError::InvalidMethod {
        method: "BAD METHOD".to_owned(),
        source: axum::http::Method::from_bytes(b"BAD METHOD").unwrap_err(),
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
            source: ResponseBuildError::Status {
                status: 1000,
                source: StatusCode::from_u16(1000).unwrap_err(),
            },
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
    assert_eq!(invalid_method().to_status_code(), StatusCode::BAD_REQUEST);
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

fn challenge(status: StatusCode) -> systemprompt_api::services::proxy::OAuthChallenge {
    systemprompt_api::services::proxy::OAuthChallenge {
        status,
        www_authenticate: axum::http::HeaderValue::from_static("Bearer realm=\"svc\""),
        body: "{}".to_owned(),
    }
}

#[test]
fn proxy_error_auth_challenge_preserves_inner_status() {
    let err = ProxyError::AuthChallenge(Box::new(challenge(StatusCode::UNAUTHORIZED)));
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

    let bad_req = invalid_method();
    assert_eq!(bad_req.into_response().status(), StatusCode::BAD_REQUEST);

    let ch = ProxyError::AuthChallenge(Box::new(challenge(StatusCode::FORBIDDEN)));
    let response = ch.into_response();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.headers().get(axum::http::header::WWW_AUTHENTICATE),
        Some(&axum::http::HeaderValue::from_static(
            "Bearer realm=\"svc\""
        ))
    );
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

#[test]
fn repository_error_variants_classify() {
    assert_eq!(
        status_of(RepositoryError::not_found("session", "s").into()),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status_of(RepositoryError::invalid_argument("value", "v").into()),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        status_of(RepositoryError::conflict("task", "t1", "c").into()),
        StatusCode::CONFLICT
    );
    assert_eq!(
        status_of(RepositoryError::from(sqlx::Error::RowNotFound).into()),
        StatusCode::NOT_FOUND,
        "a missing row is a 404, not a 500"
    );
    assert_eq!(
        status_of(RepositoryError::from(sqlx::Error::PoolClosed).into()),
        StatusCode::INTERNAL_SERVER_ERROR
    );
}

#[test]
fn the_remaining_proxy_error_variants_classify_and_render() {
    let cases = [
        (
            ProxyError::UrlConstructionFailed {
                service: "s".to_owned(),
                reason: "bad port".to_owned(),
            },
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
        (invalid_method(), StatusCode::BAD_REQUEST),
        (
            ProxyError::BodyExtractionFailed {
                source: axum::Error::new(std::io::Error::other("stream closed")),
            },
            StatusCode::BAD_REQUEST,
        ),
        (
            ProxyError::AuthenticationRequired {
                service: "s".to_owned(),
            },
            StatusCode::UNAUTHORIZED,
        ),
        (
            ProxyError::MissingContext {
                message: "no request context".to_owned(),
            },
            StatusCode::UNAUTHORIZED,
        ),
        (
            ProxyError::Forbidden {
                service: "s".to_owned(),
            },
            StatusCode::FORBIDDEN,
        ),
        (
            ProxyError::ProviderReauthRequired {
                service: "s".to_owned(),
            },
            StatusCode::CONFLICT,
        ),
        (
            ProxyError::ProviderNotConnected {
                service: "s".to_owned(),
            },
            StatusCode::CONFLICT,
        ),
    ];

    for (err, expected) in cases {
        let rendered = err.to_string();
        assert_eq!(err.to_status_code(), expected, "{rendered}");
        assert_eq!(
            err.into_response().status(),
            expected,
            "the rendered response must carry the classified status: {rendered}"
        );
    }
}

#[test]
fn a_proxy_error_converts_into_a_bare_status_code() {
    let status: StatusCode = ProxyError::ServiceNotFound {
        service: "missing".to_owned(),
    }
    .into();
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[test]
fn a_missing_request_context_is_a_401_not_a_500() {
    // The proxy cannot attribute the call without a RequestContext, so this is
    // an authentication problem the caller can fix — reporting it as a server
    // error would both mislead the caller and hide a real fault.
    let err = ProxyError::MissingContext {
        message: "RequestContext extension absent".to_owned(),
    };
    assert_eq!(err.to_status_code(), StatusCode::UNAUTHORIZED);
}

#[test]
fn a_blank_proxy_service_name_is_a_400_invalid_identifier() {
    let source = systemprompt_identifiers::ServiceName::try_new("  ")
        .expect_err("a blank service name is not an identifier");
    let err = ProxyError::from(source);
    assert_eq!(err.to_status_code(), StatusCode::BAD_REQUEST);
    assert_eq!(err.error_key(), "invalid_identifier");
    assert_eq!(err.into_response().status(), StatusCode::BAD_REQUEST);
}
