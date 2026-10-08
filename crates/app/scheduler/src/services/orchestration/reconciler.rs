//! Reconciler — given a list of [`ServiceConfig`] and a `start_service`
//! callback, drives runtime state to match desired state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;
use std::time::Duration;

use systemprompt_database::{DbPool, ServiceRepository};
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::subprocess::{self, ChildKind};

use super::service_records::ServiceConfig;
use super::state_types::{ServiceAction, ServiceType};
use super::state_verifier::ServiceStateVerifier;
use super::supervision::{stop_owned_port_holders, wait_for_port_free};
use super::verified_state::VerifiedServiceState;
use crate::error::SchedulerResult;

const STOP_GRACE: Duration = Duration::from_millis(100);
const PORT_RELEASE: Duration = Duration::from_secs(1);

const fn child_kind_of(service_type: ServiceType) -> Option<ChildKind> {
    match service_type {
        ServiceType::Agent => Some(ChildKind::Agent),
        ServiceType::Mcp => Some(ChildKind::Mcp),
        ServiceType::Api => None,
    }
}

#[derive(Debug, Default)]
pub struct ReconciliationResult {
    pub started: Vec<ServiceName>,
    pub stopped: Vec<ServiceName>,
    pub restarted: Vec<ServiceName>,
    pub cleaned_up: Vec<ServiceName>,
    pub failed: Vec<(ServiceName, String)>,
}

impl ReconciliationResult {
    pub const fn new() -> Self {
        Self {
            started: Vec::new(),
            stopped: Vec::new(),
            restarted: Vec::new(),
            cleaned_up: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub const fn is_success(&self) -> bool {
        self.failed.is_empty()
    }

    pub const fn total_actions(&self) -> usize {
        self.started.len() + self.stopped.len() + self.restarted.len() + self.cleaned_up.len()
    }
}

#[derive(Debug)]
pub struct ServiceReconciler {
    state_verifier: ServiceStateVerifier,
    services: ServiceRepository,
}

impl ServiceReconciler {
    pub fn new(db_pool: DbPool, services: ServiceRepository) -> Self {
        Self {
            state_verifier: ServiceStateVerifier::new(db_pool, services.instance_id().clone()),
            services,
        }
    }

    pub async fn reconcile<F, Fut>(
        &self,
        configs: &[ServiceConfig],
        start_service: F,
    ) -> SchedulerResult<ReconciliationResult>
    where
        F: Fn(ServiceName, u16) -> Fut + Send + Sync,
        Fut: Future<Output = SchedulerResult<()>> + Send,
    {
        let states = self.state_verifier.get_verified_states(configs).await?;
        let mut result = ReconciliationResult::new();

        for state in states {
            self.execute_action(state, &start_service, &mut result)
                .await;
        }

        Ok(result)
    }

    async fn execute_action<F, Fut>(
        &self,
        state: VerifiedServiceState,
        start_service: &F,
        result: &mut ReconciliationResult,
    ) where
        F: Fn(ServiceName, u16) -> Fut + Send + Sync,
        Fut: Future<Output = SchedulerResult<()>> + Send,
    {
        match state.needs_action {
            ServiceAction::None => {},
            ServiceAction::Start => {
                self.handle_start(state, start_service, result).await;
            },
            ServiceAction::Stop => {
                self.handle_stop(state, result).await;
            },
            ServiceAction::Restart => {
                self.handle_restart(state, start_service, result).await;
            },
            ServiceAction::CleanupDb => {
                self.handle_cleanup_db(state, result).await;
            },
            ServiceAction::CleanupProcess => {
                self.handle_cleanup_process(state, result).await;
            },
        }
    }

    async fn handle_start<F, Fut>(
        &self,
        state: VerifiedServiceState,
        start_service: &F,
        result: &mut ReconciliationResult,
    ) where
        F: Fn(ServiceName, u16) -> Fut + Send + Sync,
        Fut: Future<Output = SchedulerResult<()>> + Send,
    {
        match start_service(state.name.clone(), state.port).await {
            Ok(()) => result.started.push(state.name),
            Err(e) => result.failed.push((state.name, e.to_string())),
        }
    }

    async fn handle_stop(&self, state: VerifiedServiceState, result: &mut ReconciliationResult) {
        match self.stop_service(&state).await {
            Ok(()) => result.stopped.push(state.name),
            Err(e) => result.failed.push((state.name, e.to_string())),
        }
    }

    async fn handle_restart<F, Fut>(
        &self,
        state: VerifiedServiceState,
        start_service: &F,
        result: &mut ReconciliationResult,
    ) where
        F: Fn(ServiceName, u16) -> Fut + Send + Sync,
        Fut: Future<Output = SchedulerResult<()>> + Send,
    {
        if let Err(e) = self.stop_service(&state).await {
            result.failed.push((state.name, e.to_string()));
            return;
        }
        match start_service(state.name.clone(), state.port).await {
            Ok(()) => result.restarted.push(state.name),
            Err(e) => result.failed.push((state.name, e.to_string())),
        }
    }

    async fn handle_cleanup_db(
        &self,
        state: VerifiedServiceState,
        result: &mut ReconciliationResult,
    ) {
        match self.cleanup_db_entry(&state.name).await {
            Ok(()) => result.cleaned_up.push(state.name),
            Err(e) => result.failed.push((state.name, e.to_string())),
        }
    }

    async fn handle_cleanup_process(
        &self,
        state: VerifiedServiceState,
        result: &mut ReconciliationResult,
    ) {
        if let Err(e) = self.cleanup_process(&state).await {
            result.failed.push((state.name, e.to_string()));
            return;
        }
        match self.cleanup_db_entry(&state.name).await {
            Ok(()) => result.cleaned_up.push(state.name),
            Err(e) => result.failed.push((state.name, e.to_string())),
        }
    }

    async fn stop_service(&self, state: &VerifiedServiceState) -> SchedulerResult<()> {
        self.cleanup_process(state).await?;
        wait_for_port_free(state.port, PORT_RELEASE).await?;
        self.update_service_stopped(&state.name).await
    }

    async fn cleanup_process(&self, state: &VerifiedServiceState) -> SchedulerResult<()> {
        let Some(kind) = child_kind_of(state.service_type) else {
            return Ok(());
        };
        if let Some(pid) = state.pid {
            subprocess::stop_owned(pid, kind, &state.name, STOP_GRACE).await?;
        }
        stop_owned_port_holders(state.port, kind, &state.name, STOP_GRACE).await?;
        Ok(())
    }

    async fn cleanup_db_entry(&self, name: &ServiceName) -> SchedulerResult<()> {
        self.services.delete_service(name).await?;
        Ok(())
    }

    async fn update_service_stopped(&self, name: &ServiceName) -> SchedulerResult<()> {
        self.services.update_service_stopped(name).await?;
        Ok(())
    }
}
