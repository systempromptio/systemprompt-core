//! [`AgentRunsExtension`]: contributes the `agent_runs` and
//! `agent_run_events` tables, the run-directory retention job and the A2A
//! runner service.
//!
//! It is the crate's composition root: it provides the
//! [`AgentRunRepository`] and the [`DynAgentRunLauncher`] over it. The `admin
//! agents run` child host hands that launcher to its A2A server, the agent-runs
//! routes and the run-directory retention job read the repository, and the one
//! runner service, [`A2aRunnerService`], claims the runs the A2A server queues.
//! Its factory reads the sealed runtime `Config` and signing keys from the
//! full resolver and binds the keys into the run-credential issuer, so the
//! service's runtime context needs neither.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_manifest::Config;
use systemprompt_oauth::repository::OAuthRepositories;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{DynAgentRunLauncher, RunnerService};

use crate::launcher::RepositoryRunLauncher;
use crate::repository::AgentRunRepository;
use crate::retention::AgentRunDirsCleanupJob;
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

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.schema(
            SchemaDefinition::new("agent_runs", include_str!("../schema/agent_runs.sql"))
                .with_required_columns(vec![
                    "id".into(),
                    "user_id".into(),
                    "status".into(),
                    "request".into(),
                    "lease_until".into(),
                ]),
        )
        .schema(
            SchemaDefinition::new(
                "agent_run_events",
                include_str!("../schema/agent_run_events.sql"),
            )
            .with_required_columns(vec!["run_id".into(), "seq".into(), "kind".into()]),
        )
        .migrations(extension_migrations!())
        .provide(
            |r: &Resolved<'_>| -> Result<AgentRunRepository, ProvideError> {
                Ok(AgentRunRepository::new(&r.get::<DbPool>()?))
            },
        )
        .provide(
            |r: &Resolved<'_>| -> Result<DynAgentRunLauncher, ProvideError> {
                let runs = r.get::<AgentRunRepository>()?;
                Ok(Arc::new(RepositoryRunLauncher::new(runs)))
            },
        )
        .runner_service(
            |r: &Resolved<'_>| -> Result<Arc<dyn RunnerService>, FactoryError> {
                Ok(Arc::new(A2aRunnerService::new(
                    &r.get::<DbPool>()?,
                    r.get::<Arc<OAuthRepositories>>()?.as_ref(),
                    &r.get::<Arc<Config>>()?,
                    r.get::<Arc<AppPaths>>()?.as_ref(),
                    r.get::<SigningKeys>()?,
                )))
            },
        )
        .job(AgentRunDirsCleanupJob);
        Ok(())
    }
}
