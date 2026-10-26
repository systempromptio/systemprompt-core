//! Principal parity (credential.md §9.6): the same user reaches the gateway's
//! pre-dispatch authz with the same decision whether they present a bridge
//! JWT or a personal access token, on role-gated routes and on the
//! decision-time `model/<glob>` entitlements alike. Before 0.73 an API key
//! carried no roles, so a role-gated route allowed the JWT and refused the PAT.

use std::sync::Arc;

use anyhow::Result;
use systemprompt_api::routes::gateway::messages::auth::AuthedPrincipal;
use systemprompt_api::routes::gateway::messages::extract::authz::{
    DispatchTarget, enforce_authz_pre_dispatch,
};
use systemprompt_governance::authz::types::{Access, EntityKind, RuleType};
use systemprompt_governance::authz::{
    AccessControlRepository, AuthzError, ChainSources, NullAuditSink, RuleBasedHook,
    SharedAuthzHook, SubjectAttributeProvider, SubjectDimension, SubjectProviderSet,
    UpsertRuleParams,
};
use systemprompt_governance::policy::types::AccessScope;
use systemprompt_identifiers::{ContextId, ProviderId, RouteId, UserId};
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_users::{ApiKeyLimits, ApiKeyService, IssueApiKeyParams};

use super::common::setup_ctx;
use super::gateway_api_key_session_binding::sign_in;

const MODEL: &str = "claude-parity";
const GROUP: &str = "parity_group";

#[derive(Debug)]
struct GroupProvider;

#[async_trait::async_trait]
impl SubjectAttributeProvider for GroupProvider {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::extension_static(GROUP),
            label: "Parity group",
            precedence: 320,
        }
    }

    async fn values_for(&self, _user_id: &UserId) -> Result<Vec<String>, AuthzError> {
        Ok(vec!["platform".to_owned(), "infra".to_owned()])
    }
}

async fn gate_on_role(repo: &AccessControlRepository, route: &str, role: &str) -> Result<()> {
    repo.upsert_entity(EntityKind::GatewayRoute, route, false, "code:test")
        .await?;
    repo.upsert_rule(UpsertRuleParams {
        entity_type: EntityKind::GatewayRoute,
        entity_id: route,
        rule_type: RuleType::ROLE,
        rule_value: role,
        access: Access::Allow,
        justification: None,
        source: "code:test",
    })
    .await?;
    Ok(())
}

async fn entitle_model(
    repo: &AccessControlRepository,
    glob: &str,
    rule: (&str, &str),
) -> Result<()> {
    repo.upsert_entity(EntityKind::Model, glob, false, "code:test")
        .await?;
    let (rule_type, value) = rule;
    repo.upsert_rule(UpsertRuleParams {
        entity_type: EntityKind::Model,
        entity_id: glob,
        rule_type: RuleType::from(rule_type),
        rule_value: value,
        access: Access::Allow,
        justification: None,
        source: "code:test",
    })
    .await?;
    Ok(())
}

async fn allowed(principal: &AuthedPrincipal, route_id: &str, hook: &SharedAuthzHook) -> bool {
    allowed_for(principal, route_id, MODEL, hook).await
}

async fn allowed_for(
    principal: &AuthedPrincipal,
    route_id: &str,
    model: &str,
    hook: &SharedAuthzHook,
) -> bool {
    let route = RouteId::new(route_id);
    let provider = ProviderId::new("parity-provider");
    enforce_authz_pre_dispatch(
        principal,
        DispatchTarget::new(Some(&route), model, &provider),
        &ContextId::generate(),
        hook,
    )
    .await
    .is_ok()
}

#[tokio::test]
async fn jwt_and_pat_for_one_user_get_the_same_decision_on_every_role_gated_route() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "parity@example.invalid").await?;
    sqlx::query("UPDATE users SET roles = ARRAY['user', 'eng'] WHERE id = $1")
        .bind(cred.user_id.as_str())
        .execute(pool.pool().as_ref())
        .await?;
    let issued = ApiKeyService::new(
        Arc::clone(ctx.user_repository()),
        systemprompt_users::ApiKeyLifetime::unbounded(),
    )
    .issue(IssueApiKeyParams {
        user_id: &cred.user_id,
        name: "parity-key",
        expires_at: None,
        limits: &ApiKeyLimits::default(),
        scopes: &[],
        purpose: None,
    })
    .await?;

    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let eng_route = format!("parity-eng-{suffix}");
    let finance_route = format!("parity-finance-{suffix}");
    let pool_arc = pool.write_pool();
    let repo = AccessControlRepository::from_pool(Arc::clone(&pool_arc));
    gate_on_role(&repo, &eng_route, "eng").await?;
    gate_on_role(&repo, &finance_route, "finance").await?;
    let open_route = format!("parity-open-{suffix}");
    repo.upsert_entity(EntityKind::GatewayRoute, &open_route, false, "code:test")
        .await?;
    entitle_model(&repo, &format!("parity-group-{suffix}-*"), ("role", "eng")).await?;
    entitle_model(
        &repo,
        &format!("parity-finance-{suffix}-*"),
        ("role", "finance"),
    )
    .await?;
    let hook: SharedAuthzHook = Arc::new(RuleBasedHook::new(
        pool_arc,
        Arc::new(NullAuditSink),
        ChainSources::default(),
        systemprompt_traits::CacheEpochs::new(),
        systemprompt_test_fixtures::fixture_services_view(),
    ));

    let extractor = super::gateway_api_key_session_binding::jwt_extractor(&ctx);
    let providers = SubjectProviderSet::from_providers(vec![Arc::new(GroupProvider)]);
    let jwt = sign_in(&ctx, &extractor, cred.jwt.as_str(), None, &providers)
        .await
        .expect("jwt authenticates");
    let pat = sign_in(&ctx, &extractor, &issued.secret, None, &providers)
        .await
        .expect("pat authenticates");

    assert_eq!(jwt.authz_attributes().0, pat.authz_attributes().0);
    assert_eq!(jwt.principal.subject, pat.principal.subject);
    assert_eq!(
        pat.principal
            .subject
            .values(&RuleType::extension_static(GROUP)),
        ["platform".to_owned(), "infra".to_owned()]
    );
    assert_eq!(jwt.access_scope(), AccessScope::User);
    assert_eq!(pat.access_scope(), AccessScope::User);

    for (route_id, expected) in [(&eng_route, true), (&finance_route, false)] {
        let via_jwt = allowed(&jwt, route_id, &hook).await;
        let via_pat = allowed(&pat, route_id, &hook).await;
        assert_eq!(via_jwt, via_pat, "JWT and PAT disagree on {route_id}");
        assert_eq!(via_jwt, expected, "unexpected decision on {route_id}");
    }
    for (model, expected) in [
        (format!("parity-group-{suffix}-opus"), true),
        (format!("parity-finance-{suffix}-opus"), false),
    ] {
        let via_jwt = allowed_for(&jwt, &open_route, &model, &hook).await;
        let via_pat = allowed_for(&pat, &open_route, &model, &hook).await;
        assert_eq!(via_jwt, via_pat, "JWT and PAT disagree on model {model}");
        assert_eq!(
            via_jwt, expected,
            "unexpected entitlement for model {model}"
        );
    }
    Ok(())
}
