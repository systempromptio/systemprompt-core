//! DB-backed tests that seed a full AI/MCP trace and drive the `infra logs`
//! read commands (`request show`, `audit`, `trace show`, `tools list`,
//! `search`) through their dispatchers so the row-mapping and rendering bodies
//! execute against real rows.

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

struct Seed {
    pool: DbPool,
    raw: sqlx::PgPool,
    user_id: String,
    context_id: String,
    task_id: String,
    trace_id: String,
    tool_name: String,
    request_id: String,
    mcp_success_id: String,
}

impl Seed {
    async fn new() -> Self {
        let url = fixture_database_url().unwrap();
        let pool = fixture_db_pool(&url).await.unwrap();
        let raw = pool.pool_arc().unwrap().as_ref().clone();

        let tag = uuid::Uuid::new_v4().simple().to_string();
        let user_id = format!("seed_user_{tag}");
        let context_id = ContextId::generate().as_str().to_owned();
        let task_id = format!("seed_task_{tag}");
        let trace_id = format!("seed_trace_{tag}");
        let tool_name = format!("seed_tool_{tag}");
        let request_id = format!("seed_req_{tag}");

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

        let seed = Self {
            pool,
            raw,
            user_id,
            context_id,
            task_id,
            trace_id,
            tool_name,
            request_id,
            mcp_success_id: String::new(),
        };
        seed.populate().await
    }

