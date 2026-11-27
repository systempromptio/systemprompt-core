use std::sync::Arc;
use std::time::Duration;

use harness_engine::{Delivery, Purpose};

use crate::support::{Scripted, TestGovernor, harness, prompt, text_reply, wait_idle};

#[tokio::test]
async fn a_refused_title_call_falls_back_to_the_first_line_of_the_prompt() {
    let governor = Arc::new(TestGovernor {
        refuse: Some(Purpose::Title),
        ..TestGovernor::default()
    });
    let transport = Arc::new(Scripted::with(vec![(200, text_reply("On it."))]));
    let harness = harness(governor, transport);
    let id = harness.engine.create(&harness.project).await.unwrap().id;
    let mut events = harness.engine.subscribe();

    let text = "\n  fix   the build \nmore";
    harness
        .engine
        .prompt(prompt(&harness, &id, text, Delivery::Steer))
        .await
        .unwrap();
    wait_idle(&mut events, &id).await;

    let mut title = String::new();
    for _ in 0..200 {
        title = harness
            .engine
            .get(&id)
            .await
            .unwrap()
            .map(|session| session.title)
            .unwrap_or_default();
        if !title.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(title, "fix the build");
    harness.engine.shutdown().await;
}
