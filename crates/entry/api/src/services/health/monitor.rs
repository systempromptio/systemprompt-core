use anyhow::Result;
use std::time::Duration;
use systemprompt_database::{DbPool, ServiceRepository};
use systemprompt_scheduler::ProcessCleanup;
use tokio::task::JoinHandle;
use tracing::{info, warn};

#[derive(Debug)]
pub struct ProcessMonitor {
    db_pool: DbPool,
    monitor_handle: Option<JoinHandle<()>>,
    check_interval: Duration,
}

impl ProcessMonitor {
    pub const fn new(db_pool: DbPool) -> Self {
        Self {
            db_pool,
            monitor_handle: None,
            check_interval: Duration::from_secs(30),
        }
    }

    pub const fn with_interval(db_pool: DbPool, interval: Duration) -> Self {
        Self {
            db_pool,
            monitor_handle: None,
            check_interval: interval,
        }
    }

    pub fn start(&mut self) {
        if self.monitor_handle.is_some() {
            warn!("Process monitor already started");
            return;
        }

        info!("Starting centralized process monitoring");

        let db_pool_clone = std::sync::Arc::clone(&self.db_pool);
        let interval = self.check_interval;

        let handle = tokio::spawn(async move { Self::monitor_loop(db_pool_clone, interval).await });

        self.monitor_handle = Some(handle);
        info!("Centralized process monitoring started");
    }

    pub fn stop(&mut self) {
        if let Some(handle) = self.monitor_handle.take() {
            info!("Stopping process monitoring");
            handle.abort();
            info!("Process monitoring stopped");
        }
    }

    pub const fn is_running(&self) -> bool {
        self.monitor_handle.is_some()
    }

    async fn monitor_loop(db_pool: DbPool, check_interval: Duration) {
        info!(
            interval_secs = check_interval.as_secs(),
            "Process monitor loop started"
        );

        let mut interval = tokio::time::interval(check_interval);

        loop {
            interval.tick().await;

            if let Err(e) = Self::perform_monitoring_cycle(&db_pool).await {
                warn!(error = %e, "Monitoring cycle failed");
            }
        }
    }

    async fn perform_monitoring_cycle(db_pool: &DbPool) -> Result<()> {
        let repository = ServiceRepository::new(std::sync::Arc::clone(db_pool));
        let services = repository.get_running_services_with_pid().await?;

        if services.is_empty() {
            return Ok(());
        }

        let mut healthy_count = 0;
        let mut crashed_count = 0;

        for service in services {
            if let Some(pid) = service.pid {
                let pid = pid as u32;

                if Self::process_exists(pid) {
                    healthy_count += 1;
                } else {
                    repository.mark_service_crashed(&service.name).await?;

                    crashed_count += 1;
                    warn!(
                        module = %service.module_name,
                        service = %service.name,
                        pid = pid,
                        "Detected crashed service"
                    );
                }
            }
        }

        if crashed_count == 0 {
            info!(healthy = healthy_count, "All services healthy");
        } else {
            warn!(
                healthy = healthy_count,
                crashed = crashed_count,
                "Service health check completed with failures"
            );
        }

        Ok(())
    }

    fn process_exists(pid: u32) -> bool {
        ProcessCleanup::process_exists(pid)
    }

    pub async fn health_check_all(&self) -> Result<HealthSummary> {
        info!("Running health check on all services");

        let repository = ServiceRepository::new(std::sync::Arc::clone(&self.db_pool));
        let services = repository.get_running_services_with_pid().await?;

        let mut summary = HealthSummary::default();

        for service in services {
            if let Some(pid) = service.pid {
                let pid = pid as u32;
                let healthy = Self::process_exists(pid);

                info!(
                    module = %service.module_name,
                    service = %service.name,
                    pid = pid,
                    healthy = healthy,
                    "Service health status"
                );

                *summary
                    .modules
                    .entry(service.module_name)
                    .or_insert_with(ModuleHealth::default) += if healthy {
                    ModuleHealth {
                        healthy: 1,
                        crashed: 0,
                    }
                } else {
                    ModuleHealth {
                        healthy: 0,
                        crashed: 1,
                    }
                };
            }
        }

        let total_healthy = summary.modules.values().map(|m| m.healthy).sum::<u32>();
        let total_crashed = summary.modules.values().map(|m| m.crashed).sum::<u32>();

        if total_crashed == 0 {
            info!(healthy = total_healthy, "All services are healthy");
        } else {
            warn!(
                healthy = total_healthy,
                total = total_healthy + total_crashed,
                "Some services are unhealthy"
            );
        }

        Ok(summary)
    }
}

impl Drop for ProcessMonitor {
    fn drop(&mut self) {
        if let Some(handle) = self.monitor_handle.take() {
            handle.abort();
        }
    }
}

#[derive(Debug, Default)]
pub struct HealthSummary {
    pub modules: std::collections::HashMap<String, ModuleHealth>,
}

#[derive(Debug, Default, Copy, Clone)]
pub struct ModuleHealth {
    pub healthy: u32,
    pub crashed: u32,
}

impl std::ops::AddAssign for ModuleHealth {
    fn add_assign(&mut self, other: Self) {
        self.healthy += other.healthy;
        self.crashed += other.crashed;
    }
}

impl HealthSummary {
    pub fn total_healthy(&self) -> u32 {
        self.modules.values().map(|m| m.healthy).sum()
    }

    pub fn total_crashed(&self) -> u32 {
        self.modules.values().map(|m| m.crashed).sum()
    }

    pub fn is_all_healthy(&self) -> bool {
        self.total_crashed() == 0
    }
}
