//! An engine driven against the scripted instance: a Governor that hands out
//! the instance's model endpoint, a temporary project, prompt helpers, waits
//! on the engine's event feed, and readers over stored messages.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use systemprompt_engine::config::{Layout, Paths};
use systemprompt_engine::llm::{HttpTransport, Supports};
use systemprompt_engine::mcp::ClientInfo;
use systemprompt_engine::run::{
    Delivery, EnabledExtensions, Endpoint, Engine, EngineEvent, EngineOptions, ExtensionSet,
    GovernError, Governor, McpSpec, ModelCall, ModelSpec, PermissionAsk, PromptInput, Protocol,
    Purpose, ResponseAction, RunStatus, SessionFailure, ToolCall, Usage, Verdict,
};
use systemprompt_engine::store::{MessageWithParts, PartBody, SessionId, Store, ToolState};
use systemprompt_scripted_instance::{
    Expect, ModelFixture, ModelScript, Respond, ScriptedInstance, StopReason, ToolCall as Call,
    Turn, TurnFault, Usage as TurnUsage,
};
use tokio::sync::broadcast;
use url::Url;

pub(crate) const MODEL: &str = "scripted-model";
const SETTLE: Duration = Duration::from_secs(60);

/// Governs every call to the scripted instance: one model on the Messages
/// protocol, a bearer token, and an optional verdict and context window.
pub(crate) struct ScriptedGovernor {
    base: Url,
    token: String,
    pub(crate) deny: Option<String>,
    pub(crate) context_window: u32,
    pub(crate) purposes: parking::Purposes,
    pub(crate) authorized: AtomicU32,
    title_first: bool,
    title_done: std::sync::atomic::AtomicBool,
}

pub(crate) mod parking {
    use std::sync::Mutex;

    use systemprompt_engine::run::Purpose;

    #[derive(Debug, Default)]
    pub(crate) struct Purposes(Mutex<Vec<Purpose>>);

    impl Purposes {
        pub(crate) fn push(&self, purpose: Purpose) {
            self.0.lock().unwrap().push(purpose);
        }

        pub(crate) fn all(&self) -> Vec<Purpose> {
            self.0.lock().unwrap().clone()
        }
    }
}

#[async_trait]
impl Governor for ScriptedGovernor {
    fn models(&self) -> Vec<ModelSpec> {
        vec![ModelSpec {
            id: MODEL.to_owned(),
            name: "Scripted model".to_owned(),
            protocol: Protocol::Messages,
            context_window: self.context_window,
            max_output: 8_192,
            supports: Supports {
                tool_calls: true,
                ..Supports::default()
            },
        }]
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        Vec::new()
    }

    async fn authorize(&self, call: &ModelCall) -> Result<Endpoint, GovernError> {
        if self.title_first && call.purpose == Purpose::Step {
            for _ in 0..250 {
                if self.title_done.load(Ordering::SeqCst) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }
        self.purposes.push(call.purpose);
        self.authorized.fetch_add(1, Ordering::SeqCst);
        let mut endpoint = Endpoint::new(self.base.clone());
        endpoint
            .headers
            .push(("authorization".to_owned(), format!("Bearer {}", self.token)));
        Ok(endpoint)
    }

    async fn response(&self, _call: &ModelCall, _status: u16, _attempt: u32) -> ResponseAction {
        ResponseAction::Continue
    }

    async fn step_finished(&self, call: &ModelCall, _usage: &Usage) {
        if call.purpose == Purpose::Title {
            self.title_done.store(true, Ordering::SeqCst);
        }
    }

    async fn tool_before(&self, _call: &ToolCall) -> Verdict {
        self.deny
            .as_ref()
            .map_or(Verdict::Allow, |reason| Verdict::Deny {
                reason: reason.clone(),
            })
    }

    fn extensions(&self) -> Option<EnabledExtensions> {
        None
    }

    fn generation(&self) -> u64 {
        1
    }
}

/// How a scripted engine is set up beyond its model script.
#[derive(Default)]
pub(crate) struct Setup {
    pub(crate) settings: Option<String>,
    pub(crate) deny: Option<String>,
    pub(crate) context_window: Option<u32>,
    /// Holds the first step until the title call has been answered, so the
    /// title call takes the first scripted turn.
    pub(crate) title_first: bool,
}

pub(crate) struct Scripted {
    _dir: tempfile::TempDir,
    pub(crate) instance: ScriptedInstance,
    pub(crate) governor: Arc<ScriptedGovernor>,
    pub(crate) engine: Engine,
    pub(crate) project: PathBuf,
}

impl Scripted {
    pub(crate) async fn start(turns: Vec<Turn>) -> Self {
        Self::with(turns, Setup::default()).await
    }

