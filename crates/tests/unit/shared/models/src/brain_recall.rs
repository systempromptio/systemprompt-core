use std::collections::BTreeMap;

use chrono::Utc;
use systemprompt_identifiers::{AgentName, BrainFactId, BrainRecallId, UserId, WorkflowId};
use systemprompt_models::auth::RequestActor;
use systemprompt_models::brain::{
    FactRef, RecallAudit, RecallCtx, RetrievalTier, ScopeLevel, WithheldReason,
};
use systemprompt_models::origin::ClientKind;

#[test]
fn withheld_reasons_serialize_as_their_wire_names() {
    let all = [
        (WithheldReason::Scope, "scope"),
        (WithheldReason::Source, "source"),
        (WithheldReason::AgentCapped, "agent_capped"),
        (WithheldReason::Stale, "stale"),
        (WithheldReason::ReviewDue, "review_due"),
        (WithheldReason::Quota, "quota"),
        (WithheldReason::Budget, "budget"),
        (WithheldReason::UnknownName, "unknown_name"),
        (WithheldReason::NoSignal, "no_signal"),
        (WithheldReason::Unavailable, "unavailable"),
    ];
    for (reason, name) in all {
        assert_eq!(reason.as_str(), name);
        assert_eq!(serde_json::to_value(reason).unwrap(), name);
    }
    assert_eq!(
        serde_json::from_str::<RetrievalTier>("\"full_text\"").unwrap(),
        RetrievalTier::FullText
    );
}

#[test]
fn a_new_recall_context_spans_every_scope_until_narrowed() {
    let now = Utc::now();
    let ctx = RecallCtx::new(UserId::new("u-recall"), RequestActor::Human, now);
    assert_eq!(ctx.scopes, ScopeLevel::ALL.to_vec());
    assert_eq!(ctx.client, ClientKind::Other);
    assert!(ctx.project.is_none() && ctx.workflow.is_none());

    let workflow = WorkflowId::new("wf-recall");
    let narrowed = ctx
        .with_client(ClientKind::ClaudeCode)
        .with_project("core")
        .with_workflow(workflow.clone())
        .with_scopes(vec![ScopeLevel::User]);
    assert_eq!(narrowed.client, ClientKind::ClaudeCode);
    assert_eq!(narrowed.project.as_deref(), Some("core"));
    assert_eq!(narrowed.workflow, Some(workflow));
    assert_eq!(narrowed.scopes, vec![ScopeLevel::User]);
    assert_eq!(narrowed.now, now);
}

#[test]
fn an_agent_recall_context_round_trips_through_json() {
    let ctx = RecallCtx::new(
        UserId::new("u-agent"),
        RequestActor::Agent {
            name: AgentName::new("helper"),
            attended: false,
        },
        Utc::now(),
    );
    let back: RecallCtx = serde_json::from_value(serde_json::to_value(&ctx).unwrap()).unwrap();
    assert_eq!(back, ctx);
}

#[test]
fn the_audit_totals_every_withheld_reason() {
    let audit = RecallAudit {
        recall_id: BrainRecallId::generate(),
        injected: vec![FactRef {
            id: BrainFactId::generate(),
            revision: 2,
        }],
        withheld: BTreeMap::from([(WithheldReason::Scope, 3), (WithheldReason::Budget, 4)]),
        tokens: 120,
        as_of: Utc::now(),
    };
    assert_eq!(audit.withheld_total(), 7);
    let empty = RecallAudit {
        withheld: BTreeMap::new(),
        ..audit
    };
    assert_eq!(empty.withheld_total(), 0);
}
