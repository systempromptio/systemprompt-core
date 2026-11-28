use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_engine::llm::{HttpRequest, HttpResponse, Transport, TransportError};
use systemprompt_engine::run::{Delivery, Purpose};
use systemprompt_engine::store::{Change, SessionId};
use tokio::sync::{Notify, broadcast};

use crate::run::support::{
    Scripted, TestGovernor, harness, harness_on, prompt, text_reply, wait_idle,
};

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

const SETTLE: Duration = Duration::from_secs(30);

/// Holds the title call until `release` and leaves the step's response open
/// after its first text, so the step can only end by being aborted.
#[derive(Default)]
struct TitleHeld {
    title_sent: Notify,
    step_sent: Notify,
    release: Notify,
}

#[async_trait]
impl Transport for TitleHeld {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        if body.to_string().contains("You name conversations") {
            self.title_sent.notify_one();
            self.release.notified().await;
            let chunks: Vec<Result<Bytes, TransportError>> =
                vec![Ok(Bytes::from(text_reply("A long answer")))];
            return Ok(HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: Box::pin(futures_util::stream::iter(chunks)),
            });
        }
        let delta = json!({"choices": [{"index": 0, "delta": {"content": "Here is the first part"}, "finish_reason": null}]});
        let first: Vec<Result<Bytes, TransportError>> =
            vec![Ok(Bytes::from(format!("data: {delta}\n\n")))];
        self.step_sent.notify_one();
        Ok(HttpResponse {
            status: 200,
            headers: Vec::new(),
            body: Box::pin(futures_util::StreamExt::chain(
                futures_util::stream::iter(first),
                futures_util::stream::pending(),
            )),
        })
    }
}

async fn title_written(changes: &mut broadcast::Receiver<Change>, session: &SessionId) -> String {
    let written = async {
        loop {
            match changes.recv().await {
                Ok(Change::SessionUpdated(updated))
                    if &updated.id == session && !updated.title.is_empty() =>
                {
                    return updated.title.clone();
                },
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {},
                Err(broadcast::error::RecvError::Closed) => panic!("the change feed closed"),
            }
        }
    };
    tokio::time::timeout(SETTLE, written)
        .await
        .expect("no title was written")
}

#[tokio::test]
async fn an_abort_mid_stream_leaves_the_title_call_to_finish_and_its_reply_is_stored() {
    let governor = Arc::new(TestGovernor::default());
    let transport = Arc::new(TitleHeld::default());
    let harness = harness_on(governor, Arc::clone(&transport));
    let id = harness.engine.create(&harness.project).await.unwrap().id;
    let mut events = harness.engine.subscribe();
    let mut changes = harness.engine.store().subscribe();

    harness
        .engine
        .prompt(prompt(&harness, &id, "Explain it at length", Delivery::Steer))
        .await
        .unwrap();
    tokio::time::timeout(SETTLE, async {
        transport.title_sent.notified().await;
        transport.step_sent.notified().await;
    })
    .await
    .expect("the title call and the step were not both sent");

    assert!(harness.engine.abort(&id));
    wait_idle(&mut events, &id).await;
    let before = harness.engine.get(&id).await.unwrap().unwrap().title;
    assert_eq!(before, "", "the title call is still held");

    transport.release.notify_one();
    assert_eq!(title_written(&mut changes, &id).await, "A long answer");
    assert_eq!(
        harness.engine.get(&id).await.unwrap().unwrap().title,
        "A long answer"
    );
    harness.engine.shutdown().await;
}

#[tokio::test]
async fn a_title_call_cancelled_by_shutdown_stores_no_title() {
    let governor = Arc::new(TestGovernor::default());
    let transport = Arc::new(TitleHeld::default());
    let harness = harness_on(governor, Arc::clone(&transport));
    let id = harness.engine.create(&harness.project).await.unwrap().id;

    harness
        .engine
        .prompt(prompt(&harness, &id, "Explain it at length", Delivery::Steer))
        .await
        .unwrap();
    tokio::time::timeout(SETTLE, async {
        transport.title_sent.notified().await;
        transport.step_sent.notified().await;
    })
    .await
    .expect("the title call and the step were not both sent");

    harness.engine.shutdown().await;

    let stored = harness.engine.store().get_session(&id).unwrap().unwrap();
    assert_eq!(stored.title, "", "a cancelled title call names nothing");
}
