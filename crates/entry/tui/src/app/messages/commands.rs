use uuid::Uuid;

use super::types::SyncSubcommand;
use systemprompt_identifiers::{ContextId, UserId};

#[derive(Debug, Clone)]
pub enum Command {
    None,
    Batch(Vec<Self>),

    SendAiMessage(String),
    CancelAiStream,
    CreateNewContext,
    ExecuteTool(Uuid),
    SendInputResponse { request_id: String, value: String },
    CancelInputRequest { request_id: String },
    DeleteTask(String),

    DeleteArtifact(String),
    RefreshArtifacts,

    RefreshServices,
    StartService(String),
    StopService(String),
    RestartService(String),

    RefreshUsers,
    UpdateUserRole { user_id: UserId, role: String },

    RefreshConversations,
    SelectConversation(ContextId),
    RenameConversation { context_id: ContextId, name: String },
    DeleteConversation(String),
    CreateConversation(String),

    ExecuteDbQuery(String),

    AgentList,
    AgentEnable(Option<String>),
    AgentDisable(Option<String>),
    AgentRestart(String),
    AgentStatus,
    AgentHealth(Option<String>),
    AgentCleanup,

    AgentsDiscover,
    AgentA2aSelect(String),

    McpList,
    McpStart(Option<String>),
    McpStop(Option<String>),
    McpStatus,
    McpRestart(Option<String>),

    DbTables,
    DbInfo,
    DbDescribe(String),

    ShowConfig,
    RunCleanup,
    ShowSkills,

    WebBuild,
    WebServe,

    Sync(SyncSubcommand),

    Quit,
}

impl Command {
    pub fn batch(commands: Vec<Self>) -> Self {
        if commands.is_empty() {
            Self::None
        } else if commands.len() == 1 {
            commands.into_iter().next().unwrap_or(Self::None)
        } else {
            Self::Batch(commands)
        }
    }
}
