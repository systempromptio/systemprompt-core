//! `GET /api/v1/documents/{id}/rendered`: an HTML document's stored bytes
//! served inline under a sandbox policy that never grants the app's origin,
//! with scripts and remote assets admitted only by `renderedHtml`.

use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use serde_json::json;
use systemprompt_api::routes::documents::DocumentsState;
use systemprompt_database::DbPool;
use systemprompt_files::{DocumentsConfig, RenderedHtmlConfig};
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, fixture_app_context_documents_configured, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use uuid::Uuid;

use super::documents::{
    Reply, app, create_markdown, head_of, id_of, json_request, put_content, request, send,
};

const DESIGNED: &str = "<!doctype html><html><head><style>body{background:#123456}</style>\
                        <script>window.ran=1</script></head><body><div class=\"hero\"><h1>Deck</h1>\
                        </div></body></html>";

async fn configured(rendered_html: RenderedHtmlConfig) -> Result<(DbPool, DocumentsState)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let documents = DocumentsConfig {
        enabled: true,
        rendered_html,
        ..DocumentsConfig::default()
    };
    let ctx = fixture_app_context_documents_configured(
        &pool,
        fixture_config(&bootstrap.database_url),
        documents,
    )?;
    Ok((pool, DocumentsState::build(&ctx)?))
}

async fn designed_page(state: &DocumentsState, owner: &AuthedFixture) -> Result<String> {
    let body = json!({
        "template_id": "blank-html",
        "name": format!("deck-{}.html", Uuid::new_v4().simple()),
        "params": { "values": { "title": "Deck" } }
    });
    let created = send(app(state, owner), json_request(Method::POST, "/", &body)).await?;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.json());
    let created = created.json();
    let id = id_of(&created);
    let put = put_content(state, owner, &id, &head_of(&created), DESIGNED).await?;
    assert!(put.status.is_success(), "{}", put.json());
    Ok(id)
}

async fn rendered(state: &DocumentsState, user: &AuthedFixture, id: &str) -> Result<Reply> {
    let req = request(Method::GET, &format!("/{id}/rendered"))
        .body(Body::empty())
        .expect("request");
    send(app(state, user), req).await
}

fn csp(reply: &Reply) -> String {
    reply
        .headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|value| value.to_str().ok())
        .expect("csp")
        .to_owned()
}

#[tokio::test]
async fn a_rendered_page_is_the_stored_bytes_inline_and_runs_nothing_by_default() -> Result<()> {
    let (pool, state) = configured(RenderedHtmlConfig::default()).await?;
    let owner = seed_bridge_credential(&pool, "docs-rendered").await?;
    let id = designed_page(&state, &owner).await?;

    let reply = rendered(&state, &owner, &id).await?;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());
    assert_eq!(reply.body, DESIGNED.as_bytes());
    assert_eq!(
        reply.headers.get(header::CONTENT_TYPE).expect("type"),
        "text/html; charset=utf-8"
    );
    assert_eq!(
        reply
            .headers
            .get(header::CONTENT_DISPOSITION)
            .expect("disposition"),
        "inline"
    );
    assert_eq!(
        reply
            .headers
            .get(header::X_CONTENT_TYPE_OPTIONS)
            .expect("nosniff"),
        "nosniff"
    );
    assert_eq!(
        reply.headers.get(header::CACHE_CONTROL).expect("cache"),
        "no-store"
    );
    let policy = csp(&reply);
    assert!(policy.starts_with("sandbox;"), "{policy}");
    assert!(policy.contains("default-src 'none'"), "{policy}");
    assert!(policy.contains("style-src 'unsafe-inline'"), "{policy}");
    assert!(!policy.contains("allow-"), "{policy}");
    assert!(!policy.contains("script-src"), "{policy}");
    assert!(!policy.contains("https:"), "{policy}");
    Ok(())
}

