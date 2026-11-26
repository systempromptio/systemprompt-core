//! What the instance records about each request it receives.

use axum::http::{HeaderMap, Method};
use serde_json::Value;

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
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordedBody {
    Empty,
    Json(Value),
    Bytes(Vec<u8>),
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
