//! The process composition root: one tokio runtime and the proxy handle, built
//! once and injected everywhere below.
//!
//! Every module under this one used to reach the same state through a
//! process-global: `proxy::handle()`, `proxy::block_on`, `resolved_port()`. A
//! global can hold one value per process, which is why one test crate existed
//! per proxy start outcome. The context is one value per *context*; a test
//! builds as many as it needs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::runtime::{Handle, Runtime};
use tokio::task::JoinHandle;

use crate::activity::ActivityLog;
use crate::auth::plugin_oauth::PluginTokenCache;
use crate::gateway::GatewayClient;
use crate::mcp_registry::{self, McpRegistrySlot};
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
    runtime: OwnedRuntime,
    pub proxy: ProxyHandle,
    pub mcp_registry: Arc<McpRegistrySlot>,
    pub activity: ActivityLog,
    pub http: reqwest::Client,
    pub plugin_tokens: Arc<PluginTokenCache>,
    pub schedule: ScheduleStatusCache,
    pub start_menu: Arc<StartMenuCache>,
    // Why: sync runs several layers below anything that knows about a UI, and
    // the CLI runs the same code with no UI at all. A sink here is set by the
    // GUI for the duration of a sync and left empty everywhere else, so the
    // reporting calls inside `sync::apply` need no new parameters and cost
    // nothing when nobody is watching.
    pub sync_progress: crate::progress::SyncProgressSink,
    // Why: one administrator prompt per process — a declined prompt must not
    // re-fire from the GUI auto-sync, tray retries, or a `sync --watch` loop.
    pub elevation_attempted: AtomicBool,
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
        let runtime = OwnedRuntime::build()?;
        let activity = ActivityLog::new();
        crate::activity::install_persistent_writer(&activity);
        // Why: loaded in every mode, not just when serving — `install --apply`
        // writes the managed-MCP policy from this registry and used to run in
        // a process that had never read it.
        let mcp_registry = mcp_registry::empty_slot();
        mcp_registry::rehydrate_from_disk(&mcp_registry);
        let http = crate::gateway::build_http_client();
        let plugin_tokens = Arc::new(PluginTokenCache::default());
        let deps = ProxyDeps {
            install_id: InstallId::establish(),
            mcp_registry: Arc::clone(&mcp_registry),
            activity: activity.clone(),
            http: http.clone(),
            plugin_tokens: Arc::clone(&plugin_tokens),
        };
        let proxy = match mode {
            ProxyMode::Serve => ProxyHandle::serve(runtime.handle(), deps),
            ProxyMode::Attach => ProxyHandle::attach(runtime.handle(), deps),
        };
        Ok(Arc::new(Self {
            runtime,
            proxy,
            mcp_registry,
            activity,
            http,
            plugin_tokens,
            schedule: ScheduleStatusCache::default(),
            start_menu: Arc::new(StartMenuCache::default()),
            sync_progress: crate::progress::SyncProgressSink::default(),
            elevation_attempted: AtomicBool::new(false),
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

    pub fn spawn<F>(&self, fut: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.handle().spawn(fut)
    }
}

// Why: dropping a `Runtime` inside one of its own tasks panics, and the last
// `Arc<BridgeContext>` can legitimately go out of scope there.
// `shutdown_background` is the drop tokio documents for exactly that case.
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
