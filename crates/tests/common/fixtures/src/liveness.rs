//! A [`ProcessLiveness`] whose verdicts are the test's own: the pids it was
//! handed are ours, every other pid is dead or a stranger.

use std::collections::HashSet;

use async_trait::async_trait;
use systemprompt_identifiers::ServiceName;
use systemprompt_traits::ProcessLiveness;

#[derive(Debug, Clone, Default)]
pub struct LivePids(HashSet<u32>);

impl LivePids {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn of<I: IntoIterator<Item = u32>>(pids: I) -> Self {
        Self(pids.into_iter().collect())
    }
}

#[async_trait]
impl ProcessLiveness for LivePids {
    async fn owns(&self, pid: u32, _service: &ServiceName) -> bool {
        self.0.contains(&pid)
    }
}
