//! [`A2aRunHandler`]: one claimed A2A-origin run from claim to finish.
//!
//! The handler decodes the stored [`RunRequest`], creates the run directory,
//! issues the run's credentials to the row's user, writes the governance
//! hooks file when the request names a plugin, executes the host and
//! finishes the row with its outcome. Credentials are revoked whatever
//! happens. A failure before the outcome is recorded still finishes the row
//! (`failed`, `host_crashed`), so no run and no task is left in flight.
//!
//! A `tasks/cancel` from A2A records `cancel_requested_at`; the claim loop
//! cancels a run only on shutdown or a lost lease, so the handler polls that
//! column and cancels the run token. [`A2aRunHandler::with_host`] pins the
//! host adapter instead of resolving it from the request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use super::{A2aRunnerConfig, credential_request};
use crate::credentials::{RunCredentialService, RunCredentials};
use crate::error::AgentRunsResult;
use crate::execute::{ExecuteParams, execute, execute_with_host};
use crate::governance::hooks_file;
use crate::host::HeadlessHost;
use crate::layout::RunLayout;
use crate::outcome::{ErrorClass, RunOutcome, RunStatus};
use crate::repository::RunStarted;
use crate::request::RunRequest;
use crate::service::{ClaimedRun, DbEventSink, RunHandler};

const CANCEL_POLL: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct A2aRunHandler {
    credentials: RunCredentialService,
    config: Arc<A2aRunnerConfig>,
    host: Option<Arc<dyn HeadlessHost>>,
}

impl std::fmt::Debug for A2aRunHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aRunHandler")
            .field("config", &self.config)
            .field("host_pinned", &self.host.is_some())
            .finish_non_exhaustive()
    }
}

impl A2aRunHandler {
    #[must_use]
    pub fn new(credentials: RunCredentialService, config: A2aRunnerConfig) -> Self {
        Self {
            credentials,
            config: Arc::new(config),
            host: None,
        }
    }

    #[must_use]
    pub fn with_host(mut self, host: Arc<dyn HeadlessHost>) -> Self {
        self.host = Some(host);
        self
    }

    async fn drive(&self, claimed: &ClaimedRun) -> AgentRunsResult<()> {
        let row = &claimed.row;
        let request: RunRequest = serde_json::from_value(row.request.clone())?;
        if claimed.repo.find_cancel_requested(&row.id).await? {
            let outcome = unexecuted(RunStatus::Cancelled, ErrorClass::Ok, "cancelled before start");
            claimed.repo.finish(&row.id, &outcome, None).await?;
            return Ok(());
        }
        let layout = self.config.layout(&row.id);
        layout.create().await?;
        let credentials = self
            .credentials
            .issue(credential_request(&row.user_id, &row.id, &request))
            .await?;
        let result = self
            .run_host(claimed, &request, &layout, &credentials)
            .await;
        if let Err(error) = self.credentials.revoke(&credentials).await {
            tracing::warn!(run_id = %row.id, %error, "A2A run credentials not revoked");
        }
        result
    }

    async fn run_host(
        &self,
        claimed: &ClaimedRun,
        request: &RunRequest,
        layout: &RunLayout,
        credentials: &RunCredentials,
    ) -> AgentRunsResult<()> {
        let id = &claimed.row.id;
        let started = RunStarted {
            run_dir: &layout.root,
            pid: None,
            gateway_session_id: Some(&credentials.gateway_session_id),
            api_key_id: Some(&credentials.api_key_id),
        };
        claimed.repo.update_started(id, started).await?;
        let hooks = match (&request.plugin_id, &credentials.hook_token) {
            (Some(plugin_id), Some(token)) => {
                hooks_file(request.host, plugin_id, token, &self.config.hooks, layout).await?
            },
            _ => None,
        };
        let sink = DbEventSink::new(claimed.repo.clone(), id.clone());
        let existing = claimed.repo.count_events(id).await?;
        let params = ExecuteParams {
            request,
            layout,
            credentials,
            gateway_base_url: &self.config.gateway_base_url,
            hooks_file: hooks,
            seq_after: (existing > 0).then(|| existing - 1),
            sink: &sink,
            cancel: claimed.run_token.clone(),
        };
        let watcher = watch_cancel(claimed);
        let report = match &self.host {
            Some(host) => execute_with_host(host.as_ref(), params).await,
            None => execute(params).await,
        };
        watcher.abort();
        let report = report?;
        let diff = serde_json::to_value(&report.diff)?;
        claimed.repo.finish(id, &report.outcome, Some(&diff)).await?;
        Ok(())
    }
}

#[async_trait]
impl RunHandler for A2aRunHandler {
    async fn run(&self, claimed: ClaimedRun) -> AgentRunsResult<()> {
        let Err(error) = self.drive(&claimed).await else {
            return Ok(());
        };
        let id = &claimed.row.id;
        tracing::error!(run_id = %id, %error, "A2A run failed before it finished");
        let outcome = unexecuted(RunStatus::Failed, ErrorClass::HostCrashed, &error.to_string());
        claimed.repo.finish(id, &outcome, None).await?;
        Ok(())
    }
}

fn unexecuted(status: RunStatus, error_class: ErrorClass, error: &str) -> RunOutcome {
    RunOutcome {
        status,
        error_class,
        session_id: None,
        cost_microdollars: 0,
        input_tokens: 0,
        output_tokens: 0,
        turns: 0,
        duration_ms: 0,
        structured_output: None,
        exit_code: None,
        error: Some(error.to_owned()),
        final_text: None,
    }
}

fn watch_cancel(claimed: &ClaimedRun) -> tokio::task::JoinHandle<()> {
    let repo = claimed.repo.clone();
    let token = claimed.run_token.clone();
    let id = claimed.row.id.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                () = token.cancelled() => return,
                () = tokio::time::sleep(CANCEL_POLL) => {},
            }
            match repo.find_cancel_requested(&id).await {
                Ok(true) => {
                    token.cancel();
                    return;
                },
                Ok(false) => {},
                Err(error) => tracing::warn!(run_id = %id, %error, "A2A run cancel poll failed"),
            }
        }
    })
}
