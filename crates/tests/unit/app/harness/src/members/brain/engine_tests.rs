//! The engine half offers the four agent tools only while the brain is
//! enabled, and each answers text the model reads.

use std::sync::Arc;
use systemprompt_identifiers::engine::IdClock;

use serde_json::json;
use systemprompt_harness::members::brain::BrainStatus;

use crate::members::brain::support::{
    FACT, FakeResponse, Gate, OUTBOX, Rig, fact_json, listing, tool_context,
};

const STEP_DIR: &str = ".";

#[tokio::test]
async fn the_tools_are_the_four_agent_actions_and_none_a_person_decides() {
    let rig = Rig::start(|request| listing(request).unwrap()).await;
    assert_eq!(rig.settled().await, BrainStatus::Enabled);
    let tools = rig.engine.tools();
    let names: Vec<&str> = tools.iter().map(|tool| tool.tool.name()).collect();
    assert_eq!(
        names,
        [
            "brain_lookup",
            "brain_why",
            "brain_propose",
            "brain_dispute"
        ]
    );
    assert!(
        tools
            .iter()
            .all(|tool| tool.permission == "brain" && tool.asks_itself)
    );
    for forbidden in ["confirm", "approve", "retract", "pin", "erase", "resolve"] {
        assert!(names.iter().all(|name| !name.contains(forbidden)));
    }
    let session = systemprompt_identifiers::engine::SessionId::mint(&IdClock::new());
    let model = systemprompt_identifiers::ModelId::new("model");
    let step = systemprompt_engine::run::extension::PromptContext {
        session_id: &session,
        agent: "build",
        model_id: &model,
        directory: std::path::Path::new(STEP_DIR),
        subagent: false,
    };
    assert_eq!(rig.engine.prompt_sections(&step).len(), 1);
    let enabled = rig.engine.generation();
    rig.applier.clear();
    assert_ne!(rig.engine.generation(), enabled);
    assert_eq!(rig.engine.tools().len(), 0);
    assert_eq!(rig.engine.prompt_sections(&step).len(), 0);
}

#[tokio::test]
async fn a_lookup_asks_the_gate_and_answers_one_line_per_fact() {
    let rig = Rig::start(|request| {
        if request.query() == "limit=1" {
            return listing(request).unwrap();
        }
        FakeResponse::json(
            200,
            &json!({ "facts": [fact_json(FACT, "confirmed")], "next": "c1" }),
        )
    })
    .await;
    assert_eq!(rig.settled().await, BrainStatus::Enabled);
    let lookup = rig.engine.tools().remove(0).tool;
    let gate = Arc::new(Gate::default());
    let output = lookup
        .run(
            json!({ "subject": "client:acme" }),
            &tool_context(Arc::clone(&gate)),
        )
        .await
        .unwrap();
    assert!(output.output.contains(FACT));
    assert!(output.output.contains("Acme's primary contact is Jane Doe"));
    assert!(output.output.contains("after=\"c1\""));
    assert_eq!(rig.fake.last().query(), "subject=client%3Aacme&limit=20");
    let asked = gate.asked.lock().unwrap();
    assert_eq!(asked[0].permission, "brain");
    assert_eq!(asked[0].patterns, ["brain_lookup client:acme"]);
}

#[tokio::test]
async fn a_proposal_is_entered_as_proposed_in_the_scope_given() {
    let rig = Rig::start(|request| {
        listing(request).unwrap_or_else(|| FakeResponse::json(202, &json!({ "outbox_id": OUTBOX })))
    })
    .await;
    assert_eq!(rig.settled().await, BrainStatus::Enabled);
    let propose = rig.engine.tools().remove(2).tool;
    let input = json!({
        "kind": "client_contact",
        "subject": "client:acme",
        "fields": { "contact": "Jane Doe" },
        "scope": { "level": "project", "key": "sales" }
    });
    let output = propose
        .run(input, &tool_context(Arc::new(Gate::default())))
        .await
        .unwrap();
    assert!(output.output.contains(OUTBOX));
    let sent = rig.fake.last().json();
    assert_eq!(sent["status"], "proposed");
    assert_eq!(sent["scope"], json!({ "level": "project", "key": "sales" }));
    assert_eq!(sent["subject"], json!({ "kind": "client", "id": "acme" }));
}

#[tokio::test]
async fn a_dispute_without_a_reason_is_refused_before_sending() {
    let rig = Rig::start(|request| listing(request).unwrap()).await;
    assert_eq!(rig.settled().await, BrainStatus::Enabled);
    let asked = rig.fake.requests().len();
    let dispute = rig.engine.tools().remove(3).tool;
    let error = dispute
        .run(
            json!({ "fact_id": FACT, "reason": "   " }),
            &tool_context(Arc::new(Gate::default())),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("reason"));
    assert_eq!(rig.fake.requests().len(), asked);
}
