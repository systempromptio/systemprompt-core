use std::sync::Arc;
use std::time::Duration;

use harness_engine::{Admitted, Delivery};
use serde_json::json;

use crate::support::{
    Scripted, TestGovernor, harness, prompt, text_reply, texts, titled_session, tool_reply,
    wait_idle,
};

fn held(replies: Vec<(u16, String)>) -> Arc<Scripted> {
    Arc::new(Scripted::held(replies))
}

#[tokio::test]
async fn a_prompt_sent_mid_run_is_steered_into_the_next_step() {
    let transport = held(vec![
        (
            200,
            tool_reply("call_1", "read", &json!({"path": "notes.txt"})),
        ),
        (200, text_reply("Noted both.")),
    ]);
    let harness = harness(Arc::new(TestGovernor::default()), Arc::clone(&transport));
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    let first = harness
        .engine
        .prompt(prompt(&harness, &id, "Read the notes", Delivery::Steer))
        .await
        .unwrap();
    assert!(matches!(first, Admitted::Started { .. }));
    tokio::time::timeout(Duration::from_secs(30), transport.first_sent.notified())
        .await
        .unwrap();
    let second = harness
        .engine
        .prompt(prompt(
            &harness,
            &id,
            "Also count the words",
            Delivery::Steer,
        ))
        .await
        .unwrap();
    assert_eq!(second, Admitted::Steered);
    transport.release.notify_one();
    wait_idle(&mut events, &id).await;

    let requests = transport.requests();
    assert_eq!(requests.len(), 2);
    assert!(!requests[0].to_string().contains("Also count the words"));
    let steered = requests[1].to_string();
    assert!(steered.contains("Also count the words"));
    assert!(steered.contains("while you were working"));
    let messages = harness.engine.messages(&id).await.unwrap();
    assert_eq!(messages.len(), 4);
    assert_eq!(texts(&messages[2]), "Also count the words");
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn steers_reach_the_running_turn_and_queued_prompts_wait_for_the_next() {
    let transport = held(vec![
        (
            200,
            tool_reply("call_1", "read", &json!({"path": "notes.txt"})),
        ),
        (200, text_reply("Steer handled.")),
        (200, text_reply("Queue handled.")),
    ]);
    let harness = harness(Arc::new(TestGovernor::default()), Arc::clone(&transport));
    let id = titled_session(&harness).await;
    let mut events = harness.engine.subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "Read the notes", Delivery::Steer))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(30), transport.first_sent.notified())
        .await
        .unwrap();
    let queued = harness
        .engine
        .prompt(prompt(&harness, &id, "queued words", Delivery::Queue))
        .await
        .unwrap();
    let steered = harness
        .engine
        .prompt(prompt(&harness, &id, "steered words", Delivery::Steer))
        .await
        .unwrap();
    assert_eq!(queued, Admitted::Queued);
    assert_eq!(steered, Admitted::Steered);
    transport.release.notify_one();
    wait_idle(&mut events, &id).await;

    let requests = transport.requests();
    assert_eq!(requests.len(), 3);
    let second = requests[1].to_string();
    assert!(second.contains("steered words"));
    assert!(!second.contains("queued words"));
    assert!(requests[2].to_string().contains("queued words"));
    let messages = harness.engine.messages(&id).await.unwrap();
    let users: Vec<String> = messages
        .iter()
        .filter(|message| message.message.as_user().is_some())
        .map(texts)
        .collect();
    assert_eq!(users, ["Read the notes", "steered words", "queued words"]);
    harness.engine.shutdown().await;
}
