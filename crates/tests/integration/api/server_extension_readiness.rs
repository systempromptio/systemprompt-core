//! Extension readiness checks merged into `/readyz`: a passing check is
//! listed, and a failing, erroring or overrunning check makes the replica
//! unready — the probe fails closed.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use axum::body::{Body, to_bytes};
use axum::http::Request;
use systemprompt_api::services::server::builder::discovery_router;
use systemprompt_api::services::server::readiness::ReadinessState;
use systemprompt_extension::{
    Extension, ExtensionContext, ExtensionMetadata, ExtensionRegistry, ReadinessCheck,
    ReadinessFailure,
};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config, test_db_pool,
};
use tower::ServiceExt;

#[derive(Clone, Copy)]
enum Outcome {
    Pass,
    Unready,
    Error,
    Hang,
}

struct Check(Outcome);

#[async_trait]
impl ReadinessCheck for Check {
    fn name(&self) -> &'static str {
        "upstream"
    }

    async fn check(&self) -> Result<(), ReadinessFailure> {
        match self.0 {
            Outcome::Pass => Ok(()),
            Outcome::Unready => Err(ReadinessFailure::Unready {
                reason: Cow::Borrowed("upstream queue is not connected"),
            }),
            Outcome::Error => Err(ReadinessFailure::Failed(Box::new(std::io::Error::other(
                "probe crashed",
            )))),
            Outcome::Hang => {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(())
            },
        }
    }
}

struct ReadinessExt(Outcome);

impl Extension for ReadinessExt {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "cov-readiness",
            name: "Readiness fixture",
            version: "0.0.0",
        }
    }

    fn readiness(&self, _ctx: &dyn ExtensionContext) -> Vec<Arc<dyn ReadinessCheck>> {
        vec![Arc::new(Check(self.0))]
    }
}

async fn readyz(outcome: Outcome) -> Result<(u16, serde_json::Value)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut registry = ExtensionRegistry::new();
    registry.register(Arc::new(ReadinessExt(outcome)))?;
    let ctx = fixture_app_context_with_extensions(
        &pool,
        fixture_config(&bootstrap.database_url),
        registry,
    )?;
    let readiness = std::sync::Arc::new(ReadinessState::new());
    readiness.signal_ready();
    let app = discovery_router(&ctx).layer(axum::Extension(std::sync::Arc::clone(&readiness)));
    let response = app
        .oneshot(Request::builder().uri("/readyz").body(Body::empty())?)
        .await?;
    let status = response.status().as_u16();
    let body = to_bytes(response.into_body(), 1 << 20).await?;
    readiness.signal_shutdown();
    Ok((status, serde_json::from_slice(&body)?))
}

#[tokio::test]
async fn a_passing_check_is_listed_on_a_ready_answer() -> Result<()> {
    let (status, body) = readyz(Outcome::Pass).await?;
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["status"], "ready");
    assert_eq!(body["extensions"][0]["extension"], "cov-readiness");
    assert_eq!(body["extensions"][0]["check"], "upstream");
    assert_eq!(body["extensions"][0]["status"], "ok");
    Ok(())
}

#[tokio::test]
async fn an_unready_check_withdraws_readiness_with_its_reason() -> Result<()> {
    let (status, body) = readyz(Outcome::Unready).await?;
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["status"], "unready");
    assert_eq!(body["extensions"][0]["status"], "unready");
    assert_eq!(
        body["extensions"][0]["reason"],
        "upstream queue is not connected"
    );
    Ok(())
}

#[tokio::test]
async fn an_erroring_check_fails_closed_without_leaking_the_cause() -> Result<()> {
    let (status, body) = readyz(Outcome::Error).await?;
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["extensions"][0]["status"], "error");
    assert!(!body.to_string().contains("probe crashed"));
    Ok(())
}

#[tokio::test]
async fn a_check_that_overruns_the_probe_timeout_fails_closed() -> Result<()> {
    let (status, body) = readyz(Outcome::Hang).await?;
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["extensions"][0]["status"], "timeout");
    Ok(())
}
