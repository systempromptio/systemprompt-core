//! Shared tokio runtime and the hot-swappable runtime config behind the proxy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, OnceLock};

use tokio::runtime::Runtime;

use crate::config::{self, RuntimeConfig, SharedRuntimeConfig};
use crate::proxy::token_cache::TokenCache;

static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();
static RUNTIME_CONFIG: OnceLock<SharedRuntimeConfig> = OnceLock::new();
static TOKEN_CACHE: OnceLock<Arc<TokenCache>> = OnceLock::new();

#[must_use]
pub fn runtime_config() -> SharedRuntimeConfig {
    Arc::clone(RUNTIME_CONFIG.get_or_init(config::shared_from_loaded))
}

pub(super) fn remember_token_cache(cache: &Arc<TokenCache>) {
    _ = TOKEN_CACHE.set(Arc::clone(cache));
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

pub(super) fn runtime() -> std::io::Result<&'static Arc<Runtime>> {
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
