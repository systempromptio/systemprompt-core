//! `Extension` registration for the MCP domain: schemas, migrations,
//! dependencies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

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
        "mcp_proxy_identities",
        include_str!("../schema/mcp_proxy_identities.sql"),
        &["session_id", "user_id", "auth_token", "expires_at"],
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
];

impl Extension for McpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "mcp",
            name: "MCP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        let mut schemas = vec![
            SchemaDefinition::sql_only(include_str!("../schema/reporting_privacy.sql")),
            SchemaDefinition::sql_only(include_str!("../schema/reporting_capture.sql")),
        ];
        schemas.extend(TABLES.iter().map(|(name, sql, columns)| {
            SchemaDefinition::new(*name, *sql)
                .with_required_columns(columns.iter().map(|c| (*c).to_owned()).collect())
        }));
        schemas
    }
    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }
}

register_extension!(McpExtension);
