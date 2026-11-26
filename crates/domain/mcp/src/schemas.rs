//! The MCP member's schema contributions: its tables with their required
//! columns, the tool-usage rollup and the ledger, report and fact views.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

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

pub(crate) fn contribute_schemas(c: &mut Contributions<'_>) {
    for (name, sql, columns) in TABLES {
        c.schema(
            SchemaDefinition::new(*name, *sql)
                .with_required_columns(columns.iter().map(|c| (*c).to_owned()).collect()),
        );
    }
    c.schema(
        SchemaDefinition::new(
            "tool_usage_rollup_hourly",
            include_str!("../schema/tool_usage_rollup_hourly.sql"),
        )
        .with_required_columns(vec!["bucket".into(), "execution_count".into()]),
    )
    .schema(SchemaDefinition::sql_only(include_str!(
        "../schema/tool_call_ledger.sql"
    )))
    .schema(SchemaDefinition::sql_only(include_str!(
        "../schema/report_views.sql"
    )))
    .schema(SchemaDefinition::sql_only(include_str!(
        "../schema/tool_row_facts.sql"
    )))
    .schema(SchemaDefinition::sql_only(include_str!(
        "../schema/trace_facts_mcp.sql"
    )));
}
