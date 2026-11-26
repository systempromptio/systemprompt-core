//! The content link analytics reads are admin-only as the API mounts them.
//!
//! Each read is driven through the full router `setup_api_server` builds: a
//! caller with no credential and a signed-in non-admin are refused, and an
//! admin reaches the handler.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, header};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_admin_credential, seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

const ANALYTICS_READS: [&str; 5] = [
    "/api/v1/content/links",
    "/api/v1/content/links/journey",
    "/api/v1/content/links/{id}/clicks",
    "/api/v1/content/links/{id}/performance",
    "/api/v1/content/links/campaigns/{id}/performance",
];

async fn boot() -> anyhow::Result<(axum::Router, DbPool)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];
    let ctx = fixture_app_context_with_extensions(config, super::common::reference_admitted(&[])?)
        .await?;
    let router = setup_api_server(
        &ctx,
        Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &std::sync::Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((router, pool))
}

fn read(template: &str, bearer: Option<&str>) -> anyhow::Result<Request<Body>> {
    let mut builder = Request::builder()
        .uri(template.replace("{id}", &Uuid::new_v4().to_string()))
        .header(header::HOST, "127.0.0.1");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let mut req = builder.body(Body::empty())?;
    req.extensions_mut().insert(ConnectInfo(
        "203.0.113.9:41000"
            .parse::<SocketAddr>()
            .expect("peer address must parse"),
    ));
    Ok(req)
}

#[tokio::test]
async fn an_anonymous_caller_is_refused_every_link_analytics_read() -> anyhow::Result<()> {
    let (app, _pool) = boot().await?;
    for template in ANALYTICS_READS {
        let status = app.clone().oneshot(read(template, None)?).await?.status();
        assert!(
            status.as_u16() == 401 || status.as_u16() == 403,
            "{template} admits an anonymous caller: {status}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn a_signed_in_user_is_refused_every_link_analytics_read() -> anyhow::Result<()> {
    let (app, pool) = boot().await?;
    let user = seed_bridge_credential(&pool, "link-analytics-user").await?;
    for template in ANALYTICS_READS {
        let status = app
            .clone()
            .oneshot(read(template, Some(user.jwt.as_str()))?)
            .await?
            .status();
        assert_eq!(status.as_u16(), 403, "{template} admits a non-admin");
    }
    Ok(())
}

#[tokio::test]
async fn an_admin_reaches_the_link_analytics_handlers() -> anyhow::Result<()> {
    let (app, pool) = boot().await?;
    let admin = seed_admin_credential(&pool, "link-analytics-admin").await?;
    sqlx::query("UPDATE users SET roles = ARRAY['admin', 'user'] WHERE id = $1")
        .bind(admin.user_id.as_str())
        .execute(pool.pool().as_ref())
        .await?;
    for template in ANALYTICS_READS {
        let status = app
            .clone()
            .oneshot(read(template, Some(admin.jwt.as_str()))?)
            .await?
            .status();
        assert!(
            status.as_u16() != 401 && status.as_u16() != 403,
            "{template} refuses an admin: {status}"
        );
    }
    Ok(())
}
