use systemprompt_identifiers::{ClientId, ContextId, SessionId, TaskId, TraceId, UserId};
use tracing::Span;

pub struct RequestSpan(Span);

impl std::fmt::Debug for RequestSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RequestSpan").finish()
    }
}

impl RequestSpan {
    pub fn new(user_id: &UserId, session_id: &SessionId, trace_id: &TraceId) -> Self {
        let span = tracing::info_span!(
            "request",
            user_id = %user_id.as_str(),
            session_id = %session_id.as_str(),
            trace_id = %trace_id.as_str(),
            context_id = tracing::field::Empty,
            task_id = tracing::field::Empty,
            client_id = tracing::field::Empty,
        );

        Self(span)
    }

    pub fn enter(&self) -> tracing::span::EnteredSpan {
        self.0.clone().entered()
    }

    pub fn record_task_id(&self, task_id: &TaskId) {
        self.0.record("task_id", task_id.as_str());
    }

    pub fn record_context_id(&self, context_id: &ContextId) {
        self.0.record("context_id", context_id.as_str());
    }

    pub fn record_client_id(&self, client_id: &ClientId) {
        self.0.record("client_id", client_id.as_str());
    }

    pub const fn span(&self) -> &Span {
        &self.0
    }
}

pub struct SystemSpan(Span);

impl std::fmt::Debug for SystemSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SystemSpan").finish()
    }
}

impl SystemSpan {
    pub fn new(component: &str) -> Self {
        Self(tracing::info_span!(
            "system",
            user_id = "system",
            session_id = "system",
            trace_id = %TraceId::generate().as_str(),
            client_id = %format!("system:{component}"),
            context_id = tracing::field::Empty,
            task_id = tracing::field::Empty,
        ))
    }

    pub fn enter(&self) -> tracing::span::EnteredSpan {
        self.0.clone().entered()
    }

    pub fn record_task_id(&self, task_id: &TaskId) {
        self.0.record("task_id", task_id.as_str());
    }

    pub fn record_context_id(&self, context_id: &ContextId) {
        self.0.record("context_id", context_id.as_str());
    }

    pub const fn span(&self) -> &Span {
        &self.0
    }

    pub fn into_span(self) -> Span {
        self.0
    }
}

impl From<SystemSpan> for Span {
    fn from(system_span: SystemSpan) -> Self {
        system_span.0
    }
}

pub struct RequestSpanBuilder<'a> {
    user: &'a UserId,
    session: &'a SessionId,
    trace: &'a TraceId,
    context: Option<&'a ContextId>,
    task: Option<&'a TaskId>,
    client: Option<&'a ClientId>,
}

impl std::fmt::Debug for RequestSpanBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RequestSpanBuilder").finish_non_exhaustive()
    }
}

impl<'a> RequestSpanBuilder<'a> {
    pub const fn new(
        user_id: &'a UserId,
        session_id: &'a SessionId,
        trace_id: &'a TraceId,
    ) -> Self {
        Self {
            user: user_id,
            session: session_id,
            trace: trace_id,
            context: None,
            task: None,
            client: None,
        }
    }

    #[must_use]
    pub fn with_context_id(mut self, context_id: &'a ContextId) -> Self {
        if !context_id.as_str().is_empty() {
            self.context = Some(context_id);
        }
        self
    }

    #[must_use]
    pub const fn with_task_id(mut self, task_id: &'a TaskId) -> Self {
        self.task = Some(task_id);
        self
    }

    #[must_use]
    pub const fn with_client_id(mut self, client_id: &'a ClientId) -> Self {
        self.client = Some(client_id);
        self
    }

    pub fn build(self) -> RequestSpan {
        let span = RequestSpan::new(self.user, self.session, self.trace);

        if let Some(context_id) = self.context {
            span.record_context_id(context_id);
        }
        if let Some(task_id) = self.task {
            span.record_task_id(task_id);
        }
        if let Some(client_id) = self.client {
            span.record_client_id(client_id);
        }

        span
    }
}
