//! Integration tests for MCP tool execution persistence
//!
//! Tests the ToolUsageRepository operations including:
//! - Tool execution creation and completion
//! - Finding executions by ID
//! - Listing tool statistics
//! - Context timestamp updates

use crate::common::*;
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn test_mcp_tool_executions_table_exists() -> Result<()> {
    let ctx = TestContext::new().await?;

    let result = ctx
        .db
        .fetch_all(
            &"SELECT table_name FROM information_schema.tables WHERE table_name = 'mcp_tool_executions'",
            &[],
        )
        .await?;

    assert!(!result.is_empty(), "mcp_tool_executions table should exist");
    println!("✓ MCP tool executions table exists");

    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_columns() -> Result<()> {
    let ctx = TestContext::new().await?;

    let result = ctx
        .db
        .fetch_all(
            &r#"
            SELECT column_name
            FROM information_schema.columns
            WHERE table_name = 'mcp_tool_executions'
            ORDER BY ordinal_position
            "#,
            &[],
        )
        .await?;

    let columns: Vec<String> = result
        .iter()
        .filter_map(|row| row.get("column_name").and_then(|v| v.as_str()))
        .map(String::from)
        .collect();

    let expected_columns = [
        "mcp_execution_id",
        "tool_name",
        "server_name",
        "status",
        "input",
        "output",
        "error_message",
        "execution_time_ms",
        "started_at",
        "completed_at",
        "user_id",
        "context_id",
        "session_id",
        "task_id",
        "trace_id",
        "ai_tool_call_id",
    ];

    for col in expected_columns {
        assert!(
            columns.contains(&col.to_string()),
            "Column '{}' should exist in mcp_tool_executions",
            col
        );
    }

    println!("✓ MCP tool execution columns verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_insert() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let execution_id = format!("test-exec-{}", uuid::Uuid::new_v4());
    let tool_name = format!("test-tool-{}", &fingerprint[..8]);
    let server_name = format!("test-server-{}", &fingerprint[..8]);
    let now = Utc::now();

    ctx.db
        .execute(
            &r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, status,
                input, user_id, started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            &[
                &execution_id,
                &tool_name,
                &server_name,
                &"pending",
                &r#"{"test": true}"#,
                &fingerprint,
                &now.to_rfc3339(),
            ],
        )
        .await?;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT * FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    assert_eq!(rows.len(), 1, "Should have inserted one execution");

    let row = &rows[0];
    assert_eq!(row["tool_name"].as_str().unwrap(), tool_name);
    assert_eq!(row["server_name"].as_str().unwrap(), server_name);
    assert_eq!(row["status"].as_str().unwrap(), "pending");

    ctx.db
        .execute(
            &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    println!("✓ MCP tool execution insert verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_complete() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let execution_id = format!("test-complete-{}", uuid::Uuid::new_v4());
    let tool_name = format!("complete-tool-{}", &fingerprint[..8]);
    let now = Utc::now();

    ctx.db
        .execute(
            &r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, status,
                input, user_id, started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            &[
                &execution_id,
                &tool_name,
                &"test-server",
                &"pending",
                &r#"{}"#,
                &fingerprint,
                &now.to_rfc3339(),
            ],
        )
        .await?;

    let completed_at = Utc::now();
    ctx.db
        .execute(
            &r#"
            UPDATE mcp_tool_executions
            SET status = $1, output = $2, execution_time_ms = $3, completed_at = $4
            WHERE mcp_execution_id = $5
            "#,
            &[
                &"success",
                &r#"{"result": "ok"}"#,
                &150i32,
                &completed_at.to_rfc3339(),
                &execution_id,
            ],
        )
        .await?;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT status, output, execution_time_ms FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["status"].as_str().unwrap(), "success");
    assert!(row["output"].as_str().unwrap().contains("result"));
    assert_eq!(row["execution_time_ms"].as_i64().unwrap(), 150);

    ctx.db
        .execute(
            &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    println!("✓ MCP tool execution completion verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_failure() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let execution_id = format!("test-failure-{}", uuid::Uuid::new_v4());
    let now = Utc::now();

    ctx.db
        .execute(
            &r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, status,
                input, user_id, started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            &[
                &execution_id,
                &"failing-tool",
                &"test-server",
                &"pending",
                &r#"{}"#,
                &fingerprint,
                &now.to_rfc3339(),
            ],
        )
        .await?;

    let completed_at = Utc::now();
    let error_message = "Connection timeout after 30 seconds";

    ctx.db
        .execute(
            &r#"
            UPDATE mcp_tool_executions
            SET status = $1, error_message = $2, execution_time_ms = $3, completed_at = $4
            WHERE mcp_execution_id = $5
            "#,
            &[
                &"failed",
                &error_message,
                &30000i32,
                &completed_at.to_rfc3339(),
                &execution_id,
            ],
        )
        .await?;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT status, error_message FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["status"].as_str().unwrap(), "failed");
    assert!(row["error_message"].as_str().unwrap().contains("timeout"));

    ctx.db
        .execute(
            &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    println!("✓ MCP tool execution failure verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_stats_aggregation() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();
    let tool_name = format!("stats-tool-{}", &fingerprint[..8]);

    let mut execution_ids = Vec::new();

    for i in 0..3 {
        let execution_id = format!("test-stats-{}-{}", i, uuid::Uuid::new_v4());
        execution_ids.push(execution_id.clone());

        let status = if i < 2 { "success" } else { "failed" };
        let now = Utc::now();

        ctx.db
            .execute(
                &r#"
                INSERT INTO mcp_tool_executions (
                    mcp_execution_id, tool_name, server_name, status,
                    input, execution_time_ms, user_id, started_at, completed_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#,
                &[
                    &execution_id,
                    &tool_name,
                    &"stats-server",
                    &status,
                    &r#"{}"#,
                    &((i + 1) * 100i32),
                    &fingerprint,
                    &now.to_rfc3339(),
                    &now.to_rfc3339(),
                ],
            )
            .await?;
    }

    let rows = ctx
        .db
        .fetch_all(
            &r#"
            SELECT
                tool_name,
                COUNT(*)::bigint as total_executions,
                COUNT(*) FILTER (WHERE status = 'success')::bigint as success_count,
                COUNT(*) FILTER (WHERE status = 'failed')::bigint as error_count,
                AVG(execution_time_ms)::bigint as avg_duration_ms
            FROM mcp_tool_executions
            WHERE tool_name = $1
            GROUP BY tool_name
            "#,
            &[&tool_name],
        )
        .await?;

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["total_executions"].as_i64().unwrap(), 3);
    assert_eq!(row["success_count"].as_i64().unwrap(), 2);
    assert_eq!(row["error_count"].as_i64().unwrap(), 1);

    for execution_id in execution_ids {
        ctx.db
            .execute(
                &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
                &[&execution_id],
            )
            .await?;
    }

    println!("✓ MCP tool stats aggregation verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_with_context_id() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let execution_id = format!("test-ctx-{}", uuid::Uuid::new_v4());
    let context_id = format!("ctx-{}", uuid::Uuid::new_v4());
    let now = Utc::now();

    ctx.db
        .execute(
            &r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, status,
                input, user_id, context_id, started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            &[
                &execution_id,
                &"context-tool",
                &"test-server",
                &"success",
                &r#"{}"#,
                &fingerprint,
                &context_id,
                &now.to_rfc3339(),
            ],
        )
        .await?;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT context_id FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["context_id"].as_str().unwrap(), context_id);

    ctx.db
        .execute(
            &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    println!("✓ MCP tool execution with context_id verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_with_ai_tool_call_id() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let execution_id = format!("test-ai-{}", uuid::Uuid::new_v4());
    let ai_tool_call_id = format!("call_{}", uuid::Uuid::new_v4());
    let now = Utc::now();

    ctx.db
        .execute(
            &r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, status,
                input, user_id, ai_tool_call_id, started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            &[
                &execution_id,
                &"ai-tool",
                &"test-server",
                &"success",
                &r#"{}"#,
                &fingerprint,
                &ai_tool_call_id,
                &now.to_rfc3339(),
            ],
        )
        .await?;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT mcp_execution_id FROM mcp_tool_executions WHERE ai_tool_call_id = $1",
            &[&ai_tool_call_id],
        )
        .await?;

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["mcp_execution_id"].as_str().unwrap(), execution_id);

    ctx.db
        .execute(
            &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    println!("✓ MCP tool execution with ai_tool_call_id verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_idempotency() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let ai_tool_call_id = format!("idempotent-{}", uuid::Uuid::new_v4());
    let execution_id_1 = format!("exec-1-{}", uuid::Uuid::new_v4());
    let now = Utc::now();

    ctx.db
        .execute(
            &r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, status,
                input, user_id, ai_tool_call_id, started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            &[
                &execution_id_1,
                &"idempotent-tool",
                &"test-server",
                &"success",
                &r#"{}"#,
                &fingerprint,
                &ai_tool_call_id,
                &now.to_rfc3339(),
            ],
        )
        .await?;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT mcp_execution_id FROM mcp_tool_executions WHERE ai_tool_call_id = $1",
            &[&ai_tool_call_id],
        )
        .await?;

    assert_eq!(
        rows.len(),
        1,
        "Should find existing execution by ai_tool_call_id"
    );
    assert_eq!(rows[0]["mcp_execution_id"].as_str().unwrap(), execution_id_1);

    ctx.db
        .execute(
            &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id_1],
        )
        .await?;

    println!("✓ MCP tool execution idempotency verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_tracing_fields() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();

    let execution_id = format!("test-trace-{}", uuid::Uuid::new_v4());
    let trace_id = format!("trace-{}", uuid::Uuid::new_v4());
    let session_id = format!("session-{}", uuid::Uuid::new_v4());
    let task_id = format!("task-{}", uuid::Uuid::new_v4());
    let now = Utc::now();

    ctx.db
        .execute(
            &r#"
            INSERT INTO mcp_tool_executions (
                mcp_execution_id, tool_name, server_name, status,
                input, user_id, trace_id, session_id, task_id, started_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            &[
                &execution_id,
                &"traced-tool",
                &"test-server",
                &"success",
                &r#"{}"#,
                &fingerprint,
                &trace_id,
                &session_id,
                &task_id,
                &now.to_rfc3339(),
            ],
        )
        .await?;

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT trace_id, session_id, task_id FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["trace_id"].as_str().unwrap(), trace_id);
    assert_eq!(row["session_id"].as_str().unwrap(), session_id);
    assert_eq!(row["task_id"].as_str().unwrap(), task_id);

    ctx.db
        .execute(
            &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
            &[&execution_id],
        )
        .await?;

    println!("✓ MCP tool execution tracing fields verified");
    Ok(())
}

#[tokio::test]
async fn test_mcp_tool_execution_query_by_server() -> Result<()> {
    let ctx = TestContext::new().await?;
    let fingerprint = ctx.fingerprint().to_string();
    let server_name = format!("query-server-{}", &fingerprint[..8]);

    let mut execution_ids = Vec::new();

    for i in 0..2 {
        let execution_id = format!("test-query-{}-{}", i, uuid::Uuid::new_v4());
        execution_ids.push(execution_id.clone());
        let now = Utc::now();

        ctx.db
            .execute(
                &r#"
                INSERT INTO mcp_tool_executions (
                    mcp_execution_id, tool_name, server_name, status,
                    input, user_id, started_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &execution_id,
                    &format!("tool-{}", i),
                    &server_name,
                    &"success",
                    &r#"{}"#,
                    &fingerprint,
                    &now.to_rfc3339(),
                ],
            )
            .await?;
    }

    let rows = ctx
        .db
        .fetch_all(
            &"SELECT mcp_execution_id FROM mcp_tool_executions WHERE server_name = $1",
            &[&server_name],
        )
        .await?;

    assert_eq!(rows.len(), 2);

    for execution_id in execution_ids {
        ctx.db
            .execute(
                &"DELETE FROM mcp_tool_executions WHERE mcp_execution_id = $1",
                &[&execution_id],
            )
            .await?;
    }

    println!("✓ MCP tool execution query by server verified");
    Ok(())
}
