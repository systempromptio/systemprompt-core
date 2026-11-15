//! The content route groups on the real router, mounted from the
//! `content-http` extension's declarations:
//!
//! | group | anonymous | signed-in user | `content-http` disabled |
//! |---|---|---|---|
//! | public `/api/v1/content` | served | served | 404 |
//! | link generation `/api/v1/content/links/generate` | 401 | served | 404 |
//! | redirect `/r/{short_code}` | redirects | redirects | 404 |
//!
//! The redirect records a click for a visitor and none for a detected bot.
//! Only the `All` and `Api` roles serve the groups, and a profile that
//! disables `content` while `content-http` stays enabled is refused at boot.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_content::repository::{LinkAnalyticsRepository, LinkRepository};
use systemprompt_content::{
    GenerateLinkParams, LinkAnalyticsService, LinkGenerationService, LinkType,
};
use systemprompt_content_entry::extension::CONTENT_HTTP_EXTENSION;
use systemprompt_database::DbPool;
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::{ExtensionId, JwtToken, LinkId};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

const QUERY: &str = "/api/v1/content/query";
const GENERATE: &str = "/api/v1/content/links/generate";
const VISITOR: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) \
                       Chrome/126.0 Safari/537.36";
const CRAWLER: &str = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

async fn node(role: NodeRole, disabled: &[ExtensionId]) -> Result<(DbPool, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = role;
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_extensions(
        &pool,
        config,
        ExtensionRegistry::discover()?,
        disabled,
    )?;
    let router = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((pool, router))
}

struct Call<'a> {
    method: Method,
    uri: &'a str,
    body: Option<serde_json::Value>,
    bearer: Option<&'a JwtToken>,
    user_agent: &'a str,
}

impl<'a> Call<'a> {
    fn get(uri: &'a str) -> Self {
        Self {
            method: Method::GET,
            uri,
            body: None,
            bearer: None,
            user_agent: VISITOR,
        }
    }

    fn post(uri: &'a str, body: serde_json::Value) -> Self {
        Self {
            method: Method::POST,
            uri,
            body: Some(body),
            bearer: None,
            user_agent: VISITOR,
        }
    }

    fn bearer(mut self, token: &'a JwtToken) -> Self {
        self.bearer = Some(token);
        self
    }

    fn user_agent(mut self, user_agent: &'a str) -> Self {
        self.user_agent = user_agent;
        self
    }

    async fn status(self, app: &Router) -> Result<StatusCode> {
        let mut builder = Request::builder()
            .method(self.method)
            .uri(self.uri)
            .header(header::HOST, "127.0.0.1")
            .header(header::USER_AGENT, self.user_agent);
        if let Some(token) = self.bearer {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
        }
        let body = match self.body {
            Some(json) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(json.to_string())
            },
            None => Body::empty(),
        };
        let mut req = builder.body(body)?;
        req.extensions_mut()
            .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
        Ok(app.clone().oneshot(req).await?.status())
    }
}

fn query_body() -> serde_json::Value {
    serde_json::json!({ "query": "mounted" })
}

fn generate_body() -> serde_json::Value {
    serde_json::json!({ "target_url": "https://example.com/landing", "link_type": "redirect" })
}

async fn seed_link(pool: &DbPool) -> Result<(LinkId, String)> {
    let link = LinkGenerationService::new(LinkRepository::new(pool))
        .generate_link(GenerateLinkParams {
            target_url: "https://example.com/mounted".to_owned(),
            link_type: LinkType::Redirect,
            campaign_id: None,
            campaign_name: None,
            source_content_id: None,
            source_page: None,
            utm_params: None,
            link_text: None,
            link_position: None,
            expires_at: None,
        })
        .await?;
    Ok((link.id, link.short_code))
}

async fn clicks(pool: &DbPool, link: &LinkId) -> Result<usize> {
    let service = LinkAnalyticsService::new(
        LinkRepository::new(pool),
        LinkAnalyticsRepository::new(pool),
    );
    Ok(service.get_link_clicks(link, None, None).await?.len())
}

#[tokio::test]
async fn the_public_group_serves_an_anonymous_caller() -> Result<()> {
    let (_pool, app) = node(NodeRole::All, &[]).await?;
    assert_eq!(
        Call::post(QUERY, query_body()).status(&app).await?,
        StatusCode::OK
    );
    Ok(())
}

#[tokio::test]
async fn link_generation_refuses_an_anonymous_caller_at_the_user_gate() -> Result<()> {
    let (_pool, app) = node(NodeRole::All, &[]).await?;
    assert_eq!(
        Call::post(GENERATE, generate_body()).status(&app).await?,
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}

#[tokio::test]
async fn link_generation_serves_a_signed_in_user() -> Result<()> {
    let (pool, app) = node(NodeRole::All, &[]).await?;
    let user = seed_bridge_credential(&pool, "content-mounted").await?;
    assert_eq!(
        Call::post(GENERATE, generate_body())
            .bearer(&user.jwt)
            .status(&app)
            .await?,
        StatusCode::OK
    );
    Ok(())
}

#[tokio::test]
async fn the_redirect_records_a_click_for_a_visitor_and_none_for_a_bot() -> Result<()> {
    let (pool, app) = node(NodeRole::All, &[]).await?;
    let (link, code) = seed_link(&pool).await?;
    let uri = format!("/r/{code}");

    assert_eq!(
        Call::get(&uri).user_agent(CRAWLER).status(&app).await?,
        StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(
        clicks(&pool, &link).await?,
        0,
        "a bot visit records no click"
    );

    assert_eq!(
        Call::get(&uri).status(&app).await?,
        StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(
        clicks(&pool, &link).await?,
        1,
        "a visitor records one click"
    );
    Ok(())
}

#[tokio::test]
async fn disabling_content_http_unmounts_every_group() -> Result<()> {
    let (pool, app) = node(NodeRole::All, &[ExtensionId::new(CONTENT_HTTP_EXTENSION)]).await?;
    let user = seed_bridge_credential(&pool, "content-off").await?;
    let (_link, code) = seed_link(&pool).await?;
    let redirect = format!("/r/{code}");

    assert_eq!(
        Call::post(QUERY, query_body()).status(&app).await?,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        Call::post(GENERATE, generate_body())
            .bearer(&user.jwt)
            .status(&app)
            .await?,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        Call::get(&redirect).status(&app).await?,
        StatusCode::NOT_FOUND
    );
    Ok(())
}

#[tokio::test]
async fn disabling_content_while_its_routes_stay_enabled_is_refused_at_boot() -> Result<()> {
    let refused = node(NodeRole::All, &[ExtensionId::new("content")]).await;
    let message = match refused {
        Ok(_) => anyhow::bail!("boot accepted content-http without the content extension"),
        Err(error) => format!("{error:#}"),
    };
    assert!(message.contains("'content' is disabled"), "{message}");
    Ok(())
}

#[tokio::test]
async fn only_the_all_and_api_roles_serve_the_content_groups() -> Result<()> {
    let (_pool, api) = node(NodeRole::Api, &[]).await?;
    assert_eq!(
        Call::post(QUERY, query_body()).status(&api).await?,
        StatusCode::OK
    );
    for role in [NodeRole::Worker, NodeRole::Gateway, NodeRole::Runner] {
        let (_pool, app) = node(role, &[]).await?;
        assert_eq!(
            Call::post(QUERY, query_body()).status(&app).await?,
            StatusCode::NOT_FOUND,
            "{role:?}"
        );
    }
    Ok(())
}
