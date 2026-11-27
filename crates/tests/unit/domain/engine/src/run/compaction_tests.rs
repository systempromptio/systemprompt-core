use std::sync::Arc;

use systemprompt_engine::run::Delivery;
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
