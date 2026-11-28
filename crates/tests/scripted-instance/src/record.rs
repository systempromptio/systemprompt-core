//! What the instance records about each request it receives.

use axum::http::{HeaderMap, Method};
use serde_json::Value;
use systemprompt_models::ApiPaths;

#[derive(Debug, Clone)]
pub struct RequestRecord {
    pub sequence: u64,
    pub route: Route,
    pub method: Method,
    pub path: String,
    pub query: Option<String>,
    pub headers: HeaderMap,
    pub body: RecordedBody,
    pub unscripted: bool,
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordedBody {
    Empty,
    Json(Value),
    Bytes(Vec<u8>),
}

impl RecordedBody {
    pub const fn json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::Empty | Self::Bytes(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Discovery,
    Health,
    DeviceLink,
    CodeExchange,
    PatSession,
    Whoami,
    Manifest,
    ModelList,
    Profile,
    Messages,
    ChatCompletions,
    HookClient,
    HookToken,
    Telemetry,
    Verdict,
    Heartbeat,
    PluginBundle,
    Plan,
    Receipt,
    Drive,
    Unrouted,
}

const DRIVE_ROUTES: [&str; 9] = [
    ApiPaths::BRIDGE_DRIVE_MANIFEST,
    ApiPaths::BRIDGE_DRIVE_CHANGES,
    ApiPaths::BRIDGE_DRIVE_BLOB,
    ApiPaths::BRIDGE_DRIVE_COMMIT,
    ApiPaths::BRIDGE_DRIVE_PRESENCE,
    ApiPaths::BRIDGE_DRIVE_PROPOSALS,
    ApiPaths::BRIDGE_DRIVE_TRASH,
    ApiPaths::BRIDGE_DRIVE_MOVE,
    ApiPaths::BRIDGE_DRIVE_TOKEN,
];

impl Route {
    pub(crate) fn of(path: &str) -> Self {
        match path {
            ApiPaths::WELLKNOWN_BRIDGE
            | ApiPaths::BRIDGE_AUTH_CAPABILITIES
            | ApiPaths::BRIDGE_PUBKEY => Self::Discovery,
            ApiPaths::HEALTH => Self::Health,
            ApiPaths::BRIDGE_DEVICE_LINK_FALLBACK => Self::DeviceLink,
            ApiPaths::BRIDGE_AUTH_SESSION_PAT | ApiPaths::BRIDGE_AUTH_SESSION => Self::CodeExchange,
            ApiPaths::BRIDGE_AUTH_PAT => Self::PatSession,
            ApiPaths::BRIDGE_WHOAMI => Self::Whoami,
            ApiPaths::BRIDGE_MANIFEST => Self::Manifest,
            ApiPaths::GATEWAY_MODELS => Self::ModelList,
            ApiPaths::BRIDGE_PROFILE => Self::Profile,
            ApiPaths::GATEWAY_MESSAGES => Self::Messages,
            ApiPaths::GATEWAY_CHAT_COMPLETIONS => Self::ChatCompletions,
            ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT => Self::HookClient,
            ApiPaths::OAUTH_TOKEN => Self::HookToken,
            ApiPaths::PUBLIC_HOOKS_TRACK => Self::Telemetry,
            ApiPaths::PUBLIC_HOOKS_GOVERN => Self::Verdict,
            ApiPaths::BRIDGE_HEARTBEAT => Self::Heartbeat,
            ApiPaths::CONSUMER_RECEIPTS => Self::Receipt,
            _ if under(path, ApiPaths::BRIDGE_PLUGINS) => Self::PluginBundle,
            _ if under(path, ApiPaths::CONSUMER_RESOURCES) => Self::Plan,
            _ if DRIVE_ROUTES
                .iter()
                .any(|base| path == *base || under(path, base)) =>
            {
                Self::Drive
            },
            _ => Self::Unrouted,
        }
    }

    pub(crate) const fn is_model_call(self) -> bool {
        matches!(self, Self::Messages | Self::ChatCompletions)
    }
}

fn under(path: &str, base: &str) -> bool {
    path.strip_prefix(base)
        .is_some_and(|rest| rest.starts_with('/'))
}
