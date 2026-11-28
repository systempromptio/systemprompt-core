//! Permission requests raised by tool calls during a run: declines, sibling
//! requests, remembered approvals, single resolution, and an organisation's
//! verdict on top of the user's rules.

use serde_json::json;
use systemprompt_engine::run::{Delivery, EngineEvent, Reply};
use systemprompt_engine::store::ToolState;
use systemprompt_scripted_instance::{Respond, StopReason, ToolCall as Call, Turn};

use crate::support::{
    Scripted, Setup, read_turn, settle, settle_answering, text_turn, tool_states, tool_turn, until,
};

fn two_env_reads() -> Turn {
    let call = |id: &str, path: &str| {
        Respond::ToolCall(Call {
            id: id.to_owned(),
            name: "read".to_owned(),
            input: json!({"path": path}),
            argument_chunks: Vec::new(),
        })
    };
    Turn {
        respond: vec![call("a", ".env"), call("b", ".env.local")],
        stop: Some(StopReason::ToolUse),
        ..Turn::default()
    }
}

fn with_env_files(scripted: &Scripted) {
    std::fs::write(scripted.project.join(".env"), "KEY=1\n").unwrap();
    std::fs::write(scripted.project.join(".env.local"), "KEY=2\n").unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pending_request_resolves_exactly_once() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "read", json!({"path": ".env"})),
        text_turn("ok"),
    ])
    .await;
    with_env_files(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read env", Delivery::Steer))
        .await
        .unwrap();
    let EngineEvent::PermissionAsked(ask) = until(&mut events, |event| {
        matches!(event, EngineEvent::PermissionAsked(_))
    })
    .await
    else {
        unreachable!()
    };
    assert!(scripted.engine.reply_permission(&ask.id, Reply::Once));
    assert!(!scripted.engine.reply_permission(&ask.id, Reply::Once));
    settle(&mut events, &id).await;
    assert!(matches!(
        tool_states(&scripted.messages(&id).await).as_slice(),
        [ToolState::Completed { .. }]
    ));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: declining one request leaves its sibling in the same step pending"]
async fn declining_one_request_also_declines_its_siblings_in_the_same_step() {
    let scripted = Scripted::start(vec![two_env_reads(), text_turn("ok")]).await;
    with_env_files(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read both", Delivery::Steer))
        .await
        .unwrap();
    let EngineEvent::PermissionAsked(first) = until(&mut events, |event| {
        matches!(event, EngineEvent::PermissionAsked(_))
    })
    .await
    else {
        unreachable!()
    };
    scripted
        .engine
        .reply_permission(&first.id, Reply::Reject { feedback: None });
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let left = scripted.engine.pending_permissions();
    for ask in &left {
        scripted
            .engine
            .reply_permission(&ask.id, Reply::Reject { feedback: None });
    }
    settle(&mut events, &id).await;
    assert!(
        left.is_empty(),
        "still pending after the decline: {:?}",
        left.iter().map(|a| &a.patterns).collect::<Vec<_>>()
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decline_ends_the_turn_without_another_model_request() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "read", json!({"path": ".env"})),
        text_turn("after"),
    ])
    .await;
    with_env_files(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read env", Delivery::Steer))
        .await
        .unwrap();
    let engine = &scripted.engine;
    settle_answering(&mut events, &id, |ask| {
        engine.reply_permission(&ask.id, Reply::Reject { feedback: None });
        Some(())
    })
    .await;
    assert_eq!(scripted.model_bodies().len(), 1);
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_decline_with_a_correction_goes_back_to_the_model_and_the_turn_continues() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "read", json!({"path": ".env"})),
        text_turn("Understood."),
    ])
    .await;
    with_env_files(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read env", Delivery::Steer))
        .await
        .unwrap();
    let engine = &scripted.engine;
    settle_answering(&mut events, &id, |ask| {
        let feedback = Some("read the example file instead".to_owned());
        engine.reply_permission(&ask.id, Reply::Reject { feedback });
        Some(())
    })
    .await;
    let bodies = scripted.model_bodies();
    assert_eq!(bodies.len(), 2);
    assert!(
        bodies[1]
            .to_string()
            .contains("read the example file instead")
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_always_answer_covers_later_matching_calls_for_the_rest_of_the_session() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "read", json!({"path": ".env"})),
        tool_turn("t2", "read", json!({"path": ".env"})),
        text_turn("done"),
    ])
    .await;
    with_env_files(&scripted);
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read twice", Delivery::Steer))
        .await
        .unwrap();
    let engine = &scripted.engine;
    let settled = settle_answering(&mut events, &id, |ask| {
        engine.reply_permission(&ask.id, Reply::Always);
        Some(())
    })
    .await;
    assert_eq!(settled.asks.len(), 1);
    let states = tool_states(&scripted.messages(&id).await);
    assert!(
        states
            .iter()
            .all(|state| matches!(state, ToolState::Completed { .. })),
        "{states:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_organisation_denial_overrides_a_project_allow_and_its_reason_reaches_the_model() {
    let scripted = Scripted::with(
        vec![read_turn("t1"), text_turn("Blocked.")],
        Setup {
            settings: Some(r#"{"permission": {"read": "allow"}}"#.to_owned()),
            deny: Some("blocked by the organisation's policy".to_owned()),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read", Delivery::Steer))
        .await
        .unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.asks.is_empty());
    assert!(matches!(
        tool_states(&scripted.messages(&id).await).as_slice(),
        [ToolState::Error { .. }]
    ));
    let bodies = scripted.model_bodies();
    assert!(
        bodies[1]
            .to_string()
            .contains("blocked by the organisation's policy")
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_organisation_allow_never_grants_what_the_rules_deny() {
    let scripted = Scripted::with(
        vec![read_turn("t1"), text_turn("Denied.")],
        Setup {
            settings: Some(r#"{"permission": {"read": "deny"}}"#.to_owned()),
            ..Setup::default()
        },
    )
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read", Delivery::Steer))
        .await
        .unwrap();
    settle(&mut events, &id).await;
    assert!(matches!(
        tool_states(&scripted.messages(&id).await).as_slice(),
        [ToolState::Error { .. }]
    ));
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn reading_outside_the_project_asks_for_the_directory_before_the_read_rule() {
    let scripted = Scripted::start(vec![
        tool_turn("t1", "read", json!({"path": "/"})),
        text_turn("ok"),
    ])
    .await;
    let id = scripted.session().await;
    let mut events = scripted.engine.subscribe();
    scripted
        .engine
        .prompt(scripted.prompt(&id, "Read root", Delivery::Steer))
        .await
        .unwrap();
    let engine = &scripted.engine;
    let settled = settle_answering(&mut events, &id, |ask| {
        engine.reply_permission(&ask.id, Reply::Reject { feedback: None });
        Some(())
    })
    .await;
    assert_eq!(
        settled.asks.first().map(|ask| ask.permission.as_str()),
        Some("external_directory")
    );
    assert_eq!(
        settled.asks.len(),
        1,
        "a denial stops further authorization"
    );
    scripted.finish().await;
}
