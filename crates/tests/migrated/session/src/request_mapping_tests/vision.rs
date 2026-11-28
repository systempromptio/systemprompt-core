//! An engine against the scripted instance whose one model accepts images, so
//! an image reaches the request as an image block and not as an omission note.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use systemprompt_engine::config::{Layout, Paths};
use systemprompt_engine::llm::{HttpTransport, Supports};
use systemprompt_engine::mcp::ClientInfo;
use systemprompt_engine::run::{
    AttachmentInput, Delivery, EnabledExtensions, Endpoint, Engine, EngineOptions, ExtensionSet,
    GovernError, Governor, McpSpec, ModelCall, ModelSpec, PromptInput, Protocol, ResponseAction,
    ToolCall, Usage, Verdict,
};
use systemprompt_engine::store::{SessionId, Store};
use systemprompt_scripted_instance::{
    ModelFixture, ModelScript, RecordedBody, Route, ScriptedInstance, Turn,
};
use url::Url;

use crate::support::{MODEL, settle};

struct VisionGovernor {
    base: Url,
    token: String,
}

#[async_trait]
impl Governor for VisionGovernor {
    fn models(&self) -> Vec<ModelSpec> {
        vec![ModelSpec {
            id: MODEL.to_owned(),
            name: "Scripted vision model".to_owned(),
            protocol: Protocol::Messages,
            context_window: 200_000,
            max_output: 8_192,
            supports: Supports {
                images: true,
                tool_calls: true,
                ..Supports::default()
            },
        }]
    }

    fn mcp_servers(&self) -> Vec<McpSpec> {
        Vec::new()
    }

    async fn authorize(&self, _call: &ModelCall) -> Result<Endpoint, GovernError> {
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

pub(super) struct Vision {
    _dir: tempfile::TempDir,
    instance: ScriptedInstance,
    pub(super) engine: Engine,
    pub(super) project: PathBuf,
}

impl Vision {
    pub(super) async fn start(turns: Vec<Turn>) -> Self {
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
        let base =
            Url::parse(&format!("{}/v1", instance.base_url().trim_end_matches('/'))).unwrap();
        let governor = Arc::new(VisionGovernor {
            base,
            token: instance.pat().as_str().to_owned(),
        });
        let engine = Engine::new(EngineOptions {
            paths,
            layout: Layout::default(),
            store: Store::open_in_memory().unwrap(),
            governor,
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
            engine,
            project,
        }
    }

    /// A session with a title already set, so no title call takes a turn.
    pub(super) async fn session(&self) -> SessionId {
        let session = self.engine.create(&self.project).await.unwrap();
        self.engine
            .store()
            .set_title(&session.id, "test session")
            .unwrap();
        session.id
    }

    pub(super) fn prompt(
        &self,
        session: &SessionId,
        text: &str,
        attachments: Vec<AttachmentInput>,
    ) -> PromptInput {
        PromptInput {
            session_id: session.clone(),
            directory: self.project.clone(),
            text: text.to_owned(),
            attachments,
            agent: None,
            model: None,
            delivery: Delivery::Steer,
        }
    }

    /// Sends a prompt, waits for the run to end and returns the conversation
    /// turns of the last model request.
    // JSON: the `messages` array of the request body the instance recorded.
    pub(super) async fn ask(
        &self,
        session: &SessionId,
        text: &str,
        attachments: Vec<AttachmentInput>,
    ) -> Value {
        let mut events = self.engine.subscribe();
        self.engine
            .prompt(self.prompt(session, text, attachments))
            .await
            .unwrap();
        settle(&mut events, session).await;
        self.instance
            .requests()
            .into_iter()
            .filter(|record| record.route == Route::Messages)
            .filter_map(|record| match record.body {
                RecordedBody::Json(value) => Some(value),
                _ => None,
            })
            .next_back()
            .expect("a model request was sent")["messages"]
            .clone()
    }

    pub(super) async fn finish(self) {
        self.engine.shutdown().await;
        self.instance.shutdown().await.unwrap();
    }
}
