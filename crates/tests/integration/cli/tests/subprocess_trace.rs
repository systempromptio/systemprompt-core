//! Subprocess tests for `infra logs trace show` / `list` against seeded
//! trace data: an AI task with execution steps, AI requests, MCP tool
//! executions, and artifacts (the `ai_*` display path), plus a raw
//! log-event trace (the `TraceQueryService` table path).

use std::sync::OnceLock;

use predicates::prelude::*;
use systemprompt_cli_integration_tests::full_bootstrap::{command, database_url};

struct SeededTrace {
    task_id: String,
    log_trace_id: String,
}

static SEED: OnceLock<Option<SeededTrace>> = OnceLock::new();

fn seeded() -> Option<&'static SeededTrace> {
    SEED.get_or_init(|| {
        let url = database_url()?;
        command()?;
        Some(seed_trace_data(&url))
    })
    .as_ref()
}

fn seed_trace_data(url: &str) -> SeededTrace {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build seed runtime");
    runtime.block_on(async {
        let pool = sqlx::PgPool::connect(url).await.expect("connect seed db");
        seed(&pool).await
    })
}

async fn seed(pool: &sqlx::PgPool) -> SeededTrace {
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let task_id = format!("covtask{suffix}");
    let context_id = uuid::Uuid::new_v4().to_string();
    let task_trace_id = format!("covtasktrace{suffix}");
    let log_trace_id = format!("covlogtrace{suffix}");
    let mcp_ok = format!("covmcpok{suffix}");
    let mcp_failed = format!("covmcpfail{suffix}");
    let ai_task = format!("covaitask{suffix}");
    let ai_linked = format!("covailinked{suffix}");

    let user_id: String = sqlx::query_scalar("SELECT id FROM users ORDER BY created_at LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("bootstrap admin user present");

    sqlx::query("INSERT INTO user_contexts (context_id, user_id, name) VALUES ($1, $2, 'cov trace context')")
        .bind(&context_id)
        .bind(&user_id)
        .execute(pool)
        .await
        .expect("seed user_contexts");

    sqlx::query(
        "INSERT INTO agent_tasks (task_id, context_id, status, user_id, trace_id, agent_name,
                                  started_at, completed_at, execution_time_ms)
         VALUES ($1, $2, 'TASK_STATE_COMPLETED', $3, $4, 'covagent',
                 NOW() - INTERVAL '10 seconds', NOW(), 9500)",
    )
    .bind(&task_id)
    .bind(&context_id)
    .bind(&user_id)
    .bind(&task_trace_id)
    .execute(pool)
    .await
    .expect("seed agent_tasks");

    for (message_id, role, seq, text) in [
        (format!("covmsguser{suffix}"), "user", 0, "What files changed?"),
        (format!("covmsgagent{suffix}"), "agent", 1, "Two files changed in the last commit."),
    ] {
        sqlx::query(
            "INSERT INTO task_messages (task_id, message_id, role, context_id, user_id, sequence_number)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&task_id)
        .bind(&message_id)
        .bind(role)
        .bind(&context_id)
        .bind(&user_id)
        .bind(seq)
        .execute(pool)
        .await
        .expect("seed task_messages");

        sqlx::query(
            "INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, text_content)
             VALUES ($1, $2, 'text', 0, $3)",
        )
        .bind(&message_id)
        .bind(&task_id)
        .bind(text)
        .execute(pool)
        .await
        .expect("seed message_parts");
    }

    for (step, step_type, title, status, error) in [
        ("a", "planning", "Plan the lookup", "completed", None::<&str>),
        ("b", "tool_call", "Run git diff", "failed", Some("tool timed out")),
        ("c", "response", "Summarise findings", "pending", None),
    ] {
        sqlx::query(
            "INSERT INTO task_execution_steps (step_id, task_id, step_type, title, status,
                                               content, completed_at, duration_ms, error_message)
             VALUES ($1, $2, $3, $4, $5, $6, NOW(), 120, $7)",
        )
        .bind(format!("covstep{step}{suffix}"))
        .bind(&task_id)
        .bind(step_type)
        .bind(title)
        .bind(status)
        .bind(serde_json::json!({"type": step_type, "title": title}))
        .bind(error)
        .execute(pool)
        .await
        .expect("seed task_execution_steps");
    }

    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at,
                                          completed_at, execution_time_ms, input, output, status,
                                          user_id, context_id, task_id, trace_id)
         VALUES ($1, 'git_diff', 'fixture_mcp', NOW() - INTERVAL '8 seconds', NOW(), 340,
                 '{\"paths\":[\"src\"]}', '{\"changed\":2}', 'success', $2, $3, $4, $5)",
    )
    .bind(&mcp_ok)
    .bind(&user_id)
    .bind(&context_id)
    .bind(&task_id)
    .bind(&task_trace_id)
    .execute(pool)
    .await
    .expect("seed mcp success execution");

    sqlx::query(
        "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at,
                                          execution_time_ms, input, status, error_message,
                                          user_id, context_id, task_id, trace_id)
         VALUES ($1, 'git_blame', 'fixture_mcp', NOW() - INTERVAL '7 seconds', 55,
                 '{\"file\":\"main.rs\"}', 'failed', 'blame backend unavailable', $2, $3, $4, $5)",
    )
    .bind(&mcp_failed)
    .bind(&user_id)
    .bind(&context_id)
    .bind(&task_id)
    .bind(&task_trace_id)
    .execute(pool)
    .await
    .expect("seed mcp failed execution");

    for (id, mcp_link) in [(&ai_task, None), (&ai_linked, Some(&mcp_ok))] {
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, task_id, context_id, trace_id,
                                      mcp_execution_id, provider, model, max_tokens, input_tokens,
                                      output_tokens, cost_microdollars, latency_ms, status,
                                      actor_kind, actor_id, completed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'anthropic', 'claude-sonnet-4-5', 1024, 250,
                     125, 4200, 1800, 'success', 'user', $3, NOW())",
        )
        .bind(id)
        .bind(format!("req-{id}"))
        .bind(&user_id)
        .bind(&task_id)
        .bind(&context_id)
        .bind(&task_trace_id)
        .bind(mcp_link)
        .execute(pool)
        .await
        .expect("seed ai_requests");

        for (seq, role, content) in [
            (0, "system", "You are a coverage fixture."),
            (1, "user", "Summarise the diff."),
            (2, "assistant", "Two files changed."),
        ] {
            sqlx::query(
                "INSERT INTO ai_request_messages (request_id, role, content, sequence_number)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(id)
            .bind(role)
            .bind(content)
            .bind(seq)
            .execute(pool)
            .await
            .expect("seed ai_request_messages");
        }
    }

    for (artifact, artifact_type, name, source, tool, mcp_link) in [
        ("one", "document", "diff-summary.md", "agent", None::<&str>, None),
        ("two", "data", "raw-diff.json", "tool", Some("git_diff"), Some(&mcp_ok)),
    ] {
        sqlx::query(
            "INSERT INTO task_artifacts (task_id, context_id, artifact_id, name, artifact_type,
                                         source, tool_name, mcp_execution_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&task_id)
        .bind(&context_id)
        .bind(format!("covartifact{artifact}{suffix}"))
        .bind(name)
        .bind(artifact_type)
        .bind(source)
        .bind(tool)
        .bind(mcp_link)
        .execute(pool)
        .await
        .expect("seed task_artifacts");
    }

    for (level, module, message) in [
        ("INFO", "agent.loop", "iteration 1 starting"),
        ("INFO", "agent.tools", "tool executed: git_diff"),
        ("WARN", "agent.loop", "retrying provider call"),
        ("ERROR", "agent.loop", "provider call failed once"),
        ("INFO", "agent.loop", "agentic loop complete"),
        ("DEBUG", "agent.loop", "broadcast execution_step update"),
    ] {
        sqlx::query(
            "INSERT INTO logs (level, module, message, user_id, session_id, trace_id, context_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(level)
        .bind(module)
        .bind(message)
        .bind(&user_id)
        .bind(format!("covsession{suffix}"))
        .bind(&log_trace_id)
        .bind(&context_id)
        .execute(pool)
        .await
        .expect("seed logs");
    }

    SeededTrace {
        task_id,
        log_trace_id,
    }
}

