pub mod forward;
pub mod secret;
pub mod server;
pub mod token_cache;
pub mod usage;

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::runtime::Runtime;

use crate::config;
use crate::obs::output::diag;

pub use server::{ProxyHandle, ProxyStats};

pub const DEFAULT_PROXY_PORT: u16 = 48217;
const REFRESH_TICK: Duration = Duration::from_secs(60);
pub use forward::REFRESH_THRESHOLD_SECS;
use token_cache::TokenCache;

static HANDLE: OnceLock<ProxyHandle> = OnceLock::new();
static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

fn worker_thread_count() -> usize {
    std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(2))
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
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let token_cache = Arc::new(TokenCache::default_for_runtime());
    let handle = match server::start(rt, DEFAULT_PROXY_PORT, &gateway, Arc::clone(&token_cache)) {
        Ok(h) => h,
        Err(e) => {
            diag(&format!("proxy: bind failed on {DEFAULT_PROXY_PORT}: {e}"));
            return None;
        },
    };
    diag(&format!("proxy: listening on localhost:{}", handle.port));

    rt.spawn(refresh_loop(token_cache));

    let _ = HANDLE.set(handle);
    HANDLE.get()
}

pub fn handle() -> Option<&'static ProxyHandle> {
    HANDLE.get()
}

async fn refresh_loop(cache: Arc<TokenCache>) {
    let mut interval = tokio::time::interval(REFRESH_TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        let _ = cache.current(REFRESH_THRESHOLD_SECS).await;
    }
}
