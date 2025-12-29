use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use systemprompt_models::{AgentCard, RuntimeStatus};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusParams {
    pub status: String,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Clone, Default)]
pub enum SystemInstructionsSource {
    #[default]
    Unknown,
    Inline,
    FilePath(PathBuf),
}

#[derive(Debug, Clone, Default)]
pub struct AgentDisplayMetadata {
    pub skill_paths: HashMap<String, PathBuf>,
    pub mcp_server_paths: HashMap<String, String>,
    pub system_instructions_source: SystemInstructionsSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AgentConnectionStatus {
    Connected,
    #[default]
    Disconnected,
    Connecting,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub display_name: String,
    pub url: String,
    pub port: u16,
    pub is_primary: bool,
    pub status: AgentConnectionStatus,
}

impl AgentInfo {
    pub fn new(name: String, port: u16) -> Self {
        Self {
            display_name: name.clone(),
            url: format!("http://localhost:{}", port),
            name,
            port,
            is_primary: false,
            status: AgentConnectionStatus::Disconnected,
        }
    }

    pub fn from_card(card: &AgentCard) -> Self {
        let port = card
            .url
            .split(':')
            .next_back()
            .and_then(|p| p.trim_end_matches('/').parse::<u16>().ok())
            .unwrap_or(0);

        let service_status = card
            .capabilities
            .extensions
            .as_ref()
            .and_then(|exts| exts.iter().find(|e| e.uri == "systemprompt:service-status"))
            .and_then(|ext| ext.params.as_ref())
            .and_then(|p| serde_json::from_value::<ServiceStatusParams>(p.clone()).ok());

        let is_primary = service_status.as_ref().is_some_and(|s| s.default);

        let status =
            service_status
                .as_ref()
                .map_or(AgentConnectionStatus::Disconnected, |s| {
                    match s
                        .status
                        .parse::<RuntimeStatus>()
                        .unwrap_or(RuntimeStatus::Stopped)
                    {
                        RuntimeStatus::Running => AgentConnectionStatus::Connected,
                        RuntimeStatus::Starting => AgentConnectionStatus::Connecting,
                        RuntimeStatus::Crashed => {
                            AgentConnectionStatus::Error("Failed".to_string())
                        },
                        RuntimeStatus::Stopped | RuntimeStatus::Orphaned => {
                            AgentConnectionStatus::Disconnected
                        },
                    }
                });

        Self {
            name: card.name.clone(),
            display_name: card.name.clone(),
            url: card.url.clone(),
            port,
            is_primary,
            status,
        }
    }

    pub fn with_display_name(mut self, display_name: String) -> Self {
        self.display_name = display_name;
        self
    }

    pub const fn with_primary(mut self, is_primary: bool) -> Self {
        self.is_primary = is_primary;
        self
    }

    pub fn with_status(mut self, status: AgentConnectionStatus) -> Self {
        self.status = status;
        self
    }
}
