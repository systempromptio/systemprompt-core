//! Extension-framework registration for the agent-runs crate.
//!
//! [`AgentRunsExtension`] declares the `agent_runs` and
//! `agent_run_events` tables and registers itself with the platform
//! extension registry via [`register_extension!`].
//! [`AgentRunsExtension::launcher`] is the `AgentRunLauncher` entry hands to
//! the A2A server; its one runner service, [`A2aRunnerService`], claims the
//! runs that server queues.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_traits::RunnerService;

use crate::runner::A2aRunnerService;

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentRunsExtension;

impl AgentRunsExtension {
    #[must_use]
    pub fn launcher(
        db: &systemprompt_database::DbPool,
    ) -> systemprompt_traits::DynAgentRunLauncher {
        crate::launcher::RepositoryRunLauncher::shared(db)
    }
}

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

    fn runner_services(&self) -> Vec<Arc<dyn RunnerService>> {
        vec![Arc::new(A2aRunnerService)]
    }
}

register_extension!(AgentRunsExtension);
