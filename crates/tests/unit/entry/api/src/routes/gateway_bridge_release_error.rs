use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::routes::gateway::bridge_release::ReleaseError;

fn upstream_response(status: u16, remaining: Option<&str>) -> reqwest::Response {
    let mut builder = http::Response::builder().status(status);
    if let Some(remaining) = remaining {
        builder = builder.header("x-ratelimit-remaining", remaining);
    }
    reqwest::Response::from(builder.body(Vec::<u8>::new()).unwrap())
}

#[test]
fn unauthorized_with_token_is_token_rejected() {
    let resp = upstream_response(401, None);
    let err = ReleaseError::upstream("list releases", &resp, Some("gh_token"));
    assert!(matches!(&err, ReleaseError::TokenRejected { key, .. } if key == "gh_token"));
    assert_eq!(err.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(err.error_key(), "release_token_rejected");
    assert_eq!(err.stage(), Some("list releases"));
    assert_eq!(err.upstream_status(), Some(StatusCode::UNAUTHORIZED));
    assert!(err.to_string().contains("gh_token"));
}

#[test]
fn forbidden_rate_limit_is_upstream_not_token() {
    let resp = upstream_response(403, Some("0"));
    let err = ReleaseError::upstream("list releases", &resp, Some("gh_token"));
    assert!(matches!(err, ReleaseError::UpstreamStatus { .. }));
    assert_eq!(err.status(), StatusCode::BAD_GATEWAY);
    assert_eq!(err.upstream_status(), Some(StatusCode::FORBIDDEN));
}

#[test]
fn forbidden_without_rate_limit_is_token_rejected() {
    let resp = upstream_response(403, Some("12"));
    let err = ReleaseError::upstream("download", &resp, Some("gh_token"));
    assert!(matches!(err, ReleaseError::TokenRejected { .. }));
}

#[test]
fn refusal_without_configured_token_is_upstream_status() {
    let resp = upstream_response(401, None);
    let err = ReleaseError::upstream("download", &resp, None);
    assert!(matches!(err, ReleaseError::UpstreamStatus { .. }));
    assert_eq!(err.error_key(), "release_upstream_failed");
}

#[test]
fn server_error_is_upstream_status_even_with_token() {
    let resp = upstream_response(500, None);
    let err = ReleaseError::upstream("download", &resp, Some("gh_token"));
    assert!(
        matches!(err, ReleaseError::UpstreamStatus { status, .. } if status == StatusCode::INTERNAL_SERVER_ERROR)
    );
}

#[test]
fn configuration_answers_are_404_with_keys() {
    let cases = [
        (
            ReleaseError::NotConfigured,
            "bridge_releases_not_configured",
        ),
        (
            ReleaseError::UnknownPlatform {
                platform: "p".into(),
            },
            "platform_not_published",
        ),
        (
            ReleaseError::NoRelease {
                prefix: "bridge-v".into(),
                repo: "o/r".into(),
            },
            "release_not_found",
        ),
        (
            ReleaseError::MissingAsset {
                tag: "t".into(),
                asset: "a".into(),
            },
            "release_asset_not_found",
        ),
    ];
    for (err, key) in cases {
        assert_eq!(err.status(), StatusCode::NOT_FOUND);
        assert_eq!(err.error_key(), key);
        assert_eq!(err.stage(), None);
        assert_eq!(err.upstream_status(), None);
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }
}

#[test]
fn checksum_failures_render_bad_gateway() {
    let cases = [
        ReleaseError::NoChecksums {
            version: "1.0.0".into(),
        },
        ReleaseError::ChecksumMissing { asset: "a".into() },
    ];
    for err in cases {
        assert_eq!(err.error_key(), "release_checksum_unavailable");
        assert_eq!(err.into_response().status(), StatusCode::BAD_GATEWAY);
    }
}

#[test]
fn unconfigured_token_renders_service_unavailable() {
    let err = ReleaseError::TokenNotConfigured { key: "k".into() };
    assert_eq!(err.error_key(), "release_feed_not_ready");
    assert_eq!(
        err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[test]
fn upstream_status_renders_bad_gateway() {
    let err = ReleaseError::UpstreamStatus {
        stage: "download",
        status: StatusCode::INTERNAL_SERVER_ERROR,
    };
    assert_eq!(err.into_response().status(), StatusCode::BAD_GATEWAY);
}

#[test]
fn token_rejected_renders_service_unavailable() {
    let err = ReleaseError::TokenRejected {
        stage: "download",
        status: StatusCode::UNAUTHORIZED,
        key: "k".into(),
    };
    assert_eq!(
        err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
