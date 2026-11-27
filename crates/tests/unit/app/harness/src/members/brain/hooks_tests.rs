//! The recall switch follows the person's setting and the observer records
//! the facts each response says were injected.

use std::path::PathBuf;
use systemprompt_identifiers::ModelId;
use systemprompt_identifiers::engine::IdClock;

use serde_json::json;
use systemprompt_engine::run::extension::ResponseHead;
use systemprompt_engine::run::{ModelCall, Purpose, SessionEvent};
use systemprompt_harness::members::brain::{BRAIN_EXTENSION, BrainSettings};
use systemprompt_identifiers::engine::SessionId;

use crate::members::brain::support::{FACT, FACT2, FakeResponse, Rig, listing};

fn call(session: &SessionId) -> ModelCall {
    ModelCall {
        session_id: session.clone(),
        model_id: ModelId::new("model"),
        purpose: Purpose::Step,
        directory: PathBuf::from("."),
    }
}

fn head(ids: Option<&str>) -> ResponseHead {
    ResponseHead {
        status: 200,
        headers: ids
            .map(|value| ("X-Systemprompt-Memory-Ids".to_owned(), value.to_owned()))
            .into_iter()
            .collect(),
    }
}

#[tokio::test]
async fn recall_is_sent_off_only_when_the_person_turned_it_off() {
    let rig = Rig::start(|request| listing(request).unwrap()).await;
    let augmenter = rig.engine.augmenter().unwrap();
    let session = SessionId::mint(&IdClock::new());
    assert_eq!(augmenter.headers(&call(&session)).await.len(), 0);
    let settings = rig.ctx.settings_for(&BRAIN_EXTENSION);
    settings.set(&BrainSettings { recall: false }).unwrap();
    assert_eq!(
        augmenter.headers(&call(&session)).await,
        [("x-systemprompt-memory".to_owned(), "off".to_owned())]
    );
    settings.set(&json!({ "recall": "sometimes" })).unwrap();
    assert_eq!(augmenter.headers(&call(&session)).await.len(), 1);
}

#[tokio::test]
async fn injected_memory_is_recorded_per_session_and_forgotten_with_it() {
    let rig =
        Rig::start(|request| listing(request).unwrap_or_else(|| FakeResponse::status(500))).await;
    let observer = rig.engine.observer().unwrap();
    let session = SessionId::mint(&IdClock::new());
    let other = SessionId::mint(&IdClock::new());
    let before = rig.service.feed().memory;
    observer
        .response_head(
            &call(&session),
            &head(Some(&format!("{FACT}@3, {FACT2}@1"))),
        )
        .await;
    let recorded = rig.service.memory(&session);
    assert_eq!(recorded.len(), 2);
    assert_eq!((recorded[0].id.as_str(), recorded[0].revision), (FACT, 3));
    assert_eq!((recorded[1].id.as_str(), recorded[1].revision), (FACT2, 1));
    assert_eq!(rig.service.memory(&other).len(), 0);
    assert_eq!(rig.service.feed().memory, before + 1);
    observer
        .response_head(&call(&session), &head(Some("not-a-fact@x")))
        .await;
    assert_eq!(rig.service.memory(&session).len(), 2);
    observer.response_head(&call(&session), &head(None)).await;
    assert_eq!(rig.service.memory(&session).len(), 0);
    observer
        .response_head(&call(&session), &head(Some(&format!("{FACT}@3"))))
        .await;
    observer.session_event(&SessionEvent::Deleted {
        session_id: session.clone(),
    });
    assert_eq!(rig.service.memory(&session).len(), 0);
}
