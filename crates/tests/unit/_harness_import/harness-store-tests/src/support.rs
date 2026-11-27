use std::error::Error;
use std::path::Path;

use harness_store::{
    AssistantMessage, MessageId, NewSession, Part, PartBody, Session, SessionId, Store, UserMessage,
};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) fn session(store: &Store) -> TestResult<Session> {
    let project = store.ensure_project(Path::new("/work/demo"), "demo")?;
    Ok(store.create_session(NewSession::new(project.id, "/work/demo"))?)
}

pub(crate) fn user(store: &Store, session_id: &SessionId, text: &str) -> TestResult<MessageId> {
    let message = UserMessage::new(session_id.clone(), "build", "model-a");
    let id = message.id.clone();
    store.upsert_message(&message.into())?;
    store.upsert_part(&Part::new(
        session_id.clone(),
        id.clone(),
        PartBody::Text {
            text: text.to_owned(),
            synthetic: false,
        },
    ))?;
    Ok(id)
}

pub(crate) fn assistant(
    store: &Store,
    session_id: &SessionId,
    parent: &MessageId,
    text: &str,
) -> TestResult<MessageId> {
    let message = AssistantMessage::new(session_id.clone(), parent.clone(), "build", "model-a");
    let id = message.id.clone();
    store.upsert_message(&message.into())?;
    store.upsert_part(&Part::new(
        session_id.clone(),
        id.clone(),
        PartBody::StepStart { snapshot: None },
    ))?;
    store.upsert_part(&Part::new(
        session_id.clone(),
        id.clone(),
        PartBody::Text {
            text: text.to_owned(),
            synthetic: false,
        },
    ))?;
    Ok(id)
}

pub(crate) fn empty_reply(store: &Store, session_id: &SessionId) -> TestResult<MessageId> {
    let user_id = user(store, session_id, "go")?;
    let message = AssistantMessage::new(session_id.clone(), user_id, "build", "model-a");
    let id = message.id.clone();
    store.upsert_message(&message.into())?;
    Ok(id)
}

pub(crate) fn text_of(parts: &[Part]) -> Vec<&str> {
    parts.iter().filter_map(|part| part.body.text()).collect()
}
