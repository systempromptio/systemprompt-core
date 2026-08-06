//! `infra logs trace show` text renderers that only run for specific row
//! shapes: the empty-trace reports, the per-event attribution/metadata lines,
//! and the artifact text/data content blocks.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::logs::{self, LogsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::ContextId;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: LogsCommands,
}

fn parse(args: &[&str]) -> LogsCommands {
    Harness::try_parse_from(std::iter::once("logs").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn ctx(pool: &DbPool, json: bool) -> CommandContext {
    let mut cli = CliConfig::new().with_interactive(false);
    if json {
        cli = cli.with_output_format(OutputFormat::Json);
    }
    CommandContext::with_database(
        cli,
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

struct Fixture {
    pool: DbPool,
    raw: sqlx::PgPool,
    user_id: String,
    context_id: String,
    task_id: String,
    trace_id: String,
}

impl Fixture {
    async fn new(tag_prefix: &str) -> Self {
        let url = fixture_database_url().unwrap();
        let pool = fixture_db_pool(&url).await.unwrap();
        let raw = pool.pool_arc().unwrap().as_ref().clone();

        let tag = uuid::Uuid::new_v4().simple().to_string();
        let user_id = format!("{tag_prefix}_user_{tag}");
        let context_id = ContextId::generate().as_str().to_owned();
        let task_id = format!("{tag_prefix}_task_{tag}");
        let trace_id = format!("{tag_prefix}_trace_{tag}");

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $2, $3)")
            .bind(&user_id)
            .bind(&user_id)
            .bind(format!("{user_id}@test.invalid"))
            .execute(&raw)
            .await
            .unwrap();

        sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, $3)")
            .bind(&context_id)
            .bind(&user_id)
            .bind(format!("ctx-{tag}"))
            .execute(&raw)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO agent_tasks (task_id, context_id, user_id, session_id, trace_id, \
             agent_name) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&task_id)
        .bind(&context_id)
        .bind(&user_id)
        .bind(format!("sess-{tag}"))
        .bind(&trace_id)
        .bind(format!("agent_{tag}"))
        .execute(&raw)
        .await
        .unwrap();

        Self {
            pool,
            raw,
            user_id,
            context_id,
            task_id,
            trace_id,
        }
    }

    async fn insert_ai_request(&self) {
        let id = format!("{}_req", self.trace_id);
        sqlx::query(
            "INSERT INTO ai_requests \
             (id, request_id, user_id, context_id, provider, model, requested_model, actor_kind, actor_id, \
              task_id, trace_id, status, input_tokens, output_tokens, cost_microdollars, \
              latency_ms) \
             VALUES ($1, $1, $2, '00000000-0000-0000-0000-00000000c0de', 'openai', 'gpt-4', 'gpt-4o', 'user', $2, $3, $4, 'completed', \
              13, 27, 4321, 30)",
        )
        .bind(&id)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.trace_id)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn insert_mcp_execution(&self) {
        let id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO mcp_tool_executions \
             (mcp_execution_id, tool_name, server_name, started_at, execution_time_ms, input, \
              output, status, user_id, session_id, task_id, context_id, trace_id) \
             VALUES ($1, 'render_probe', 'srv', now(), 9, '{\"q\":\"x\"}', 'out', 'success', $2, \
              'sess', $3, $4, $5)",
        )
        .bind(&id)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.trace_id)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn insert_attributed_log(&self, level: &str, message: &str, metadata: serde_json::Value) {
        sqlx::query(
            "INSERT INTO logs (level, module, message, trace_id, task_id, context_id, user_id, \
             session_id, metadata) VALUES ($1, 'cli.trace.render', $2, $3, $4, $5, $6, \
             'session-with-a-long-identifier', $7)",
        )
        .bind(level)
        .bind(message)
        .bind(&self.trace_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.user_id)
        .bind(metadata.to_string())
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn insert_artifact_with_parts(&self) {
        let artifact_id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO task_artifacts \
             (artifact_id, task_id, context_id, artifact_type, name, source, tool_name) \
             VALUES ($1, $2, $3, 'report', 'Quarterly Report', 'tool', 'render_probe')",
        )
        .bind(&artifact_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .execute(&self.raw)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO artifact_parts \
             (artifact_id, context_id, sequence_number, part_kind, text_content) \
             VALUES ($1, $2, 0, 'text', 'the rendered narrative body')",
        )
        .bind(&artifact_id)
        .bind(&self.context_id)
        .execute(&self.raw)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO artifact_parts \
             (artifact_id, context_id, sequence_number, part_kind, data_content) \
             VALUES ($1, $2, 1, 'data', $3)",
        )
        .bind(&artifact_id)
        .bind(&self.context_id)
        .bind(serde_json::json!({"rows": 3, "unit": "eur"}))
        .execute(&self.raw)
        .await
        .unwrap();

        // A part kind the renderer has no arm for must be skipped, not panic.
        sqlx::query(
            "INSERT INTO artifact_parts \
             (artifact_id, context_id, sequence_number, part_kind, file_name, file_mime_type, \
              file_uri) \
             VALUES ($1, $2, 2, 'file', 'report.pdf', 'application/pdf', 'file:///tmp/r.pdf')",
        )
        .bind(&artifact_id)
        .bind(&self.context_id)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn log_count(&self) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM logs WHERE trace_id = $1")
            .bind(&self.trace_id)
            .fetch_one(&self.raw)
            .await
            .unwrap()
    }

    async fn cleanup(&self) {
        for stmt in [
            "DELETE FROM artifact_parts WHERE context_id = $1",
            "DELETE FROM task_artifacts WHERE context_id = $1",
        ] {
            let _ = sqlx::query(stmt)
                .bind(&self.context_id)
                .execute(&self.raw)
                .await;
        }
        for stmt in [
            "DELETE FROM ai_requests WHERE trace_id = $1",
            "DELETE FROM mcp_tool_executions WHERE trace_id = $1",
            "DELETE FROM logs WHERE trace_id = $1",
            "DELETE FROM agent_tasks WHERE trace_id = $1",
        ] {
            let _ = sqlx::query(stmt)
                .bind(&self.trace_id)
                .execute(&self.raw)
                .await;
        }
        let _ = sqlx::query("DELETE FROM user_contexts WHERE context_id = $1")
            .bind(&self.context_id)
            .execute(&self.raw)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&self.user_id)
            .execute(&self.raw)
            .await;
    }
}