    async fn insert_mcp(&self, status: &str, error: Option<&str>, elapsed: i32) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO mcp_tool_executions \
             (mcp_execution_id, tool_name, server_name, started_at, execution_time_ms, input, \
              output, status, error_message, user_id, session_id, task_id, context_id, trace_id) \
             VALUES ($1, $9, 'srv', now(), $2, '{\"q\":\"x\"}', 'result-body', $3, $4, $5, 'sess', \
              $6, $7, $8)",
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
        .execute(&self.raw)
        .await
        .unwrap();
        id
    }

    async fn insert_ai_request(&self, id: &str, mcp_execution_id: Option<&str>) {
        sqlx::query(
            "INSERT INTO ai_requests \
             (id, request_id, user_id, provider, model, requested_model, actor_kind, actor_id, \
              task_id, trace_id, status, mcp_execution_id, input_tokens, output_tokens, \
              cost_microdollars, latency_ms) \
             VALUES ($1, $1, $2, 'openai', 'gpt-4', 'gpt-4o', 'user', $2, $3, $4, 'completed', \
              $5, 11, 22, 5, 30)",
        )
        .bind(id)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.trace_id)
        .bind(mcp_execution_id)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn insert_message(&self, seq: i32, role: &str, content: &str) {
        sqlx::query(
            "INSERT INTO ai_request_messages (request_id, role, content, sequence_number) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&self.request_id)
        .bind(role)
        .bind(content)
        .bind(seq)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn insert_tool_call(&self, seq: i32, mcp_execution_id: &str) {
        sqlx::query(
            "INSERT INTO ai_request_tool_calls \
             (request_id, tool_name, tool_input, mcp_execution_id, sequence_number) \
             VALUES ($1, $2, '{\"arg\":1}', $3, $4)",
        )
        .bind(&self.request_id)
        .bind(&self.tool_name)
        .bind(mcp_execution_id)
        .bind(seq)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn insert_step(&self, step_type: &str, status: &str) {
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
        .bind(serde_json::json!({"type": step_type}))
        .execute(&self.raw)
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
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn populate(mut self) -> Self {
        let mcp_success = self.insert_mcp("success", None, 11).await;
        self.insert_mcp("failed", Some(&"e".repeat(120)), 42).await;

        self.insert_ai_request(&self.request_id.clone(), None).await;
        let linked_req = format!("{}_linked", self.request_id);
        self.insert_ai_request(&linked_req, Some(&mcp_success)).await;

        self.insert_message(0, "system", &"s".repeat(150)).await;
        self.insert_message(1, "user", "user question").await;
        self.insert_tool_call(0, &mcp_success).await;

        self.insert_step("tool_execution", "completed").await;
        self.insert_step("planning", "in_progress").await;
        self.insert_step("other", "failed").await;

        self.insert_tool_log().await;

        self.mcp_success_id = mcp_success;
        self
    }

    async fn cleanup(&self) {
        for stmt in [
            "DELETE FROM ai_request_tool_calls WHERE request_id LIKE $1",
            "DELETE FROM ai_request_messages WHERE request_id LIKE $1",
        ] {
            let _ = sqlx::query(stmt)
                .bind(format!("{}%", self.request_id))
                .execute(&self.raw)
                .await;
        }
        let _ = sqlx::query("DELETE FROM ai_requests WHERE trace_id = $1")
            .bind(&self.trace_id)
            .execute(&self.raw)
            .await;
        for stmt in [
            "DELETE FROM logs WHERE trace_id = $1",
            "DELETE FROM mcp_tool_executions WHERE trace_id = $1",
            "DELETE FROM task_execution_steps WHERE task_id = $1",
        ] {
            let key = if stmt.contains("task_id = $1") {
                &self.task_id
            } else {
                &self.trace_id
            };
            let _ = sqlx::query(stmt).bind(key).execute(&self.raw).await;
        }
        let _ = sqlx::query("DELETE FROM agent_tasks WHERE task_id = $1")
            .bind(&self.task_id)
            .execute(&self.raw)
            .await;
        let _ = sqlx::query("DELETE FROM user_contexts WHERE user_id = $1")
            .bind(&self.user_id)
            .execute(&self.raw)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(&self.user_id)
            .execute(&self.raw)
            .await;
    }
}

#[tokio::test]
async fn request_show_renders_messages_and_linked_tools() {
    let seed = Seed::new().await;
    let human = ctx(&seed.pool, false);

    logs::execute(
        parse(&[
            "request",
            "show",
            &seed.request_id,
            "--messages",
            "--tools",
        ]),
        &human,
    )
    .await
    .unwrap();

    logs::execute(
        parse(&["request", "show", &seed.request_id]),
        &ctx(&seed.pool, true),
    )
    .await
    .unwrap();

    logs::execute(parse(&["request", "show", "req_absent_zzz"]), &human)
        .await
        .unwrap();

    seed.cleanup().await;
}

#[tokio::test]
async fn audit_resolves_request_with_messages_and_tool_calls() {
    let seed = Seed::new().await;

    logs::execute(parse(&["audit", &seed.request_id]), &ctx(&seed.pool, false))
        .await
        .unwrap();
    logs::execute(parse(&["audit", &seed.request_id]), &ctx(&seed.pool, true))
        .await
        .unwrap();

    seed.cleanup().await;
}

#[tokio::test]
async fn trace_show_by_task_renders_ai_and_mcp_sections() {
    let seed = Seed::new().await;
    let ctx = ctx(&seed.pool, false);

    logs::execute(
        parse(&["trace", "show", &seed.task_id, "--all"]),
        &ctx,
    )
    .await
    .unwrap();
    logs::execute(
        parse(&["trace", "show", &seed.task_id, "--ai", "--mcp", "--verbose"]),
        &ctx,
    )
    .await
    .unwrap();

    seed.cleanup().await;
}

#[tokio::test]
async fn trace_show_by_trace_renders_summary_and_json() {
    let seed = Seed::new().await;

    logs::execute(
        parse(&["trace", "show", &seed.trace_id, "--all"]),
        &ctx(&seed.pool, false),
    )
    .await
    .unwrap();
    logs::execute(
        parse(&["trace", "show", &seed.trace_id, "--json"]),
        &ctx(&seed.pool, false),
    )
    .await
    .unwrap();

    seed.cleanup().await;
}

#[tokio::test]
async fn tools_list_filters_and_renders_executions() {
    let seed = Seed::new().await;

    logs::execute(
        parse(&[
            "tools",
            "list",
            "--name",
            &seed.tool_name,
            "--server",
            "srv",
            "--since",
            "1h",
        ]),
        &ctx(&seed.pool, false),
    )
    .await
    .unwrap();
    logs::execute(
        parse(&["tools", "list", "--status", "failed"]),
        &ctx(&seed.pool, true),
    )
    .await
    .unwrap();

    seed.cleanup().await;
}

#[tokio::test]
async fn search_surfaces_seeded_logs_and_tools() {
    let seed = Seed::new().await;

    logs::execute(
        parse(&[
            "search",
            &seed.tool_name,
            "--module",
            "agent_tools",
            "--since",
            "1h",
        ]),
        &ctx(&seed.pool, false),
    )
    .await
    .unwrap();
    logs::execute(
        parse(&["search", "Tool executed", "--level", "info"]),
        &ctx(&seed.pool, true),
    )
    .await
    .unwrap();

    seed.cleanup().await;
}
