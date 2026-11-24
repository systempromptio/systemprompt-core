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
use systemprompt_extension::prelude::*;
use systemprompt_governance::authz::types::RuleType;
use systemprompt_governance::authz::{
    SubjectAttributeError, SubjectAttributeProvider, SubjectDimension,
};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_admitted_of, fixture_app_context_with_extensions,
    fixture_config, fixture_core_members, mint_bridge_jwt, seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

use super::common::setup_ctx;

const GROUP: &str = "group";
const PRACTICE: &str = "practice";

/// One subject dimension as an instance's extension declares it: the user is
/// in two groups and in no practice. `fails` makes every read fail, as an
/// unreachable membership store does.
#[derive(Debug)]
struct Membership {
    rule_type: &'static str,
    precedence: u16,
    values: &'static [&'static str],
    fails: bool,
}

#[async_trait::async_trait]
impl SubjectAttributeProvider for Membership {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::extension_static(self.rule_type),
            label: self.rule_type,
            precedence: self.precedence,
        }
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, SubjectAttributeError> {
        if self.fails {
            return Err(SubjectAttributeError::Lookup(
                "the membership store is unreachable".into(),
            ));
        }
        Ok(self.values.iter().map(|v| (*v).to_owned()).collect())
    }
}

struct MembershipMember {
    fails: bool,
}

impl Extension for MembershipMember {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "fixture-memberships",
            name: "Fixture memberships",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        let fails = self.fails;
        c.subject_attributes(move |_: &Resolved<'_>| {
            let provider: Arc<dyn SubjectAttributeProvider> = Arc::new(Membership {
                rule_type: GROUP,
                precedence: 300,
                values: &["delivery", "leads"],
                fails,
            });
            Ok(provider)
        })
        .subject_attributes(move |_: &Resolved<'_>| {
            let provider: Arc<dyn SubjectAttributeProvider> = Arc::new(Membership {
                rule_type: PRACTICE,
                precedence: 310,
                values: &[],
                fails,
            });
            Ok(provider)
        });
        Ok(())
    }
}

async fn whoami_with_members(fails: bool, email: &str) -> anyhow::Result<serde_json::Value> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut members = fixture_core_members();
    members.push(Box::new(MembershipMember { fails }));
    let ctx = fixture_app_context_with_extensions(
        fixture_config(&bootstrap.database_url),
        fixture_admitted_of(members),
    )
    .await?;
    let app = gateway_router(&ctx, ctx.admitted(), ctx.kernel()).expect("gateway router builds");
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
    Ok((
        gateway_router(&ctx, ctx.admitted(), ctx.kernel()).expect("gateway router builds"),
        pool,
    ))
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
async fn whoami_carries_the_core_subject_dimensions() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "bridge-core-dims@example.invalid").await?;
    let resp = app
        .oneshot(authed_get("/bridge/whoami", cred.jwt.as_str()))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let body: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        body["attributes"]["role"],
        serde_json::json!(["user"]),
        "{body}"
    );
    assert!(body["attributes"].get(GROUP).is_none(), "{body}");
    Ok(())
}

#[tokio::test]
async fn whoami_carries_each_declared_dimension_with_the_users_values() -> anyhow::Result<()> {
    let body = whoami_with_members(false, "bridge-groups@example.invalid").await?;
    assert_eq!(
        body["attributes"][GROUP],
        serde_json::json!(["delivery", "leads"]),
        "{body}"
    );
    assert_eq!(
        body["attributes"][PRACTICE],
        serde_json::json!([]),
        "{body}"
    );
    Ok(())
}

#[tokio::test]
async fn whoami_still_answers_when_the_subject_source_fails() -> anyhow::Result<()> {
    let body = whoami_with_members(true, "bridge-nosource@example.invalid").await?;
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
