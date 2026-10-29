//! The Tokio runtime the context owns, shut down in the background on drop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use tokio::runtime::{Handle, Runtime};

// Why: Tokio runtime drop panics inside an async task; the final owner may be
// dropped there.
pub(super) struct OwnedRuntime {
    runtime: Option<Runtime>,
    handle: Handle,
}

impl OwnedRuntime {
    pub(super) fn build() -> std::io::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_thread_count())
            .thread_name("bridge-rt")
            .enable_all()
            .build()?;
        let handle = rt.handle().clone();
        Ok(Self {
            runtime: Some(rt),
            handle,
        })
    }

    pub(super) const fn handle(&self) -> &Handle {
        &self.handle
    }
}

impl Drop for OwnedRuntime {
    fn drop(&mut self) {
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_background();
        }
    }
}

fn worker_thread_count() -> usize {
    std::thread::available_parallelism().map_or(2, |n| (n.get() / 2).max(2))
}
