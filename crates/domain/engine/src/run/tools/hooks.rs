//! What tools call back into: the todo store, file write notifications (format,
//! then diagnostics) and the end of background shell jobs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::lsp::{DiagnosticFormat, DiagnosticScope, LspManager, Severity, format_diagnostics};
use crate::store::{SessionId, Todo, TodoPriority, TodoStatus};
use crate::tools::{FileObserver, JobListener, JobReport, TodoItem, TodoSink, ToolError};
use crate::workspace::Workspace;
use async_trait::async_trait;

use crate::run::db::Db;
use crate::run::inner::Inner;
use crate::run::session::job_notice;

const DIAGNOSTICS_WAIT: Duration = Duration::from_secs(5);

pub(crate) struct StoreTodos {
    db: Db,
}

impl StoreTodos {
    pub(crate) const fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TodoSink for StoreTodos {
    async fn replace(&self, session_id: &SessionId, todos: Vec<TodoItem>) -> Result<(), ToolError> {
        let id = session_id.clone();
        let items: Vec<Todo> = todos.into_iter().map(stored_todo).collect();
        self.db
            .run(move |store| store.replace_todos(&id, &items))
            .await
            .map_err(|error| ToolError::caused("could not save the todo list", error))
    }
}

fn stored_todo(todo: TodoItem) -> Todo {
    let status = match todo.status {
        crate::tools::TodoStatus::Pending => TodoStatus::Pending,
        crate::tools::TodoStatus::InProgress => TodoStatus::InProgress,
        crate::tools::TodoStatus::Completed => TodoStatus::Completed,
        crate::tools::TodoStatus::Cancelled => TodoStatus::Cancelled,
    };
    let priority = match todo.priority {
        Some(crate::tools::TodoPriority::High) => TodoPriority::High,
        Some(crate::tools::TodoPriority::Low) => TodoPriority::Low,
        Some(crate::tools::TodoPriority::Medium) | None => TodoPriority::Medium,
    };
    Todo {
        content: todo.content,
        status,
        priority,
    }
}

pub(crate) struct Observer {
    workspace: Workspace,
    lsp: Arc<LspManager>,
    worktree: PathBuf,
}

impl Observer {
    pub(crate) const fn new(workspace: Workspace, lsp: Arc<LspManager>, worktree: PathBuf) -> Self {
        Self {
            workspace,
            lsp,
            worktree,
        }
    }
}

#[async_trait]
impl FileObserver for Observer {
    async fn before_write(&self, _path: &Path) {}

    async fn after_write(&self, path: &Path) -> Option<String> {
        if path.exists()
            && let Err(error) = self.workspace.format(path).await
        {
            tracing::debug!(path = %path.display(), error = %error, "formatting failed");
        }
        if !self.lsp.handles(path) {
            return None;
        }
        match tokio::time::timeout(DIAGNOSTICS_WAIT, self.lsp.touch(path, true)).await {
            Ok(Ok(())) => {},
            Ok(Err(error)) => {
                let path = path.display();
                tracing::debug!(path = %path, error = %error, "language server sync failed");
            },
            Err(_) => {
                tracing::debug!(path = %path.display(), "language server diagnostics timed out");
            },
        }
        let found = self.lsp.diagnostics(DiagnosticScope::File(path));
        if found.is_empty() {
            return None;
        }
        let format = DiagnosticFormat {
            base: Some(self.worktree.clone()),
            min_severity: Severity::Error,
            ..DiagnosticFormat::default()
        };
        let text = format_diagnostics(&found, &format);
        (!text.trim().is_empty()).then_some(text)
    }
}

pub(crate) struct JobNotices {
    inner: Weak<Inner>,
}

impl JobNotices {
    pub(crate) const fn new(inner: Weak<Inner>) -> Self {
        Self { inner }
    }
}

impl JobListener for JobNotices {
    fn finished(&self, report: &JobReport) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        if tokio::runtime::Handle::try_current().is_err() {
            tracing::debug!(job = %report.job.id, "background job ended outside the runtime");
            return;
        }
        let engine = Arc::clone(&inner);
        let report = report.clone();
        inner.tasks.spawn("background job notice", async move {
            let job = report.job.id.clone();
            if let Err(error) = job_notice::job_finished(&engine, report).await {
                let error = error.describe();
                tracing::warn!(job = %job, error = %error, "background job notice not posted");
            }
        });
    }
}
