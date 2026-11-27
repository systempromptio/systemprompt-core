//! The set of configured servers: connects them concurrently, tracks each one's
//! status, keeps the tool catalogue current (including on
//! `notifications/tools/list_changed`) and routes tool calls to the right
//! session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod job;
mod state;

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

pub use config::{ManagerConfig, ServerConfig, ServerSpec, ServerStatus, StatusMap};
use harness_tasks::BackgroundTasks;
use parking_lot::Mutex;
use serde_json::Value;
use state::{ConnectSettings, Shared, Slot, State};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::catalog::McpTool;
use crate::client::{McpClient, RequestOptions};
use crate::error::McpError;
use crate::reconcile::{self, Existing, Plan};
use crate::types::CallResult;

const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Runs a set of named MCP servers on the tokio runtime it was created in.
///
/// Cheap to clone; servers
/// stop when the last clone is dropped, and [`McpManager::shutdown`] stops them
/// and waits a few seconds for them to exit, after which a later
/// [`McpManager::reconcile`] starts servers again. [`McpManager::reconcile`]
/// returns at once: follow progress with [`McpManager::subscribe`] or
/// wait for it with [`McpManager::settle`].
#[derive(Clone)]
pub struct McpManager {
    shared: Arc<Shared>,
}

impl fmt::Debug for McpManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpManager")
            .field("servers", &*self.shared.status.borrow())
            .finish()
    }
}

impl McpManager {
    pub fn new(config: ManagerConfig) -> Result<Self, McpError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(McpError::NoRuntime)?;
        let http = match config.http {
            Some(http) => http,
            None => reqwest::Client::builder()
                .connect_timeout(HTTP_CONNECT_TIMEOUT)
                .build()
                .map_err(McpError::http)?,
        };
        let (status, _) = watch::channel(StatusMap::new());
        Ok(Self {
            shared: Arc::new(Shared {
                runtime,
                connect: Arc::new(ConnectSettings {
                    http,
                    client_info: config.client_info,
                    request_timeout: config.request_timeout,
                }),
                connect_timeout: config.connect_timeout,
                call_timeout: config.call_timeout,
                state: Mutex::new(State::default()),
                status,
                root: CancellationToken::new(),
                tasks: BackgroundTasks::new(),
            }),
        })
    }

    pub fn reconcile(&self, servers: BTreeMap<String, ServerConfig>) {
        let mut next = servers;
        let mut state = self.shared.state.lock();
        let plan = {
            let current: BTreeMap<&str, Existing<'_>> = state
                .servers
                .iter()
                .map(|(name, slot)| {
                    (
                        name.as_str(),
                        Existing {
                            config: &slot.config,
                            live: slot.status.is_live(),
                        },
                    )
                })
                .collect();
            reconcile::plan(&current, &next)
        };
        self.apply(&mut state, &plan, &mut next);
        self.shared.publish(&mut state);
        drop(state);
    }

    pub fn restart(&self, name: &str) -> Result<(), McpError> {
        let mut state = self.shared.state.lock();
        let config = state
            .servers
            .get(name)
            .map(|slot| slot.config.clone())
            .ok_or_else(|| McpError::UnknownServer(name.to_owned()))?;
        if !config.enabled {
            return Ok(());
        }
        self.shared.launch(&mut state, name, config);
        self.shared.publish(&mut state);
        drop(state);
        Ok(())
    }

    pub fn status(&self) -> StatusMap {
        StatusMap::clone(&self.shared.status.borrow())
    }

    pub fn subscribe(&self) -> watch::Receiver<StatusMap> {
        self.shared.status.subscribe()
    }

    pub async fn settle(&self) {
        let mut feed = self.subscribe();
        let settled = |statuses: &StatusMap| {
            !statuses
                .values()
                .any(|status| matches!(status, ServerStatus::Connecting))
        };
        if feed.wait_for(settled).await.is_err() {
            tracing::debug!("the status feed closed while waiting for servers to settle");
        }
    }

    pub fn tools(&self) -> Vec<McpTool> {
        self.shared.state.lock().catalog.tools().to_vec()
    }

    pub fn client(&self, server: &str) -> Option<McpClient> {
        self.shared
            .state
            .lock()
            .servers
            .get(server)
            .and_then(|slot| slot.client.clone())
    }

    // JSON: tool arguments are the object the model wrote for the tool's input
    // schema.
    pub async fn call(
        &self,
        qualified_name: &str,
        arguments: Value,
        cancel: CancellationToken,
    ) -> Result<CallResult, McpError> {
        let (client, server, tool) = {
            let state = self.shared.state.lock();
            let entry = state
                .catalog
                .get(qualified_name)
                .ok_or_else(|| McpError::UnknownTool(qualified_name.to_owned()))?;
            let server = entry.server.clone();
            let tool = entry.name.clone();
            let client = state
                .servers
                .get(&server)
                .and_then(|slot| slot.client.clone());
            drop(state);
            (
                client.ok_or_else(|| McpError::NotConnected(server.clone()))?,
                server,
                tool,
            )
        };
        tracing::debug!(server = %server, tool = %tool, "calling tool");
        let options = RequestOptions {
            timeout: Some(self.shared.call_timeout),
            cancel: Some(cancel),
        };
        client.call_tool(&tool, arguments, options).await
    }

    pub async fn shutdown(&self) {
        {
            let mut state = self.shared.state.lock();
            for (_, mut slot) in std::mem::take(&mut state.servers) {
                slot.stop();
            }
            self.shared.publish(&mut state);
            drop(state);
        }
        if !self
            .shared
            .tasks
            .shutdown(SHUTDOWN_GRACE)
            .await
            .is_drained()
        {
            tracing::warn!("some servers did not stop in time");
        }
    }

    fn apply(&self, state: &mut State, plan: &Plan, next: &mut BTreeMap<String, ServerConfig>) {
        for name in &plan.remove {
            if let Some(mut slot) = state.servers.remove(name) {
                slot.stop();
                tracing::debug!(server = %name, "removed server");
            }
        }
        for name in &plan.disable {
            let Some(config) = next.remove(name) else {
                continue;
            };
            match state.servers.entry(name.clone()) {
                Entry::Occupied(mut occupied) => {
                    let slot = occupied.get_mut();
                    slot.stop();
                    slot.config = config;
                    slot.status = ServerStatus::Disabled;
                },
                Entry::Vacant(vacant) => {
                    vacant.insert(Slot::disabled(config));
                },
            }
        }
        for name in &plan.swap_headers {
            if let (Some(config), Some(slot)) = (next.remove(name), state.servers.get_mut(name)) {
                *slot.headers.write() = config.spec.headers().cloned().unwrap_or_default();
                slot.config = config;
                tracing::debug!(server = %name, "updated server headers in place");
            }
        }
        for name in &plan.update {
            if let (Some(config), Some(slot)) = (next.remove(name), state.servers.get_mut(name)) {
                slot.config = config;
            }
        }
        for name in &plan.start {
            if let Some(config) = next.remove(name) {
                self.shared.launch(state, name, config);
            }
        }
    }
}
