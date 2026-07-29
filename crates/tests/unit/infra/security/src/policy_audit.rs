use systemprompt_identifiers::{AgentId, PluginId, PolicyId, SessionId, UserId};
use systemprompt_security::authz::types::{Decision, MatchedBy};
use systemprompt_security::policy::types::AccessScope;
use systemprompt_security::policy::{
    AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult, DecisionAudit, PrincipalSnapshot,
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
}
