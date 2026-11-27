//! The governed call path: what an extension contributes follows the
//! Governor's enablement, and no extension can widen what the Governor decided.

use std::sync::Arc;

use serde_json::Value;
use systemprompt_engine::run::{
    Delivery, EnabledExtensions, EngineEvent, ExtensionSet, Review, denied,
};
use systemprompt_engine::store::{MessageWithParts, ToolState};
use systemprompt_identifiers::engine::ExtensionId;
use tokio::sync::broadcast;

use crate::run::support::{
    Probe, Recorder, Reviewing, Scripted, Switch, TestGovernor, harness_extended, prompt,
    read_then, text_reply, titled_session, tool_names, tool_state, wait_idle,
};

fn enabling(ids: &[&str]) -> EnabledExtensions {
    ids.iter()
        .map(|id| ExtensionId::parse(id).unwrap())
        .collect()
}

fn governed_by(enabled: Option<EnabledExtensions>) -> Arc<TestGovernor> {
    Arc::new(TestGovernor {
        enabled,
        ..TestGovernor::default()
    })
}

fn chatting() -> Arc<Scripted> {
    Arc::new(Scripted::with(vec![(200, text_reply("Fine."))]))
}

async fn run_turn(
    governor: Arc<TestGovernor>,
    transport: &Arc<Scripted>,
    extensions: ExtensionSet,
    text: &str,
) -> Vec<MessageWithParts> {
    let harness = harness_extended(governor, Arc::clone(transport), None, extensions);
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();
    harness
        .engine
        .prompt(prompt(&harness, &id, text, Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;
    let messages = harness.engine.messages(&id).await.unwrap();
    harness.engine.shutdown().await;
    messages
}

fn denial(messages: &[MessageWithParts]) -> (String, Option<String>) {
    match tool_state(messages) {
        ToolState::Error {
            error, metadata, ..
        } => {
            assert_eq!(
                metadata.get(denied::KEY).and_then(Value::as_str),
                Some(denied::POLICY)
            );
            let reason = metadata
                .get(denied::REASON)
                .and_then(Value::as_str)
                .map(str::to_owned);
            (error, reason)
        },
        other => panic!("the call was not denied: {other:?}"),
    }
}

#[tokio::test]
async fn an_extension_tool_is_offered_only_while_its_extension_is_admitted() {
    let cases = [
        (None, false),
        (Some(enabling(&["other"])), false),
        (Some(enabling(&["probe"])), true),
    ];
    for (enabled, offered) in cases {
        let transport = chatting();
        let set = Probe::set(vec![Probe::new("probe").tool("probe_echo")]);
        run_turn(governed_by(enabled), &transport, set, "Hello").await;
        let tools = tool_names(&transport.requests()[0]);
        assert_eq!(
            tools.iter().any(|tool| tool == "probe_echo"),
            offered,
            "{tools:?}"
        );
    }
}

#[tokio::test]
async fn a_required_extension_contributes_without_enablement() {
    let transport = chatting();
    let set = Probe::set(vec![
        Probe::new("governance").required().tool("governance_echo"),
    ]);
    run_turn(governed_by(None), &transport, set, "Hello").await;
    let tools = tool_names(&transport.requests()[0]);
    assert!(
        tools.iter().any(|tool| tool == "governance_echo"),
        "{tools:?}"
    );
}

#[tokio::test]
async fn a_prompt_section_is_sent_only_while_its_extension_is_admitted() {
    for (enabled, sent) in [(None, false), (Some(enabling(&["probe"])), true)] {
        let transport = chatting();
        let set = Probe::set(vec![
            Probe::new("probe").section("Probe notes", "The probe is listening."),
        ]);
        run_turn(governed_by(enabled), &transport, set, "Hello").await;
        let request = transport.requests()[0].to_string();
        assert_eq!(request.contains("# Probe notes"), sent, "{request}");
        assert_eq!(request.contains("The probe is listening."), sent);
    }
}

#[tokio::test]
async fn a_reviewer_cannot_allow_a_call_the_governor_denied() {
    let reviewer = Arc::new(Reviewing::new(Review::Pass));
    let governor = Arc::new(TestGovernor {
        deny: Some("reading is blocked".to_owned()),
        enabled: Some(enabling(&["probe"])),
        ..TestGovernor::default()
    });
    let transport = Arc::new(read_then("Understood."));
    let set = Probe::set(vec![Probe::new("probe").reviewing(&reviewer)]);
    let messages = run_turn(governor, &transport, set, "Read the notes").await;
    let (error, reason) = denial(&messages);
    assert_eq!(reason.as_deref(), Some("reading is blocked"));
    assert!(!error.contains("hello from the notes file"), "{error}");
    assert_eq!(reviewer.calls(), 0);
}

#[tokio::test]
async fn a_reviewer_denies_a_call_the_governor_allowed() {
    let reviewer = Arc::new(Reviewing::new(Review::Deny {
        reason: "the probe says no".to_owned(),
    }));
    let governor = governed_by(Some(enabling(&["probe"])));
    let transport = Arc::new(read_then("Understood."));
    let set = Probe::set(vec![Probe::new("probe").reviewing(&reviewer)]);
    let messages = run_turn(Arc::clone(&governor), &transport, set, "Read the notes").await;
    let (error, reason) = denial(&messages);
    assert_eq!(reason.as_deref(), Some("the probe says no"));
    assert!(error.contains("the probe says no"), "{error}");
    assert_eq!(governor.tool_calls.lock().len(), 1);
    assert_eq!(reviewer.calls(), 1);
}

#[tokio::test]
async fn a_withheld_reviewer_is_not_asked() {
    let reviewer = Arc::new(Reviewing::new(Review::Deny {
        reason: "the probe says no".to_owned(),
    }));
    let transport = Arc::new(read_then("Done."));
    let set = Probe::set(vec![Probe::new("probe").reviewing(&reviewer)]);
    let messages = run_turn(governed_by(None), &transport, set, "Read the notes").await;
    assert!(matches!(tool_state(&messages), ToolState::Completed { .. }));
    assert_eq!(reviewer.calls(), 0);
}

#[tokio::test]
async fn an_augmenter_cannot_override_the_governors_headers() {
    let governor = Arc::new(TestGovernor {
        enabled: Some(enabling(&["probe"])),
        endpoint_headers: vec![("Authorization".to_owned(), "Bearer governed".to_owned())],
        ..TestGovernor::default()
    });
    let transport = chatting();
    let set = Probe::set(vec![
        Probe::new("probe")
            .adding("authorization", "Bearer forged")
            .adding("x-probe", "on"),
        Probe::new("other").adding("x-other", "on"),
    ]);
    run_turn(governor, &transport, set, "Hello").await;
    let headers = &transport.request_headers()[0];
    let has = |name: &str, value: &str| {
        headers
            .iter()
            .any(|(key, found)| key.eq_ignore_ascii_case(name) && found == value)
    };
    assert!(has("authorization", "Bearer governed"), "{headers:?}");
    assert!(has("x-probe", "on"), "{headers:?}");
    assert!(!has("authorization", "Bearer forged"), "{headers:?}");
    assert!(
        !headers.iter().any(|(key, _)| key == "x-other"),
        "{headers:?}"
    );
}

#[tokio::test]
async fn observers_hear_prompts_tools_sessions_and_response_heads() {
    let heard = Arc::new(Recorder::default());
    let withheld = Arc::new(Recorder::default());
    let transport = Arc::new(read_then("Done.").answering_with(&[("X-Probe-Head", "seen")]));
    let set = Probe::set(vec![
        Probe::new("probe").observing(&heard),
        Probe::new("other").observing(&withheld),
    ]);
    run_turn(
        governed_by(Some(enabling(&["probe"]))),
        &transport,
        set,
        "Read the notes",
    )
    .await;
    assert_eq!(
        heard.heard(),
        [
            "session:created",
            "prompt:Read the notes",
            "head:200:seen",
            "tool:read:ok",
            "head:200:seen",
            "session:idle",
        ]
    );
    assert_eq!(withheld.heard(), Vec::<String>::new());
}

#[tokio::test]
async fn a_withheld_tool_is_offered_once_its_extension_bumps_its_generation() {
    let switch = Arc::new(Switch::default());
    let transport = chatting();
    let set = Probe::set(vec![
        Probe::new("probe").tool("probe_echo").switched(&switch),
    ]);
    let harness = harness_extended(
        governed_by(Some(enabling(&["probe"]))),
        Arc::clone(&transport),
        None,
        set,
    );
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();
    let offered_on = |turn: usize| {
        tool_names(&transport.requests()[turn])
            .iter()
            .any(|tool| tool == "probe_echo")
    };
    let turn = async |events: &mut broadcast::Receiver<EngineEvent>| {
        harness
            .engine
            .prompt(prompt(&harness, &id, "Hello", Delivery::Steer))
            .await
            .unwrap();
        wait_idle(events, &id).await;
    };

    turn(&mut events).await;
    assert!(!offered_on(0));
    switch.offer();
    turn(&mut events).await;
    assert!(!offered_on(1), "re-read without any generation moving");
    switch.bump();
    turn(&mut events).await;
    assert!(offered_on(2));
    harness.engine.shutdown().await;
}
