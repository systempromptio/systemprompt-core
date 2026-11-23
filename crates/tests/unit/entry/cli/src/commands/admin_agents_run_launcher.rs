//! `admin agent-host run` resolves its agent-run launcher from the host's
//! extension state: a state carrying a `DynAgentRunLauncher` yields
//! `RunLauncher::Linked` with that launcher, and a state without one (an
//! install without agent-runs, or a profile that disables it) yields
//! `RunLauncher::Absent`.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_cli::admin::agent_host::run::RunLauncher;
use systemprompt_identifiers::{AgentRunId, TaskId};
use systemprompt_traits::{
    AgentRunLaunch, AgentRunLaunchError, AgentRunLauncher, Dependencies, DynAgentRunLauncher,
};

struct RecordingLauncher;

#[async_trait]
impl AgentRunLauncher for RecordingLauncher {
    async fn enqueue(&self, _request: AgentRunLaunch) -> Result<AgentRunId, AgentRunLaunchError> {
        Ok(AgentRunId::generate())
    }

    async fn cancel(&self, _run_id: &AgentRunId) -> Result<(), AgentRunLaunchError> {
        Ok(())
    }

    async fn run_for_task(
        &self,
        _task_id: &TaskId,
    ) -> Result<Option<AgentRunId>, AgentRunLaunchError> {
        Ok(None)
    }
}

#[test]
fn a_state_without_a_launcher_resolves_absent() {
    let resolved = RunLauncher::resolve(&Dependencies::new());
    assert!(matches!(resolved, RunLauncher::Absent));
    assert!(!resolved.is_linked());
}

#[test]
fn a_state_with_a_launcher_resolves_linked_to_that_launcher() {
    let launcher: DynAgentRunLauncher = Arc::new(RecordingLauncher);
    let extensions = Dependencies::new().with(Arc::clone(&launcher));
    match RunLauncher::resolve(&extensions) {
        RunLauncher::Linked(found) => assert!(Arc::ptr_eq(&found, &launcher)),
        RunLauncher::Absent => panic!("the provided launcher was not found"),
    }
}
