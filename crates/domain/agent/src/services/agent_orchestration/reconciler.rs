//! Reconciliation of recorded agent state against the actual process table.
//!
//! [`AgentReconciler`] detects drift — agents marked running whose process has
//! died — produces a [`ConsistencyReport`], and repairs the discrepancies by
//! marking affected agents failed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::repository::agent_service::AgentServiceRepository;
use crate::services::agent_orchestration::OrchestrationResult;
use crate::services::agent_orchestration::database::AgentDatabaseService;
use systemprompt_identifiers::AgentName;

#[derive(Debug)]
pub struct AgentReconciler {
    db_service: AgentDatabaseService,
}

impl AgentReconciler {
    pub fn new(agent_service_repo: AgentServiceRepository) -> OrchestrationResult<Self> {
        let db_service = AgentDatabaseService::new(agent_service_repo)?;

        Ok(Self { db_service })
    }

    #[must_use]
    pub const fn with_db_service(db_service: AgentDatabaseService) -> Self {
        Self { db_service }
    }

    pub async fn reconcile_running_services(&self) -> OrchestrationResult<u32> {
        tracing::debug!("Reconciling running services with actual processes");

        let all_agents = self.db_service.list_all_agents().await?;
        let mut reconciled = 0;

        for (agent_name, status) in all_agents {
            match status {
                crate::services::agent_orchestration::AgentStatus::Running { pid, .. } => {
                    if !systemprompt_loader::subprocess::is_running(pid).await {
                        tracing::warn!(
                            agent_name = %agent_name,
                            pid = %pid,
                            "Agent marked as running but process not found - marking as failed"
                        );
                        self.db_service.mark_failed(&agent_name).await?;
                        reconciled += 1;
                    }
                },
                crate::services::agent_orchestration::AgentStatus::Failed { .. } => {},
            }
        }

        if reconciled > 0 {
            tracing::info!(reconciled = %reconciled, "Reconciled services");
        } else {
            tracing::debug!("All services are correctly synchronized");
        }

        Ok(reconciled)
    }

    pub async fn perform_consistency_check(&self) -> OrchestrationResult<ConsistencyReport> {
        tracing::debug!("Performing database consistency check");

        let mut report = ConsistencyReport::new();
        let all_agents = self.db_service.list_all_agents().await?;

        for (agent_name, status) in all_agents {
            match status {
                crate::services::agent_orchestration::AgentStatus::Running { pid, .. } => {
                    if systemprompt_loader::subprocess::is_running(pid).await {
                        report.consistent_running.push(agent_name);
                    } else {
                        report.inconsistent_running.push((agent_name, pid));
                    }
                },
                crate::services::agent_orchestration::AgentStatus::Failed { .. } => {
                    report.failed.push(agent_name);
                },
            }
        }

        report.log_summary();
        Ok(report)
    }

    pub async fn fix_inconsistencies(
        &self,
        report: &ConsistencyReport,
    ) -> OrchestrationResult<u32> {
        let mut fixed = 0;

        for (agent_name, pid) in &report.inconsistent_running {
            tracing::warn!(agent_name = %agent_name, pid = %pid, "Fixing inconsistent agent");
            self.db_service.mark_failed(agent_name).await?;
            fixed += 1;
        }

        if fixed > 0 {
            tracing::info!(fixed = %fixed, "Fixed inconsistencies");
        }

        Ok(fixed)
    }
}

#[derive(Debug)]
pub struct ConsistencyReport {
    pub consistent_running: Vec<AgentName>,
    pub inconsistent_running: Vec<(AgentName, u32)>,
    pub failed: Vec<AgentName>,
}

impl Default for ConsistencyReport {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsistencyReport {
    pub const fn new() -> Self {
        Self {
            consistent_running: Vec::new(),
            inconsistent_running: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub const fn has_inconsistencies(&self) -> bool {
        !self.inconsistent_running.is_empty()
    }

    pub const fn total_agents(&self) -> usize {
        self.consistent_running.len() + self.inconsistent_running.len() + self.failed.len()
    }

    pub fn log_summary(&self) {
        tracing::debug!(
            consistent_running = %self.consistent_running.len(),
            inconsistent_running = %self.inconsistent_running.len(),
            failed = %self.failed.len(),
            "Consistency check results"
        );

        if self.has_inconsistencies() {
            tracing::warn!("Inconsistencies detected - run fix_inconsistencies() to repair");
        } else {
            tracing::debug!("All services are consistent");
        }
    }
}
