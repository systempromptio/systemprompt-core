//! High-level service-management orchestration: start/stop/cleanup wrappers
//! around `systemprompt_database::ServiceRepository` and the marker-verified
//! stops in [`super::orchestration::supervision`].
//!
//! A recorded pid is signalled only through
//! [`systemprompt_loader::subprocess::stop_owned`], which confirms the live
//! process still carries this service's spawn marker — registry pids outlive
//! the processes that minted them and are recycled by the kernel, so an
//! unverified pid is cleared without signalling and reported as
//! [`StopOutcome::NotOurs`]. A port holder is stopped on a service's behalf
//! only when it carries the same marker. The API port stops
//! ([`ServiceManagementService::stop_api_by_port`], the API sweep in
//! [`ServiceManagementService::cleanup_all_orphans`]) signal only a listener
//! carrying the API server marker; any other listener is reported as
//! [`StopOutcome::NotOurs`] and left running.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_database::{ServiceConfig, ServiceRepository};
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, StopOutcome};
use systemprompt_models::services::ServiceModule;
use tracing::warn;

use super::orchestration::{
    ApiListenerStop, child_kind, stop_api_listeners, stop_owned_port_holders, wait_for_port_free,
};
use crate::error::{SchedulerError, SchedulerResult};

const STOP_GRACE: Duration = Duration::from_millis(100);
const API_PORT_RELEASE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrphanDisposition {
    StaleEntry,
    Stopped,
    NotOurs,
}

#[derive(Debug, Clone)]
pub struct OrphanOutcome {
    pub name: ServiceName,
    pub pid: i32,
    pub port: i32,
    pub disposition: OrphanDisposition,
}

#[derive(Debug, Clone, Default)]
pub struct OrphanCleanupReport {
    pub outcomes: Vec<OrphanOutcome>,
    pub api: Vec<ApiListenerStop>,
    pub stale_entries_removed: u64,
}

impl OrphanCleanupReport {
    #[must_use]
    pub fn services_cleaned(&self) -> usize {
        self.outcomes.len() + self.api_stopped().count()
    }

    pub fn api_stopped(&self) -> impl Iterator<Item = &ApiListenerStop> {
        self.api
            .iter()
            .filter(|stop| matches!(stop.outcome, StopOutcome::Stopped(_)))
    }

    pub fn api_not_ours(&self) -> impl Iterator<Item = &ApiListenerStop> {
        self.api
            .iter()
            .filter(|stop| stop.outcome == StopOutcome::NotOurs)
    }
}

#[derive(Clone, Debug)]
pub struct ServiceManagementService {
    service_repo: ServiceRepository,
}

impl ServiceManagementService {
    pub const fn new(service_repo: ServiceRepository) -> Self {
        Self { service_repo }
    }

    pub async fn get_services_by_type(
        &self,
        module_name: ServiceModule,
    ) -> SchedulerResult<Vec<ServiceConfig>> {
        self.service_repo
            .list_services_by_type(module_name)
            .await
            .map_err(SchedulerError::from)
    }

    pub async fn get_running_services_with_pid(&self) -> SchedulerResult<Vec<ServiceConfig>> {
        self.service_repo
            .list_running_services_with_pid()
            .await
            .map_err(SchedulerError::from)
    }

    pub async fn mark_service_stopped(&self, service_name: &ServiceName) -> SchedulerResult<()> {
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

    pub async fn stop_service(
        &self,
        service: &ServiceConfig,
        force: bool,
    ) -> SchedulerResult<StopOutcome> {
        self.stop_recorded(service, stop_grace(force)).await
    }

    pub async fn cleanup_orphaned_service(
        &self,
        service: &ServiceConfig,
    ) -> SchedulerResult<Option<StopOutcome>> {
        if stored_pid(service).is_none() {
            return Ok(None);
        }
        self.stop_recorded(service, STOP_GRACE).await.map(Some)
    }

    pub async fn stop_api_by_port(port: u16, force: bool) -> SchedulerResult<Vec<ApiListenerStop>> {
        let stops = stop_api_listeners(port, stop_grace(force)).await?;
        await_api_port_release(port, &stops).await?;
        Ok(stops)
    }

    pub async fn cleanup_all_orphans(&self, api_port: u16) -> SchedulerResult<OrphanCleanupReport> {
        let running_services = self.get_running_services_with_pid().await?;

        let mut outcomes = Vec::with_capacity(running_services.len());
        for service in &running_services {
            let Some(pid) = service.pid else { continue };

            let disposition = match self.stop_recorded(service, STOP_GRACE).await? {
                StopOutcome::Stopped(_) => OrphanDisposition::Stopped,
                StopOutcome::NotOurs => OrphanDisposition::NotOurs,
                StopOutcome::NotRunning => OrphanDisposition::StaleEntry,
            };
            outcomes.push(OrphanOutcome {
                name: service.name.clone(),
                pid,
                port: service.port,
                disposition,
            });
        }

        let api = stop_api_listeners(api_port, STOP_GRACE).await?;
        await_api_port_release(api_port, &api).await?;

        let stale_entries_removed = match self.cleanup_stale_entries().await {
            Ok(removed) => removed,
            Err(e) => {
                warn!(error = %e, "Failed to clean stale service entries");
                0
            },
        };

        Ok(OrphanCleanupReport {
            outcomes,
            api,
            stale_entries_removed,
        })
    }

    async fn stop_recorded(
        &self,
        service: &ServiceConfig,
        grace: Duration,
    ) -> SchedulerResult<StopOutcome> {
        let kind = child_kind(service.module_name);
        let outcome = match stored_pid(service) {
            Some(pid) => subprocess::stop_owned(pid, kind, &service.name, grace).await?,
            None => StopOutcome::NotRunning,
        };
        if let Some(port) = service_port(service) {
            stop_owned_port_holders(port, kind, &service.name, grace).await?;
        }

        if let Err(e) = self.mark_service_stopped(&service.name).await {
            warn!(service = %service.name, error = %e, "Failed to mark service stopped");
        }
        Ok(outcome)
    }
}

async fn await_api_port_release(port: u16, stops: &[ApiListenerStop]) -> SchedulerResult<()> {
    if stops
        .iter()
        .any(|stop| stop.outcome == StopOutcome::NotOurs)
    {
        return Ok(());
    }
    wait_for_port_free(port, API_PORT_RELEASE).await
}

const fn stop_grace(force: bool) -> Duration {
    if force { Duration::ZERO } else { STOP_GRACE }
}

fn stored_pid(service: &ServiceConfig) -> Option<u32> {
    service.pid.and_then(|pid| u32::try_from(pid).ok())
}

fn service_port(service: &ServiceConfig) -> Option<u16> {
    u16::try_from(service.port).ok().filter(|port| *port != 0)
}
