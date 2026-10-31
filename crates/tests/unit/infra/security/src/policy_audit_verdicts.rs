use systemprompt_governance::authz::types::{
    Decision, DenyReason, EntityRef, MatchedBy, PendingReason,
};
use systemprompt_governance::policy::types::AccessScope;
use systemprompt_governance::policy::{
    AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult, DecisionAudit,
    PrincipalSnapshot, record_decision,
};
use systemprompt_identifiers::{CallId, McpToolName, PolicyId, SessionId, SkillId, UserId};
use systemprompt_test_fixtures::test_db_pool;

fn entry(policy: &str, result: ChainEntryResult) -> ChainEntryOutcome {
    ChainEntryOutcome {
        policy_id: PolicyId::new(policy),
        result,
        detail: String::new(),
        duration_ms: 0.1,
    }
}

fn audit(decision: Decision, chain: Vec<ChainEntryOutcome>) -> DecisionAudit {
    let tag = uuid::Uuid::new_v4().simple().to_string();
    DecisionAudit {
        id: format!("dec-verdict-{tag}"),
        call_id: CallId::new(format!("call-verdict-{tag}")),
        origin: AuditOrigin::Governed,
        decision,
        principal: PrincipalSnapshot {
            user_id: UserId::new("u-verdict"),
            session_id: SessionId::new("sess-verdict"),
            agent_session: None,
            agent_id: None,
            agent_scope: AccessScope::User,
            client_id: None,
        },
        target: AuditTarget {
            tool_name: McpToolName::new("write_file"),
            plugin_id: None,
            tool_use_id: None,
        },
        chain,
        approver: None,
        act_chain: Vec::new(),
        context_id: None,
        trace_id: None,
    }
}

fn refusal() -> DenyReason {
    DenyReason::NotAssigned {
        entity: EntityRef::Skill(SkillId::new("writer")),
        user_id: UserId::new("u-verdict"),
        roles: vec!["viewer".to_owned()],
    }
}

fn pending() -> Decision {
    Decision::Pending {
        reason: PendingReason::ApprovalRequired {
            tool: McpToolName::new("write_file"),
            rule: "write_*".to_owned(),
        },
    }
}

async fn recorded(audit: &DecisionAudit) -> (String, String, String) {
    let pool = test_db_pool().await;
    record_decision(&pool.pool(), audit)
        .await
        .expect("insert should succeed");
    sqlx::query_as::<_, (String, String, String)>(
        "SELECT decision, policy, reason FROM governance_decisions WHERE id = $1",
    )
    .bind(&audit.id)
    .fetch_one(pool.pool().as_ref())
    .await
    .expect("row written")
}

#[tokio::test]
async fn an_allow_with_every_policy_disabled_records_governance_disabled() {
    let audit = audit(
        Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded,
        },
        vec![
            entry("secret_scan", ChainEntryResult::Disabled),
            entry("scope_check", ChainEntryResult::Disabled),
        ],
    );
    let (decision, policy, reason) = recorded(&audit).await;
    assert_eq!(decision, "allow");
    assert_eq!(policy, "governance_disabled");
    assert_eq!(reason, "");
}

#[tokio::test]
async fn a_warn_names_the_warning_policy_and_keeps_the_reason() {
    let audit = audit(
        Decision::Warn { reason: refusal() },
        vec![
            entry("scope_check", ChainEntryResult::Pass),
            entry("secret_scan", ChainEntryResult::Warn),
        ],
    );
    let (decision, policy, reason) = recorded(&audit).await;
    assert_eq!(decision, "warn");
    assert_eq!(policy, "secret_scan");
    assert_eq!(reason, refusal().to_string());
}

#[tokio::test]
async fn a_warn_without_a_warning_entry_records_unknown() {
    let audit = audit(Decision::Warn { reason: refusal() }, vec![]);
    let (decision, policy, _) = recorded(&audit).await;
    assert_eq!(decision, "warn");
    assert_eq!(policy, "unknown");
}

#[tokio::test]
async fn a_pending_names_the_holding_policy() {
    let audit = audit(
        pending(),
        vec![
            entry("scope_check", ChainEntryResult::Pass),
            entry("require_approval", ChainEntryResult::Hold),
        ],
    );
    let (decision, policy, reason) = recorded(&audit).await;
    assert_eq!(decision, "pending");
    assert_eq!(policy, "require_approval");
    assert!(reason.contains("requires human approval"), "{reason}");
}

#[tokio::test]
async fn a_pending_without_a_hold_entry_records_unknown() {
    let (decision, policy, _) = recorded(&audit(pending(), vec![])).await;
    assert_eq!(decision, "pending");
    assert_eq!(policy, "unknown");
}
