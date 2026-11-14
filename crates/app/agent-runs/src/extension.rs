//! Extension-framework registration for the agent-runs crate.
//!
//! [`AgentRunsExtension`] declares the `agent_runs` and
//! `agent_run_events` tables and registers itself with the platform
//! extension registry via [`register_extension!`].
//!
//! It is the crate's composition root: `provide` builds the one
//! [`AgentRunRepository`] and puts it, and the [`DynAgentRunLauncher`] over
//! it, into the extension state. The `admin agents run` child host hands that
//! launcher to its A2A server, the agent-runs routes and the run-directory
//! retention job read the repository, and the one runner service,
//! [`A2aRunnerService`], claims the runs the A2A server queues.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_traits::{DynAgentRunLauncher, RunnerService};

use crate::launcher::RepositoryRunLauncher;
use crate::repository::AgentRunRepository;
use crate::runner::A2aRunnerService;

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentRunsExtension;

impl Extension for AgentRunsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "agent_runs",
            name: "Agent runs",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new("agent_runs", include_str!("../schema/agent_runs.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "user_id".into(),
                    "status".into(),
                    "request".into(),
                    "lease_until".into(),
                ]),
            SchemaDefinition::new(
                "agent_run_events",
                include_str!("../schema/agent_run_events.sql"),
            )
            .with_required_columns(vec!["run_id".into(), "seq".into(), "kind".into()]),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn provide(
        &self,
        _ctx: &dyn ExtensionContext,
        state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        let runs = AgentRunRepository::new(state.state().get::<DbPool>()?);
        let launcher: DynAgentRunLauncher = Arc::new(RepositoryRunLauncher::new(runs.clone()));
        state.provide(launcher)?;
        state.provide(runs)
    }

    fn runner_services(&self) -> Vec<Arc<dyn RunnerService>> {
        vec![Arc::new(A2aRunnerService)]
    }
}

register_extension!(AgentRunsExtension);
