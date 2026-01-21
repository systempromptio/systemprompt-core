use std::future::Future;
use std::sync::Arc;

use anyhow::Result;
use systemprompt_database::{DatabaseProvider, DatabaseQuery, DbPool};

use super::process_cleanup::ProcessCleanup;
use super::state_manager::{ServiceConfig, ServiceStateManager};
use super::state_types::ServiceAction;
use super::verified_state::VerifiedServiceState;

const DELETE_SERVICE_BY_NAME: DatabaseQuery =
    DatabaseQuery::new("DELETE FROM services WHERE name = $1");
const UPDATE_SERVICE_TO_STOPPED: DatabaseQuery =
    DatabaseQuery::new("UPDATE services SET status = 'stopped', pid = NULL WHERE name = $1");

#[derive(Debug, Default)]
pub struct ReconciliationResult {
    pub started: Vec<String>,
    pub stopped: Vec<String>,
    pub restarted: Vec<String>,
    pub cleaned_up: Vec<String>,
    pub failed: Vec<(String, String)>,
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

    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }

    pub fn total_actions(&self) -> usize {
        self.started.len() + self.stopped.len() + self.restarted.len() + self.cleaned_up.len()
    }
}

#[derive(Debug)]
pub struct ServiceReconciler {
    state_manager: ServiceStateManager,
    db_pool: DbPool,
}

impl ServiceReconciler {
    pub fn new(db_pool: DbPool) -> Self {
        Self {
            state_manager: ServiceStateManager::new(Arc::clone(&db_pool)),
            db_pool,
        }
    }

    pub async fn reconcile<F, Fut>(
        &self,
        configs: &[ServiceConfig],
        start_service: F,
    ) -> Result<ReconciliationResult>
    where
        F: Fn(String, u16) -> Fut + Send + Sync,
        Fut: Future<Output = Result<()>> + Send,
    {
        let states = self.state_manager.get_verified_states(configs).await?;
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
        F: Fn(String, u16) -> Fut + Send + Sync,
        Fut: Future<Output = Result<()>> + Send,
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
        F: Fn(String, u16) -> Fut + Send + Sync,
        Fut: Future<Output = Result<()>> + Send,
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
        F: Fn(String, u16) -> Fut + Send + Sync,
        Fut: Future<Output = Result<()>> + Send,
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
        self.cleanup_process(&state).await;
        match self.cleanup_db_entry(&state.name).await {
            Ok(()) => result.cleaned_up.push(state.name),
            Err(e) => result.failed.push((state.name, e.to_string())),
        }
    }

    async fn stop_service(&self, state: &VerifiedServiceState) -> Result<()> {
        if let Some(pid) = state.pid {
            ProcessCleanup::terminate_gracefully(pid, 100).await;
        }
        ProcessCleanup::kill_port(state.port);
        ProcessCleanup::wait_for_port_free(state.port, 5, 200).await?;
        self.update_service_stopped(&state.name).await
    }

    async fn cleanup_process(&self, state: &VerifiedServiceState) {
        if let Some(pid) = state.pid {
            ProcessCleanup::terminate_gracefully(pid, 100).await;
        }
        ProcessCleanup::kill_port(state.port);
    }

    async fn cleanup_db_entry(&self, name: &str) -> Result<()> {
        self.db_pool
            .as_ref()
            .execute(&DELETE_SERVICE_BY_NAME, &[&name])
            .await?;
        Ok(())
    }

    async fn update_service_stopped(&self, name: &str) -> Result<()> {
        self.db_pool
            .as_ref()
            .execute(&UPDATE_SERVICE_TO_STOPPED, &[&name])
            .await?;
        Ok(())
    }
}
