//! The brain route groups on the real router, mounted from the `brain-http`
//! extension's declarations:
//!
//! | group | anonymous | inactive account | signed-in user | admin | `brain-http` disabled |
//! |---|---|---|---|---|---|
//! | `/api/v1/brain` | 401 | 401 | served (404 `brain_disabled` while the profile disables the brain) | served | 404 |
//! | `/api/v1/admin/brain` | 401 | 401 | 403 | served | 404 |
//!
//! A profile that disables `brain` while `brain-http` stays enabled is
//! refused at boot.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_brain_entry::extension::BRAIN_HTTP_EXTENSION;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ExtensionId, JwtToken};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_admin_credential, seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

const FACTS: &str = "/api/v1/brain/facts";
const SIMULATE: &str = "/api/v1/admin/brain/simulate";

async fn node(disabled: &[ExtensionId]) -> Result<(DbPool, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = NodeRole::All;
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx =
        fixture_app_context_with_extensions(config, super::common::reference_admitted(disabled)?)
            .await?;
    let router = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((pool, router))
}

async fn call(
    app: &Router,
    method: Method,
    uri: &str,
    bearer: Option<&JwtToken>,
) -> Result<(StatusCode, serde_json::Value)> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
    }
    let mut request = builder.body(Body::from(r#"{"user_id":"u","prompt":"p"}"#))?;
    request
        .extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024).await?;
    Ok((
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    ))
}

#[tokio::test]
async fn the_brain_groups_refuse_an_anonymous_caller_at_the_user_gate() -> Result<()> {
    let (_pool, app) = node(&[]).await?;
    let (status, _) = call(&app, Method::GET, FACTS, None).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(&app, Method::POST, SIMULATE, None).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn a_signed_in_user_reaches_the_brain_handler_and_is_refused_the_simulator() -> Result<()> {
    let (pool, app) = node(&[]).await?;
    let user = seed_bridge_credential(&pool, "brain-mounted").await?;
    let (status, body) = call(&app, Method::GET, FACTS, Some(&user.jwt)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error_key"], serde_json::json!("brain_disabled"), "{body}");
    let (status, _) = call(&app, Method::POST, SIMULATE, Some(&user.jwt)).await?;
    assert_eq!(status, StatusCode::FORBIDDEN);
    Ok(())
}

#[tokio::test]
async fn an_admin_reaches_the_simulator_handler() -> Result<()> {
    let (pool, app) = node(&[]).await?;
    let admin = seed_admin_credential(&pool, "brain-mounted-admin").await?;
    let (status, body) = call(&app, Method::POST, SIMULATE, Some(&admin.jwt)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error_key"], serde_json::json!("brain_disabled"), "{body}");
    Ok(())
}

#[tokio::test]
async fn an_inactive_account_is_refused_at_the_gate_on_both_groups() -> Result<()> {
    let (pool, app) = node(&[]).await?;
    let user = seed_admin_credential(&pool, "brain-suspended").await?;
    sqlx::query("UPDATE users SET status = 'suspended' WHERE id = $1")
        .bind(user.user_id.as_str())
        .execute(pool.write_pool().as_ref())
        .await?;
    for (method, uri) in [(Method::GET, FACTS), (Method::POST, SIMULATE)] {
        let (status, body) = call(&app, method, uri, Some(&user.jwt)).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}: {body}");
    }
    Ok(())
}

#[tokio::test]
async fn disabling_brain_http_unmounts_both_groups() -> Result<()> {
    let (pool, app) = node(&[ExtensionId::new(BRAIN_HTTP_EXTENSION)]).await?;
    let user = seed_bridge_credential(&pool, "brain-off").await?;
    let (status, _) = call(&app, Method::GET, FACTS, None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = call(&app, Method::GET, FACTS, Some(&user.jwt)).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(body["error_key"], serde_json::json!("brain_disabled"), "{body}");
    let (status, _) = call(&app, Method::POST, SIMULATE, None).await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn disabling_brain_while_its_routes_stay_enabled_is_refused_at_boot() -> Result<()> {
    let refused = node(&[ExtensionId::new("brain")]).await;
    let message = match refused {
        Ok(_) => anyhow::bail!("boot accepted brain-http without the brain extension"),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        message.contains("its core provider brain is disabled"),
        "{message}"
    );
    Ok(())
}
