//! DB-backed tests seeding `ai_requests`, `ai_request_messages`,
//! `ai_request_tool_calls`, `task_messages`/`message_parts`, and `logs` rows so
//! the found-path row-mapping closures in `ai_trace_queries`, `audit_queries`,
//! `request_queries`, `log_lookup_queries`, `log_search_queries`, and
//! `log_summary_queries` execute against real rows.

use chrono::{Duration as ChronoDuration, Utc};
use systemprompt_identifiers::{AiRequestId, ContextId, TaskId, TraceId};
use systemprompt_logging::{AiTraceService, TraceQueryService};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

struct AuditSeed {
    pool: sqlx::PgPool,
    user_id: String,
    context_id: String,
    task_id: String,
    trace_id: String,
    request_id: String,
    model: String,
}

impl AuditSeed {
    async fn new() -> Option<Self> {
        let url = fixture_database_url().ok()?;
        let db = fixture_db_pool(&url).await.ok()?;
        let pool = db.pool_arc().ok()?.as_ref().clone();

        let tag = uuid::Uuid::new_v4().simple().to_string();
        let user_id = format!("audit_user_{tag}");
        let context_id = ContextId::generate().as_str().to_owned();
        let task_id = format!("audit_task_{tag}");
        let trace_id = format!("audit_trace_{tag}");
        let request_id = format!("audit_req_{tag}");
        let model = format!("model-{tag}");

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
             agent_name, status, execution_time_ms, error_message) VALUES ($1, $2, $3, 'sess', \
             $4, 'auditor', 'TASK_STATE_COMPLETED', 77, 'boom')",
        )
        .bind(&task_id)
        .bind(&context_id)
        .bind(&user_id)
        .bind(&trace_id)
        .execute(&pool)
        .await
        .ok()?;

        sqlx::query(
            "INSERT INTO ai_requests \
             (id, request_id, user_id, session_id, task_id, context_id, trace_id, provider, \
              model, requested_model, max_tokens, actor_kind, actor_id, status, error_message, \
              input_tokens, output_tokens, cost_microdollars, latency_ms) \
             VALUES ($1, $1, $2, 'sess', $3, $4, $5, 'anthropic', $6, 'requested-alias', 4096, \
                     'user', $2, 'completed', 'partial failure', 100, 50, 250, 900)",
        )
        .bind(&request_id)
        .bind(&user_id)
        .bind(&task_id)
        .bind(&context_id)
        .bind(&trace_id)
        .bind(&model)
        .execute(&pool)
        .await
        .ok()?;

        Some(Self {
            pool,
            user_id,
            context_id,
            task_id,
            trace_id,
            request_id,
            model,
        })
    }

    async fn insert_request_message(&self, role: &str, content: &str, seq: i32) {
        sqlx::query(
            "INSERT INTO ai_request_messages (request_id, role, content, sequence_number) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&self.request_id)
        .bind(role)
        .bind(content)
        .bind(seq)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn insert_tool_call_with_mcp(&self) -> String {
        let mcp_id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO mcp_tool_executions \
             (mcp_execution_id, tool_name, server_name, started_at, execution_time_ms, input, \
              output, status, user_id, session_id, task_id, context_id, trace_id) \
             VALUES ($1, 'linked_tool', 'linked_srv', now(), 33, '{}', 'out', 'success', $2, \
                     'sess', $3, $4, $5)",
        )
        .bind(&mcp_id)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.trace_id)
        .execute(&self.pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO ai_request_tool_calls \
             (request_id, tool_name, tool_input, mcp_execution_id, sequence_number) \
             VALUES ($1, 'linked_tool', '{\"q\":1}', $2, 0)",
        )
        .bind(&self.request_id)
        .bind(&mcp_id)
        .execute(&self.pool)
        .await
        .unwrap();
        mcp_id
    }

    async fn insert_task_message(&self, role: &str, seq: i32, text: &str) {
        let message_id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO task_messages (task_id, message_id, role, context_id, sequence_number) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&self.task_id)
        .bind(&message_id)
        .bind(role)
        .bind(&self.context_id)
        .bind(seq)
        .execute(&self.pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, \
             text_content) VALUES ($1, $2, 'text', 0, $3)",
        )
        .bind(&message_id)
        .bind(&self.task_id)
        .bind(text)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn insert_step(&self, content: serde_json::Value, status: &str) {
        let step_id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO task_execution_steps \
             (step_id, task_id, step_type, title, status, content, started_at, duration_ms, \
              error_message) VALUES ($1, $2, 'tool_execution', 'seeded', $3, $4, now(), 5, NULL)",
        )
        .bind(&step_id)
        .bind(&self.task_id)
        .bind(status)
        .bind(content)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn insert_log(&self, level: &str, message: &str, metadata: Option<&str>) -> String {
        let id: String = sqlx::query_scalar(
            "INSERT INTO logs (level, module, message, metadata, trace_id, task_id, context_id, \
             user_id, session_id) VALUES ($1, 'audit_seed_mod', $2, $3, $4, $5, $6, $7, 'sess') \
             RETURNING id",
        )
        .bind(level)
        .bind(message)
        .bind(metadata)
        .bind(&self.trace_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.user_id)
        .fetch_one(&self.pool)
        .await
        .unwrap();
        id
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
async fn ai_trace_service_maps_seeded_task_and_message_rows() {
    let Some(seed) = AuditSeed::new().await else {
        return;
    };

    seed.insert_task_message("user", 0, "what time is it").await;
    seed.insert_task_message("agent", 1, "it is late").await;
    seed.insert_step(
        serde_json::json!({"type": "tool_execution", "title": "run grep"}),
        "completed",
    )
    .await;
    seed.insert_request_message("system", "you are a clock", 0)
        .await;
    seed.insert_request_message("user", &"x".repeat(600), 1)
        .await;

    let svc = AiTraceService::new(std::sync::Arc::new(seed.pool.clone()));

    let partial = &seed.task_id[..seed.task_id.len() - 4];
    let resolved = svc.resolve_task_id(partial).await.unwrap();
    assert_eq!(resolved.as_str(), seed.task_id);

    let task_id = TaskId::new(seed.task_id.clone());
    let info = svc.get_task_info(&task_id).await.unwrap();
    assert_eq!(info.task_id.as_str(), seed.task_id);
    assert_eq!(info.context_id.as_str(), seed.context_id);
    assert_eq!(info.agent_name.as_deref(), Some("auditor"));
    assert_eq!(info.status, "TASK_STATE_COMPLETED");
    assert_eq!(info.execution_time_ms, Some(77));
    assert_eq!(info.error_message.as_deref(), Some("boom"));

    assert_eq!(
        svc.get_user_input(&task_id).await.unwrap().as_deref(),
        Some("what time is it")
    );
    assert_eq!(
        svc.get_agent_response(&task_id).await.unwrap().as_deref(),
        Some("it is late")
    );

    let steps = svc.get_execution_steps(&task_id).await.unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].step_type.as_deref(), Some("tool_execution"));
    assert_eq!(steps[0].title.as_deref(), Some("run grep"));
    assert_eq!(steps[0].duration_ms, Some(5));

    let requests = svc.get_ai_requests(&task_id).await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].id.as_str(), seed.request_id);
    assert_eq!(requests[0].provider, "anthropic");
    assert_eq!(requests[0].max_tokens, Some(4096));
    assert_eq!(requests[0].cost_microdollars, 250);

    let request_id = AiRequestId::new(seed.request_id.clone());
    assert_eq!(
        svc.get_system_prompt(&request_id).await.unwrap().as_deref(),
        Some("you are a clock")
    );

    let messages = svc.get_conversation_messages(&request_id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "system");
    assert_eq!(messages[1].content.len(), 600);

    let previews = svc
        .get_ai_request_message_previews(&request_id)
        .await
        .unwrap();
    assert_eq!(previews.len(), 2);
    assert_eq!(
        previews[1].content.len(),
        500,
        "preview must truncate content to 500 chars"
    );
    assert_eq!(previews[0].content, "you are a clock");

    seed.cleanup().await;
}

