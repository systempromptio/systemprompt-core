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

use tokio::runtime::{Handle, Runtime};
use tokio::task::JoinHandle;

use crate::proxy::ProxyHandle;

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
        let proxy = match mode {
            ProxyMode::Serve => ProxyHandle::serve(runtime.handle()),
            ProxyMode::Attach => ProxyHandle::attach(runtime.handle()),
        };
        Ok(Arc::new(Self { runtime, proxy }))
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
