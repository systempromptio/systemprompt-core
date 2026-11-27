//! Site auth over the static content on the full `setup_api_server` router:
//! the static group owns the tree's fallback and is layered whole, so a page
//! the fallback serves under a protected prefix redirects an unauthenticated
//! caller to the login path, while a public page and an unmatched API path
//! are answered as without the declaration.

use std::net::SocketAddr;
use std::sync::Arc;
use systemprompt_test_fixtures::AppContextSeeds as _;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_extension::{
    ContributionError, Contributions, Extension, ExtensionMetadata, SiteAuthConfig,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_admitted_of, fixture_app_context_with_extensions,
    fixture_config, fixture_core_members,
};
use tower::ServiceExt;

const LOGIN: &str = "/site-auth-login";
const PAGE: &str = "site-auth-public-page";

struct SiteAuthMember;

impl Extension for SiteAuthMember {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "probe-site-auth",
            name: "Site auth probe",
            version: "0.0.0",
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.site_auth(SiteAuthConfig {
            login_path: LOGIN,
            protected_prefixes: &["/admin"],
            public_prefixes: &[],
            required_scope: "admin",
        });
        Ok(())
    }
}

async fn full_router(site_auth: bool) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut members = fixture_core_members();
    if site_auth {
        members.push(Box::new(SiteAuthMember));
    }
    let ctx = fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        fixture_admitted_of(members),
    )
    .await?;
    let page = ctx.app_paths().web().dist().join(PAGE);
    std::fs::create_dir_all(&page)?;
    std::fs::write(page.join("index.html"), "<h1>public</h1>")?;
    systemprompt_api::services::server::setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

async fn get(router: &Router, uri: &str) -> axum::response::Response {
    let mut request = Request::get(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .expect("request build");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4100))));
    router.clone().oneshot(request).await.expect("infallible")
}

fn location(response: &axum::response::Response) -> &str {
    response
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_fallback_page_under_a_protected_prefix_redirects_to_login() -> anyhow::Result<()> {
    let router = full_router(true).await?;
    let gated = get(&router, "/admin/agents").await;
    assert!(
        gated.status().is_redirection(),
        "an unauthenticated fallback page under /admin is redirected: {}",
        gated.status()
    );
    assert!(
        location(&gated).starts_with(LOGIN),
        "the redirect names the login path: {}",
        location(&gated)
    );
    Ok(())
}

#[tokio::test]
async fn a_public_page_and_an_unmatched_api_path_are_not_gated() -> anyhow::Result<()> {
    let router = full_router(true).await?;
    let page = get(&router, &format!("/{PAGE}")).await;
    assert_eq!(
        page.status(),
        StatusCode::OK,
        "a page outside /admin is served"
    );
    let api = get(&router, "/api/v1/nope").await;
    assert_eq!(
        api.status(),
        StatusCode::NOT_FOUND,
        "an unmatched API path is the JSON 404"
    );
    assert!(
        api.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/json")),
        "the API 404 is JSON"
    );
    Ok(())
}

#[tokio::test]
async fn without_a_declarer_the_protected_page_is_not_redirected() -> anyhow::Result<()> {
    let router = full_router(false).await?;
    let page = get(&router, "/admin/agents").await;
    assert!(
        !page.status().is_redirection(),
        "negative control: no declaration, no gate: {}",
        page.status()
    );
    Ok(())
}
