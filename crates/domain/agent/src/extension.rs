//! [`AgentExtension`]: contributes the agent tables, migrations and jobs, and
//! provides the conversation-context seams other members and the kernel read
//! by type: `DynContextDirectory`, `DynContextMaterializer` and
//! `DynSessionContextIssuer`, each open (database-only), the open
//! live `Arc<dyn AgentRegistryProvider>` (the proxy's agent OAuth lookup), the
//! open status-only `Arc<AgentStatusView>` and the open `Arc<AgentCardSite>`
//! (the public URL and skills directory, narrowed from the configuration). It
//! contributes its supervised process set to the sealed `ServiceProcessSlot`.
//!
//! It owns the schema objects that cannot exist without its tables: the
//! `agent_task_rollup_hourly` rollup, the triggers on `agent_tasks` that keep
//! the ai member's `ai_task_agents` current, and the analytics report views
//! over its tables. Its schema names the ai member's tables and the mcp
//! member's executions, so the member requires the ai member's request
//! attribution lookup (`DynRequestAttributionLookup`) and the mcp member's
//! tool-execution lookup (`DynToolExecutionLookup`); a composition without
//! either refuses it by name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_extension::prelude::*;
use systemprompt_extension::service_process::ServiceProcessSlot;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::attribution::DynRequestAttributionLookup;
use systemprompt_traits::{
    AgentRegistryProvider, DynContextDirectory, DynContextMaterializer, DynServiceProcessSet,
    DynSessionContextIssuer, DynToolExecutionLookup,
};

use crate::jobs::{CleanupEmptyContextsJob, UserContextCountersRollupJob};
use crate::repository::ContextRepository;
use crate::repository::agent_service::AgentServiceRepository;
use crate::services::ContextProviderService;
use crate::services::agent_orchestration::process_set::AgentProcessSet;
use crate::services::agent_orchestration::status_view::AgentStatusView;
use crate::services::registry::AgentCardSite;
use crate::services::registry_provider::LiveAgentRegistry;

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentExtension;

impl Extension for AgentExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "agent",
            name: "Agent",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for schema in conversation_schemas()
            .into_iter()
            .chain(artifact_schemas())
            .chain(context_schemas())
            .chain(task_tracking_schemas())
        {
            c.schema(schema);
        }
        c.schema(SchemaDefinition::sql_only(include_str!(
            "../schema/user_context_counters.sql"
        )))
        .schema(SchemaDefinition::sql_only(include_str!(
            "../schema/trace_facts_agent.sql"
        )))
        .schema(
            SchemaDefinition::new(
                "agent_task_rollup_hourly",
                include_str!("../schema/agent_task_rollup_hourly.sql"),
            )
            .with_required_columns(vec!["bucket".into(), "task_count".into()]),
        )
        .schema(SchemaDefinition::sql_only(include_str!(
            "../schema/ai_task_agents_sync.sql"
        )))
        .schema(SchemaDefinition::sql_only(include_str!(
            "../schema/report_views.sql"
        )))
        .migrations(extension_migrations!())
        .requires::<DynRequestAttributionLookup>()
        .requires::<DynToolExecutionLookup>()
        .cross_table("ai_requests")
        .cross_table("services")
        .job(CleanupEmptyContextsJob)
        .job(UserContextCountersRollupJob);
        crate::jobs::rollups::contribute(c);
        provide_seams(c);
        c.user_purge(UserPurgeTable {
            table: "agent_tasks",
            column: "user_id",
            predicate: None,
        });
        c.user_purge(UserPurgeTable {
            table: "task_messages",
            column: "user_id",
            predicate: None,
        });
        c.owner_reassignment(OwnerReassignment {
            table: "user_contexts",
            column: "user_id",
            action: ReassignAction::Update {
                touch: Some("updated_at"),
                keep_target_on: &[],
            },
        });
        c.owner_reassignment(OwnerReassignment {
            table: "agent_tasks",
            column: "user_id",
            action: ReassignAction::Update {
                touch: Some("updated_at"),
                keep_target_on: &[],
            },
        });
        c.owner_reassignment(OwnerReassignment {
            table: "task_messages",
            column: "user_id",
            action: ReassignAction::UPDATE,
        });
        Ok(())
    }
}

