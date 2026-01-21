use anyhow::Result;
use std::time::Duration;
use systemprompt_traits::StartupEventSender;
use tokio::task::JoinHandle;

use super::AgentOrchestrator;
use crate::services::agent_orchestration::monitor::AgentMonitor;
use crate::services::agent_orchestration::OrchestrationResult;

impl AgentOrchestrator {
    pub async fn run_daemon(&mut self) -> OrchestrationResult<()> {
        tracing::info!("Starting Agent Orchestrator daemon");

        self.start_health_monitoring();

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received shutdown signal");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    if let Err(e) = self.cleanup_crashed_agents().await {
                        tracing::error!(error = %e, "Cleanup error");
                    }
                }
            }
        }

        self.shutdown().await;
        Ok(())
    }

    pub(super) async fn startup_reconciliation(
        &self,
        _events: Option<&StartupEventSender>,
    ) -> OrchestrationResult<()> {
        tracing::debug!("Performing startup reconciliation");

        let reconciled = self.reconciler.reconcile_running_services().await?;
        let started_fixed = self.reconciler.reconcile_starting_services().await?;

        let report = self.reconciler.perform_consistency_check().await?;
        if report.has_inconsistencies() {
            let fixed = self.reconciler.fix_inconsistencies(&report).await?;
            tracing::info!(fixed = %fixed, "Fixed inconsistencies");
        }

        let total_fixed = reconciled + started_fixed;
        if total_fixed > 0 {
            tracing::info!(fixed = %total_fixed, "Startup reconciliation complete");
        } else {
            tracing::debug!("Startup reconciliation complete - no issues found");
        }

        Ok(())
    }

    pub(super) fn start_health_monitoring(&mut self) {
        let pool = self.agent_state.db_pool().clone();

        let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            let monitor = match AgentMonitor::new(pool).await {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to initialize health monitor");
                    return Ok(());
                },
            };

            let mut interval = tokio::time::interval(Duration::from_secs(60));

            interval.tick().await;

            loop {
                interval.tick().await;

                match monitor.monitor_all_agents().await {
                    Ok(report) => {
                        if report.total_agents() > 0 {
                            tracing::debug!(
                                healthy = %report.healthy_agents.len(),
                                total = %report.total_agents(),
                                percentage = %format!("{:.1}", report.healthy_percentage()),
                                "Health check complete"
                            );
                        }
                    },
                    Err(e) => {
                        tracing::error!(error = %e, "Health monitoring error");
                    },
                }

                if let Err(e) = monitor.cleanup_unresponsive_agents(3).await {
                    tracing::error!(error = %e, "Cleanup error");
                }
            }
        });

        self.monitoring_handle = Some(handle);
    }

    pub async fn shutdown(&mut self) {
        tracing::info!("Shutting down Agent Orchestrator");

        if let Some(handle) = self.monitoring_handle.take() {
            handle.abort();
            tracing::debug!("Stopped health monitoring");
        }

        tracing::info!("Agent Orchestrator shutdown complete");
    }
}
