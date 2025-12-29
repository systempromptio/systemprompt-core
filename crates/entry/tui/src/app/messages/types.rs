use systemprompt_identifiers::ContextId;
use systemprompt_models::a2a::Task;
pub use systemprompt_models::admin::{LogEntry, LogLevel};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollDirection {
    Up,
    #[default]
    Down,
    PageUp,
    PageDown,
    Top,
    Bottom,
}

#[derive(Debug, Clone)]
pub enum ServiceAction {
    Start(String),
    Stop(String),
    Restart(String),
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ContextStreamTaskEvent {
    Created(Task),
    StatusChanged(Task),
    Completed(Task),
}

#[derive(Debug, Clone)]
pub enum ContextLifecycleEvent {
    Created {
        context_id: ContextId,
        name: Option<String>,
    },
    Updated {
        context_id: ContextId,
    },
    Deleted {
        context_id: ContextId,
    },
    AgentChanged {
        context_id: ContextId,
        agent_name: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotData {
    pub context_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncSubcommand {
    All,
    Code,
    Migrate,
    Restart,
}
