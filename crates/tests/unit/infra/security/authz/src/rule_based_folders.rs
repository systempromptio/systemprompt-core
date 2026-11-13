use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_governance::authz::types::{Access, EntityKind, RuleType};
use systemprompt_governance::authz::{
    AccessControlRepository, AuthzContext, AuthzDecision, AuthzDecisionHook, AuthzRequest,
    AuthzWiring, ChainSources, DenyReason, EntityRef, NullAuditSink, RuleBasedHook,
    UpsertRuleParams,
};
use systemprompt_identifiers::{DocumentFolderId, TraceId, UserId};
use systemprompt_models::documents::DocumentAction;
use systemprompt_test_fixtures::{closed_db_pool, test_db_pool, unique_user_id};

fn hook(pool: Arc<PgPool>) -> RuleBasedHook {
    RuleBasedHook::new(
        pool,
        Arc::new(NullAuditSink),
        AuthzWiring {
            sources: ChainSources::default(),
            epochs: systemprompt_traits::CacheEpochs::new(),
            services: systemprompt_test_fixtures::fixture_services_view(),
            subjects: Default::default(),
            providers: Default::default(),
            secrets: systemprompt_test_fixtures::fixture_secret_store(),
            state: Default::default(),
        },
    )
}

fn folder_request(folder: &DocumentFolderId, user: &UserId, ctx: AuthzContext) -> AuthzRequest {
    AuthzRequest {
        entity: EntityRef::Folder(folder.clone()),
        user_id: user.clone(),
        actor: None,
        client_id: None,
        access_scope: None,
        roles: Vec::new(),
        attributes: std::collections::BTreeMap::new(),
        trace_id: TraceId::new("trace-rb-folder"),
        session_id: None,
        context: ctx,
        context_id: None,
        task_id: None,
        act_chain: Vec::new(),
    }
}

fn unique_folder() -> DocumentFolderId {
    DocumentFolderId::new(uuid::Uuid::new_v4().to_string())
}

async fn rule(
    repo: &AccessControlRepository,
    folder: &DocumentFolderId,
    user: &UserId,
    access: Access,
) {
    repo.upsert_entity(EntityKind::Folder, folder.as_str(), false, "code:test")
        .await
        .expect("entity");
    repo.upsert_rule(UpsertRuleParams {
        entity_type: EntityKind::Folder,
        entity_id: folder.as_str(),
        rule_type: RuleType::USER,
        rule_value: user.as_str(),
        access,
        justification: None,
        source: "code:test",
    })
    .await
    .expect("rule");
}

fn view_ctx(ancestors: &[DocumentFolderId]) -> AuthzContext {
    AuthzContext::document_access(DocumentAction::View, None, None).with_folder_ancestors(ancestors)
}

#[tokio::test]
async fn a_grant_on_an_ancestor_admits_a_ruleless_folder() {
    let db = test_db_pool().await;
    let pool = db.write_pool();
    let repo = AccessControlRepository::from_pool(Arc::clone(&pool));
    let user = unique_user_id("rb-folder-allow");
    let parent = unique_folder();
    let child = unique_folder();
    rule(&repo, &parent, &user, Access::Allow).await;

    let hook = hook(pool);
    let without = hook
        .evaluate(folder_request(&child, &user, view_ctx(&[])))
        .await;
    assert!(matches!(without, AuthzDecision::Deny { .. }), "{without:?}");

    let with = hook
        .evaluate(folder_request(&child, &user, view_ctx(&[parent.clone()])))
        .await;
    assert!(matches!(with, AuthzDecision::Allow), "{with:?}");
    let probed = hook
        .probe(folder_request(&child, &user, view_ctx(&[parent])))
        .await;
    assert!(matches!(probed, AuthzDecision::Allow), "{probed:?}");
}

#[tokio::test]
async fn an_ancestor_deny_overrides_the_folders_own_allow() {
    let db = test_db_pool().await;
    let pool = db.write_pool();
    let repo = AccessControlRepository::from_pool(Arc::clone(&pool));
    let user = unique_user_id("rb-folder-deny");
    let root = unique_folder();
    let child = unique_folder();
    rule(&repo, &child, &user, Access::Allow).await;
    rule(&repo, &root, &user, Access::Deny).await;

    let decision = hook(pool)
        .evaluate(folder_request(&child, &user, view_ctx(&[root])))
        .await;
    match decision {
        AuthzDecision::Deny { policy, .. } => assert_eq!(policy, "authz_rule_based"),
        AuthzDecision::Allow => panic!("ancestor deny must win"),
    }
}

#[tokio::test]
async fn malformed_ancestors_fail_closed_as_a_hook_fault() {
    let db = test_db_pool().await;
    let user = unique_user_id("rb-folder-bad");
    let child = unique_folder();
    let ctx = AuthzContext::extension(
        AuthzContext::DOCUMENT_ACCESS_KIND,
        serde_json::json!({ "action": "view", "folder_ancestors": [42] }),
    );
    let decision = hook(db.write_pool())
        .evaluate(folder_request(&child, &user, ctx))
        .await;
    assert!(
        matches!(
            decision,
            AuthzDecision::Deny {
                reason: DenyReason::HookUnavailable { .. },
                ..
            }
        ),
        "{decision:?}"
    );
}

#[tokio::test]
async fn probe_fails_closed_on_a_dead_pool() {
    let db = closed_db_pool().await;
    let user = unique_user_id("rb-probe-dead");
    let decision = hook(db.write_pool())
        .probe(folder_request(&unique_folder(), &user, view_ctx(&[])))
        .await;
    match decision {
        AuthzDecision::Deny { policy, reason } => {
            assert_eq!(policy, "authz_rule_based");
            assert!(matches!(reason, DenyReason::HookUnavailable { .. }));
        },
        AuthzDecision::Allow => panic!("dead pool must not allow"),
    }
}