#[tokio::test]
async fn audit_and_request_queries_map_seeded_rows() {
    let Some(seed) = AuditSeed::new().await else {
        return;
    };

    seed.insert_request_message("user", "audit me", 0).await;
    seed.insert_tool_call_with_mcp().await;

    let svc = TraceQueryService::new(std::sync::Arc::new(seed.pool.clone()));

    let by_request = svc
        .find_ai_request_for_audit(&seed.request_id)
        .await
        .unwrap()
        .expect("audit hit by request id");
    assert_eq!(by_request.id.as_str(), seed.request_id);
    assert_eq!(by_request.provider, "anthropic");
    assert_eq!(
        by_request.requested_model.as_deref(),
        Some("requested-alias")
    );
    assert_eq!(by_request.input_tokens, Some(100));
    assert_eq!(
        by_request.task_id.as_ref().map(TaskId::as_str),
        Some(seed.task_id.as_str())
    );
    assert_eq!(
        by_request.trace_id.as_ref().map(TraceId::as_str),
        Some(seed.trace_id.as_str())
    );

    let by_task = svc
        .find_ai_request_for_audit(&seed.task_id)
        .await
        .unwrap()
        .expect("audit hit by task id");
    assert_eq!(by_task.id.as_str(), seed.request_id);

    let by_trace = svc
        .find_ai_request_for_audit(&seed.trace_id)
        .await
        .unwrap()
        .expect("audit hit by trace id");
    assert_eq!(by_trace.id.as_str(), seed.request_id);

    let request_id = AiRequestId::new(seed.request_id.clone());
    let audit_messages = svc.list_audit_messages(&request_id).await.unwrap();
    assert_eq!(audit_messages.len(), 1);
    assert_eq!(audit_messages[0].content, "audit me");

    let tool_calls = svc.list_audit_tool_calls(&request_id).await.unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].tool_name, "linked_tool");
    assert_eq!(tool_calls[0].tool_input, "{\"q\":1}");

    let linked = svc.list_linked_mcp_calls(&request_id).await.unwrap();
    assert_eq!(linked.len(), 1);
    assert_eq!(linked[0].server_name, "linked_srv");
    assert_eq!(linked[0].execution_time_ms, Some(33));

    let detail = svc
        .find_ai_request_detail(&seed.request_id)
        .await
        .unwrap()
        .expect("detail found");
    assert_eq!(detail.model, seed.model);
    assert_eq!(detail.status, "completed");
    assert_eq!(detail.error_message.as_deref(), Some("partial failure"));
    assert_eq!(detail.latency_ms, Some(900));

    let listed = svc
        .list_ai_requests(None, Some(seed.model.as_str()), Some("anthropic"), 10)
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id.as_str(), seed.request_id);
    assert_eq!(
        listed[0].trace_id.as_ref().map(TraceId::as_str),
        Some(seed.trace_id.as_str())
    );
    assert_eq!(listed[0].cost_microdollars, 250);

    let stats = svc
        .get_ai_request_stats(Some(Utc::now() - ChronoDuration::hours(1)))
        .await
        .unwrap();
    assert!(stats.total_requests >= 1);
    assert!(stats.total_input_tokens >= 100);
    assert!(
        stats
            .by_provider
            .iter()
            .any(|p| p.provider == "anthropic" && p.request_count >= 1)
    );
    assert!(stats.by_model.iter().all(|m| m.request_count >= 1));

    let trace_id = TraceId::new(seed.trace_id.clone());
    let summary = svc.get_ai_request_summary(&trace_id).await.unwrap();
    assert_eq!(summary.request_count, 1);
    assert_eq!(summary.total_tokens, 150);
    assert_eq!(summary.total_cost_microdollars, 250);
    assert_eq!(summary.total_latency_ms, 900);

    let events = svc.get_ai_request_events(&trace_id).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "AI");
    assert!(events[0].details.contains("anthropic"));
    assert!(events[0].details.contains("in:100, out:50, 900ms"));
    assert_eq!(
        events[0].context_id.as_ref().map(ContextId::as_str),
        Some(seed.context_id.as_str())
    );
    let metadata = events[0].metadata.as_deref().expect("AI event metadata");
    assert!(metadata.contains("\"tokens_used\":150"));

    seed.cleanup().await;
}

