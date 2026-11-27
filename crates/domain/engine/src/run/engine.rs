//! The engine handle the shell holds: construction, the session operations and
//! shutdown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::path::Path;
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::config::{AgentDef, Command, Skill};
use systemprompt_identifiers::engine::{PermissionId, QuestionId};
use crate::llm::ModelSpec;
use crate::mcp::StatusMap;
use crate::store::{MessageId, MessagePage, MessageWithParts, Session, SessionId, Store};
use harness_tasks::CancellationToken;
use crate::workspace::FileDiff;
use tokio::sync::broadcast;

use crate::run::error::EngineError;
use crate::run::events::{EngineEvent, PermissionAsk, QuestionAsk, Reply, RunStatus};
use crate::run::governor::SessionEvent;
use crate::run::inner::{Inner, project_name};
use crate::run::options::EngineOptions;
use crate::run::session::{self, Admitted, CommandInfo, CommandInput, CommandOutcome, PromptInput};

const GENERATION_POLL: Duration = Duration::from_secs(2);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// The agent engine. Cheap to clone; every clone drives the same sessions.
/// Create it inside a tokio runtime and call [`Engine::shutdown`] before the
/// runtime ends.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<Inner>,
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("store", &self.inner.store)
            .finish_non_exhaustive()
    }
}

impl Engine {
    pub fn new(options: EngineOptions) -> Result<Self, EngineError> {
        tokio::runtime::Handle::try_current().map_err(EngineError::NoRuntime)?;
        let inner = Inner::new(options);
        let weak = Arc::downgrade(&inner);
        let poll = move |cancel| poll_generation(weak, cancel);
        inner.tasks.spawn_cancellable("model generation poll", poll);
        tracing::info!(models = inner.models().len(), "engine started");
        Ok(Self { inner })
    }

    pub fn store(&self) -> &Store {
        &self.inner.store
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.inner.events.subscribe()
    }

    pub async fn create(&self, directory: &Path) -> Result<Session, EngineError> {
        session::create(&self.inner, directory, None, None, None).await
    }

    pub async fn list(&self, directory: &Path) -> Result<Vec<Session>, EngineError> {
        let dir = self.inner.dir(directory).await?;
        let root = dir.config().worktree.root.clone();
        let name = project_name(&root);
        let project = self
            .inner
            .db
            .run(move |store| store.ensure_project(&root, &name))
            .await?;
        self.inner
            .db
            .run(move |store| store.root_sessions(&project.id, false))
            .await
    }

    pub async fn get(&self, session: &SessionId) -> Result<Option<Session>, EngineError> {
        let id = session.clone();
        self.inner.db.run(move |store| store.get_session(&id)).await
    }

    pub async fn messages(
        &self,
        session: &SessionId,
    ) -> Result<Vec<MessageWithParts>, EngineError> {
        let id = session.clone();
        self.inner
            .db
            .run(move |store| store.messages(&id, &MessagePage::all()))
            .await
    }

    pub fn status(&self, session: &SessionId) -> RunStatus {
        self.inner.runs.status(session)
    }

    pub async fn prompt(&self, input: PromptInput) -> Result<Admitted, EngineError> {
        session::admit::prompt(&self.inner, input).await
    }

    pub async fn command(&self, input: CommandInput) -> Result<CommandOutcome, EngineError> {
        session::commands::run(&self.inner, input).await
    }

    pub fn abort(&self, session: &SessionId) -> bool {
        self.inner.runs.abort(session)
    }

    pub async fn revert(
        &self,
        session: &SessionId,
        message: &MessageId,
    ) -> Result<Session, EngineError> {
        session::revert::revert(&self.inner, session, message).await
    }

    pub async fn unrevert(&self, session: &SessionId) -> Result<Session, EngineError> {
        session::revert::unrevert(&self.inner, session).await
    }

    pub async fn fork(
        &self,
        session: &SessionId,
        before: Option<&MessageId>,
    ) -> Result<Session, EngineError> {
        let id = session.clone();
        let cut = before.cloned();
        let forked = self
            .inner
            .db
            .run(move |store| store.fork_session(&id, cut.as_ref()))
            .await?;
        self.inner.governed.session_event(&SessionEvent::Created {
            session_id: forked.id.clone(),
            parent_id: None,
            directory: forked.directory.clone(),
        });
        Ok(forked)
    }

    pub async fn delete(&self, session: &SessionId) -> Result<bool, EngineError> {
        session::delete(&self.inner, session).await
    }

    pub async fn compact(&self, session: &SessionId) -> Result<(), EngineError> {
        session::run::start_compaction(&self.inner, session).await
    }

    pub async fn diff(&self, session: &SessionId) -> Result<Vec<FileDiff>, EngineError> {
        session::revert::diff(&self.inner, session).await
    }

    pub fn models(&self) -> Vec<ModelSpec> {
        self.inner.models()
    }

    pub async fn agents(&self, directory: &Path) -> Result<Vec<AgentDef>, EngineError> {
        let config = self.inner.config(directory).await?;
        Ok(config
            .agents
            .values()
            .filter(|agent| !agent.hidden)
            .cloned()
            .collect())
    }

    pub async fn skills(&self, directory: &Path) -> Result<Vec<Skill>, EngineError> {
        let config = self.inner.config(directory).await?;
        Ok(config.skills.values().cloned().collect())
    }

    pub async fn commands(&self, directory: &Path) -> Result<Vec<CommandInfo>, EngineError> {
        let config = self.inner.config(directory).await?;
        let mut list = session::commands::builtin_commands();
        list.extend(
            config
                .commands
                .values()
                .map(|command: &Command| CommandInfo {
                    name: command.name.clone(),
                    description: command.description.clone(),
                    builtin: false,
                }),
        );
        Ok(list)
    }

    pub async fn mcp_status(&self, directory: &Path) -> Result<StatusMap, EngineError> {
        let dir = self.inner.dir(directory).await?;
        Ok(dir.mcp.status())
    }

    pub fn reply_permission(&self, id: &PermissionId, reply: Reply) -> bool {
        self.inner.asks.reply_permission(&self.inner, id, reply)
    }

    pub fn answer_question(&self, id: &QuestionId, answers: Option<Vec<Vec<String>>>) -> bool {
        self.inner.asks.answer_question(&self.inner, id, answers)
    }

    pub fn pending_permissions(&self) -> Vec<PermissionAsk> {
        self.inner.asks.pending_permissions()
    }

    pub fn pending_questions(&self) -> Vec<QuestionAsk> {
        self.inner.asks.pending_questions()
    }

    pub async fn shutdown(&self) {
        let inner = &self.inner;
        inner.closing.cancel();
        inner.runs.abort_all();
        inner.asks.cancel_all();
        let stopped = inner.jobs.shutdown();
        inner.close_dirs().await;
        let report = inner.tasks.shutdown(SHUTDOWN_GRACE).await;
        let drained = report.is_drained();
        tracing::info!(background_jobs = stopped, drained, "engine shut down");
    }
}

async fn poll_generation(engine: Weak<Inner>, cancel: CancellationToken) {
    loop {
        tokio::select! {
            () = cancel.cancelled() => break,
            () = tokio::time::sleep(GENERATION_POLL) => {}
        }
        let Some(inner) = engine.upgrade() else { break };
        inner.check_generation().await;
    }
}
