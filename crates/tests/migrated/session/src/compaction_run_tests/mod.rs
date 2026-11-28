//! Compaction inside a run: requests made while a turn is going, the summary
//! call and its failures, summary input cut to fit, recovery from a context
//! overflow, output limits, and the verbatim tail kept beside a summary.
//!
//! History that only sets a scene is written straight into the store, so a
//! script holds just the model turns the behaviour under test consumes.

mod during_run;
mod limits;
mod overflow;
mod rig;
mod shrinking;
mod summary_call;
mod tail;

use std::time::Duration;

use serde_json::{Value, json};
use systemprompt_engine::run::{Engine, PromptInput};
use systemprompt_engine::store::{
    AssistantMessage, CallId, Change, FinishReason, JsonMap, Message, MessageId, MessageWithParts,
    Part, PartBody, SessionId, Store, ToolState, Usage, UserMessage, now_ms,
};
use tokio::sync::broadcast;

use crate::support::{MODEL, Settled, settle, texts};

const AGENT: &str = "build";
const WAIT: Duration = Duration::from_secs(60);

/// Base64 payload of the image attachments these tests seed.
const IMAGE_DATA: &str = "QUJDREVGR0hJSktMTU5PUA==";

fn put_part(store: &Store, session: &SessionId, message: &MessageId, body: PartBody) {
    let part = Part::new(store.ids(), session.clone(), message.clone(), body);
    store.upsert_part(&part).unwrap();
}

fn text_part(text: &str) -> PartBody {
    PartBody::Text {
        text: text.to_owned(),
        synthetic: false,
    }
}

fn image_part() -> PartBody {
    PartBody::File {
        mime: "image/png".to_owned(),
        name: Some("pixel.png".to_owned()),
        url: format!("data:image/png;base64,{IMAGE_DATA}"),
    }
}

fn seed_user(store: &Store, session: &SessionId, text: &str) -> MessageId {
    let message = UserMessage::new(store.ids(), session.clone(), AGENT, MODEL);
    let id = message.id.clone();
    store.upsert_message(&Message::User(message)).unwrap();
    put_part(store, session, &id, text_part(text));
    id
}

fn seed_assistant(
    store: &Store,
    session: &SessionId,
    parent: &MessageId,
    finish: FinishReason,
) -> AssistantMessage {
    let mut message =
        AssistantMessage::new(store.ids(), session.clone(), parent.clone(), AGENT, MODEL);
    message.finish = Some(finish);
    message.completed_at = Some(now_ms());
    message
}

/// A finished reply whose step reported `input` prompt tokens.
fn seed_reply(store: &Store, session: &SessionId, parent: &MessageId, text: &str, input: u64) {
    let mut message = seed_assistant(store, session, parent, FinishReason::Stop);
    message.usage = Usage {
        input,
        ..Usage::default()
    };
    let id = message.id.clone();
    store.upsert_message(&Message::Assistant(message)).unwrap();
    put_part(store, session, &id, text_part(text));
}

fn seed_exchange(store: &Store, session: &SessionId, question: &str, answer: &str, input: u64) {
    let asked = seed_user(store, session, question);
    seed_reply(store, session, &asked, answer, input);
}

/// One finished step of `read` calls, one call per output.
fn seed_tool_step(
    store: &Store,
    session: &SessionId,
    parent: &MessageId,
    step: usize,
    outputs: &[String],
) {
    let message = seed_assistant(store, session, parent, FinishReason::ToolCalls);
    let id = message.id.clone();
    store.upsert_message(&Message::Assistant(message)).unwrap();
    for (index, output) in outputs.iter().enumerate() {
        let now = now_ms();
        let state = ToolState::Completed {
            input: json!({"path": "notes.txt"}),
            output: output.clone(),
            title: "notes.txt".to_owned(),
            metadata: JsonMap::new(),
            attachments: Vec::new(),
            started: now,
            ended: now,
        };
        let body = PartBody::Tool {
            call_id: CallId::try_new(format!("call_{step}_{index}")).unwrap(),
            tool: "read".to_owned(),
            state: Box::new(state),
        };
        put_part(store, session, &id, body);
    }
}

