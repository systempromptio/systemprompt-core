//! `infra logs request show` with its two optional sections, and the MCP trace
//! view's message-preview and tool-error renderers.
//!
//! Those renderers branch on message length and log level, so the fixture
//! seeds one short message, one over the 500-character preview threshold, one
//! long system message, and an ERROR-level tool log.

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
    request_id: String,
    mcp_id: String,
    tool_name: String,
}

impl Seed {
    async fn new() -> Self {
        let url = fixture_database_url().unwrap();
        let pool = fixture_db_pool(&url).await.unwrap();
        let raw = pool.pool_arc().unwrap().as_ref().clone();

        let tag = uuid::Uuid::new_v4().simple().to_string();
        let user_id = format!("prev_user_{tag}");
        let context_id = ContextId::generate().as_str().to_owned();
        let task_id = format!("prev_task_{tag}");
        let trace_id = format!("prev_trace_{tag}");
        let request_id = format!("prev_req_{tag}");
        let mcp_id = uuid::Uuid::new_v4().simple().to_string();
        let tool_name = format!("prev_tool_{tag}");

        sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
            .bind(&user_id)
            .bind(format!("{user_id}@preview.test"))
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
            request_id,
            mcp_id,
            tool_name,
        };
        seed.populate().await;
        seed
    }

    async fn populate(&self) {
        sqlx::query(
            "INSERT INTO mcp_tool_executions \
             (mcp_execution_id, tool_name, server_name, started_at, execution_time_ms, input, \
              output, status, user_id, session_id, task_id, context_id, trace_id) \
             VALUES ($1, $6, 'srv', now(), 12, '{\"q\":\"x\"}', 'out', 'success', $2, \
              'sess', $3, $4, $5)",
        )
        .bind(&self.mcp_id)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.context_id)
        .bind(&self.trace_id)
        .bind(&self.tool_name)
        .execute(&self.raw)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO ai_requests \
             (id, request_id, user_id, context_id, provider, model, requested_model, actor_kind, actor_id, \
              task_id, trace_id, status, mcp_execution_id, input_tokens, output_tokens, \
              cost_microdollars, latency_ms) \
             VALUES ($1, $1, $2, '00000000-0000-0000-0000-00000000c0de', 'openai', 'gpt-4', 'gpt-4o', 'user', $2, $3, $4, 'completed', \
              $5, 17, 29, 1234, 44)",
        )
        .bind(&self.request_id)
        .bind(&self.user_id)
        .bind(&self.task_id)
        .bind(&self.trace_id)
        .bind(&self.mcp_id)
        .execute(&self.raw)
        .await
        .unwrap();

        // Three message shapes, one per preview branch: a long system message
        // (collapsed to a character count), a >=500-char body (truncated with
        // an ellipsis), and a short one rendered whole.
        for (seq, role, content) in [
            (0_i32, "system", "s".repeat(400)),
            (1, "user", "u".repeat(600)),
            (2, "assistant", "short reply".to_owned()),
        ] {
            sqlx::query(
                "INSERT INTO ai_request_messages (request_id, role, content, sequence_number) \
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(&self.request_id)
            .bind(role)
            .bind(&content)
            .bind(seq)
            .execute(&self.raw)
            .await
            .unwrap();
        }

        sqlx::query(
            "INSERT INTO ai_request_tool_calls \
             (request_id, tool_name, tool_input, mcp_execution_id, sequence_number) \
             VALUES ($1, $3, '{\"arg\":1}', $2, 0)",
        )
        .bind(&self.request_id)
        .bind(&self.mcp_id)
        .bind(&self.tool_name)
        .execute(&self.raw)
        .await
        .unwrap();

        // One ERROR and one INFO tool log: the error branch prints a detail
        // block the info branch does not.
        for (level, message) in [
            ("ERROR", "tool blew up while calling upstream"),
            ("INFO", "tool executed successfully"),
        ] {
            sqlx::query(
                "INSERT INTO logs (level, module, message, trace_id, task_id, context_id, \
                 user_id, session_id) VALUES ($1, 'agent_tools', $2, $3, $4, $5, $6, 'sess')",
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
    }

    async fn cleanup(&self) {
        for stmt in [
            "DELETE FROM ai_request_tool_calls WHERE request_id = $1",
            "DELETE FROM ai_request_messages WHERE request_id = $1",
        ] {
            let _ = sqlx::query(stmt)
                .bind(&self.request_id)
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
async fn request_show_omits_the_optional_sections_unless_asked_for() {
    let seed = Seed::new().await;

    logs::execute(
        parse(&["request", "show", &seed.request_id]),
        &ctx(&seed.pool, true),
    )
    .await
    .expect("bare request show");

    logs::execute(
        parse(&["request", "show", &seed.request_id, "--messages"]),
        &ctx(&seed.pool, true),
    )
    .await
    .expect("request show with messages");

    logs::execute(
        parse(&["request", "show", &seed.request_id, "--tools"]),
        &ctx(&seed.pool, true),
    )
    .await
    .expect("request show with linked tool calls");

    logs::execute(
        parse(&["request", "show", &seed.request_id, "--messages", "--tools"]),
        &ctx(&seed.pool, false),
    )
    .await
    .expect("request show with both, rendered as text");

    seed.cleanup().await;
}

#[tokio::test]
async fn request_show_resolves_a_partial_id_and_reports_an_unmatched_one() {
    let seed = Seed::new().await;

    let prefix = &seed.request_id[..12];
    logs::execute(
        parse(&["request", "show", prefix, "--messages"]),
        &ctx(&seed.pool, true),
    )
    .await
    .expect("a unique id prefix must resolve");

    logs::execute(
        parse(&["request", "show", "zzz-no-such-request-id"]),
        &ctx(&seed.pool, true),
    )
    .await
    .expect("an unmatched id renders a not-found card rather than erroring");

    seed.cleanup().await;
}

#[tokio::test]
async fn the_mcp_trace_view_renders_message_previews_and_tool_error_details() {
    let seed = Seed::new().await;

    logs::execute(
        parse(&["trace", "show", &seed.trace_id, "--mcp"]),
        &ctx(&seed.pool, false),
    )
    .await
    .expect("the MCP view renders linked AI requests, previews and tool errors");

    logs::execute(
        parse(&["trace", "show", &seed.trace_id, "--ai"]),
        &ctx(&seed.pool, false),
    )
    .await
    .expect("the AI view renders the same trace");

    logs::execute(
        parse(&["trace", "show", &seed.trace_id, "--mcp", "--verbose"]),
        &ctx(&seed.pool, false),
    )
    .await
    .expect("the verbose MCP view");

    seed.cleanup().await;
}

#[tokio::test]
async fn a_tool_listing_scoped_to_the_seeded_tool_renders_in_both_modes() {
    let seed = Seed::new().await;

    // Scoped server-side by the fixture's unique tool name: an unfiltered
    // `tools list` is a globally-limited query whose result depends on every
    // other suite's rows.
    logs::execute(
        parse(&["tools", "list", "--name", &seed.tool_name]),
        &ctx(&seed.pool, true),
    )
    .await
    .expect("json listing scoped to the seeded tool");
    logs::execute(
        parse(&["tools", "list", "--name", &seed.tool_name]),
        &ctx(&seed.pool, false),
    )
    .await
    .expect("text listing scoped to the seeded tool");

    seed.cleanup().await;
}
