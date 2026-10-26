//! Extension router mounting through `setup_api_server`: nested and
//! root-merged base paths, the public/auth wrapping split, and the
//! frame-options override stamp.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use axum::routing::get;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ExtensionRouter,
};
use systemprompt_manifest::profile::FrameOptions;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_db_pool,
};
use tower::ServiceExt;

struct NestedPublicExt;

impl Extension for NestedPublicExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covmount",
            name: "Coverage Mount Extension",
            version: "0.0.1",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        let mut ext = ExtensionRouter::public(
            Router::new().route("/ping", get(|| async { "ext-ok" })),
            "/api/covmount",
        );
        ext.frame_options = Some(FrameOptions::AllowAll);
        Some(ext)
    }
}

struct RootMergedExt;

impl Extension for RootMergedExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covroot",
            name: "Coverage Root Extension",
            version: "0.0.1",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        Some(ExtensionRouter::public(
            Router::new().route("/covroot-ping", get(|| async { "root-ok" })),
            "/",
        ))
    }
}

struct AuthedExt;

impl Extension for AuthedExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covauth",
            name: "Coverage Authed Extension",
            version: "0.0.1",
        }
    }

    fn router(&self, _ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        Some(ExtensionRouter::new(
            Router::new().route("/secret", get(|| async { "authed" })),
            "/api/covauth",
        ))
    }
}

struct SiteAuthExt;

impl Extension for SiteAuthExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "covsiteauth",
            name: "Coverage Site Auth Extension",
            version: "0.0.1",
        }
    }

    fn site_auth(&self) -> Option<systemprompt_extension::SiteAuthConfig> {
        Some(systemprompt_extension::SiteAuthConfig {
            login_path: "/covlogin",
            protected_prefixes: &[],
            public_prefixes: &["/covpublic"],
            required_scope: "admin",
        })
    }
}

async fn app_with_extensions(injected: Vec<Arc<dyn Extension>>) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;

    let mut config = fixture_config(&bootstrap.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];

    let mut registry =
        ExtensionRegistry::discover().map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    registry
        .merge(injected)
        .map_err(|e| anyhow::anyhow!("registry: {e}"))?;
    registry
        .validate()
        .map_err(|e| anyhow::anyhow!("registry: {e}"))?;

    let ctx = fixture_app_context_with_extensions(&pool, config, registry)?;

    setup_api_server(
        &ctx,
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

fn get_req(uri: &str) -> Request<Body> {
    let mut req = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request build");
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.11:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    req
}

#[tokio::test]
async fn extension_routes_mount_across_nested_root_and_authed_paths() -> anyhow::Result<()> {
    let app = app_with_extensions(vec![
        Arc::new(NestedPublicExt),
        Arc::new(RootMergedExt),
        Arc::new(AuthedExt),
    ])
    .await?;

    let nested = app.clone().oneshot(get_req("/api/covmount/ping")).await?;
    assert_eq!(nested.status().as_u16(), 200, "{}", nested.status());
    assert!(
        nested.headers().get("x-frame-options").is_none(),
        "AllowAll frame override must suppress X-Frame-Options, got {:?}",
        nested.headers().get("x-frame-options")
    );
    let body = http_body_util::BodyExt::collect(nested.into_body())
        .await?
        .to_bytes();
    assert_eq!(&body[..], b"ext-ok");

    let root = app.clone().oneshot(get_req("/covroot-ping")).await?;
    assert_eq!(root.status().as_u16(), 200, "{}", root.status());

    let authed = app.oneshot(get_req("/api/covauth/secret")).await?;
    assert_eq!(
        authed.status().as_u16(),
        401,
        "auth-required extension route must reject anonymous requests, got {}",
        authed.status()
    );
    Ok(())
}

#[tokio::test]
async fn site_auth_extension_gates_static_pages_behind_login_redirect() -> anyhow::Result<()> {
    let app = app_with_extensions(vec![Arc::new(SiteAuthExt)]).await?;

    let gated = app.clone().oneshot(get_req("/some-protected-page")).await?;
    assert!(
        gated.status().is_redirection(),
        "unauthenticated page must redirect to login, got {}",
        gated.status()
    );
    let location = gated
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(location.starts_with("/covlogin"), "{location}");

    let public = app.oneshot(get_req("/covpublic/page")).await?;
    assert!(
        !public.status().is_redirection(),
        "public prefix must bypass the gate, got {}",
        public.status()
    );
    Ok(())
}
