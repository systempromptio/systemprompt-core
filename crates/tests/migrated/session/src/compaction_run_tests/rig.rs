//! An engine whose model limits the test chooses, served either by the
//! scripted instance or, for streams the scripted instance cannot produce, by
//! a loopback endpoint that answers each request with a prepared event stream.

use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{Value, json};
use systemprompt_engine::config::{Layout, Paths};
use systemprompt_engine::llm::{HttpTransport, Supports};
use systemprompt_engine::mcp::ClientInfo;
use systemprompt_engine::run::{
    Delivery, EnabledExtensions, Endpoint, Engine, EngineOptions, ExtensionSet, GovernError,
    Governor, McpSpec, ModelCall, ModelSpec, PromptInput, Protocol, Purpose, ResponseAction,
    ToolCall, Usage, Verdict,
};
use systemprompt_engine::store::{MessageWithParts, SessionId, Store};
use systemprompt_scripted_instance::{
    ModelFixture, ModelScript, RecordedBody, Route, ScriptedInstance, Turn,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use url::Url;

use crate::support::MODEL;

/// The limits and abilities the one model on offer states.
#[derive(Debug, Clone, Copy)]
pub(super) struct Model {
    pub(super) window: u32,
    pub(super) max_output: u32,
    pub(super) images: bool,
}

impl Model {
    pub(super) const fn sized(window: u32, max_output: u32) -> Self {
        Self {
            window,
            max_output,
            images: false,
        }
    }
}

struct RigGovernor {
    base: Url,
    token: String,
    model: Model,
    purposes: Mutex<Vec<Purpose>>,
}

#[async_trait]
impl Governor for RigGovernor {
    fn models(&self) -> Vec<ModelSpec> {
        vec![ModelSpec {
            id: MODEL.to_owned(),
            name: "Sized model".to_owned(),
            protocol: Protocol::Messages,
            context_window: self.model.window,
            max_output: self.model.max_output,
            supports: Supports {
                tool_calls: true,
                images: self.model.images,
                ..Supports::default()
            },
        }]
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        Vec::new()
    }

    async fn authorize(&self, call: &ModelCall) -> Result<Endpoint, GovernError> {
        self.purposes.lock().unwrap().push(call.purpose);
        let mut endpoint = Endpoint::new(self.base.clone());
        endpoint
            .headers
            .push(("authorization".to_owned(), format!("Bearer {}", self.token)));
        Ok(endpoint)
    }

    async fn response(&self, _call: &ModelCall, _status: u16, _attempt: u32) -> ResponseAction {
        ResponseAction::Continue
    }

    async fn step_finished(&self, _call: &ModelCall, _usage: &Usage) {}

    async fn tool_before(&self, _call: &ToolCall) -> Verdict {
        Verdict::Allow
    }

    fn extensions(&self) -> Option<EnabledExtensions> {
        None
    }

    fn generation(&self) -> u64 {
        1
    }
}

pub(super) struct Rig {
    _dir: tempfile::TempDir,
    instance: Option<ScriptedInstance>,
    canned: Option<Canned>,
    governor: Arc<RigGovernor>,
    pub(super) engine: Engine,
    pub(super) project: PathBuf,
}

impl Rig {
    /// An engine whose model calls the scripted instance answers.
    pub(super) async fn scripted(turns: Vec<Turn>, model: Model) -> Self {
        let instance = ScriptedInstance::builder()
            .models([ModelFixture::messages(MODEL)])
            .script(ModelScript::from_turns(turns))
            .start()
            .await
            .unwrap();
        let base =
            Url::parse(&format!("{}/v1", instance.base_url().trim_end_matches('/'))).unwrap();
        let token = instance.pat().as_str().to_owned();
        let mut rig = Self::over(base, token, model);
        rig.instance = Some(instance);
        rig
    }

    /// An engine whose model calls are answered, in order, with the given
    /// event streams.
    pub(super) async fn canned(replies: Vec<String>, model: Model) -> Self {
        let canned = Canned::start(replies).await;
        let mut rig = Self::over(canned.base.clone(), "canned".to_owned(), model);
        rig.canned = Some(canned);
        rig
    }

    fn over(base: Url, token: String, model: Model) -> Self {
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
        let governor = Arc::new(RigGovernor {
            base,
            token,
            model,
            purposes: Mutex::new(Vec::new()),
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
            instance: None,
            canned: None,
            governor,
            engine,
            project,
        }
    }

    /// A session with a title already set, so no title call is made.
    pub(super) async fn session(&self) -> SessionId {
        let session = self.engine.create(&self.project).await.unwrap();
        self.engine
            .store()
            .set_title(&session.id, "test session")
            .unwrap();
        session.id
    }

    pub(super) fn prompt(&self, session: &SessionId, text: &str) -> PromptInput {
        PromptInput {
            session_id: session.clone(),
            directory: self.project.clone(),
            text: text.to_owned(),
            attachments: Vec::new(),
            agent: None,
            model: None,
            delivery: Delivery::Steer,
        }
    }

    pub(super) async fn messages(&self, session: &SessionId) -> Vec<MessageWithParts> {
        self.engine.messages(session).await.unwrap()
    }

    /// Model requests received, oldest first, as JSON bodies.
    pub(super) fn bodies(&self) -> Vec<Value> {
        if let Some(canned) = &self.canned {
            return canned.seen.lock().unwrap().clone();
        }
        self.instance
            .iter()
            .flat_map(ScriptedInstance::requests)
            .filter(|record| record.route == Route::Messages)
            .filter_map(|record| match record.body {
                RecordedBody::Json(value) => Some(value),
                _ => None,
            })
            .collect()
    }

    pub(super) fn purposes(&self) -> Vec<Purpose> {
        self.governor.purposes.lock().unwrap().clone()
    }

    pub(super) async fn finish(self) {
        self.engine.shutdown().await;
        if let Some(instance) = self.instance {
            instance.shutdown().await.unwrap();
        }
        if let Some(canned) = self.canned {
            canned.server.abort();
        }
    }
}

/// A loopback model endpoint: every request is recorded and answered with the
/// next prepared event stream.
struct Canned {
    base: Url,
    seen: Arc<Mutex<Vec<Value>>>,
    server: JoinHandle<()>,
}

impl Canned {
    async fn start(replies: Vec<String>) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);
        let server = tokio::spawn(async move {
            let mut replies = replies.into_iter();
            while let Ok((mut socket, _)) = listener.accept().await {
                let Some(body) = read_body(&mut socket).await else {
                    continue;
                };
                recorded.lock().unwrap().push(body);
                let reply = replies.next().unwrap_or_default();
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: \
                     {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                if socket.write_all(response.as_bytes()).await.is_ok() {
                    socket.shutdown().await.ok();
                }
            }
        });
        Self {
            base: Url::parse(&format!("http://127.0.0.1:{port}/v1")).unwrap(),
            seen,
            server,
        }
    }
}

// JSON: a provider request body, recorded as sent.
async fn read_body(socket: &mut TcpStream) -> Option<Value> {
    let mut received = Vec::new();
    let mut chunk = vec![0_u8; 8192];
    loop {
        if let Some(body) = body_of(&received) {
            return serde_json::from_slice(body).ok();
        }
        let read = socket.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        received.extend_from_slice(&chunk[..read]);
    }
}

fn body_of(received: &[u8]) -> Option<&[u8]> {
    let end = received
        .windows(4)
        .position(|window| window == b"\r\n\r\n")?
        + 4;
    let head = String::from_utf8_lossy(&received[..end]).to_ascii_lowercase();
    let length: usize = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))?
        .trim()
        .parse()
        .ok()?;
    received.get(end..end + length)
}