    pub(crate) async fn with(turns: Vec<Turn>, setup: Setup) -> Self {
        let instance = ScriptedInstance::builder()
            .models([ModelFixture::messages(MODEL)])
            .script(ModelScript::from_turns(turns))
            .start()
            .await
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
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
        if let Some(settings) = &setup.settings {
            std::fs::write(project.join("harness.json"), settings).unwrap();
        }
        let base =
            Url::parse(&format!("{}/v1", instance.base_url().trim_end_matches('/'))).unwrap();
        let governor = Arc::new(ScriptedGovernor {
            base,
            token: instance.pat().as_str().to_owned(),
            deny: setup.deny,
            context_window: setup.context_window.unwrap_or(200_000),
            purposes: parking::Purposes::default(),
            authorized: AtomicU32::new(0),
            title_first: setup.title_first,
            title_done: std::sync::atomic::AtomicBool::new(false),
        });
        let engine = Engine::new(EngineOptions {
            paths,
            layout: Layout::default(),
            store: Store::open_in_memory().unwrap(),
            governor: Arc::clone(&governor) as Arc<dyn Governor>,
            transport: Arc::new(HttpTransport::new().unwrap()),
            client_info: ClientInfo::new("migrated-session-tests", "0"),
            search_path: Some(OsString::new()),
            path_ext: None,
            extensions: ExtensionSet::default(),
            lsp_client_name: None,
        })
        .unwrap();
        Self {
            _dir: dir,
            instance,
            governor,
            engine,
            project,
        }
    }

    /// A session with no title, so the first prompt asks for one.
    pub(crate) async fn untitled(&self) -> SessionId {
        self.engine.create(&self.project).await.unwrap().id
    }

    /// A session with a title already set, so no title call takes a turn.
    pub(crate) async fn session(&self) -> SessionId {
        let session = self.engine.create(&self.project).await.unwrap();
        self.engine
            .store()
            .set_title(&session.id, "test session")
            .unwrap();
        session.id
    }

    pub(crate) fn prompt(
        &self,
        session: &SessionId,
        text: &str,
        delivery: Delivery,
    ) -> PromptInput {
        PromptInput {
            session_id: session.clone(),
            directory: self.project.clone(),
            text: text.to_owned(),
            attachments: Vec::new(),
            agent: None,
            model: None,
            delivery,
        }
    }

    pub(crate) async fn messages(&self, session: &SessionId) -> Vec<MessageWithParts> {
        self.engine.messages(session).await.unwrap()
    }

    /// Model requests the instance answered, oldest first, as JSON bodies.
    pub(crate) fn model_bodies(&self) -> Vec<Value> {
        self.instance
            .requests()
            .into_iter()
            .filter(|record| record.route == systemprompt_scripted_instance::Route::Messages)
            .filter_map(|record| match record.body {
                systemprompt_scripted_instance::RecordedBody::Json(value) => Some(value),
                _ => None,
            })
            .collect()
    }