fn conversation_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new("user_contexts", include_str!("../schema/user_contexts.sql"))
            .with_required_columns(vec![
                "context_id".into(),
                "user_id".into(),
                "created_at".into(),
            ]),
        SchemaDefinition::new("agent_tasks", include_str!("../schema/agent_tasks.sql"))
            .with_required_columns(vec![
                "task_id".into(),
                "context_id".into(),
                "status".into(),
                "created_at".into(),
            ]),
        SchemaDefinition::new("task_messages", include_str!("../schema/task_messages.sql"))
            .with_required_columns(vec![
                "id".into(),
                "task_id".into(),
                "role".into(),
                "created_at".into(),
            ]),
        SchemaDefinition::new("message_parts", include_str!("../schema/message_parts.sql"))
            .with_required_columns(vec!["id".into(), "message_id".into(), "part_kind".into()]),
    ]
}

fn context_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "context_agents",
            include_str!("../schema/context_agents.sql"),
        )
        .with_required_columns(vec!["id".into(), "context_id".into(), "agent_name".into()]),
        SchemaDefinition::new(
            "context_notifications",
            include_str!("../schema/context_notifications.sql"),
        )
        .with_required_columns(vec![
            "id".into(),
            "context_id".into(),
            "notification_type".into(),
        ]),
    ]
}

fn task_tracking_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "task_execution_steps",
            include_str!("../schema/task_execution_steps.sql"),
        )
        .with_required_columns(vec![
            "step_id".into(),
            "task_id".into(),
            "step_type".into(),
        ]),
    ]
}

fn artifact_schemas() -> Vec<SchemaDefinition> {
    vec![
        SchemaDefinition::new(
            "task_artifacts",
            include_str!("../schema/task_artifacts.sql"),
        )
        .with_required_columns(vec!["id".into(), "task_id".into(), "artifact_id".into()]),
        SchemaDefinition::new(
            "artifact_parts",
            include_str!("../schema/artifact_parts.sql"),
        )
        .with_required_columns(vec!["id".into(), "artifact_id".into(), "part_kind".into()]),
    ]
}

fn provide_seams(c: &mut Contributions<'_>) {
    c.provide(
        |r: &Resolved<'_>| -> Result<DynContextDirectory, ProvideError> {
            Ok(Arc::new(context_service(r)?))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<DynContextMaterializer, ProvideError> {
            Ok(Arc::new(context_service(r)?))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<DynSessionContextIssuer, ProvideError> {
            Ok(Arc::new(context_service(r)?))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<Arc<AgentCardSite>, ProvideError> {
            let config = r.get::<Arc<Config>>()?;
            Ok(Arc::new(AgentCardSite {
                api_external_url: config.api_external_url.clone(),
                skills_dir: std::path::PathBuf::from(&config.skills_path),
                cloud: config.is_cloud,
            }))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<Arc<dyn AgentRegistryProvider>, ProvideError> {
            Ok(Arc::new(LiveAgentRegistry::new(
                r.get::<ServicesView>()?,
                r.get::<Arc<Config>>()?.is_cloud,
            )))
        },
    )
    .provide(
        |r: &Resolved<'_>| -> Result<Arc<AgentStatusView>, ProvideError> {
            Ok(Arc::new(AgentStatusView::new(
                AgentServiceRepository::new(&r.get::<DbPool>()?, r.get::<InstanceId>()?),
                r.get::<ServicesView>()?,
            )))
        },
    );
    c.slot::<ServiceProcessSlot>().process_set(
        |r: &Resolved<'_>| -> Result<DynServiceProcessSet, FactoryError> {
            Ok(Arc::new(AgentProcessSet::from_resolved(r)?))
        },
    );
}

fn context_service(r: &Resolved<'_>) -> Result<ContextProviderService, ProvideError> {
    let db = r.get::<DbPool>()?;
    Ok(ContextProviderService::new(ContextRepository::new(&db)))
}
