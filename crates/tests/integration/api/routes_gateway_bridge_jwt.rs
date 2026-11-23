//! User-tier (non-admin) bridge JWT path. Asserts the success branch in
//! `decode_for_gateway` for the User permission set + reconciliation against
//! a non-admin user row.

use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, header};
use http::StatusCode;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::authz::{
    RuleType, SubjectAttributeSource, SubjectAttributes, SubjectDimension, SubjectSourceError,
};
use systemprompt_test_fixtures::{mint_bridge_jwt, seed_bridge_credential};
use tower::ServiceExt;

use super::common::{setup_ctx, setup_ctx_customised};

const GROUP: RuleType = RuleType::extension_static("group");
const PRACTICE: RuleType = RuleType::extension_static("practice");

/// A subject source as an instance binds one: the user is in two groups and
/// in no practice. `fails` makes every read fail, as an unreachable provider
/// does.
struct Memberships {
    dimensions: Vec<SubjectDimension>,
    fails: bool,
}

impl Memberships {
    fn new(fails: bool) -> Self {
        Self {
            dimensions: vec![
                SubjectDimension {
                    rule_type: GROUP,
                    label: "Group",
                    precedence: 100,
                },
                SubjectDimension {
                    rule_type: PRACTICE,
                    label: "Practice",
                    precedence: 110,
                },
            ],
            fails,
        }
    }

    fn unreadable(&self) -> SubjectSourceError {
        SubjectSourceError::Provider {
            dimension: GROUP.as_str().to_owned(),
            source: "the membership store is unreachable".into(),
        }
    }
}

#[async_trait::async_trait]
impl SubjectAttributeSource for Memberships {
    fn dimensions(&self) -> &[SubjectDimension] {
        &self.dimensions
    }

    async fn values_for(
        &self,
        _user: &UserId,
        dimension: &SubjectDimension,
    ) -> Result<Vec<String>, SubjectSourceError> {
        if self.fails {
            return Err(self.unreadable());
        }
        if dimension.rule_type == GROUP {
            return Ok(vec!["delivery".to_owned(), "leads".to_owned()]);
        }
        Ok(Vec::new())
    }

    async fn resolve_all(&self, _user: &UserId) -> Result<SubjectAttributes, SubjectSourceError> {
        if self.fails {
            return Err(self.unreadable());
        }
        Ok([
            (GROUP, vec!["delivery".to_owned(), "leads".to_owned()]),
            (PRACTICE, Vec::new()),
        ]
        .into_iter()
        .collect())
    }
}

async fn whoami_with_source(source: Memberships, email: &str) -> anyhow::Result<serde_json::Value> {
    let (pool, ctx) = setup_ctx_customised(|_config| {}).await?;
    ctx.subject_handle()
        .bind(Arc::new(source))
        .expect("a fresh context has no source bound");
    let app = gateway_router(&ctx).expect("gateway router builds");
    let cred = seed_bridge_credential(&pool, email).await?;
    let resp = app
        .oneshot(authed_get("/bridge/whoami", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(body["user_id"], cred.user_id.as_str());
    Ok(body)
}

async fn router_and_pool() -> anyhow::Result<(Router, DbPool)> {
    let (pool, ctx) = setup_ctx().await?;
    Ok((gateway_router(&ctx).expect("gateway router builds"), pool))
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

#[tokio::test]
async fn whoami_for_unseeded_bridge_user_is_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let user = UserId::generate();
    let jwt = mint_bridge_jwt(
        &user,
        "missing@example.invalid",
        systemprompt_test_fixtures::FIXTURE_JWT_ISSUER,
    );
    let resp = app
        .oneshot(authed_get("/bridge/whoami", jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn whoami_for_seeded_bridge_user_returns_envelope() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "bridge-ok@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/whoami", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(body["user_id"], cred.user_id.as_str());
    assert_eq!(body["email"].as_str(), Some(cred.email.as_str()));
    Ok(())
}

#[tokio::test]
async fn whoami_without_a_bound_subject_source_carries_no_attributes() -> anyhow::Result<()> {
    let (pool, ctx) = setup_ctx_customised(|_config| {}).await?;
    let app = gateway_router(&ctx).expect("gateway router builds");
    let cred = seed_bridge_credential(&pool, "bridge-unbound@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/whoami", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(body["user_id"], cred.user_id.as_str());
    assert!(body.get("attributes").is_none(), "{body}");
    Ok(())
}

#[tokio::test]
async fn whoami_carries_the_bound_sources_attributes_per_dimension() -> anyhow::Result<()> {
    let body = whoami_with_source(Memberships::new(false), "bridge-groups@example.invalid").await?;
    assert_eq!(
        body["attributes"],
        serde_json::json!({ "group": ["delivery", "leads"], "practice": [] })
    );
    Ok(())
}

#[tokio::test]
async fn whoami_still_answers_when_the_subject_source_fails() -> anyhow::Result<()> {
    let body =
        whoami_with_source(Memberships::new(true), "bridge-nosource@example.invalid").await?;
    assert!(body.get("attributes").is_none(), "{body}");
    assert!(body["email"].is_string());
    Ok(())
}

#[tokio::test]
async fn profile_usage_for_seeded_bridge_user_returns_ok() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "bridge-usage@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/profile/usage", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn pubkey_endpoint_is_public_no_auth_needed() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/pubkey")
                .body(Body::empty())
                .unwrap(),
        )
        .await?;
    let s = resp.status();
    assert!(s.is_success() || s.is_server_error(), "{s}");
    Ok(())
}
