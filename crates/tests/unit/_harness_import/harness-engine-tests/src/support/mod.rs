use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use harness_config::{Layout, Paths};
use harness_engine::{
    Delivery, Engine, EngineEvent, EngineOptions, ExtensionSet, Governor, ModelSpec, PermissionAsk,
    PromptInput, Protocol, RunStatus,
};
use harness_llm::Supports;
use harness_mcp::ClientInfo;
use harness_store::{MessageWithParts, PartBody, SessionId, Store, ToolState};
use serde_json::Value;
use tokio::sync::broadcast;

mod extensions;
mod fakes;

pub(crate) use extensions::{Probe, Recorder, Reviewing, Switch, read_then};
pub(crate) use fakes::{Scripted, TestGovernor, text_reply, tool_reply};

const SETTLE: Duration = Duration::from_secs(30);

pub(crate) fn model() -> ModelSpec {
    ModelSpec {
        id: "test-model".to_owned(),
        name: "Test model".to_owned(),
        protocol: Protocol::ChatCompletions,
        context_window: 200_000,
        max_output: 4_096,
        supports: Supports {
            tool_calls: true,
            ..Supports::default()
        },
    }
}

pub(crate) struct Harness {
    _dir: tempfile::TempDir,
    pub(crate) engine: Engine,
    pub(crate) project: PathBuf,
}

pub(crate) fn harness(governor: Arc<dyn Governor>, transport: Arc<Scripted>) -> Harness {
    harness_with(governor, transport, None)
}

pub(crate) fn harness_with(
    governor: Arc<dyn Governor>,
    transport: Arc<Scripted>,
    settings: Option<&str>,
) -> Harness {
    harness_extended(governor, transport, settings, ExtensionSet::default())
}

pub(crate) fn harness_extended(
    governor: Arc<dyn Governor>,
    transport: Arc<Scripted>,
    settings: Option<&str>,
    extensions: ExtensionSet,
) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let paths = Paths {
        home: root.join("home"),
        data: root.join("data"),
        config: root.join("config"),
        cache: root.join("cache"),
        state: root.join("state"),
        tmp: root.join("tmp"),
    };
    let project = root.join("project");
    std::fs::create_dir_all(project.join(".git")).unwrap();
    std::fs::write(project.join("notes.txt"), "hello from the notes file\n").unwrap();
    if let Some(settings) = settings {
        std::fs::write(project.join("harness.json"), settings).unwrap();
    }
    let engine = Engine::new(EngineOptions {
        paths,
        layout: Layout::default(),
        store: Store::open_in_memory().unwrap(),
        governor,
        transport,
        client_info: ClientInfo::new("harness-test", "0"),
        search_path: Some(OsString::new()),
        path_ext: None,
        extensions,
        lsp_client_name: None,
    })
    .unwrap();
    Harness {
        _dir: dir,
        engine,
        project,
    }
}

pub(crate) async fn titled_session(harness: &Harness) -> SessionId {
    let session = harness.engine.create(&harness.project).await.unwrap();
    harness
        .engine
        .store()
        .set_title(&session.id, "test session")
        .unwrap();
    session.id
}

pub(crate) fn prompt(
    harness: &Harness,
    session: &SessionId,
    text: &str,
    delivery: Delivery,
) -> PromptInput {
    PromptInput {
        session_id: session.clone(),
        directory: harness.project.clone(),
        text: text.to_owned(),
        attachments: Vec::new(),
        agent: None,
        model: None,
        delivery,
    }
}

pub(crate) async fn wait_idle(events: &mut broadcast::Receiver<EngineEvent>, session: &SessionId) {
    let settled = async {
        loop {
            match events.recv().await {
                Ok(EngineEvent::Status {
                    session: id,
                    status: RunStatus::Idle,
                }) if &id == session => return,
                Ok(EngineEvent::PermissionAsked(ask)) => {
                    panic!("unexpected permission request for {}", ask.permission)
                },
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {},
                Err(broadcast::error::RecvError::Closed) => panic!("the event feed closed"),
            }
        }
    };
    tokio::time::timeout(SETTLE, settled)
        .await
        .expect("the run did not settle");
}

pub(crate) async fn wait_idle_answering<F>(
    events: &mut broadcast::Receiver<EngineEvent>,
    session: &SessionId,
    mut answer: F,
) -> usize
where
    F: FnMut(&PermissionAsk),
{
    let settled = async {
        let mut asked = 0;
        loop {
            match events.recv().await {
                Ok(EngineEvent::PermissionAsked(ask)) => {
                    asked += 1;
                    answer(&ask);
                },
                Ok(EngineEvent::Status {
                    session: id,
                    status: RunStatus::Idle,
                }) if &id == session => return asked,
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {},
                Err(broadcast::error::RecvError::Closed) => panic!("the event feed closed"),
            }
        }
    };
    tokio::time::timeout(SETTLE, settled)
        .await
        .expect("the run did not settle")
}

pub(crate) fn texts(message: &MessageWithParts) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match &part.body {
            PartBody::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

pub(crate) fn tool_state(messages: &[MessageWithParts]) -> ToolState {
    messages
        .iter()
        .flat_map(|message| message.parts.iter())
        .find_map(|part| match &part.body {
            PartBody::Tool { state, .. } => Some((**state).clone()),
            _ => None,
        })
        .expect("a tool part")
}

pub(crate) fn tool_names(request: &Value) -> Vec<String> {
    request["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|tool| tool["function"]["name"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}
