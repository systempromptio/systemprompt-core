//! The SSO routes for a provider id nothing registered: the callback, by query
//! or form post, refuses back to the site root with `sso=unavailable` and
//! clears the binder cookie; start and metadata answer 404.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::routes::sso::{SsoRouteState, binder_from_headers, router};
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, test_app_context, test_database_url, test_db_pool,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

use crate::middleware::security_trace_served_by::ensure_config;

async fn app() -> axum::Router {
    ensure_config();
    let url = test_database_url();
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context(&pool, &url);
    let oauth = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
    );
    router(SsoRouteState::build(&ctx, &oauth))
}

fn assert_refused_unavailable(resp: &axum::response::Response) {
    assert!(
        resp.status().is_redirection(),
        "refusal is a redirect: {}",
        resp.status()
    );
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/?sso=unavailable")
    );
    let cookie = resp
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|v| v.to_str().ok())
        .expect("the binder cookie is cleared");
    assert!(cookie.contains("Max-Age=0"), "{cookie}");
}

#[tokio::test]
async fn query_callback_for_an_unknown_provider_refuses_as_unavailable() {
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .uri("/no-such-idp/callback?code=abc&state=xyz")
                .header(header::COOKIE, "other=1")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_refused_unavailable(&resp);
}

#[tokio::test]
async fn form_callback_for_an_unknown_provider_refuses_as_unavailable() {
    let resp = app()
        .await
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/no-such-idp/callback")
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from("SAMLResponse=abc&RelayState=xyz"))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_refused_unavailable(&resp);
}

#[tokio::test]
async fn start_and_metadata_for_an_unknown_provider_are_not_found() {
    let app = app().await;
    for uri in [
        "/no-such-idp/start?redirect=/admin",
        "/no-such-idp/metadata",
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri}");
    }
}

#[test]
fn binder_is_read_from_any_cookie_header_and_absent_otherwise() {
    let mut headers = axum::http::HeaderMap::new();
    assert_eq!(binder_from_headers(&headers), None);
    headers.append(header::COOKIE, "theme=dark".parse().expect("header"));
    assert_eq!(binder_from_headers(&headers), None);
    headers.append(
        header::COOKIE,
        "a=1; sp_sso_bind=bound-value".parse().expect("header"),
    );
    assert_eq!(
        binder_from_headers(&headers).as_deref(),
        Some("bound-value")
    );
}
