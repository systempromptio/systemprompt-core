//! What a session remembers and how it answers when it is not there: the
//! agent and model in use, operations on an unknown session, archived
//! sessions, and a session whose folder was removed.

use std::path::Path;

use systemprompt_engine::run::{Delivery, EngineError, ErrorKind, PromptInput, Purpose};
use systemprompt_engine::store::{Change, MessageId, MessagePage, NewSession, SessionId, Store};

use super::helpers::{drain, outside, run, run_in};
use crate::support::{MODEL, Scripted, purposes, settle, text_turn};

#[derive(Debug, Clone, Copy)]
enum Pick {
    Agent(&'static str),
    Model(&'static str),
}

#[test]
#[ignore = "parity ledger: a changed agent or model is stored on the session and announced as a session change; no entry is added to the transcript"]
fn changing_the_agent_or_model_leaves_an_entry_in_the_transcript() {
    let cases = [
        (
            "an agent replacing the stored one",
            Some("build"),
            None,
            Pick::Agent("plan"),
        ),
        (
            "an agent where none was stored",
            None,
            None,
            Pick::Agent("plan"),
        ),
        (
            "a model replacing the stored one",
            None,
            Some("model-a"),
            Pick::Model("model-b"),
        ),
    ];
    for (what, agent, model, pick) in cases {
        let store = Store::open_in_memory().unwrap();
        let project = store
            .ensure_project(Path::new("/work/demo"), "demo")
            .unwrap();
        let mut new = NewSession::new(project.id, "/work/demo");
        new.agent = agent.map(str::to_owned);
        new.model = model.map(str::to_owned);
        let id = store.create_session(new).unwrap().id;
        let changed = store
            .update_session(&id, |session| match pick {
                Pick::Agent(name) => session.agent = Some(name.to_owned()),
                Pick::Model(name) => session.model = Some(name.to_owned()),
            })
            .unwrap();
        match pick {
            Pick::Agent(name) => assert_eq!(changed.agent.as_deref(), Some(name), "{what}"),
            Pick::Model(name) => assert_eq!(changed.model.as_deref(), Some(name), "{what}"),
        }
        let transcript = store.messages(&id, &MessagePage::all()).unwrap();
        assert_eq!(transcript.len(), 1, "{what}: one entry records the change");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn choosing_the_agent_and_model_already_in_use_records_nothing_new() {
    let scripted = Scripted::start(vec![text_turn("one"), text_turn("two")]).await;
    let id = scripted.session().await;
    let chosen = |text: &str| PromptInput {
        agent: Some("build".to_owned()),
        model: Some(MODEL.to_owned()),
        ..scripted.prompt(&id, text, Delivery::Steer)
    };
    let mut events = scripted.engine.subscribe();
    scripted.engine.prompt(chosen("first")).await.unwrap();
    settle(&mut events, &id).await;
    let before = scripted.engine.get(&id).await.unwrap().unwrap();
    assert_eq!(before.agent.as_deref(), Some("build"));
    assert_eq!(before.model.as_deref(), Some(MODEL));
    let mut feed = scripted.engine.store().subscribe();
    scripted.engine.prompt(chosen("second")).await.unwrap();
    settle(&mut events, &id).await;
    for change in drain(&mut feed) {
        if let Change::SessionUpdated(session) = change {
            assert_eq!(session.agent, before.agent);
            assert_eq!(session.model, before.model);
            assert_eq!(session.metadata, before.metadata);
        }
    }
    let messages = scripted.messages(&id).await;
    assert_eq!(messages.len(), 4, "two prompts and their replies, no more");
    assert!(messages.iter().all(|message| !message.message.is_system()));
    scripted.finish().await;
}

fn kind_of<T>(outcome: Result<T, EngineError>) -> Option<ErrorKind> {
    match outcome {
        Ok(_) => None,
        Err(error) => Some(error.kind()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn operations_on_an_unknown_session_fail_as_not_found() {
    let scripted = Scripted::start(Vec::new()).await;
    let engine = &scripted.engine;
    let unknown = SessionId::mint(engine.store().ids());
    let message = MessageId::mint(engine.store().ids());
    let with_agent = PromptInput {
        agent: Some("plan".to_owned()),
        ..scripted.prompt(&unknown, "hello", Delivery::Steer)
    };
    let with_model = PromptInput {
        model: Some(MODEL.to_owned()),
        ..scripted.prompt(&unknown, "hello", Delivery::Steer)
    };
    let outcomes = [
        (
            "a prompt choosing an agent",
            kind_of(engine.prompt(with_agent).await),
        ),
        (
            "a prompt choosing a model",
            kind_of(engine.prompt(with_model).await),
        ),
        ("a revert", kind_of(engine.revert(&unknown, &message).await)),
        ("a redo", kind_of(engine.unrevert(&unknown).await)),
        ("a diff", kind_of(engine.diff(&unknown).await)),
        ("a compaction", kind_of(engine.compact(&unknown).await)),
    ];
    for (what, kind) in outcomes {
        assert_eq!(kind, Some(ErrorKind::NotFound), "{what}");
    }
    assert!(scripted.model_bodies().is_empty());
    assert!(engine.get(&unknown).await.unwrap().is_none());
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: the messages of an unknown session read as an empty list, not as a not-found error"]
async fn reading_the_transcript_of_an_unknown_session_fails_as_not_found() {
    let scripted = Scripted::start(Vec::new()).await;
    let unknown = SessionId::mint(scripted.engine.store().ids());
    let outcome = scripted
        .engine
        .messages(&unknown)
        .await
        .map(|messages| messages.len());
    assert!(
        matches!(&outcome, Err(error) if error.kind() == ErrorKind::NotFound),
        "{outcome:?}"
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "parity ledger: a prompt leaves an archived session archived"]
async fn a_prompt_brings_an_archived_session_back() {
    let scripted = Scripted::start(vec![text_turn("hello")]).await;
    let id = scripted.session().await;
    scripted.engine.store().set_archived(&id, true).unwrap();
    run(&scripted, &id, "hello").await;
    assert!(
        !scripted
            .engine
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .is_archived()
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn input_the_engine_originates_leaves_an_archived_session_archived() {
    let scripted =
        Scripted::start(vec![text_turn("answer"), text_turn("summary of the talk")]).await;
    let id = scripted.session().await;
    run(&scripted, &id, "hello").await;
    scripted.engine.store().set_archived(&id, true).unwrap();
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert!(purposes(&scripted).contains(&Purpose::Compaction));
    assert!(
        scripted
            .engine
            .get(&id)
            .await
            .unwrap()
            .unwrap()
            .is_archived()
    );
    scripted.finish().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_compaction_is_accepted_and_run_for_a_session_whose_folder_is_gone() {
    let scripted =
        Scripted::start(vec![text_turn("answer"), text_turn("summary of the talk")]).await;
    let folder = outside(&scripted, "pruned");
    let id = scripted.engine.create(&folder).await.unwrap().id;
    scripted.engine.store().set_title(&id, "stranded").unwrap();
    run_in(&scripted, &id, &folder, "hello").await;
    std::fs::remove_dir_all(&folder).unwrap();
    let mut events = scripted.engine.subscribe();
    scripted.engine.compact(&id).await.unwrap();
    let settled = settle(&mut events, &id).await;
    assert!(settled.failures.is_empty(), "{:?}", settled.failures);
    assert!(purposes(&scripted).contains(&Purpose::Compaction));
    let summarised = scripted.messages(&id).await.iter().any(|message| {
        message
            .message
            .as_assistant()
            .is_some_and(|reply| reply.system)
    });
    assert!(summarised, "the summary was stored");
    scripted.finish().await;
}
