pub mod dispatch;
pub mod forward;
pub mod heartbeat;
pub mod secret;
pub mod server;
pub mod session;
pub mod token_cache;
pub mod usage;

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::runtime::Runtime;

use crate::config::{self, RuntimeConfig, SharedRuntimeConfig};
use crate::obs::output::diag;

pub use server::{ProxyHandle, ProxyStats};

pub const DEFAULT_PROXY_PORT: u16 = 48217;
const REFRESH_TICK: Duration = Duration::from_mins(1);
pub use forward::REFRESH_THRESHOLD_SECS;
use session::SessionContext;
use token_cache::TokenCache;

static HANDLE: OnceLock<ProxyHandle> = OnceLock::new();
static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();
static RUNTIME_CONFIG: OnceLock<SharedRuntimeConfig> = OnceLock::new();
static TOKEN_CACHE: OnceLock<Arc<TokenCache>> = OnceLock::new();

#[must_use]
pub fn runtime_config() -> SharedRuntimeConfig {
    RUNTIME_CONFIG
        .get_or_init(config::shared_from_loaded)
        .clone()
}

fn swap_runtime_config(next: RuntimeConfig) {
    runtime_config().store(Arc::new(next));
    if let Some(cache) = TOKEN_CACHE.get() {
        let cache = Arc::clone(cache);
        if let Ok(rt) = runtime() {
            rt.spawn(async move { cache.invalidate().await });
        }
    }
    tracing::info!(target: "bridge::config", "runtime config swapped");
}

pub fn reload_runtime_config() {
    swap_runtime_config(RuntimeConfig::from_loaded());
}

fn worker_thread_count() -> usize {
    std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(2))
}

pub fn runtime_handle() -> std::io::Result<tokio::runtime::Handle> {
    runtime().map(|rt| rt.handle().clone())
}

pub fn block_on<F: Future>(fut: F) -> std::io::Result<F::Output> {
    runtime().map(|rt| rt.block_on(fut))
}

fn runtime() -> std::io::Result<&'static Arc<Runtime>> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(worker_thread_count())
        .thread_name("bridge-rt")
        .enable_all()
        .build()?;
    let arc = Arc::new(rt);
    RUNTIME.set(arc).ok();
    RUNTIME
        .get()
        .ok_or_else(|| std::io::Error::other("runtime init lost the race"))
}

pub fn start_default() -> Option<&'static ProxyHandle> {
    if let Some(h) = HANDLE.get() {
        return Some(h);
    }
    let rt = match runtime() {
        Ok(rt) => rt,
        Err(e) => {
            diag(&format!("proxy: tokio runtime build failed: {e}"));
            return None;
        },
    };
    let shared = runtime_config();
    let session_context = Arc::new(SessionContext::new());
    let token_cache = Arc::new(TokenCache::default_for_runtime(
        session_context.session_id().clone(),
    ));
    // Why: idempotent one-shot init — a prior set on a re-entered start() is
    // already the correct value, so a returned-Err is the no-op we want.
    _ = TOKEN_CACHE.set(Arc::clone(&token_cache));
    let handle = match server::start(
        rt,
        DEFAULT_PROXY_PORT,
        Arc::clone(&shared),
        Arc::clone(&token_cache),
        Arc::clone(&session_context),
    ) {
        Ok(h) => h,
        Err(e) => {
            diag(&format!("proxy: bind failed on {DEFAULT_PROXY_PORT}: {e}"));
            return None;
        },
    };
    diag(&format!("proxy: listening on localhost:{}", handle.port));

    rt.spawn(refresh_loop(token_cache));

    // Why: idempotent one-shot init — a prior set on a re-entered start() already
    // holds a live handle, so a returned-Err is the no-op we want.
    _ = HANDLE.set(handle);
    HANDLE.get()
}

pub fn handle() -> Option<&'static ProxyHandle> {
    HANDLE.get()
}

/// Loopback URL a host config (`.mcp.json`, MDM registry value) should target
/// for the managed MCP server identified by `slug`. The proxy verifies the
/// loopback secret, strips it, and injects the rotating gateway JWT before
/// forwarding to the upstream registered under `slug` in `mcp_registry`.
///
/// Uses the live bound port when the proxy is running, else the default — the
/// proxy always binds the default unless it was taken, so the fallback is
/// correct for the config-write-before-proxy-start ordering.
#[must_use]
pub fn mcp_url(slug: &str) -> String {
    let port = handle().map_or(DEFAULT_PROXY_PORT, |h| h.port);
    format!("http://127.0.0.1:{port}/mcp/{slug}")
}

/// The `Authorization` header value a host presents to the loopback proxy.
///
/// Read-or-mint via [`secret::proxy_init`] so a config write that happens
/// before the proxy has started still emits a header that matches the secret
/// the proxy will load (both go through the same on-disk file + `OnceLock`).
pub fn loopback_bearer() -> std::io::Result<String> {
    secret::proxy_init().map(|s| format!("Bearer {}", s.as_str()))
}

async fn refresh_loop(cache: Arc<TokenCache>) {
    let mut interval = tokio::time::interval(REFRESH_TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        // Why: proactive cache warm-up; refresh failures are logged inside
        // current() and the next tick retries, so the loop must not propagate them.
        _ = cache.current(REFRESH_THRESHOLD_SECS).await;
    }
}
