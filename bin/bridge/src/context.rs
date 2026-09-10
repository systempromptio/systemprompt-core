//! Process composition root for the Tokio runtime and injected bridge services.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::runtime::{Handle, Runtime};


use crate::activity::ActivityLog;
use crate::auth::plugin_oauth::PluginTokenCache;
use crate::gateway::GatewayClient;
use crate::mcp_registry::{self, McpRegistrySlot};
pub use crate::obs::StartupFault;
use crate::probe_cache::StartMenuCache;
use crate::proxy::identity::InstallId;
use crate::proxy::{ProxyDeps, ProxyHandle};
use crate::schedule::status::ScheduleStatusCache;

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
    pub elevation_attempted: AtomicBool,
    pub startup_faults: Vec<StartupFault>,
}

impl std::fmt::Debug for BridgeContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeContext")
            .field("proxy", &self.proxy)
            .finish_non_exhaustive()
    }
}

impl BridgeContext {
    pub fn start(mode: ProxyMode) -> std::io::Result<Arc<Self>> {
        Self::start_with_policy_store(
            mode,
            crate::config::store::PolicyStore::new(crate::config::store::managed_policy_store()),
        )
    }

    pub fn start_with_policy_store(
        mode: ProxyMode,
        policy_store: crate::config::store::PolicyStore,
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
        let mcp_registry = mcp_registry::empty_slot();
        if let Err(e) = mcp_registry::rehydrate_from_disk(&mcp_registry) {
            faults.push(StartupFault::new("mcp registry cache", e));
        }
        let http = crate::gateway::build_http_client();
        let plugin_tokens = Arc::new(PluginTokenCache::default());
        let install_id = match InstallId::establish() {
            Ok(id) => id,
            Err(e) => {
                faults.push(StartupFault::new("install identity", e));
                InstallId::ephemeral()
            },
        };
        let deps = ProxyDeps {
            install_id,
            mcp_registry: Arc::clone(&mcp_registry),
            activity: activity.clone(),
            http: http.clone(),
            plugin_tokens: Arc::clone(&plugin_tokens),
        };
        let proxy = match mode {
            ProxyMode::Serve => ProxyHandle::serve(runtime.handle(), deps, &mut faults),
            ProxyMode::Attach => ProxyHandle::attach(deps, &mut faults),
        };
        Ok(Arc::new(Self {
            tasks: crate::tasks::TaskOwner::new(runtime.handle(), activity.clone()),
            runtime,
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
            elevation_attempted: AtomicBool::new(false),
            startup_faults: faults,
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
    pub fn gateway_client(
        &self,
        base_url: systemprompt_identifiers::ValidatedUrl,
    ) -> GatewayClient {
        GatewayClient::new(base_url, self.http.clone())
    }

    #[must_use]
    pub fn handle(&self) -> &Handle {
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

// Why: Tokio runtime drop panics inside an async task; the final owner may be
// dropped there.
struct OwnedRuntime(Option<Runtime>);

impl OwnedRuntime {
    fn build() -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_thread_count())
            .thread_name("bridge-rt")
            .enable_all()
            .build()?;
        Ok(Self(Some(rt)))
    }

    fn handle(&self) -> &Handle {
        self.0.as_ref().map_or_else(
            || unreachable!("runtime is only taken in Drop"),
            Runtime::handle,
        )
    }
}

impl Drop for OwnedRuntime {
    fn drop(&mut self) {
        if let Some(rt) = self.0.take() {
            rt.shutdown_background();
        }
    }
}

fn worker_thread_count() -> usize {
    std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(2))
}
