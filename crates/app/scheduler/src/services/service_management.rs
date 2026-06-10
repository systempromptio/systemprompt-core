//! High-level service-management orchestration: start/stop/cleanup wrappers
//! around `systemprompt_database::ServiceRepository` and the platform's
//! [`ProcessCleanup`] primitive.
//!
//! Stored PIDs are signalled only after
//! [`systemprompt_models::subprocess::live_pid_is_subprocess`] confirms the
//! live process still carries this installation's spawn markers — registry
//! PIDs outlive the processes that minted them and are recycled by the
//! kernel, so an unverified PID is cleared without signalling. Port-derived
//! PIDs ([`ServiceManagementService::stop_api_by_port`], the API sweep in
//! [`ServiceManagementService::cleanup_all_orphans`]) carry no service
//! identity and stay unverified by design.

use systemprompt_database::{DbPool, ServiceConfig, ServiceRepository};
use systemprompt_models::subprocess::{AGENT_NAME_ENV, MCP_SERVICE_ID_ENV, live_pid_is_subprocess};
use tracing::warn;

use super::orchestration::ProcessCleanup;
use crate::error::{SchedulerError, SchedulerResult};

const STOP_GRACE_MS: u64 = 100;
const API_SERVE_PATTERN: &str = "systemprompt serve api";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanDisposition {
    StaleEntry,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct OrphanOutcome {
    pub name: String,
    pub pid: i32,
    pub port: i32,
    pub disposition: OrphanDisposition,
}

#[derive(Debug, Clone, Default)]
pub struct OrphanCleanupReport {
    pub outcomes: Vec<OrphanOutcome>,
    pub api_stopped: bool,
    pub stale_entries_removed: u64,
}

impl OrphanCleanupReport {
    #[must_use]
    pub fn services_cleaned(&self) -> usize {
        self.outcomes.len() + usize::from(self.api_stopped)
    }
}

#[derive(Clone, Debug)]
pub struct ServiceManagementService {
    service_repo: ServiceRepository,
}

impl ServiceManagementService {
    pub fn new(db_pool: &DbPool) -> SchedulerResult<Self> {
        Ok(Self {
            service_repo: ServiceRepository::new(db_pool)?,
        })
    }

    pub async fn get_services_by_type(
        &self,
        module_name: &str,
    ) -> SchedulerResult<Vec<ServiceConfig>> {
        self.service_repo
            .get_services_by_type(module_name)
            .await
            .map_err(SchedulerError::from)
    }

    pub async fn get_running_services_with_pid(&self) -> SchedulerResult<Vec<ServiceConfig>> {
        self.service_repo
            .get_running_services_with_pid()
            .await
            .map_err(SchedulerError::from)
    }

    pub async fn mark_service_stopped(&self, service_name: &str) -> SchedulerResult<()> {
        self.service_repo
            .update_service_stopped(service_name)
            .await
            .map_err(SchedulerError::from)
    }

    pub async fn cleanup_stale_entries(&self) -> SchedulerResult<u64> {
        self.service_repo
            .cleanup_stale_entries()
            .await
            .map_err(SchedulerError::from)
    }

    pub async fn stop_service(&self, service: &ServiceConfig, force: bool) -> SchedulerResult<()> {
        if let Some(pid) = stored_pid(service)
            && ProcessCleanup::process_exists(pid)
            && pid_is_our_service(pid, service)
        {
            if force {
                ProcessCleanup::kill_process(pid);
            } else {
                ProcessCleanup::terminate_gracefully(pid, STOP_GRACE_MS).await;
            }
            ProcessCleanup::kill_port(service.port as u16, pid);
        }

        if let Err(e) = self.mark_service_stopped(&service.name).await {
            warn!(service = %service.name, error = %e, "Failed to mark service stopped");
        }
        Ok(())
    }

    pub async fn cleanup_orphaned_service(&self, service: &ServiceConfig) -> SchedulerResult<bool> {
        let Some(pid) = stored_pid(service) else {
            return Ok(false);
        };

        if !ProcessCleanup::process_exists(pid) {
            if let Err(e) = self.mark_service_stopped(&service.name).await {
                warn!(service = %service.name, error = %e, "Failed to mark orphaned service stopped");
            }
            return Ok(true);
        }

        if pid_is_our_service(pid, service) {
            ProcessCleanup::terminate_gracefully(pid, STOP_GRACE_MS).await;
            ProcessCleanup::kill_port(service.port as u16, pid);
        }
        if let Err(e) = self.mark_service_stopped(&service.name).await {
            warn!(service = %service.name, error = %e, "Failed to mark terminated service stopped");
        }
        Ok(true)
    }

    pub async fn stop_api_by_port(port: u16, force: bool) -> SchedulerResult<Option<u32>> {
        let listener = ProcessCleanup::check_port(port);
        if let Some(pid) = listener {
            if force {
                ProcessCleanup::kill_process(pid);
            } else {
                ProcessCleanup::terminate_gracefully(pid, STOP_GRACE_MS).await;
            }
            ProcessCleanup::kill_port(port, pid);
        }

        ProcessCleanup::wait_for_port_free(port, 5, 200).await?;
        Ok(listener)
    }

    pub async fn cleanup_all_orphans(&self, api_port: u16) -> SchedulerResult<OrphanCleanupReport> {
        let running_services = self.get_running_services_with_pid().await?;

        let mut outcomes = Vec::with_capacity(running_services.len());
        for service in &running_services {
            let Some(pid) = service.pid else { continue };

            let disposition = if stored_pid(service).is_some_and(ProcessCleanup::process_exists) {
                self.cleanup_orphaned_service(service).await?;
                OrphanDisposition::Stopped
            } else {
                if let Err(e) = self.mark_service_stopped(&service.name).await {
                    warn!(service = %service.name, error = %e, "mark_service_stopped failed");
                }
                OrphanDisposition::StaleEntry
            };
            outcomes.push(OrphanOutcome {
                name: service.name.clone(),
                pid,
                port: service.port,
                disposition,
            });
        }

        let api_stopped = sweep_api_port(api_port).await?;

        let stale_entries_removed = match self.cleanup_stale_entries().await {
            Ok(removed) => removed,
            Err(e) => {
                warn!(error = %e, "Failed to clean stale service entries");
                0
            },
        };

        Ok(OrphanCleanupReport {
            outcomes,
            api_stopped,
            stale_entries_removed,
        })
    }
}

async fn sweep_api_port(api_port: u16) -> SchedulerResult<bool> {
    let killed = ProcessCleanup::check_port(api_port)
        .map_or_else(Vec::new, |pid| ProcessCleanup::kill_port(api_port, pid));
    ProcessCleanup::kill_by_pattern(API_SERVE_PATTERN);
    ProcessCleanup::wait_for_port_free(api_port, 3, 1000).await?;
    Ok(!killed.is_empty())
}

fn stored_pid(service: &ServiceConfig) -> Option<u32> {
    service.pid.and_then(|pid| u32::try_from(pid).ok())
}

fn pid_is_our_service(pid: u32, service: &ServiceConfig) -> bool {
    let Some(name_key) = subprocess_name_key(&service.module_name) else {
        warn!(
            service = %service.name,
            module = %service.module_name,
            pid,
            "No subprocess identity marker for this module type; refusing to signal stored PID"
        );
        return false;
    };

    if live_pid_is_subprocess(pid, name_key, &service.name) {
        return true;
    }
    warn!(
        service = %service.name,
        pid,
        "Recorded PID is alive but is not our child (recycled/stale); skipping signal"
    );
    false
}

fn subprocess_name_key(module_name: &str) -> Option<&'static str> {
    match module_name {
        "agent" => Some(AGENT_NAME_ENV),
        "mcp" => Some(MCP_SERVICE_ID_ENV),
        _ => None,
    }
}
