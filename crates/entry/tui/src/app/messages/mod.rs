mod commands;
mod subcommands;
mod types;

pub use commands::*;
pub use subcommands::*;
pub use types::*;

use crossterm::event::{KeyEvent, MouseEvent};
use uuid::Uuid;

use crate::state::{
    ActiveTab, AnalyticsData, ConversationDisplay, FocusedPanel, ServiceStatus, SseStatus,
    UserDisplay,
};
use crate::tools::PendingToolCall;
use systemprompt_identifiers::{ArtifactId, TaskId};
use systemprompt_models::a2a::Artifact;
use systemprompt_models::{AgUiEvent, AgentCard};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageDomain {
    Input,
    Navigation,
    Chat,
    Services,
    Users,
    Conversations,
    Analytics,
    Logs,
    Commands,
    Tools,
    Agents,
    Context,
    Artifacts,
    System,
}

#[derive(Debug, Clone)]
pub enum Message {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
    Quit,

    FocusPanel(FocusedPanel),
    SwitchTab(ActiveTab),
    ToggleLogs,
    ToggleSidebar,

    SlashCommand(SlashCommand),

    ChatInputChanged(String),
    ChatSend,
    ChatCancelStream,
    ChatClearConversation,
    ChatScroll(ScrollDirection),
    ChatTaskSelectNext,
    ChatTaskSelectPrev,
    ChatTaskOpenDetail,
    ChatTaskCloseDetail,
    ChatTaskDelete,

    AiToolCallReceived(Box<PendingToolCall>),

    AgUiEvent(AgUiEvent),

    ContextStreamTask(Box<ContextStreamTaskEvent>),
    ContextLifecycle(ContextLifecycleEvent),
    ContextSnapshot(SnapshotData),

    TaskProgressStarted {
        task_id: TaskId,
    },
    TaskProgressFinished,
    TaskProgressError(String),
    TaskDeleted(String),

    ArtifactsSelect(usize),
    ArtifactsScroll(ScrollDirection),
    ArtifactsSelectNext,
    ArtifactsSelectPrevious,
    ArtifactsLoaded(Vec<Artifact>),
    ArtifactDeleted(ArtifactId),
    ArtifactsRefresh,

    ServiceStatusUpdate(Vec<ServiceStatus>),
    ServiceRefresh,
    ServiceSelect(usize),
    ServiceAction(ServiceAction),

    UsersRefresh,
    UsersUpdate(Vec<UserDisplay>),
    UsersSelect(usize),

    ConversationsRefresh,
    ConversationsUpdate(Vec<ConversationDisplay>),
    ConversationSelect(String),
    ConversationDeleted(String),

    AnalyticsRefresh,
    AnalyticsUpdate(AnalyticsData),
    AnalyticsScroll(ScrollDirection),
    AnalyticsNextView,
    AnalyticsPrevView,

    LogEntry(LogEntry),
    LogsBatch(Vec<LogEntry>),
    LogsRefresh,
    LogsToggleFollow,
    LogsSetFilter(Option<LogLevel>),
    LogsClear,

    CommandOutput(String),
    CommandError(String),
    CommandExecuting,

    CommandTreeToggle,
    CommandTreeExpand,
    CommandTreeCollapse,

    CommandModalOpen,
    CommandModalClose,
    CommandModalSubmit,

    CommandCliOutput(String),
    CommandCliError(String),

    CommandRequestAiParams {
        command_path: Vec<String>,
        description: String,
    },

    ToolApprove(Uuid),
    ToolReject(Uuid),
    ToolExecutionComplete(Uuid, ToolExecutionResult),

    AgentsRefresh,
    AgentsLoading(bool),
    AgentsUpdate(Vec<AgentCard>),
    AgentsError(String),
    AgentSelect(String),
    AgentSelectNext,
    AgentSelectPrevious,

    SseStatusUpdate(SseStatus),
}

impl Message {
    pub const fn domain(&self) -> MessageDomain {
        match self {
            Self::Key(_) | Self::Mouse(_) | Self::Resize(_, _) | Self::Tick => MessageDomain::Input,

            Self::FocusPanel(_)
            | Self::SwitchTab(_)
            | Self::ToggleLogs
            | Self::ToggleSidebar
            | Self::SlashCommand(_) => MessageDomain::Navigation,

            Self::ChatInputChanged(_)
            | Self::ChatSend
            | Self::ChatCancelStream
            | Self::ChatClearConversation
            | Self::ChatScroll(_)
            | Self::ChatTaskSelectNext
            | Self::ChatTaskSelectPrev
            | Self::ChatTaskOpenDetail
            | Self::ChatTaskCloseDetail
            | Self::ChatTaskDelete
            | Self::AiToolCallReceived(_)
            | Self::AgUiEvent(_) => MessageDomain::Chat,

            Self::ServiceStatusUpdate(_)
            | Self::ServiceRefresh
            | Self::ServiceSelect(_)
            | Self::ServiceAction(_) => MessageDomain::Services,

            Self::UsersRefresh | Self::UsersUpdate(_) | Self::UsersSelect(_) => {
                MessageDomain::Users
            },

            Self::ConversationsRefresh
            | Self::ConversationsUpdate(_)
            | Self::ConversationSelect(_)
            | Self::ConversationDeleted(_) => MessageDomain::Conversations,

            Self::AnalyticsRefresh
            | Self::AnalyticsUpdate(_)
            | Self::AnalyticsScroll(_)
            | Self::AnalyticsNextView
            | Self::AnalyticsPrevView => MessageDomain::Analytics,

            Self::LogEntry(_)
            | Self::LogsBatch(_)
            | Self::LogsRefresh
            | Self::LogsToggleFollow
            | Self::LogsSetFilter(_)
            | Self::LogsClear => MessageDomain::Logs,

            Self::CommandOutput(_)
            | Self::CommandError(_)
            | Self::CommandExecuting
            | Self::CommandTreeToggle
            | Self::CommandTreeExpand
            | Self::CommandTreeCollapse
            | Self::CommandModalOpen
            | Self::CommandModalClose
            | Self::CommandModalSubmit
            | Self::CommandCliOutput(_)
            | Self::CommandCliError(_)
            | Self::CommandRequestAiParams { .. } => MessageDomain::Commands,

            Self::ToolApprove(_) | Self::ToolReject(_) | Self::ToolExecutionComplete(_, _) => {
                MessageDomain::Tools
            },

            Self::AgentsRefresh
            | Self::AgentsLoading(_)
            | Self::AgentsUpdate(_)
            | Self::AgentsError(_)
            | Self::AgentSelect(_)
            | Self::AgentSelectNext
            | Self::AgentSelectPrevious => MessageDomain::Agents,

            Self::SseStatusUpdate(_)
            | Self::ContextStreamTask(_)
            | Self::ContextLifecycle(_)
            | Self::ContextSnapshot(_)
            | Self::TaskProgressStarted { .. }
            | Self::TaskProgressFinished
            | Self::TaskProgressError(_)
            | Self::TaskDeleted(_) => MessageDomain::Context,

            Self::ArtifactsSelect(_)
            | Self::ArtifactsScroll(_)
            | Self::ArtifactsSelectNext
            | Self::ArtifactsSelectPrevious
            | Self::ArtifactsLoaded(_)
            | Self::ArtifactDeleted(_)
            | Self::ArtifactsRefresh => MessageDomain::Artifacts,

            Self::Quit => MessageDomain::System,
        }
    }
}
