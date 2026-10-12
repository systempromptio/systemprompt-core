//! Extension-framework registration for the headless crate.
//!
//! [`AgentRunsExtension`] declares the `agent_runs` and
//! `agent_run_events` tables and registers itself with the platform
//! extension registry via [`register_extension!`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

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
}

register_extension!(AgentRunsExtension);
