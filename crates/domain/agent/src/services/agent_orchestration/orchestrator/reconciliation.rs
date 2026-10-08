//! Startup reconciliation of recorded agent state against live processes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_traits::StartupEventSender;

use super::AgentOrchestrator;
use crate::services::agent_orchestration::OrchestrationResult;

impl AgentOrchestrator {
    pub(super) async fn startup_reconciliation(
        &self,
        _events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<()> {
        tracing::debug!("Performing startup reconciliation");

        let reconciled = self.reconciler.reconcile_running_services().await?;

        let report = self.reconciler.perform_consistency_check().await?;
        if report.has_inconsistencies() {
            let fixed = self.reconciler.fix_inconsistencies(&report).await?;
            tracing::info!(fixed = %fixed, "Fixed inconsistencies");
        }

        if reconciled > 0 {
            tracing::info!(fixed = %reconciled, "Startup reconciliation complete");
        } else {
            tracing::debug!("Startup reconciliation complete - no issues found");
        }

        Ok(())
    }
}
