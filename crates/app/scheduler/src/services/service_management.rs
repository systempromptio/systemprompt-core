use anyhow::Result;
use systemprompt_core_database::{DbPool, ServiceConfig, ServiceRepository};

use super::orchestration::ProcessCleanup;

#[derive(Clone, Debug)]
pub struct ServiceManagementService {
    service_repo: ServiceRepository,
}

impl ServiceManagementService {
    pub const fn new(db_pool: DbPool) -> Self {
        Self {
            service_repo: ServiceRepository::new(db_pool),
        }
    }

    pub async fn get_services_by_type(&self, module_name: &str) -> Result<Vec<ServiceConfig>> {
        self.service_repo.get_services_by_type(module_name).await
    }

    pub async fn get_running_services_with_pid(&self) -> Result<Vec<ServiceConfig>> {
        self.service_repo.get_running_services_with_pid().await
    }

    pub async fn mark_service_stopped(&self, service_name: &str) -> Result<()> {
        self.service_repo.update_service_stopped(service_name).await
    }

    pub async fn cleanup_stale_entries(&self) -> Result<u64> {
        self.service_repo.cleanup_stale_entries().await
    }

    pub async fn stop_service(&self, service: &ServiceConfig, force: bool) -> Result<()> {
        if let Some(pid) = service.pid {
            if force {
                ProcessCleanup::kill_process(pid as u32);
            } else {
                ProcessCleanup::terminate_gracefully(pid as u32, 100).await;
            }
        }

        ProcessCleanup::kill_port(service.port as u16);
        self.mark_service_stopped(&service.name).await.ok();
        Ok(())
    }

    pub async fn cleanup_orphaned_service(&self, service: &ServiceConfig) -> Result<bool> {
        if let Some(pid) = service.pid {
            let pid = pid as u32;

            if !ProcessCleanup::process_exists(pid) {
                self.mark_service_stopped(&service.name).await.ok();
                return Ok(true);
            }

            ProcessCleanup::terminate_gracefully(pid, 100).await;
            ProcessCleanup::kill_port(service.port as u16);
            self.mark_service_stopped(&service.name).await.ok();
            return Ok(true);
        }
        Ok(false)
    }
}