#[tokio::test]
async fn a_trace_with_no_rows_at_all_reports_that_it_may_still_be_populating() {
    let fixture = Fixture::new("empty").await;
    assert_eq!(
        fixture.log_count().await,
        0,
        "the fixture trace must start with no log rows"
    );

    logs::execute(
        parse(&["trace", "show", &fixture.trace_id]),
        &ctx(&fixture.pool, false),
    )
    .await
    .expect("an empty trace must render a hint rather than erroring");

    fixture.cleanup().await;
}

#[tokio::test]
async fn a_trace_with_ai_and_mcp_activity_but_no_log_rows_summarises_the_activity() {
    let fixture = Fixture::new("activity").await;
    fixture.insert_ai_request().await;
    fixture.insert_mcp_execution().await;
    assert_eq!(
        fixture.log_count().await,
        0,
        "this case is specifically activity without any log rows"
    );

    logs::execute(
        parse(&["trace", "show", &fixture.trace_id]),
        &ctx(&fixture.pool, false),
    )
    .await
    .expect("activity without log events must summarise rather than claim nothing happened");

    fixture.cleanup().await;
}

#[tokio::test]
async fn attributed_log_events_render_their_identifier_and_metadata_lines() {
    let fixture = Fixture::new("attrib").await;
    fixture
        .insert_attributed_log(
            "ERROR",
            "failed with attribution",
            serde_json::json!({"attempt": 3, "reason": "timeout", "ignored": null}),
        )
        .await;
    fixture
        .insert_attributed_log("WARN", "degraded", serde_json::json!({"backoff_ms": 250}))
        .await;
    fixture
        .insert_attributed_log("INFO", "recovered", serde_json::json!({}))
        .await;
    assert_eq!(fixture.log_count().await, 3);

    logs::execute(
        parse(&["trace", "show", &fixture.trace_id]),
        &ctx(&fixture.pool, false),
    )
    .await
    .expect("text render");
    logs::execute(
        parse(&["trace", "show", &fixture.trace_id, "--verbose"]),
        &ctx(&fixture.pool, false),
    )
    .await
    .expect("verbose text render");
    logs::execute(
        parse(&["trace", "show", &fixture.trace_id]),
        &ctx(&fixture.pool, true),
    )
    .await
    .expect("json render");

    fixture.cleanup().await;
}

#[tokio::test]
async fn artifact_text_and_data_parts_both_render_and_unknown_kinds_are_skipped() {
    let fixture = Fixture::new("artifact").await;
    fixture.insert_ai_request().await;
    fixture.insert_mcp_execution().await;
    fixture.insert_artifact_with_parts().await;
    fixture
        .insert_attributed_log("INFO", "artifact produced", serde_json::json!({"parts": 3}))
        .await;

    logs::execute(
        parse(&["trace", "show", &fixture.trace_id, "--ai"]),
        &ctx(&fixture.pool, false),
    )
    .await
    .expect("the AI view renders the artifact content blocks");

    logs::execute(
        parse(&["trace", "show", &fixture.trace_id, "--mcp"]),
        &ctx(&fixture.pool, false),
    )
    .await
    .expect("the MCP view renders alongside it");

    fixture.cleanup().await;
}
