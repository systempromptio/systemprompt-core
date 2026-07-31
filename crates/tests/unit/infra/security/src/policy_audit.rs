use systemprompt_identifiers::{AgentId, PluginId, PolicyId, SessionId, SkillId, UserId};
use systemprompt_security::authz::types::{Decision, EntityRef, MatchedBy};
use systemprompt_security::policy::types::AccessScope;
use systemprompt_security::policy::{
    AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult, DecisionAudit,
    PrincipalSnapshot, record_decision,
};

fn sample_audit() -> DecisionAudit {
    DecisionAudit {
        id: "dec-1".to_owned(),
        call_id: "call-1".to_owned(),
        origin: AuditOrigin::Governed,
        decision: Decision::Allow {
            matched_by: MatchedBy::DefaultIncluded,
        },
        principal: PrincipalSnapshot {
            user_id: UserId::new("u1"),
            session_id: SessionId::new("sess-1"),
            agent_session: Some(SessionId::new("agent-sess-1")),
            agent_id: Some(AgentId::new("agent-1")),
            agent_scope: AccessScope::User,
        },
        target: AuditTarget {
            tool_name: "read_file".to_owned(),
            plugin_id: Some(PluginId::new("plug-1")),
        },
        chain: vec![ChainEntryOutcome {
            policy_id: PolicyId::new("secret_scan"),
            result: ChainEntryResult::Pass,
            detail: "clean".to_owned(),
            duration_ms: 0.42,
        }],
        approver: None,
        act_chain: Vec::new(),
        context_id: None,
    }
}

// Why: the serialized form lands in governance_decisions.evaluated_rules and
// is rendered by dashboards — this test pins the persisted contract.
#[test]
fn decision_audit_blob_shape_is_stable() {
    let blob = serde_json::to_value(sample_audit()).unwrap();

    let mut keys: Vec<&str> = blob
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "call_id",
            "chain",
            "decision",
            "id",
            "origin",
            "principal",
            "target"
        ]
    );

    assert_eq!(blob["origin"], "governed");
    let entry = &blob["chain"][0];
    assert_eq!(entry["result"], "pass");
    assert_eq!(entry["policy_id"], "secret_scan");
    assert_eq!(entry["detail"], "clean");
    assert!(entry["duration_ms"].is_number());
    assert_eq!(blob["principal"]["agent_scope"], "user");
    assert_eq!(blob["target"]["tool_name"], "read_file");
}

#[test]
fn chain_entry_result_serializes_as_a_flattened_tag() {
    for (result, tag) in [
        (ChainEntryResult::Pass, "pass"),
        (ChainEntryResult::Fail, "fail"),
        (ChainEntryResult::Skip, "skip"),
    ] {
        let entry = ChainEntryOutcome {
            policy_id: PolicyId::new("p"),
            result,
            detail: String::new(),
            duration_ms: 0.0,
        };
        let v = serde_json::to_value(&entry).unwrap();
        assert_eq!(v["result"], tag);
    }
}

#[test]
fn act_chain_and_approver_are_omitted_when_empty() {
    let blob = serde_json::to_value(sample_audit()).unwrap();
    let obj = blob.as_object().unwrap();
    assert!(!obj.contains_key("act_chain"));
    assert!(!obj.contains_key("approver"));
    assert!(!obj.contains_key("context_id"));
}

// record_decision derives the flat columns from the blob: `policy` is the first
// failing chain entry for a deny (and "default_allow" for an allow), so a
// mis-derivation would mislabel which policy actually refused a call.
async fn pool() -> Option<systemprompt_database::DbPool> {
    let url = systemprompt_test_fixtures::fixture_database_url().ok()?;
    systemprompt_test_fixtures::fixture_db_pool(&url).await.ok()
}

fn pg(pool: &systemprompt_database::DbPool) -> std::sync::Arc<sqlx::PgPool> {
    pool.pool_arc().expect("fixture pool is connected")
}

fn unique_audit() -> DecisionAudit {
    let mut audit = sample_audit();
    let tag = uuid::Uuid::new_v4().simple().to_string();
    audit.id = format!("dec-{tag}");
    audit.call_id = format!("call-{tag}");
    audit
}

#[derive(Debug)]
struct Row {
    decision: String,
    policy: String,
    reason: String,
    actor_kind: String,
    actor_id: String,
    tool_name: String,
    agent_id: Option<String>,
    plugin_id: Option<String>,
    evaluated_rules: serde_json::Value,
}