#[tokio::test]
async fn each_rendered_html_switch_widens_only_its_own_directives() -> Result<()> {
    let cases = [
        (true, false, true, false),
        (false, true, false, true),
        (true, true, true, true),
    ];
    for (scripts, remote_assets, runs, remote) in cases {
        let (pool, state) = configured(RenderedHtmlConfig {
            scripts,
            remote_assets,
        })
        .await?;
        let owner = seed_bridge_credential(&pool, "docs-rendered-on").await?;
        let id = designed_page(&state, &owner).await?;
        let policy = csp(&rendered(&state, &owner, &id).await?);
        assert_eq!(policy.contains("sandbox allow-scripts;"), runs, "{policy}");
        assert_eq!(policy.contains("script-src"), runs, "{policy}");
        assert_eq!(policy.contains("https:"), remote, "{policy}");
        assert_eq!(
            policy.contains("script-src 'unsafe-inline' 'unsafe-eval' https:"),
            runs && remote,
            "{policy}"
        );
        for never in [
            "allow-same-origin",
            "allow-top-navigation",
            "allow-popups",
            "allow-forms",
            "connect-src",
        ] {
            assert!(!policy.contains(never), "{never} in {policy}");
        }
        assert!(policy.contains("default-src 'none'"), "{policy}");
        assert!(policy.contains("form-action 'none'"), "{policy}");
    }
    Ok(())
}

#[tokio::test]
async fn only_an_html_document_renders_and_only_for_a_reader() -> Result<()> {
    let (pool, state) = configured(RenderedHtmlConfig::default()).await?;
    let owner = seed_bridge_credential(&pool, "docs-rendered-owner").await?;
    let stranger = seed_bridge_credential(&pool, "docs-rendered-stranger").await?;

    let markdown = id_of(&create_markdown(&state, &owner).await?);
    let refused = rendered(&state, &owner, &markdown).await?;
    assert!(refused.status.is_client_error(), "{}", refused.status);
    assert!(
        refused
            .headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_none_or(|value| !value.starts_with("text/html")),
        "a refusal is never an HTML page"
    );

    let page = designed_page(&state, &owner).await?;
    let hidden = rendered(&state, &stranger, &page).await?;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND, "{}", hidden.json());
    Ok(())
}

#[tokio::test]
async fn the_mounted_rendered_route_is_framed_by_the_app_alone_and_needs_a_session() -> Result<()> {
    use std::net::SocketAddr;

    use axum::extract::ConnectInfo;
    use systemprompt_api::services::server::{ReadinessState, setup_api_server};
    use systemprompt_manifest::profile::RateLimitsConfig;
    use systemprompt_test_fixtures::fixture_app_context_with_config;

    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(&pool, config)?;
    let server = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let state = DocumentsState::build(&ctx)?;
    let owner = seed_bridge_credential(&pool, "docs-rendered-mounted").await?;
    let id = designed_page(&state, &owner).await?;
    let get = |bearer: Option<String>| {
        let mut builder = request(Method::GET, &format!("/api/v1/documents/{id}/rendered"))
            .header(header::HOST, "127.0.0.1")
            .header(header::USER_AGENT, "documents-test/1.0");
        if let Some(bearer) = bearer {
            builder = builder.header(header::AUTHORIZATION, bearer);
        }
        let mut req = builder.body(Body::empty()).expect("request");
        req.extensions_mut().insert(ConnectInfo(
            "203.0.113.9:40000".parse::<SocketAddr>().expect("addr"),
        ));
        send(server.clone(), req)
    };

    let anonymous = get(None).await?;
    assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);

    let reply = get(Some(format!("Bearer {}", owner.jwt.as_str()))).await?;
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.json());
    assert_eq!(
        reply
            .headers
            .get("x-frame-options")
            .expect("x-frame-options"),
        "SAMEORIGIN"
    );
    let policy = csp(&reply);
    assert!(policy.starts_with("sandbox;"), "{policy}");
    assert!(policy.contains("frame-ancestors 'self'"), "{policy}");
    Ok(())
}
