//! The manager's shared state: one slot per configured server, the published
//! tool catalogue and status feed, and the owner of every server's background
//! job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_traits::BackgroundTasks;
use parking_lot::Mutex;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use super::config::{ServerConfig, ServerStatus, StatusMap};
use super::job::ServerJob;
use crate::mcp::catalog::Catalog;
use crate::mcp::client::McpClient;
use crate::mcp::headers::{self, HeaderCell};
use crate::mcp::types::{ClientInfo, ToolInfo};

pub(super) struct ConnectSettings {
    pub(super) http: reqwest::Client,
    pub(super) client_info: ClientInfo,
    pub(super) request_timeout: Duration,
}

pub(super) struct Slot {
    pub(super) config: ServerConfig,
    pub(super) headers: HeaderCell,
    pub(super) epoch: u64,
    pub(super) cancel: CancellationToken,
    pub(super) client: Option<McpClient>,
    pub(super) tools: Vec<ToolInfo>,
    pub(super) status: ServerStatus,
}

impl Slot {
    pub(super) fn disabled(config: ServerConfig) -> Self {
        let headers = headers::cell(config.spec.headers().cloned().unwrap_or_default());
        Self {
            config,
            headers,
            epoch: 0,
            cancel: CancellationToken::new(),
            client: None,
            tools: Vec::new(),
            status: ServerStatus::Disabled,
        }
    }

    pub(super) fn stop(&mut self) {
        self.cancel.cancel();
        self.client = None;
        self.tools.clear();
    }
}

#[derive(Default)]
pub(super) struct State {
    pub(super) servers: BTreeMap<String, Slot>,
    pub(super) catalog: Catalog,
    next_epoch: u64,
}

pub(super) struct Shared {
    pub(super) runtime: tokio::runtime::Handle,
    pub(super) connect: Arc<ConnectSettings>,
    pub(super) connect_timeout: Duration,
    pub(super) call_timeout: Duration,
    pub(super) state: Mutex<State>,
    pub(super) status: watch::Sender<StatusMap>,
    pub(super) root: CancellationToken,
    pub(super) tasks: BackgroundTasks,
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.root.cancel();
    }
}

impl Shared {
    pub(super) fn publish(&self, state: &mut State) {
        state.catalog = Catalog::build(
            state
                .servers
                .iter()
                .filter(|(_, slot)| slot.client.is_some())
                .map(|(name, slot)| (name.as_str(), slot.tools.as_slice())),
        );
        let statuses: StatusMap = state
            .servers
            .iter()
            .map(|(name, slot)| (name.clone(), slot.status.clone()))
            .collect();
        self.status.send_replace(statuses);
    }

    pub(super) fn update_slot(
        &self,
        name: &str,
        epoch: u64,
        change: impl FnOnce(&mut Slot),
    ) -> bool {
        let mut state = self.state.lock();
        let Some(slot) = state.servers.get_mut(name) else {
            return false;
        };
        if slot.epoch != epoch {
            return false;
        }
        change(slot);
        self.publish(&mut state);
        drop(state);
        true
    }

    pub(super) fn launch(self: &Arc<Self>, state: &mut State, name: &str, config: ServerConfig) {
        if let Some(mut old) = state.servers.remove(name) {
            old.stop();
        }
        state.next_epoch += 1;
        let epoch = state.next_epoch;
        let headers = headers::cell(config.spec.headers().cloned().unwrap_or_default());
        let cancel = self.root.child_token();
        let job = ServerJob {
            shared: Arc::downgrade(self),
            settings: Arc::clone(&self.connect),
            name: name.to_owned(),
            epoch,
            spec: config.spec.clone(),
            headers: Arc::clone(&headers),
            timeout: config.connect_timeout.unwrap_or(self.connect_timeout),
            cancel: cancel.clone(),
        };
        let _entered = self.runtime.enter();
        self.tasks.spawn("mcp-server", job.run());
        tracing::debug!(server = %name, epoch, "starting server");
        let slot = Slot {
            config,
            headers,
            epoch,
            cancel,
            client: None,
            tools: Vec::new(),
            status: ServerStatus::Connecting,
        };
        state.servers.insert(name.to_owned(), slot);
    }
}
