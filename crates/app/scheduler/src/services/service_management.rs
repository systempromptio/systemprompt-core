//! High-level service-management orchestration: start/stop/cleanup wrappers
//! around `systemprompt_database::ServiceRepository` and the platform's
//! [`ProcessCleanup`] primitive.

use systemprompt_database::{DbPool, ServiceConfig, ServiceRepository};
use tracing::warn;

use super::orchestration::ProcessCleanup;
use crate::error::{SchedulerError, SchedulerResult};

/// Service-management façade combining DB persistence with process control.
#[derive(Clone, Debug)]
pub struct ServiceManagementService {
    service_repo: ServiceRepository,
}

impl ServiceManagementService {
    /// Construct a new instance from a shared [`DbPool`].
    pub fn new(db_pool: &DbPool) -> SchedulerResult<Self> {
        Ok(Self {
            service_repo: ServiceRepository::new(db_pool).map_err(SchedulerError::Other)?,
        })
    }

    /// List service rows whose `module_name` equals the supplied filter.
    pub async fn get_services_by_type(
        &self,
        module_name: &str,
    ) -> SchedulerResult<Vec<ServiceConfig>> {
        self.service_repo
            .get_services_by_type(module_name)
            .await
            .map_err(SchedulerError::Other)
    }

    /// List services whose status is `running` and that recorded a PID.
    pub async fn get_running_services_with_pid(&self) -> SchedulerResult<Vec<ServiceConfig>> {
        self.service_repo
            .get_running_services_with_pid()
            .await
            .map_err(SchedulerError::Other)
    }

    /// Update a service's status to `stopped` and clear its PID.
    pub async fn mark_service_stopped(&self, service_name: &str) -> SchedulerResult<()> {
        self.service_repo
            .update_service_stopped(service_name)
            .await
            .map_err(SchedulerError::Other)
    }

    /// Delete service rows with no live process backing them.
    pub async fn cleanup_stale_entries(&self) -> SchedulerResult<u64> {
        self.service_repo
            .cleanup_stale_entries()
            .await
            .map_err(SchedulerError::Other)
    }

    /// Stop the supplied service, optionally forcing termination.
    pub async fn stop_service(&self, service: &ServiceConfig, force: bool) -> SchedulerResult<()> {
        if let Some(pid) = service.pid {
            if force {
                ProcessCleanup::kill_process(pid as u32);
            } else {
                ProcessCleanup::terminate_gracefully(pid as u32, 100).await;
            }
        }

        ProcessCleanup::kill_port(service.port as u16);
        if let Err(e) = self.mark_service_stopped(&service.name).await {
            warn!(service = %service.name, error = %e, "Failed to mark service stopped");
        }
        Ok(())
    }

    /// Detect orphaned service processes and reap them, returning whether
    /// any cleanup action took place.
    pub async fn cleanup_orphaned_service(&self, service: &ServiceConfig) -> SchedulerResult<bool> {
        if let Some(pid) = service.pid {
            let pid = pid as u32;

            if !ProcessCleanup::process_exists(pid) {
                if let Err(e) = self.mark_service_stopped(&service.name).await {
                    warn!(service = %service.name, error = %e, "Failed to mark orphaned service stopped");
                }
                return Ok(true);
            }

            ProcessCleanup::terminate_gracefully(pid, 100).await;
            ProcessCleanup::kill_port(service.port as u16);
            if let Err(e) = self.mark_service_stopped(&service.name).await {
                warn!(service = %service.name, error = %e, "Failed to mark terminated service stopped");
            }
            return Ok(true);
        }
        Ok(false)
    }
}