/// An earlier manual compaction: its boundary and the summary it produced.
fn seed_summary(store: &Store, session: &SessionId, summary: &str) {
    let mut boundary = UserMessage::new(store.ids(), session.clone(), "compaction", MODEL);
    boundary.system = true;
    let boundary_id = boundary.id.clone();
    store.upsert_message(&Message::User(boundary)).unwrap();
    put_part(
        store,
        session,
        &boundary_id,
        PartBody::Compaction { auto: false },
    );
    let mut message = seed_assistant(store, session, &boundary_id, FinishReason::Stop);
    message.system = true;
    let id = message.id.clone();
    store.upsert_message(&Message::Assistant(message)).unwrap();
    put_part(store, session, &id, text_part(summary));
}

/// Sends a prompt and waits for its session to go idle.
async fn ask(engine: &Engine, input: PromptInput) -> Settled {
    let session = input.session_id.clone();
    let mut events = engine.subscribe();
    engine.prompt(input).await.unwrap();
    settle(&mut events, &session).await
}

/// Requests a manual compaction of an idle session and waits for it to end.
async fn compact(engine: &Engine, session: &SessionId) -> Settled {
    let mut events = engine.subscribe();
    engine.compact(session).await.unwrap();
    settle(&mut events, session).await
}

/// Waits until a compaction has written its boundary, which it does before it
/// asks the model for the summary.
async fn compaction_started(changes: &mut broadcast::Receiver<Change>) {
    let wait = async {
        loop {
            match changes.recv().await {
                Ok(Change::PartUpdated(part))
                    if matches!(part.body, PartBody::Compaction { .. }) =>
                {
                    return;
                },
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {},
                Err(broadcast::error::RecvError::Closed) => panic!("the change feed closed"),
            }
        }
    };
    tokio::time::timeout(WAIT, wait)
        .await
        .expect("no compaction started");
}

/// The `auto` flag of every compaction boundary in the transcript.
fn boundaries(messages: &[MessageWithParts]) -> Vec<bool> {
    messages
        .iter()
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| match part.body {
            PartBody::Compaction { auto } => Some(auto),
            _ => None,
        })
        .collect()
}

/// The text of every stored summary, oldest first.
fn summaries(messages: &[MessageWithParts]) -> Vec<String> {
    messages
        .iter()
        .filter(|message| {
            message
                .message
                .as_assistant()
                .is_some_and(|assistant| assistant.system)
        })
        .map(texts)
        .collect()
}

/// The newest assistant message that is not a summary.
fn last_reply(messages: &[MessageWithParts]) -> &MessageWithParts {
    messages
        .iter()
        .rev()
        .find(|message| {
            message
                .message
                .as_assistant()
                .is_some_and(|assistant| !assistant.system)
        })
        .expect("an assistant reply")
}

fn roles(body: &Value) -> Vec<&str> {
    body["messages"]
        .as_array()
        .map(|messages| {
            messages
                .iter()
                .filter_map(|message| message["role"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

fn only_user_turns(body: &Value) -> bool {
    let roles = roles(body);
    !roles.is_empty() && roles.iter().all(|role| *role == "user")
}

fn max_tokens(body: &Value) -> u64 {
    body["max_tokens"]
        .as_u64()
        .expect("a request states its output limit")
}

/// The size of each request's conversation, as sent.
fn sizes(bodies: &[Value]) -> Vec<usize> {
    bodies
        .iter()
        .map(|body| body["messages"].to_string().len())
        .collect()
}

fn shrinking(sizes: &[usize]) -> bool {
    sizes.windows(2).all(|pair| pair[1] < pair[0])
}