async fn fetch(pool: &systemprompt_database::DbPool, id: &str) -> Row {
    let r = sqlx::query_as::<_, (String, String, String, String, String, String, Option<String>, Option<String>, serde_json::Value)>(
        "SELECT decision, policy, reason, actor_kind, actor_id, tool_name, agent_id, plugin_id, evaluated_rules \
         FROM governance_decisions WHERE id = $1",
    )
    .bind(id)
    .fetch_one(pg(pool).as_ref())
    .await
    .expect("the decision row should have been written");
    Row {
        decision: r.0,
        policy: r.1,
        reason: r.2,
        actor_kind: r.3,
        actor_id: r.4,
        tool_name: r.5,
        agent_id: r.6,
        plugin_id: r.7,
        evaluated_rules: r.8,
    }
}

#[tokio::test]
async fn allow_decision_records_default_allow_policy_and_empty_reason() {
    let Some(pool) = pool().await else {
        return;
    };
    let audit = unique_audit();
    record_decision(&pg(&pool), &audit)
        .await
        .expect("insert should succeed");

    let row = fetch(&pool, &audit.id).await;
    assert_eq!(row.decision, "allow");
    assert_eq!(row.policy, "default_allow");
    assert_eq!(row.reason, "", "an allow carries no refusal reason");
    assert_eq!(row.tool_name, "read_file");
    assert_eq!(row.agent_id.as_deref(), Some("agent-1"));
    assert_eq!(row.plugin_id.as_deref(), Some("plug-1"));
    assert_eq!(row.actor_kind, "agent");
    assert!(!row.actor_id.is_empty());

    assert_eq!(
        row.evaluated_rules["call_id"], audit.call_id,
        "the whole audit blob is persisted, not just the flat columns"
    );
}

#[tokio::test]
async fn deny_decision_records_the_first_failing_policy() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut audit = unique_audit();
    audit.decision = Decision::Deny {
        reason: systemprompt_security::authz::types::DenyReason::NotAssigned {
            entity: EntityRef::Skill(SkillId::new("writer")),
            user_id: UserId::new("u1"),
            roles: vec!["viewer".to_owned()],
        },
    };
    audit.chain = vec![
        ChainEntryOutcome {
            policy_id: PolicyId::new("scope_check"),
            result: ChainEntryResult::Pass,
            detail: "in scope".to_owned(),
            duration_ms: 0.1,
        },
        ChainEntryOutcome {
            policy_id: PolicyId::new("tool_blocklist"),
            result: ChainEntryResult::Fail,
            detail: "blocked".to_owned(),
            duration_ms: 0.2,
        },
        ChainEntryOutcome {
            policy_id: PolicyId::new("secret_scan"),
            result: ChainEntryResult::Fail,
            detail: "also failed".to_owned(),
            duration_ms: 0.3,
        },
    ];

    record_decision(&pg(&pool), &audit)
        .await
        .expect("insert should succeed");

    let row = fetch(&pool, &audit.id).await;
    assert_eq!(row.decision, "deny");
    assert_eq!(
        row.policy, "tool_blocklist",
        "the FIRST failing entry names the policy, not the last"
    );
    assert!(
        !row.reason.is_empty(),
        "a deny must persist its reason: {row:?}",
    );
}

#[tokio::test]
async fn deny_without_a_failing_chain_entry_records_unknown() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut audit = unique_audit();
    audit.decision = Decision::Deny {
        reason: systemprompt_security::authz::types::DenyReason::NotAssigned {
            entity: EntityRef::Skill(SkillId::new("writer")),
            user_id: UserId::new("u1"),
            roles: vec!["viewer".to_owned()],
        },
    };
    audit.chain = vec![ChainEntryOutcome {
        policy_id: PolicyId::new("scope_check"),
        result: ChainEntryResult::Pass,
        detail: "passed".to_owned(),
        duration_ms: 0.1,
    }];

    record_decision(&pg(&pool), &audit)
        .await
        .expect("insert should succeed");

    let row = fetch(&pool, &audit.id).await;
    assert_eq!(row.decision, "deny");
    assert_eq!(
        row.policy, "unknown",
        "a deny with no failing entry cannot name a policy"
    );
}

#[tokio::test]
async fn agentless_audit_records_a_user_actor() {
    let Some(pool) = pool().await else {
        return;
    };
    let mut audit = unique_audit();
    audit.principal.agent_id = None;
    audit.target.plugin_id = None;

    record_decision(&pg(&pool), &audit)
        .await
        .expect("insert should succeed");

    let row = fetch(&pool, &audit.id).await;
    assert_eq!(row.agent_id, None);
    assert_eq!(row.plugin_id, None);
    assert_eq!(
        row.actor_kind, "user",
        "with no agent the actor is the user, derived from the tool name"
    );
}
