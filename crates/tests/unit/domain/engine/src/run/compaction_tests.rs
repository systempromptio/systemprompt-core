use std::sync::Arc;

use systemprompt_engine::run::{Delivery, EngineEvent, ErrorKind, RunStatus};
use systemprompt_engine::store::PartBody;

use crate::run::support::{
    Scripted, TestGovernor, harness, prompt, text_reply, titled_session, wait_idle,
};

#[tokio::test]
async fn a_compaction_replaces_the_history_before_it_with_its_summary() {
    let transport = Arc::new(Scripted::with(vec![
        (200, text_reply("old answer")),
        (200, text_reply("the condensed summary")),
        (200, text_reply("fresh answer")),
    ]));
    let harness = harness(Arc::new(TestGovernor::default()), Arc::clone(&transport));
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "old question", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;
    harness.engine.compact(&id).await.unwrap();
    wait_idle(&mut events, &id).await;
    harness
        .engine
        .prompt(prompt(&harness, &id, "new question", Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;

    let requests = transport.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests[1].to_string().contains("old question"));
    let after = requests[2].to_string();
    assert!(after.contains("the condensed summary"));
    assert!(after.contains("new question"));
    assert!(!after.contains("old question"));
    assert!(!after.contains("old answer"));

    let messages = harness.engine.messages(&id).await.unwrap();
    let boundaries = messages
        .iter()
        .flat_map(|message| message.parts.iter())
        .filter(|part| matches!(part.body, PartBody::Compaction { auto: false }))
        .count();
    assert_eq!(boundaries, 1);
    harness.engine.shutdown().await;
}

const OVERFLOW: &str = r#"{"error": {"message": "maximum context length exceeded", "code": "context_length_exceeded"}}"#;

#[tokio::test]
async fn a_failed_recovery_summary_leaves_the_overflow_as_the_turns_failure() {
    let transport = Arc::new(Scripted::with(vec![
        (400, OVERFLOW.to_owned()),
        (400, r#"{"error": {"message": "bad request"}}"#.to_owned()),
    ]));
    let harness = harness(Arc::new(TestGovernor::default()), Arc::clone(&transport));
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "Continue", Delivery::Steer))
        .await
        .unwrap();
    let mut failures = Vec::new();
    let settled = async {
        loop {
            match events.recv().await {
                Ok(EngineEvent::SessionError { session, error }) if session == id => {
                    failures.push(error.kind);
                },
                Ok(EngineEvent::Status {
                    session,
                    status: RunStatus::Idle,
                }) if session == id => return,
                Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {},
                Err(tokio::sync::broadcast::error::RecvError::Closed) => panic!("feed closed"),
            }
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(30), settled)
        .await
        .expect("the run did not settle");

    assert_eq!(failures, [ErrorKind::ContextOverflow]);
    assert_eq!(
        transport.requests().len(),
        2,
        "one step and one summary request"
    );
    harness.engine.shutdown().await;
}
