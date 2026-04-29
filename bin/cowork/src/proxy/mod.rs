pub mod forward;
pub mod secret;
pub mod server;

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::runtime::Runtime;

use crate::obs::output::diag;
use crate::{auth, config};

pub use server::{ProxyHandle, ProxyStats};

pub const DEFAULT_PROXY_PORT: u16 = 48217;
const REFRESH_TICK: Duration = Duration::from_secs(60);
const REFRESH_THRESHOLD_SECS: u64 = 300;

static HANDLE: OnceLock<ProxyHandle> = OnceLock::new();
static RUNTIME: OnceLock<Arc<Runtime>> = OnceLock::new();

fn runtime() -> std::io::Result<&'static Arc<Runtime>> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("cowork-rt")
        .enable_all()
        .build()?;
    let _ = RUNTIME.set(Arc::new(rt));
    #[allow(clippy::expect_used)]
    Ok(RUNTIME.get().expect("RUNTIME populated on the previous line"))
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
    let handle = match server::start(rt, DEFAULT_PROXY_PORT, &gateway) {
        Ok(h) => h,
        Err(e) => {
            diag(&format!("proxy: bind failed on {DEFAULT_PROXY_PORT}: {e}"));
            return None;
        },
    };
    diag(&format!("proxy: listening on localhost:{}", handle.port));

    rt.spawn(refresh_loop());

    let _ = HANDLE.set(handle);
    HANDLE.get()
}

pub fn handle() -> Option<&'static ProxyHandle> {
    HANDLE.get()
}

async fn refresh_loop() {
    let mut interval = tokio::time::interval(REFRESH_TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    interval.tick().await;
    loop {
        interval.tick().await;
        let _ = tokio::task::spawn_blocking(|| {
            let cfg = config::load();
            let _ = auth::read_or_refresh(&cfg, REFRESH_THRESHOLD_SECS);
        })
        .await;
    }
}
