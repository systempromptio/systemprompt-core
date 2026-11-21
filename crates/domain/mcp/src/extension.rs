//! [`McpExtension`]: contributes the MCP tables, migrations, jobs and
//! retention purge, and provides the tool-execution lookup and the MCP server
//! registry (`Arc<dyn McpRegistryProvider>`, which sign-in reads for MCP
//! resource scopes) and the read-only `Arc<McpServiceObserver>` (server
//! health, recorded process ids and the running set; no spawner, no
//! secrets). It opens the [`UiSlot`] and the [`AccessRecorderSlot`],
//! so both built values are present whenever this member is admitted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_config::paths::AppPaths;
use systemprompt_database::{DbPool, ServiceRepository};
use systemprompt_extension::prelude::*;
use systemprompt_identifiers::InstanceId;
use systemprompt_manifest::Config;
use systemprompt_manifest::services::{ServicesView, SystemAdmin};
use systemprompt_traits::{DynToolExecutionLookup, McpRegistryProvider};

use crate::contract::{AccessRecorderSlot, UiSlot};
use crate::repository::ToolUsageRepository;
use crate::services::McpServiceObserver;
use crate::services::registry::RegistryService;

#[derive(Debug, Clone, Copy, Default)]
pub struct McpExtension;

const TABLES: &[(&str, &str, &[&str])] = &[
    (
        "mcp_external_sessions",
        include_str!("../schema/mcp_external_sessions.sql"),
        &[
            "server_name",
            "session_id",
            "user_id",
            "credential_hash",
            "expires_at",
        ],
    ),
    (
        "mcp_tool_executions",
        include_str!("../schema/mcp_tool_executions.sql"),
        &[
            "mcp_execution_id",
            "tool_name",
            "server_name",
            "source",
            "correlation",
            "created_at",
        ],
    ),
    (
        "mcp_sessions",
        include_str!("../schema/mcp_sessions.sql"),
        &["session_id", "status", "created_at"],
    ),
    (
        "artifact_payloads",
        include_str!("../schema/artifact_payloads.sql"),
        &["sha256", "byte_len", "body", "ref_count"],
    ),
    (
        "mcp_artifacts",
        include_str!("../schema/mcp_artifacts.sql"),
        &[
            "artifact_id",
            "mcp_execution_id",
            "server_name",
            "artifact_type",
            "source",
            "ai_tool_call_id",
            "payload_sha256",
            "is_structured",
            "data",
            "created_at",
        ],
    ),
    (
        "mcp_artifact_findings",
        include_str!("../schema/mcp_artifact_findings.sql"),
        &["artifact_id", "phase", "category", "scanner"],
    ),
    (
        "tool_result_ingest_marks",
        include_str!("../schema/tool_result_ingest_marks.sql"),
        &[
            "user_id",
            "scope_id",
            "gateway_conversation_id",
            "last_message_index",
            "boundary_tool_use_id",
        ],
    ),
];

impl Extension for McpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "mcp",
            name: "MCP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        for (name, sql, columns) in TABLES {
            c.schema(
                SchemaDefinition::new(*name, *sql)
                    .with_required_columns(columns.iter().map(|c| (*c).to_owned()).collect()),
            );
        }
        c.schema(SchemaDefinition::sql_only(include_str!(
            "../schema/tool_row_facts.sql"
        )))
        .schema(SchemaDefinition::sql_only(include_str!(
            "../schema/trace_facts_mcp.sql"
        )))
        .migrations(extension_migrations!())
        .provide(
            |r: &Resolved<'_>| -> Result<DynToolExecutionLookup, ProvideError> {
                let db = r.get::<DbPool>()?;
                Ok(Arc::new(ToolUsageRepository::new(&db)))
            },
        )
        .provide(
            |r: &Resolved<'_>| -> Result<Arc<dyn McpRegistryProvider>, ProvideError> {
                Ok(Arc::new(RegistryService::new(
                    r.get::<SystemAdmin>()?.id().clone(),
                    r.get::<ServicesView>()?,
                    r.get::<Arc<Config>>()?,
                )))
            },
        );
        c.provide(
            |r: &Resolved<'_>| -> Result<Arc<McpServiceObserver>, ProvideError> {
                let db = r.get::<DbPool>()?;
                let registry = RegistryService::new(
                    r.get::<SystemAdmin>()?.id().clone(),
                    r.get::<ServicesView>()?,
                    r.get::<Arc<Config>>()?,
                );
                Ok(Arc::new(McpServiceObserver::new(
                    ServiceRepository::new(&db, r.get::<InstanceId>()?),
                    r.get::<Arc<AppPaths>>()?,
                    registry,
                )))
            },
        );
        crate::jobs::contribute(c);
        c.slot::<UiSlot>();
        c.slot::<AccessRecorderSlot>();
        contribute_user_data(c);
        Ok(())
    }
}

fn contribute_user_data(c: &mut Contributions<'_>) {
    c.user_purge(UserPurgeTable {
        table: "mcp_artifacts",
        column: "user_id",
        predicate: None,
    });
    c.user_purge(UserPurgeTable {
        table: "mcp_tool_executions",
        column: "user_id",
        predicate: None,
    });
    c.user_purge(UserPurgeTable {
        table: "mcp_sessions",
        column: "user_id",
        predicate: None,
    });
    c.user_purge(UserPurgeTable {
        table: "mcp_external_sessions",
        column: "user_id",
        predicate: None,
    });
    c.orphan_sweep(OrphanSweep {
        table: "artifact_payloads",
        key: "sha256",
        referenced_by: "mcp_artifacts",
        via: "payload_sha256",
        user_column: "user_id",
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_tool_executions",
        column: "user_id",
        action: ReassignAction::UPDATE,
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_artifacts",
        column: "user_id",
        action: ReassignAction::UPDATE,
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_sessions",
        column: "user_id",
        action: ReassignAction::UPDATE,
    });
    c.owner_reassignment(OwnerReassignment {
        table: "mcp_external_sessions",
        column: "user_id",
        action: ReassignAction::DELETE,
    });
}
