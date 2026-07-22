//! DB-backed tests for the MCP section of `infra logs trace show` — tool I/O
//! truncation, linked AI-request message previews, and the log fallback
//! rendered when a trace has tool logs but no MCP execution rows.

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

fn ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Table),
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
}

impl Seed {
    async fn new() -> Self {
        let url = fixture_database_url().unwrap();
        let pool = fixture_db_pool(&url).await.unwrap();
        let raw = pool.pool_arc().unwrap().as_ref().clone();

        let tag = uuid::Uuid::new_v4().simple().to_string();
        let user_id = format!("mcp_user_{tag}");
        let context_id = ContextId::generate().as_str().to_owned();
        let task_id = format!("mcp_task_{tag}");
        let trace_id = format!("mcp_trace_{tag}");
        let tool_name = format!("mcp_tool_{tag}");

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
            tool_name,
        }
    }

    async fn insert_mcp(&self, input: &str, output: Option<&str>) -> String {
        let id = uuid::Uuid::new_v4().simple().to_string();
        sqlx::query(
            "INSERT INTO mcp_tool_executions \
             (mcp_execution_id, tool_name, server_name, started_at, execution_time_ms, input, \
              output, status, user_id, session_id, task_id, context_id, trace_id) \
             VALUES ($1, $2, 'srv', now(), 7, $3, $4, 'success', $5, 'sess', $6, $7, $8)",
        )
        .bind(&id)
        .bind(&self.tool_name)
        .bind(input)
        .bind(output)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.trace_id)
        .execute(&self.raw)
        .await
        .unwrap();
        id
    }

    async fn insert_linked_ai_request(&self, id: &str, mcp_execution_id: &str) {
        sqlx::query(
            "INSERT INTO ai_requests \
             (id, request_id, user_id, provider, model, requested_model, actor_kind, actor_id, \
              task_id, trace_id, status, mcp_execution_id, input_tokens, output_tokens, \
              cost_microdollars) \
             VALUES ($1, $1, $2, 'openai', 'gpt-4', 'gpt-4o', 'user', $2, $3, $4, 'completed', \
              $5, 13, 8, 5)",
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

    async fn insert_message(&self, request_id: &str, seq: i32, role: &str, content: &str) {
        sqlx::query(
            "INSERT INTO ai_request_messages (request_id, role, content, sequence_number) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(request_id)
        .bind(role)
        .bind(content)
        .bind(seq)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn insert_log(&self, level: &str, message: &str) {
        sqlx::query(
            "INSERT INTO logs (level, module, message, trace_id, task_id, context_id, user_id, \
             session_id) VALUES ($1, 'agent_tools', $2, $3, $4, $5, $6, 'sess')",
        )
        .bind(level)
        .bind(message)
        .bind(&self.trace_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.user_id)
        .execute(&self.raw)
        .await
        .unwrap();
    }

    async fn cleanup(&self) {
        for (stmt, key) in [
            (
                "DELETE FROM ai_request_messages WHERE request_id IN \
                 (SELECT id FROM ai_requests WHERE trace_id = $1)",
                &self.trace_id,
            ),
            (
                "DELETE FROM ai_requests WHERE trace_id = $1",
                &self.trace_id,
            ),
            ("DELETE FROM logs WHERE trace_id = $1", &self.trace_id),
            (
                "DELETE FROM mcp_tool_executions WHERE trace_id = $1",
                &self.trace_id,
            ),
            ("DELETE FROM agent_tasks WHERE task_id = $1", &self.task_id),
            (
                "DELETE FROM user_contexts WHERE user_id = $1",
                &self.user_id,
            ),
            ("DELETE FROM users WHERE id = $1", &self.user_id),
        ] {
            let _ = sqlx::query(stmt).bind(key).execute(&self.raw).await;
        }
    }
}

#[tokio::test]
async fn trace_show_truncates_large_tool_io_and_previews_linked_messages() {
    let seed = Seed::new().await;

    let big_input = format!("payload {}", "i".repeat(400));
    let big_output = "o".repeat(700);
    let exec_id = seed.insert_mcp(&big_input, Some(&big_output)).await;
    let json_exec = seed
        .insert_mcp(r#"{"query":"x"}"#, Some(r#"{"ok":true}"#))
        .await;

    let linked = format!("req_{}", uuid::Uuid::new_v4().simple());
    seed.insert_linked_ai_request(&linked, &exec_id).await;
    seed.insert_message(&linked, 0, "system", &"s".repeat(150))
        .await;
    seed.insert_message(&linked, 1, "user", &"u".repeat(600))
        .await;
    seed.insert_message(&linked, 2, "assistant", "short answer")
        .await;

    let other = format!("req_{}", uuid::Uuid::new_v4().simple());
    seed.insert_linked_ai_request(&other, &json_exec).await;

    let ctx = ctx(&seed.pool);
    logs::execute(parse(&["trace", "show", &seed.task_id, "--mcp"]), &ctx)
        .await
        .unwrap();
    logs::execute(
        parse(&["trace", "show", &seed.task_id, "--mcp", "--verbose"]),
        &ctx,
    )
    .await
    .unwrap();

    seed.cleanup().await;
}

#[tokio::test]
async fn trace_show_falls_back_to_tool_logs_without_mcp_rows() {
    let seed = Seed::new().await;

    seed.insert_log("ERROR", "tool blew up").await;
    seed.insert_log("WARN", "tool warned").await;
    seed.insert_log("DEBUG", "tool debug detail").await;
    seed.insert_log("INFO", "tool ran").await;

    logs::execute(
        parse(&["trace", "show", &seed.task_id, "--mcp"]),
        &ctx(&seed.pool),
    )
    .await
    .unwrap();

    seed.cleanup().await;
}