// JSON: provider stream events in the Messages wire shape; no engine type
// encodes them.
fn stream(events: &[Value]) -> String {
    let mut out = String::new();
    for event in events {
        let name = event["type"].as_str().unwrap_or("ping");
        write!(out, "event: {name}\ndata: {event}\n\n").unwrap();
    }
    out
}

fn opening() -> Value {
    json!({"type": "message_start", "message": {"usage": {"input_tokens": 10, "output_tokens": 0}}})
}

fn text_block(text: &str) -> [Value; 2] {
    [
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": text}}),
    ]
}

/// A complete text reply as a Messages event stream.
pub(super) fn text_stream(text: &str) -> String {
    let [start, delta] = text_block(text);
    stream(&[
        opening(),
        start,
        delta,
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 5}}),
        json!({"type": "message_stop"}),
    ])
}

/// A stream that reports the prompt as too long, after `partial` text when
/// some is given.
pub(super) fn overflow_stream(partial: Option<&str>) -> String {
    let mut events = vec![opening()];
    if let Some(text) = partial {
        events.extend(text_block(text));
    }
    events.push(json!({"type": "error", "error": {
        "type": "invalid_request_error",
        "message": "prompt is too long: 250000 tokens > 200000 maximum"
    }}));
    stream(&events)
}
