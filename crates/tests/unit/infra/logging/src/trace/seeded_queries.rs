//! DB-backed tests that seed real rows so the row-mapping closures in the
//! trace query modules execute.
//!
//! The zero-result coverage in `queries.rs` never enters the `.map(|row| …)`
//! bodies of `step_queries`/`mcp_trace_queries` (empty result set). Here we
//! insert representative rows for a unique trace/task/context and assert the
//! mapped [`TraceEvent`]/[`McpToolExecution`]/[`TaskArtifact`] shapes, driving
//! the detail/metadata formatting and the failed-status truncation branches.

use chrono::{Duration as ChronoDuration, Utc};
use systemprompt_identifiers::{ContextId, TaskId, TraceId};
use systemprompt_logging::trace::{ToolExecutionFilter, TraceListFilter};
use systemprompt_logging::{AiTraceService, TraceQueryService};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

struct Seed {
    pool: sqlx::PgPool,
    user_id: String,
    context_id: String,
    task_id: String,
    trace_id: String,
    agent_name: String,
    tool_name: String,
}

impl Seed {
    async fn new() -> Option<Self> {
        let url = fixture_database_url().ok()?;
        let db = fixture_db_pool(&url).await.ok()?;
        let pool = db.pool_arc().ok()?.as_ref().clone();

        let tag = uuid::Uuid::new_v4().simple().to_string();
        let user_id = format!("seed_user_{tag}");
        let context_id = ContextId::generate().as_str().to_owned();
        let task_id = format!("seed_task_{tag}");
        let trace_id = format!("seed_trace_{tag}");
        let agent_name = format!("seed_agent_{tag}");
        let tool_name = format!("seed_tool_{tag}");

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(&user_id)
            .bind(&user_id)
            .bind(format!("{user_id}@test.invalid"))
            .execute(&pool)
            .await
            .ok()?;

        sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
            .bind(&context_id)
            .bind(&user_id)
            .bind(format!("ctx-{tag}"))
            .execute(&pool)
            .await
            .ok()?;

        sqlx::query(
            "INSERT INTO agent_tasks (task_id, context_id, user_id, session_id, trace_id, \
             agent_name) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&task_id)
        .bind(&context_id)
        .bind(&user_id)
        .bind(format!("sess-{tag}"))
        .bind(&trace_id)
        .bind(&agent_name)
        .execute(&pool)
        .await
        .ok()?;

        Some(Self {
            pool,
            user_id,
            context_id,
            task_id,
            trace_id,
            agent_name,
            tool_name,
        })
    }

    async fn insert_mcp(&self, status: &str, error: Option<&str>, elapsed: i32) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO mcp_tool_executions \
             (mcp_execution_id, tool_name, server_name, started_at, execution_time_ms, input, \
              output, status, error_message, user_id, session_id, task_id, context_id, trace_id) \
             VALUES ($1, $9, 'srv', now(), $2, '{}', 'out', $3, $4, $5, 'sess', $6, $7, $8)",
        )
        .bind(&id)
        .bind(elapsed)
        .bind(status)
        .bind(error)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.trace_id)
        .bind(&self.tool_name)
        .execute(&self.pool)
        .await
        .unwrap();
        id
    }

    async fn insert_step(&self, step_type: &str, status: &str, content: serde_json::Value) {
        let step_id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO task_execution_steps \
             (step_id, task_id, step_type, title, status, content, started_at, duration_ms) \
             VALUES ($1, $2, $3, $4, $5, $6, now(), 12)",
        )
        .bind(&step_id)
        .bind(&self.task_id)
        .bind(step_type)
        .bind(format!("title-{step_type}"))
        .bind(status)
        .bind(content)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn insert_artifact(&self) {
        let artifact_id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO task_artifacts \
             (task_id, context_id, artifact_id, name, artifact_type, source, tool_name) \
             VALUES ($1, $2, $3, 'doc', 'text', 'agent', 'writer')",
        )
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&artifact_id)
        .execute(&self.pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO artifact_parts \
             (artifact_id, context_id, part_kind, sequence_number, text_content) \
             VALUES ($1, $2, 'text', 0, 'body text')",
        )
        .bind(&artifact_id)
        .bind(&self.context_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn insert_tool_log(&self) {
        sqlx::query(
            "INSERT INTO logs (level, module, message, trace_id, task_id, context_id, user_id, \
             session_id) VALUES ('INFO', 'agent_tools', 'Tool executed successfully', $1, $2, $3, \
             $4, 'sess')",
        )
        .bind(&self.trace_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.user_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn insert_linked_ai_request(&self, mcp_execution_id: &str) {
        let id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO ai_requests \
             (id, request_id, user_id, provider, model, actor_kind, actor_id, trace_id, status, \
              mcp_execution_id, input_tokens, output_tokens, cost_microdollars, latency_ms) \
             VALUES ($1, $2, $3, 'openai', 'gpt-4', 'user', $3, $4, 'completed', $5, 10, 20, 5, 30)",
        )
        .bind(&id)
        .bind(&id)
        .bind(&self.user_id)
        .bind(&self.trace_id)
        .bind(mcp_execution_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn cleanup(&self) {
        let _ = sqlx::query("DELETE FROM logs WHERE trace_id = $1")
            .bind(&self.trace_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM ai_requests WHERE trace_id = $1")
            .bind(&self.trace_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM mcp_tool_executions WHERE trace_id = $1")
            .bind(&self.trace_id)
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&self.user_id)
            .execute(&self.pool)
            .await;
    }
}

#[tokio::test]
async fn step_queries_map_seeded_mcp_and_step_rows() {
    let Some(seed) = Seed::new().await else {
        return;
    };

    let long_error: String = "e".repeat(120);
    seed.insert_mcp("failed", Some(&long_error), 42).await;
    seed.insert_mcp("success", None, 7).await;

    seed.insert_step(
        "tool_execution",
        "completed",
        serde_json::json!({"type": "tool_execution", "tool_name": "grep"}),
    )
    .await;
    seed.insert_step(
        "skill_usage",
        "completed",
        serde_json::json!({"type": "skill_usage", "skill_name": "research"}),
    )
    .await;
    seed.insert_step(
        "understanding",
        "completed",
        serde_json::json!({"type": "understanding"}),
    )
    .await;
    seed.insert_step(
        "planning",
        "in_progress",
        serde_json::json!({"type": "planning"}),
    )
    .await;
    seed.insert_step(
        "completion",
        "completed",
        serde_json::json!({"type": "completion"}),
    )
    .await;
    seed.insert_step("other", "failed", serde_json::json!({"type": "other"}))
        .await;

    let svc = TraceQueryService::new(std::sync::Arc::new(seed.pool.clone()));
    let trace_id = TraceId::new(seed.trace_id.as_str());

    let mcp_summary = svc.get_mcp_execution_summary(&trace_id).await.unwrap();
    assert_eq!(mcp_summary.execution_count, 2);
    assert_eq!(mcp_summary.total_execution_time_ms, 49);

    let mcp_events = svc.get_mcp_execution_events(&trace_id).await.unwrap();
    assert_eq!(mcp_events.len(), 2);
    assert!(mcp_events.iter().all(|e| e.event_type == "MCP"));
    let failed = mcp_events
        .iter()
        .find(|e| e.details.contains("failed"))
        .expect("a failed MCP event");
    assert!(
        failed.details.contains("..."),
        "long error message must be truncated: {}",
        failed.details
    );
    assert_eq!(
        failed.context_id.as_ref().map(|c| c.as_str()),
        Some(seed.context_id.as_str())
    );

    let step_summary = svc.get_execution_step_summary(&trace_id).await.unwrap();
    assert_eq!(step_summary.total, 6);
    assert_eq!(step_summary.completed, 4);
    assert_eq!(step_summary.failed, 1);
    assert_eq!(step_summary.pending, 1);

    let step_events = svc.get_execution_step_events(&trace_id).await.unwrap();
    assert_eq!(step_events.len(), 6);
    assert!(step_events.iter().all(|e| e.event_type == "STEP"));
    assert!(step_events.iter().any(|e| e.details.contains("grep")));
    assert!(step_events.iter().any(|e| e.details.contains("research")));
    assert!(step_events.iter().any(|e| e.details.contains("Planning")));
    assert!(step_events.iter().any(|e| e.details.contains("Complete")));

    let task_id = svc.get_task_id(&trace_id).await.unwrap();
    assert_eq!(
        task_id.as_ref().map(TaskId::as_str),
        Some(seed.task_id.as_str())
    );

    seed.cleanup().await;
}

#[tokio::test]
async fn mcp_trace_queries_map_seeded_rows() {
    let Some(seed) = Seed::new().await else {
        return;
    };

    let mcp_id = seed.insert_mcp("success", None, 11).await;
    seed.insert_linked_ai_request(&mcp_id).await;
    seed.insert_artifact().await;
    seed.insert_tool_log().await;

    let svc = AiTraceService::new(std::sync::Arc::new(seed.pool.clone()));
    let task_id = TaskId::new(seed.task_id.clone());
    let ctx_id = ContextId::new(seed.context_id.clone());

    let executions = svc.get_mcp_executions(&task_id, &ctx_id).await.unwrap();
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].tool_name, seed.tool_name);
    assert_eq!(executions[0].server_name, "srv");
    assert_eq!(executions[0].status, "success");
    assert_eq!(executions[0].execution_time_ms, Some(11));

    let linked = svc
        .get_mcp_linked_ai_requests(&executions[0].mcp_execution_id)
        .await
        .unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].provider, "openai");
    assert_eq!(linked[0].model, "gpt-4");
    assert_eq!(linked[0].input_tokens, Some(10));
    assert_eq!(linked[0].cost_microdollars, 5);

    let artifacts = svc.get_task_artifacts(&task_id, &ctx_id).await.unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].artifact_type, "text");
    assert_eq!(artifacts[0].part_kind.as_deref(), Some("text"));
    assert_eq!(artifacts[0].text_content.as_deref(), Some("body text"));

    let tool_logs = svc.get_tool_logs(&task_id, &ctx_id).await.unwrap();
    assert!(
        tool_logs
            .iter()
            .any(|l| l.message.contains("Tool executed"))
    );

    seed.cleanup().await;
}