    pub(crate) async fn finish(self) {
        self.engine.shutdown().await;
        self.instance.shutdown().await.unwrap();
    }
}

pub(crate) fn text_turn(text: &str) -> Turn {
    Turn {
        expect: Expect::default(),
        respond: vec![Respond::Text(text.to_owned())],
        stop: Some(StopReason::End),
        usage: Some(TurnUsage {
            input: 10,
            output: 5,
        }),
        fault: None,
    }
}

pub(crate) fn tool_turn(id: &str, name: &str, input: Value) -> Turn {
    Turn {
        expect: Expect::default(),
        respond: vec![Respond::ToolCall(Call {
            id: id.to_owned(),
            name: name.to_owned(),
            input,
            argument_chunks: Vec::new(),
        })],
        stop: Some(StopReason::ToolUse),
        usage: Some(TurnUsage {
            input: 10,
            output: 5,
        }),
        fault: None,
    }
}

pub(crate) fn read_turn(id: &str) -> Turn {
    tool_turn(id, "read", json!({"path": "notes.txt"}))
}

pub(crate) fn fault(fault: TurnFault) -> Turn {
    Turn {
        fault: Some(fault),
        ..Turn::default()
    }
}

pub(crate) fn stalled(mut turn: Turn, ms: u64) -> Turn {
    turn.fault = Some(TurnFault::StallMs(ms));
    turn
}

pub(crate) fn with_usage(mut turn: Turn, input: u32, output: u32) -> Turn {
    turn.usage = Some(TurnUsage { input, output });
    turn
}

/// What a run reported until its session went idle.
#[derive(Debug, Default)]
pub(crate) struct Settled {
    pub(crate) failures: Vec<SessionFailure>,
    pub(crate) retries: Vec<u32>,
    pub(crate) asks: Vec<PermissionAsk>,
}

pub(crate) async fn settle(
    events: &mut broadcast::Receiver<EngineEvent>,
    session: &SessionId,
) -> Settled {
    settle_answering(events, session, |_| None).await
}

/// Waits for the session to go idle, answering permission requests with the
/// reply `answer` gives (none leaves the request pending).
pub(crate) async fn settle_answering<F>(
    events: &mut broadcast::Receiver<EngineEvent>,
    session: &SessionId,
    mut answer: F,
) -> Settled
where
    F: FnMut(&PermissionAsk) -> Option<()>,
{
    let mut settled = Settled::default();
    let wait = async {
        loop {
            match events.recv().await {
                Ok(EngineEvent::Status {
                    session: id,
                    status,
                }) if &id == session => match status {
                    RunStatus::Idle => return,
                    RunStatus::Retry { attempt, .. } => settled.retries.push(attempt),
                    RunStatus::Busy => {},
                },
                Ok(EngineEvent::SessionError { session: id, error }) if &id == session => {
                    settled.failures.push(error);
                },
                Ok(EngineEvent::PermissionAsked(ask)) => {
                    let _ = answer(&ask);
                    settled.asks.push(*ask);
                },
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {},
                Err(broadcast::error::RecvError::Closed) => panic!("the event feed closed"),
            }
        }
    };
    tokio::time::timeout(SETTLE, wait)
        .await
        .expect("the run did not settle");
    settled
}

pub(crate) async fn until<F>(
    events: &mut broadcast::Receiver<EngineEvent>,
    mut pick: F,
) -> EngineEvent
where
    F: FnMut(&EngineEvent) -> bool,
{
    let wait = async {
        loop {
            match events.recv().await {
                Ok(event) if pick(&event) => return event,
                Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => {},
                Err(broadcast::error::RecvError::Closed) => panic!("the event feed closed"),
            }
        }
    };
    tokio::time::timeout(SETTLE, wait)
        .await
        .expect("the awaited event never came")
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

pub(crate) fn user_texts(messages: &[MessageWithParts]) -> Vec<String> {
    messages
        .iter()
        .filter(|message| message.message.as_user().is_some_and(|user| !user.system))
        .map(texts)
        .collect()
}

pub(crate) fn tool_states(messages: &[MessageWithParts]) -> Vec<ToolState> {
    messages
        .iter()
        .flat_map(|message| message.parts.iter())
        .filter_map(|part| match &part.body {
            PartBody::Tool { state, .. } => Some((**state).clone()),
            _ => None,
        })
        .collect()
}

pub(crate) fn purposes(scripted: &Scripted) -> Vec<Purpose> {
    scripted.governor.purposes.all()
}