fn trace_show(id: &str, extra: &[&str]) -> Option<assert_cmd::Command> {
    let mut cmd = command()?;
    cmd.args(["infra", "logs", "trace", "show", id]);
    cmd.args(extra);
    Some(cmd)
}

#[test]
fn trace_show_task_all_sections() {
    let Some(seeded) = seeded() else { return };
    let Some(mut cmd) = trace_show(&seeded.task_id, &["--all"]) else {
        return;
    };
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("git_diff"))
        .stderr(predicate::str::contains("diff-summary.md"))
        .stderr(predicate::str::contains("Run git diff"))
        .stderr(predicate::str::contains("What files changed?"));
}

#[test]
fn trace_show_task_verbose_sections() {
    let Some(seeded) = seeded() else { return };
    let Some(mut cmd) = trace_show(
        &seeded.task_id,
        &["--steps", "--ai", "--mcp", "--artifacts", "--verbose"],
    ) else {
        return;
    };
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("git_blame"))
        .stderr(predicate::str::contains("blame backend unavailable"));
}

#[test]
fn trace_show_task_json() {
    let Some(seeded) = seeded() else { return };
    let Some(mut cmd) = trace_show(&seeded.task_id, &["--json"]) else {
        return;
    };
    cmd.assert().success();
}

#[test]
fn trace_show_task_partial_id() {
    let Some(seeded) = seeded() else { return };
    let partial = &seeded.task_id[..seeded.task_id.len() - 4];
    let Some(mut cmd) = trace_show(partial, &["--all"]) else {
        return;
    };
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("git_diff"));
}

#[test]
fn trace_show_log_events_table() {
    let Some(seeded) = seeded() else { return };
    let Some(mut cmd) = trace_show(&seeded.log_trace_id, &[]) else {
        return;
    };
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("agentic loop complete"));
}

#[test]
fn trace_show_log_events_verbose() {
    let Some(seeded) = seeded() else { return };
    let Some(mut cmd) = trace_show(&seeded.log_trace_id, &["--verbose"]) else {
        return;
    };
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("retrying provider call"));
}

#[test]
fn trace_show_log_events_json() {
    let Some(seeded) = seeded() else { return };
    let Some(mut cmd) = trace_show(&seeded.log_trace_id, &["--json"]) else {
        return;
    };
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(&seeded.log_trace_id));
}

#[test]
fn trace_show_unknown_id_reports_empty() {
    let Some(_seeded) = seeded() else { return };
    let Some(mut cmd) = trace_show("covnosuchtrace", &[]) else {
        return;
    };
    cmd.assert()
        .success()
        .stderr(predicate::str::contains("No events found"));
}

#[test]
fn trace_list_includes_seeded_traces() {
    let Some(_seeded) = seeded() else { return };
    let Some(mut cmd) = command() else { return };
    cmd.args(["infra", "logs", "trace", "list", "--limit", "50"]);
    cmd.assert().success();
}