#[tokio::test]
async fn filtered_lists_surface_seeded_rows() {
    let Some(seed) = Seed::new().await else {
        return;
    };

    seed.insert_mcp("success", None, 21).await;
    seed.insert_tool_log().await;

    let svc = TraceQueryService::new(std::sync::Arc::new(seed.pool.clone()));

    let f = ToolExecutionFilter::new(10)
        .with_name(seed.tool_name.clone())
        .with_server("srv".to_owned())
        .with_status("success".to_owned())
        .with_since(Utc::now() - ChronoDuration::hours(1));
    let executions = svc.list_tool_executions(&f).await.unwrap();
    assert_eq!(
        executions.len(),
        1,
        "unique tool name must match exactly once"
    );
    assert_eq!(executions[0].trace_id.as_str(), seed.trace_id);
    assert_eq!(executions[0].tool_name, seed.tool_name);
    assert_eq!(executions[0].server_name.as_deref(), Some("srv"));
    assert_eq!(executions[0].status, "success");
    assert_eq!(executions[0].execution_time_ms, Some(21));

    let mismatched = ToolExecutionFilter::new(10)
        .with_name(seed.tool_name.clone())
        .with_status("failed".to_owned());
    assert!(
        svc.list_tool_executions(&mismatched)
            .await
            .unwrap()
            .is_empty()
    );

    let searched = svc
        .search_tool_executions(&seed.tool_name, None, 10)
        .await
        .unwrap();
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].trace_id.as_str(), seed.trace_id);

    let traces = svc
        .list_traces(
            &TraceListFilter::new(10)
                .with_agent(seed.agent_name.clone())
                .with_has_mcp(true),
        )
        .await
        .unwrap();
    let item = traces
        .iter()
        .find(|t| t.trace_id.as_str() == seed.trace_id)
        .expect("seeded trace must surface through the agent filter");
    assert_eq!(item.agent.as_deref(), Some(seed.agent_name.as_str()));
    assert_eq!(item.mcp_calls, 1);

    let by_tool = svc
        .list_traces(&TraceListFilter::new(10).with_tool(seed.tool_name.clone()))
        .await
        .unwrap();
    assert!(
        by_tool.iter().any(|t| t.trace_id.as_str() == seed.trace_id),
        "seeded trace must surface through the tool filter"
    );

    let logs = svc
        .find_logs_by_trace_id(&TraceId::new(seed.trace_id.as_str()))
        .await
        .unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].module, "agent_tools");

    seed.cleanup().await;
}
