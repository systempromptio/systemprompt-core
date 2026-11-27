use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use async_trait::async_trait;
use harness_engine::extension::{PromptContext, PromptSection};
use harness_engine::{
    CallAugmenter, EngineExtension, ExtensionMetadata, ExtensionSet, ExtensionTool,
    GovernorObserver, ModelCall, ResponseHead, Review, SessionEvent, ToolCall, ToolOutcome,
    ToolReviewer, UserPrompt,
};
use harness_ids::ExtensionId;
use harness_tools::{Tool, ToolContext, ToolError, ToolOutput};
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{Scripted, text_reply, tool_reply};

pub(crate) fn read_then(reply: &str) -> Scripted {
    Scripted::with(vec![
        (
            200,
            tool_reply("call_1", "read", &json!({"path": "notes.txt"})),
        ),
        (200, text_reply(reply)),
    ])
}

pub(crate) struct Probe {
    metadata: ExtensionMetadata,
    tool: Option<&'static str>,
    switch: Option<Arc<Switch>>,
    section: Option<(&'static str, &'static str)>,
    observer: Option<Arc<Recorder>>,
    reviewer: Option<Arc<Reviewing>>,
    headers: Vec<(String, String)>,
}

impl Probe {
    pub(crate) fn new(id: &str) -> Self {
        Self {
            metadata: ExtensionMetadata::new(ExtensionId::parse(id).unwrap(), "probe"),
            tool: None,
            switch: None,
            section: None,
            observer: None,
            reviewer: None,
            headers: Vec::new(),
        }
    }

    pub(crate) fn required(mut self) -> Self {
        self.metadata = self.metadata.mark_required();
        self
    }

    pub(crate) fn tool(mut self, name: &'static str) -> Self {
        self.tool = Some(name);
        self
    }

    pub(crate) fn switched(mut self, switch: &Arc<Switch>) -> Self {
        self.switch = Some(Arc::clone(switch));
        self
    }

    pub(crate) fn section(mut self, heading: &'static str, text: &'static str) -> Self {
        self.section = Some((heading, text));
        self
    }

    pub(crate) fn observing(mut self, recorder: &Arc<Recorder>) -> Self {
        self.observer = Some(Arc::clone(recorder));
        self
    }

    pub(crate) fn reviewing(mut self, reviewer: &Arc<Reviewing>) -> Self {
        self.reviewer = Some(Arc::clone(reviewer));
        self
    }

    pub(crate) fn adding(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub(crate) fn set(probes: Vec<Self>) -> ExtensionSet {
        ExtensionSet::resolve(
            probes
                .into_iter()
                .map(|probe| -> Arc<dyn EngineExtension> { Arc::new(probe) }),
        )
        .unwrap()
    }
}

impl EngineExtension for Probe {
    fn metadata(&self) -> ExtensionMetadata {
        self.metadata.clone()
    }

    fn generation(&self) -> u64 {
        self.switch
            .as_ref()
            .map_or(0, |switch| switch.generation.load(Ordering::SeqCst))
    }

    fn tools(&self) -> Vec<ExtensionTool> {
        if self
            .switch
            .as_ref()
            .is_some_and(|switch| !switch.offered.load(Ordering::SeqCst))
        {
            return Vec::new();
        }
        self.tool
            .map(|name| ExtensionTool::gated(Arc::new(Echo(name)), name))
            .into_iter()
            .collect()
    }

    fn prompt_sections(&self, _step: &PromptContext<'_>) -> Vec<PromptSection> {
        self.section
            .map(|(heading, text)| PromptSection {
                heading: heading.to_owned(),
                text: text.to_owned(),
            })
            .into_iter()
            .collect()
    }

    fn observer(&self) -> Option<Arc<dyn GovernorObserver>> {
        self.observer
            .clone()
            .map(|recorder| -> Arc<dyn GovernorObserver> { recorder })
    }

    fn reviewer(&self) -> Option<Arc<dyn ToolReviewer>> {
        self.reviewer
            .clone()
            .map(|reviewer| -> Arc<dyn ToolReviewer> { reviewer })
    }

    fn augmenter(&self) -> Option<Arc<dyn CallAugmenter>> {
        (!self.headers.is_empty())
            .then(|| -> Arc<dyn CallAugmenter> { Arc::new(Headers(self.headers.clone())) })
    }
}

/// Whether a probe offers its tool, and the generation it reports; flipped by
/// a test to stand for an extension whose own state settles later.
#[derive(Default)]
pub(crate) struct Switch {
    offered: AtomicBool,
    generation: AtomicU64,
}

impl Switch {
    pub(crate) fn offer(&self) {
        self.offered.store(true, Ordering::SeqCst);
    }

    pub(crate) fn bump(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

struct Echo(&'static str);

#[async_trait]
impl Tool for Echo {
    fn name(&self) -> &str {
        self.0
    }

    fn description(&self) -> String {
        "Echoes.".to_owned()
    }

    fn schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn run(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        Ok(ToolOutput::new("echo", "echoed"))
    }
}

struct Headers(Vec<(String, String)>);

#[async_trait]
impl CallAugmenter for Headers {
    async fn headers(&self, _call: &ModelCall) -> Vec<(String, String)> {
        self.0.clone()
    }
}

#[derive(Default)]
pub(crate) struct Recorder {
    heard: Mutex<Vec<String>>,
}

impl Recorder {
    pub(crate) fn heard(&self) -> Vec<String> {
        self.heard.lock().clone()
    }

    fn note(&self, line: String) {
        self.heard.lock().push(line);
    }
}

#[async_trait]
impl GovernorObserver for Recorder {
    async fn user_prompt(&self, prompt: &UserPrompt) {
        self.note(format!("prompt:{}", prompt.text));
    }

    async fn tool_after(&self, call: &ToolCall, outcome: &ToolOutcome) {
        let ended = if outcome.error.is_some() {
            "failed"
        } else {
            "ok"
        };
        self.note(format!("tool:{}:{ended}", call.tool));
    }

    fn session_event(&self, event: &SessionEvent) {
        let moment = match event {
            SessionEvent::Created { .. } => "created",
            SessionEvent::Deleted { .. } => "deleted",
            SessionEvent::Idle { .. } => "idle",
        };
        self.note(format!("session:{moment}"));
    }

    async fn response_head(&self, _call: &ModelCall, head: &ResponseHead) {
        let marker = head.header("x-probe-head").unwrap_or("none");
        self.note(format!("head:{}:{marker}", head.status));
    }
}

pub(crate) struct Reviewing {
    review: Review,
    calls: AtomicU32,
}

impl Reviewing {
    pub(crate) fn new(review: Review) -> Self {
        Self {
            review,
            calls: AtomicU32::new(0),
        }
    }

    pub(crate) fn calls(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ToolReviewer for Reviewing {
    async fn review(&self, _call: &ToolCall) -> Review {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.review.clone()
    }
}