#[tokio::test]
async fn log_lookup_search_and_summaries_map_seeded_rows() {
    let Some(seed) = AuditSeed::new().await else {
        return;
    };

    let since = Utc::now() - ChronoDuration::minutes(5);
    let good_id = seed
        .insert_log("ERROR", "seeded lookup marker", Some("{\"seed\":true}"))
        .await;
    seed.insert_log("INFO", "seeded second marker", Some("not-json"))
        .await;

    let svc = TraceQueryService::new(std::sync::Arc::new(seed.pool.clone()));

    let found = svc
        .find_log_by_id(&good_id)
        .await
        .unwrap()
        .expect("log by id");
    assert_eq!(found.message, "seeded lookup marker");
    assert_eq!(found.level.as_str(), "ERROR");
    assert_eq!(
        found.metadata.as_ref().and_then(|m| m.get("seed")).cloned(),
        Some(serde_json::json!(true))
    );
    assert_eq!(
        found.context_id.as_ref().map(ContextId::as_str),
        Some(seed.context_id.as_str())
    );

    let by_partial = svc
        .find_log_by_partial_id(&good_id[..good_id.len() - 2])
        .await
        .unwrap()
        .expect("log by partial id");
    assert_eq!(by_partial.trace_id.as_str(), seed.trace_id);

    let trace_id = TraceId::new(seed.trace_id.clone());
    let log_events = svc.get_log_events(&trace_id).await.unwrap();
    assert_eq!(log_events.len(), 2);
    assert!(
        log_events
            .iter()
            .any(|e| e.details == "audit_seed_mod: seeded lookup marker")
    );
    assert!(log_events.iter().all(|e| e.task_id.is_some()));

    let trace_logs = svc.find_logs_by_trace_id(&trace_id).await.unwrap();
    assert_eq!(trace_logs.len(), 2);
    let bad_metadata_row = trace_logs
        .iter()
        .find(|l| l.message == "seeded second marker")
        .expect("second row");
    assert!(
        bad_metadata_row.metadata.is_none(),
        "malformed metadata JSON must be dropped, not fail the query"
    );

    let filtered = svc
        .list_logs_filtered(Some(since), Some("ERROR"), 50)
        .await
        .unwrap();
    assert!(filtered.iter().any(|l| l.id.as_str() == good_id));
    assert!(filtered.iter().all(|l| l.level.as_str() == "ERROR"));

    let searched = svc
        .search_logs("%seeded lookup marker%", Some(since), Some("ERROR"), 10)
        .await
        .unwrap();
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].id.as_str(), good_id);
    assert_eq!(searched[0].module, "audit_seed_mod");

    let levels = svc.count_logs_by_level(Some(since)).await.unwrap();
    let error_count = levels
        .iter()
        .find(|l| l.level == "ERROR")
        .map_or(0, |l| l.count);
    assert!(error_count >= 1);

    let modules = svc.top_modules(Some(since), 100).await.unwrap();
    assert!(
        modules
            .iter()
            .any(|m| m.module == "audit_seed_mod" && m.count == 2)
    );

    let range = svc.log_time_range(Some(since)).await.unwrap();
    assert!(range.earliest.is_some() && range.latest.is_some());
    assert!(range.earliest <= range.latest);

    assert!(svc.total_log_count().await.unwrap() >= 2);

    seed.cleanup().await;
}
