//! Process composition root for the Tokio runtime and injected bridge services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::runtime::Handle;

use crate::activity::ActivityLog;
use crate::auth::plugin_oauth::{PluginTokenCache, SecretStore};
use crate::gateway::GatewayClient;
use crate::mcp_registry::{self, McpRegistrySlot};
pub use crate::obs::StartupFault;
use crate::probe_cache::StartMenuCache;
use crate::proxy::identity::InstallId;
use crate::proxy::notifications::{InboxHandle, NotificationSignals};
use crate::proxy::{ProxyDeps, ProxyHandle};
use crate::schedule::status::ScheduleStatusCache;

mod background;
mod runtime;

use background::{spawn_drive, spawn_inbox};
use runtime::OwnedRuntime;

/// Whether this process should own the loopback port or find the process that
/// does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyMode {
    Serve,
    Attach,
}

/// Everything a command or the GUI needs that outlives a single call.
pub struct BridgeContext {
    tasks: crate::tasks::TaskOwner,
    runtime: OwnedRuntime,
    install_id_durable: bool,
    pub proxy: ProxyHandle,
    pub mcp_registry: Arc<McpRegistrySlot>,
    pub activity: ActivityLog,
    pub http: reqwest::Client,
    pub plugin_tokens: Arc<PluginTokenCache>,
    pub schedule: ScheduleStatusCache,
    pub start_menu: Arc<StartMenuCache>,
    pub sync_progress: crate::progress::SyncProgressSink,
    pub policy_store: crate::config::store::PolicyStore,
    pub sync_lock: Arc<tokio::sync::Mutex<()>>,
    pub update_gate: crate::update::automatic::UpdateGate,
    pub elevation_attempted: AtomicBool,
    pub startup_faults: Vec<StartupFault>,
    pub drive: Arc<crate::drive::DriveHandle>,
    pub inbox: Arc<InboxHandle>,
    pub warn_once: Arc<crate::warn_once::WarnOnce>,
    pub elevation_gate: Arc<crate::elevation_gate::ElevationGate>,
}

impl std::fmt::Debug for BridgeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeContext")
            .field("proxy", &self.proxy)
            .finish_non_exhaustive()
    }
}

impl BridgeContext {
    pub fn start(
        mode: ProxyMode,
        desktop_policy_key_fp: crate::proxy::DesktopPolicyKeyProbe,
    ) -> std::io::Result<Arc<Self>> {
        Self::start_with_policy_store(
            mode,
            crate::config::store::PolicyStore::new(crate::config::store::managed_policy_store()),
            desktop_policy_key_fp,
        )
    }

    pub fn start_with_policy_store(
        mode: ProxyMode,
        policy_store: crate::config::store::PolicyStore,
        desktop_policy_key_fp: crate::proxy::DesktopPolicyKeyProbe,
    ) -> std::io::Result<Arc<Self>> {
        let runtime = OwnedRuntime::build()?;
        tracing::info!(
            version = crate::brand::brand().version,
            commit = crate::buildinfo::short_sha(),
            mode = ?mode,
            "bridge starting"
        );
        let mut faults = Vec::new();
        if let Some(error) = crate::obs::logging_fault() {
            faults.push(StartupFault::new("log file", error));
        }
        let activity = ActivityLog::new();
        if let Err(e) = crate::activity::install_persistent_writer(&activity) {
            faults.push(StartupFault::new("activity log", e));
        }
        if let Err(e) = crate::config::gateways::migrate::migrate_legacy_layout() {
            faults.push(StartupFault::new("gateway profile migration", e));
        }
        let mcp_registry = mcp_registry::empty_slot();
        match crate::config::load() {
            Ok(cfg) => {
                let gateway = crate::config::gateway_url_or_default(&cfg);
                if let Err(e) = mcp_registry::rehydrate_from_disk(&mcp_registry, &gateway) {
                    faults.push(StartupFault::new("mcp registry cache", e));
                }
            },
            Err(e) => faults.push(StartupFault::new("mcp registry cache", e)),
        }
        let http = crate::gateway::build_http_client();
        let plugin_tokens = Arc::new(PluginTokenCache::new(SecretStore::resolve()));
        let (install_id, install_id_durable) = match InstallId::establish() {
            Ok(id) => (id, true),
            Err(e) => {
                faults.push(StartupFault::new("install identity", e));
                (InstallId::ephemeral(), false)
            },
        };
        let update_gate = crate::update::automatic::UpdateGate::default();
        let (drive_signals, drive_receiver) = crate::proxy::drive_signal::DriveSignals::channel();
        let (notification_signals, notification_receiver) = NotificationSignals::channel();
        let deps = ProxyDeps {
            install_id,
            mcp_registry: Arc::clone(&mcp_registry),
            activity: activity.clone(),
            http: http.clone(),
            plugin_tokens: Arc::clone(&plugin_tokens),
            update_gate: update_gate.clone(),
            drive_signals,
            notification_signals,
            desktop_policy_key_fp,
        };
        let proxy = match mode {
            ProxyMode::Serve => ProxyHandle::serve(runtime.handle(), deps, &mut faults),
            ProxyMode::Attach => ProxyHandle::attach(deps, &mut faults),
        };
        let tasks = crate::tasks::TaskOwner::new(runtime.handle(), activity.clone());
        let drive = crate::drive::DriveHandle::new();
        spawn_drive(&tasks, &proxy, &drive, &http, drive_receiver);
        let inbox = spawn_inbox(&tasks, &proxy, &http, notification_receiver);
        Ok(Arc::new(Self {
            tasks,
            drive,
            inbox,
            runtime,
            install_id_durable,
            proxy,
            mcp_registry,
            activity,
            http,
            plugin_tokens,
            schedule: ScheduleStatusCache::default(),
            start_menu: Arc::new(StartMenuCache::default()),
            sync_progress: crate::progress::SyncProgressSink::default(),
            policy_store,
            sync_lock: Arc::new(tokio::sync::Mutex::new(())),
            update_gate,
            elevation_attempted: AtomicBool::new(false),
            startup_faults: faults,
            warn_once: Arc::default(),
            elevation_gate: Arc::default(),
        }))
    }

    #[must_use]
    pub fn mcp_registry(&self) -> Arc<mcp_registry::McpRegistry> {
        mcp_registry::snapshot(&self.mcp_registry)
    }

    #[must_use]
    pub const fn install_id(&self) -> &InstallId {
        self.proxy.install_id()
    }

    #[must_use]
    pub fn durable_install_id(&self) -> Option<&InstallId> {
        self.install_id_durable.then(|| self.install_id())
    }

    #[must_use]
    pub fn secrets(&self) -> &SecretStore {
        self.plugin_tokens.secrets()
    }

    #[must_use]
    pub fn gateway_client(
        &self,
        base_url: systemprompt_identifiers::ValidatedUrl,
    ) -> GatewayClient {
        GatewayClient::new(base_url, self.http.clone())
    }

    #[must_use]
    pub const fn handle(&self) -> &Handle {
        self.runtime.handle()
    }

    pub fn block_on<F: Future>(&self, fut: F) -> F::Output {
        self.runtime.handle().block_on(fut)
    }

    #[track_caller]
    pub fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static) {
        self.tasks.spawn(fut);
    }
}
