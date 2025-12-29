use crate::config::TuiConfig;
use systemprompt_models::Profile;

use super::{
    AgentsState, AnalyticsState, ArtifactsState, ChatState, CommandsState, ConversationsState,
    LogsState, ServicesState, ToolsState, UsersState,
};

#[derive(Debug, Clone)]
pub enum TuiModeInfo {
    Cloud {
        cloud_api_url: String,
        user_email: Option<String>,
        tenant_id: Option<String>,
        profile: Profile,
    },
}

impl TuiModeInfo {
    pub fn display_name(&self) -> String {
        match self {
            Self::Cloud { profile, .. } => profile.display_name.clone(),
        }
    }

    pub fn environment(&self) -> &str {
        match self {
            Self::Cloud { cloud_api_url, .. } => {
                if cloud_api_url.contains("sandbox") {
                    "Sandbox"
                } else {
                    "Production"
                }
            },
        }
    }

    pub fn user_display(&self) -> Option<String> {
        match self {
            Self::Cloud { user_email, .. } => user_email.clone(),
        }
    }

    pub fn cloud_api_url(&self) -> &str {
        match self {
            Self::Cloud { cloud_api_url, .. } => cloud_api_url,
        }
    }

    pub fn api_external_url(&self) -> &str {
        match self {
            Self::Cloud { profile, .. } => &profile.server.api_external_url,
        }
    }

    pub const fn profile(&self) -> &Profile {
        match self {
            Self::Cloud { profile, .. } => profile,
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    pub chat: ChatState,
    pub conversations: ConversationsState,
    pub services: ServicesState,
    pub logs: LogsState,
    pub tools: ToolsState,
    pub users: UsersState,
    pub analytics: AnalyticsState,
    pub agents: AgentsState,
    pub artifacts: ArtifactsState,
    pub commands: CommandsState,
    pub focus: FocusedPanel,
    pub input_mode: InputMode,
    pub active_tab: ActiveTab,
    pub sidebar_visible: bool,
    pub show_services_panel: bool,
    pub should_quit: bool,
    pub mode_info: TuiModeInfo,
    pub init_status: InitStatus,
    pub sse_status: SseStatus,
}

#[derive(Debug, Clone, Default)]
pub struct InitStatus {
    pub is_initializing: bool,
    pub current_step: String,
    pub steps_completed: usize,
    pub total_steps: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SseStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

impl AppState {
    pub fn new(config: &TuiConfig, mode_info: TuiModeInfo) -> Self {
        Self {
            chat: ChatState::new(),
            conversations: ConversationsState::new(),
            services: ServicesState::new(),
            logs: LogsState::new(1000),
            tools: ToolsState::new(),
            users: UsersState::new(),
            analytics: AnalyticsState::new(),
            agents: AgentsState::new(),
            artifacts: ArtifactsState::new(),
            commands: CommandsState::new(),
            focus: FocusedPanel::Chat,
            input_mode: InputMode::Insert,
            active_tab: ActiveTab::Chat,
            sidebar_visible: config.layout.sidebar_visible,
            show_services_panel: false,
            should_quit: false,
            mode_info,
            init_status: InitStatus {
                is_initializing: true,
                current_step: "Starting...".to_string(),
                steps_completed: 0,
                total_steps: 6,
            },
            sse_status: SseStatus::Disconnected,
        }
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Chat => ActiveTab::Conversations,
            ActiveTab::Conversations => ActiveTab::Agents,
            ActiveTab::Agents => ActiveTab::Artifacts,
            ActiveTab::Artifacts => ActiveTab::Users,
            ActiveTab::Users => ActiveTab::Analytics,
            ActiveTab::Analytics => ActiveTab::Services,
            ActiveTab::Services => ActiveTab::Config,
            ActiveTab::Config => ActiveTab::Shortcuts,
            ActiveTab::Shortcuts => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Chat,
        };
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Chat => ActiveTab::Logs,
            ActiveTab::Conversations => ActiveTab::Chat,
            ActiveTab::Agents => ActiveTab::Conversations,
            ActiveTab::Artifacts => ActiveTab::Agents,
            ActiveTab::Users => ActiveTab::Artifacts,
            ActiveTab::Analytics => ActiveTab::Users,
            ActiveTab::Services => ActiveTab::Analytics,
            ActiveTab::Config => ActiveTab::Services,
            ActiveTab::Shortcuts => ActiveTab::Config,
            ActiveTab::Logs => ActiveTab::Shortcuts,
        };
    }

    pub fn has_pending_approval(&self) -> bool {
        !self.tools.pending_approvals.is_empty()
    }

    pub const fn is_streaming(&self) -> bool {
        self.chat.is_processing()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    Chat,
    Sidebar,
    Logs,
    ApprovalDialog,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Insert,
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    Chat,
    Conversations,
    Agents,
    Artifacts,
    Users,
    Analytics,
    Services,
    Config,
    Shortcuts,
    Logs,
}
