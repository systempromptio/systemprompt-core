pub mod forward;
pub mod secret;
pub mod server;

use std::sync::OnceLock;
use std::time::Duration;

use crate::output::diag;
use crate::{auth, config};

pub use server::{ProxyHandle, ProxyStats};

pub const DEFAULT_PROXY_PORT: u16 = 48217;
const REFRESH_TICK: Duration = Duration::from_secs(60);
const REFRESH_THRESHOLD_SECS: u64 = 300;

static HANDLE: OnceLock<ProxyHandle> = OnceLock::new();

pub fn start_default() -> Option<&'static ProxyHandle> {
    if let Some(h) = HANDLE.get() {
        return Some(h);
    }
    let cfg = config::load();
    let gateway = config::gateway_url_or_default(&cfg);
    let handle = match server::start(DEFAULT_PROXY_PORT, gateway) {
        Ok(h) => h,
        Err(e) => {
            diag(&format!("proxy: bind failed on {DEFAULT_PROXY_PORT}: {e}"));
            return None;
        },
    };
    diag(&format!("proxy: listening on 127.0.0.1:{}", handle.port));

    std::thread::Builder::new()
        .name("cowork-proxy-refresh".into())
        .spawn(refresh_loop)
        .ok();

    let _ = HANDLE.set(handle);
    HANDLE.get()
}

pub fn handle() -> Option<&'static ProxyHandle> {
    HANDLE.get()
}

fn refresh_loop() {
    loop {
        std::thread::sleep(REFRESH_TICK);
        let cfg = config::load();
        let _ = auth::read_or_refresh(&cfg, REFRESH_THRESHOLD_SECS);
    }
}
