//! What the manager is told to run: each server's endpoint and switches, the
//! settings shared by every server, and the status it reports back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use crate::headers::RemoteHeaders;
use crate::types::ClientInfo;

/// Every server's status by name.
pub type StatusMap = BTreeMap<String, ServerStatus>;

/// Where a server lives. `Debug` shows neither header values nor environment
/// values.
#[derive(Clone)]
pub enum ServerSpec {
    Remote {
        url: String,
        headers: RemoteHeaders,
    },
    Local {
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
        cwd: Option<PathBuf>,
    },
}

impl fmt::Debug for ServerSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Remote { url, headers } => f
                .debug_struct("Remote")
                .field("url", url)
                .field("headers", headers)
                .finish(),
            Self::Local {
                command,
                args,
                env,
                cwd,
            } => f
                .debug_struct("Local")
                .field("command", command)
                .field("args", &args.len())
                .field("env", &env.keys().collect::<Vec<_>>())
                .field("cwd", cwd)
                .finish(),
        }
    }
}

impl ServerSpec {
    pub(crate) fn same_endpoint(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Remote { url: a, .. }, Self::Remote { url: b, .. }) => a == b,
            (
                Self::Local {
                    command: c1,
                    args: a1,
                    env: e1,
                    cwd: d1,
                },
                Self::Local {
                    command: c2,
                    args: a2,
                    env: e2,
                    cwd: d2,
                },
            ) => c1 == c2 && a1 == a2 && e1 == e2 && d1 == d2,
            _ => false,
        }
    }

    pub(crate) fn same_headers(&self, other: &Self) -> bool {
        match (self.headers(), other.headers()) {
            (Some(a), Some(b)) => a.same_as(b),
            (None, None) => true,
            _ => false,
        }
    }

    pub(crate) const fn headers(&self) -> Option<&RemoteHeaders> {
        match self {
            Self::Remote { headers, .. } => Some(headers),
            Self::Local { .. } => None,
        }
    }
}

/// One named server in the manager's configuration. A `connect_timeout`
/// overrides [`ManagerConfig::connect_timeout`] for this server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub spec: ServerSpec,
    pub enabled: bool,
    pub connect_timeout: Option<Duration>,
}

impl ServerConfig {
    pub const fn new(spec: ServerSpec) -> Self {
        Self {
            spec,
            enabled: true,
            connect_timeout: None,
        }
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        self.enabled == other.enabled
            && self.connect_timeout == other.connect_timeout
            && self.spec.same_endpoint(&other.spec)
            && self.spec.same_headers(&other.spec)
    }
}

/// Where a server is in its life. `NeedsAuth` means the server answered 401;
/// this crate runs no authorization flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Connecting,
    Connected { tools: usize },
    Failed { error: String },
    Disabled,
    NeedsAuth,
}

impl ServerStatus {
    pub(crate) const fn is_live(&self) -> bool {
        matches!(self, Self::Connecting | Self::Connected { .. })
    }
}

/// Settings shared by every server the manager runs.
///
/// `connect_timeout` bounds the whole connect
/// (spawn or first request, handshake and the first tool list);
/// `request_timeout` bounds each list request and the handshake request itself.
/// Remote servers use `http`; without one, a client with a short connect
/// timeout is built.
#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub client_info: ClientInfo,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub call_timeout: Duration,
    pub http: Option<reqwest::Client>,
}

impl ManagerConfig {
    pub const fn new(client_info: ClientInfo) -> Self {
        Self {
            client_info,
            connect_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(30),
            call_timeout: Duration::from_secs(300),
            http: None,
        }
    }
}
