//! The `/.well-known` OAuth discovery route table.
//!
//! Every OAuth and MCP client begins here, before it holds any credential, and
//! each path is declared twice — with and without a trailing slash — because
//! clients disagree about which they send. A missing twin is a client that
//! cannot discover the server at all, and nothing else in the tree would
//! notice. The suite drives the router rather than the handlers so the routing
//! is what is under test.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_api::routes::oauth::wellknown_routes;
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_database_url, fixture_db_pool,
};
use tower::ServiceExt;

async fn router() -> Router {
    let url = fixture_database_url().expect("DATABASE_URL");
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url)
        .await
        .expect("the discovery route tests need a reachable test database");
    let ctx = fixture_app_context(&pool, &url).expect("app context");
    wellknown_routes(&ctx)
}

async fn status(method: &str, uri: &str) -> StatusCode {
    router()
        .await
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response")
        .status()
}

fn declared_paths() -> Vec<String> {
    vec![
        ApiPaths::WELLKNOWN_OAUTH_SERVER.to_owned(),
        ApiPaths::WELLKNOWN_OPENID_CONFIG.to_owned(),
        ApiPaths::WELLKNOWN_OAUTH_PROTECTED.to_owned(),
    ]
}

// Why: this is the discovery surface an unauthenticated client reads first. A
// path that 404s is a client that never gets as far as asking for a token, so
// each one is driven rather than assumed from the route table's shape.
#[tokio::test]
async fn every_discovery_path_is_served() {
    for path in declared_paths() {
        let status = status("GET", &path).await;
        assert_ne!(
            status,
            StatusCode::NOT_FOUND,
            "{path} is part of the discovery contract and must be routed"
        );
    }
}

// Why: axum does not treat `/x` and `/x/` as the same route, so each path is
// registered twice by hand. Deleting one of the `format!("{}/", …)` arms is
// invisible to every other test and breaks exactly the clients that append a
// slash. This is the test that notices.
#[tokio::test]
async fn each_discovery_path_answers_with_and_without_a_trailing_slash() {
    for path in declared_paths() {
        let bare = status("GET", &path).await;
        let slashed = status("GET", &format!("{path}/")).await;
        assert_eq!(
            bare, slashed,
            "{path} and {path}/ must answer alike; clients send both"
        );
    }
}

// Why: the protected-resource document is also served per-resource under a
// wildcard, which is how a client discovers the metadata for one specific
// resource rather than the server default.
#[tokio::test]
async fn the_protected_resource_document_is_served_for_a_nested_resource() {
    let status = status(
        "GET",
        &format!("{}/mcp/some-server", ApiPaths::WELLKNOWN_OAUTH_PROTECTED),
    )
    .await;

    assert_ne!(
        status,
        StatusCode::NOT_FOUND,
        "the wildcard arm is how a client discovers metadata for one resource"
    );
}

// Why: discovery is read cross-origin by browser-based clients, so every path
// carries an OPTIONS arm. Without it the preflight fails and the browser never
// issues the GET — a failure that looks like a CORS misconfiguration rather
// than a missing route.
#[tokio::test]
async fn every_discovery_path_answers_a_cors_preflight() {
    for path in declared_paths() {
        for uri in [path.clone(), format!("{path}/")] {
            assert_eq!(
                status("OPTIONS", &uri).await,
                StatusCode::OK,
                "{uri} must answer a preflight or browsers never send the GET"
            );
        }
    }
}

#[tokio::test]
async fn a_path_outside_the_discovery_contract_is_not_served() {
    assert_eq!(
        status("GET", "/.well-known/something-we-do-not-publish").await,
        StatusCode::NOT_FOUND,
        "the router must not answer for paths it does not declare"
    );
}
