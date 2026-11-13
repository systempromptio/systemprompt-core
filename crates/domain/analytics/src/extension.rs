//! Registers analytics-owned schemas, rollups, report views and migrations.
//!
//! That covers the engagement and reputation schemas, the hourly tool and
//! agent-task rollups, and the report views over source tables. It depends on
//! `agent` (and through it `ai` and `mcp`) because the rollup triggers sit on
//! their tables.
//!
//! On a host that seeds the analytics repositories (the API), it provides the
//! [`DynSessionBehaviourScorer`](systemprompt_traits::DynSessionBehaviourScorer)
//! the request-analytics middleware hands each tracked request's session to.
//! A host without them gets no scorer, and its requests record no
//! behavioural verdict.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_traits::{DynIpBanStore, DynSessionStore};

use crate::SessionBehaviourService;
use crate::repository::AnalyticsRepositories;

#[derive(Debug, Clone, Copy, Default)]
pub struct AnalyticsExtension;

impl Extension for AnalyticsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "analytics",
            name: "Analytics",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![
            SchemaDefinition::new(
                "engagement_events",
                include_str!("../schema/engagement_events.sql"),
            )
            .with_required_columns(vec![
                "id".into(),
                "session_id".into(),
                "created_at".into(),
            ]),
            SchemaDefinition::new(
                "fingerprint_reputation",
                include_str!("../schema/fingerprint_reputation.sql"),
            )
            .with_required_columns(vec!["fingerprint_hash".into()]),
            SchemaDefinition::new(
                "tool_usage_rollup_hourly",
                include_str!("../schema/usage_rollups.sql"),
            )
            .with_required_columns(vec!["bucket".into(), "execution_count".into()]),
            SchemaDefinition::new(
                "agent_task_rollup_hourly",
                include_str!("../schema/agent_task_rollup_hourly.sql"),
            )
            .with_required_columns(vec!["bucket".into(), "task_count".into()]),
            SchemaDefinition::sql_only(include_str!("../schema/report_views.sql")),
        ]
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users", "agent", "authz"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn requires(&self) -> Vec<StateRequirement> {
        vec![StateRequirement::of::<DynIpBanStore>()]
    }

    fn provide(
        &self,
        _ctx: &dyn ExtensionContext,
        state: &mut StateBuilder<'_>,
    ) -> Result<(), ExtensionStateError> {
        let Some(repositories) = state.state().find::<Arc<AnalyticsRepositories>>() else {
            return Ok(());
        };
        let signals = repositories.session_signals.clone();
        let sessions = Arc::clone(state.state().get::<DynSessionStore>()?);
        let scorer = SessionBehaviourService::new(sessions, signals).into_scorer();
        state.provide(scorer)
    }
}

register_extension!(AnalyticsExtension);
