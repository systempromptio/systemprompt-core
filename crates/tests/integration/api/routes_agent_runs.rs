//! `/api/v1/agent-runs`: origin registration, idempotent enqueue, server-owned
//! paths and per-user scoping, driven through the router.

use anyhow::Result;
use axum::{Extension, Router};
use http::StatusCode;
use serde_json::{Value, json};
use systemprompt_api::routes::agent_runs::{AgentRunsState, router};
use systemprompt_test_fixtures::seed_admin_credential;
use tower::ServiceExt;

use super::common::{body_to_string, empty_get, json_post, request_context, setup_ctx};

const ORIGIN: &str = "sp_test_api_origin";

systemprompt_agent_runs::register_run_origin!(ORIGIN);

fn request(prompt: &str) -> Value {
    json!({
        "host": "opencode-cli",
        "prompt": prompt,
        "permission_mode": "default",
        "workspace": "/etc",
        "wall_clock": { "secs": 60, "nanos": 0 },
        "traffic": "evaluation"
    })
}

fn app(state: AgentRunsState, user: &str) -> Router {
    router(state).layer(Extension(request_context(user)))
}

async fn send(app: Router, req: http::Request<axum::body::Body>) -> Result<(StatusCode, Value)> {
    let resp = app.oneshot(req).await?;
    let (status, body) = body_to_string(resp).await?;
    Ok((status, serde_json::from_str(&body).unwrap_or(Value::Null)))
}

#[tokio::test]
async fn enqueue_is_idempotent_and_scoped_to_the_caller() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_admin_credential(&pool, "agent-runs-owner").await?;
    let other = seed_admin_credential(&pool, "agent-runs-other").await?;
    let state = AgentRunsState::build(&ctx);
    let user = owner.user_id.as_str();
    let key = format!("key-{}", owner.user_id);

    let body = json!({ "origin_kind": ORIGIN, "idempotency_key": key, "request": request("a") });
    let (status, created) = send(app(state.clone(), user), json_post("/", body.clone())).await?;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    let id = created["id"].as_str().expect("id").to_owned();

    let (status, again) = send(app(state.clone(), user), json_post("/", body)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again["id"], id.as_str());
    assert_eq!(again["existing"], true);

    let changed = json!({ "origin_kind": ORIGIN, "idempotency_key": key, "request": request("b") });
    let (status, _) = send(app(state.clone(), user), json_post("/", changed)).await?;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, run) = send(app(state.clone(), user), empty_get(&format!("/{id}"))).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(run["status"], "queued");
    let stored = state
        .runs()
        .find_run(&systemprompt_identifiers::AgentRunId::new(&id))
        .await?
        .expect("row");
    let workspace = stored.request["workspace"].as_str().expect("workspace");
    assert_ne!(workspace, "/etc", "the server owns the workspace");
    assert!(workspace.ends_with(&format!("{id}/workspace")));

    let (status, _) = send(
        app(state.clone(), other.user_id.as_str()),
        empty_get(&format!("/{id}")),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, events) = send(
        app(state.clone(), user),
        empty_get(&format!("/{id}/events")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(events["events"], json!([]));

    let (status, _) = send(
        app(state, user),
        json_post(&format!("/{id}/cancel"), json!({})),
    )
    .await?;
    assert_eq!(status, StatusCode::ACCEPTED);
    Ok(())
}

#[tokio::test]
async fn enqueue_refuses_unregistered_origins_and_server_paths() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_admin_credential(&pool, "agent-runs-refuse").await?;
    let state = AgentRunsState::build(&ctx);
    let user = owner.user_id.as_str();

    let unregistered =
        json!({ "origin_kind": "nobody", "idempotency_key": "k", "request": request("a") });
    let (status, _) = send(app(state.clone(), user), json_post("/", unregistered)).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut dirs = request("a");
    dirs["add_dirs"] = json!(["/root"]);
    let body = json!({ "origin_kind": ORIGIN, "idempotency_key": "k-dirs", "request": dirs });
    let (status, _) = send(app(state.clone(), user), json_post("/", body)).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut stdio = request("a");
    stdio["mcp_servers"] = json!({ "x": { "type": "stdio", "command": "sh" } });
    let body = json!({ "origin_kind": ORIGIN, "idempotency_key": "k-stdio", "request": stdio });
    let (status, _) = send(app(state, user), json_post("/", body)).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn the_event_stream_replays_from_last_event_id_and_ends_on_a_terminal_run() -> Result<()> {
    use systemprompt_agent_runs::{
        ErrorClass, RunEvent, RunEventKind, RunEventPayload, RunOutcome, RunStatus,
    };
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_admin_credential(&pool, "agent-runs-stream").await?;
    let state = AgentRunsState::build(&ctx);
    let user = owner.user_id.as_str();
    let body = json!({ "origin_kind": ORIGIN, "idempotency_key": format!("stream-{user}"), "request": request("s") });
    let (_, created) = send(app(state.clone(), user), json_post("/", body)).await?;
    let id = systemprompt_identifiers::AgentRunId::new(created["id"].as_str().expect("id"));
    let events: Vec<RunEvent> = (0..3)
        .map(|seq| RunEvent {
            seq,
            kind: RunEventKind::AssistantText,
            tool_name: None,
            tool_use_id: None,
            payload: RunEventPayload::AssistantText {
                text: format!("t{seq}"),
            },
            created_at: chrono::Utc::now(),
        })
        .collect();
    state.runs().insert_events_batch(&id, &events).await?;
    let outcome = RunOutcome {
        status: RunStatus::Succeeded,
        error_class: ErrorClass::Ok,
        session_id: None,
        cost_microdollars: 0,
        input_tokens: 0,
        output_tokens: 0,
        turns: 1,
        duration_ms: 1,
        structured_output: None,
        exit_code: Some(0),
        error: None,
        final_text: Some("t2".into()),
    };
    state.runs().finish(&id, &outcome, None).await?;

    let req = http::Request::builder()
        .uri(format!("/{id}/events/stream"))
        .header("last-event-id", "0")
        .body(axum::body::Body::empty())?;
    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        body_to_string(app(state, user).oneshot(req).await?),
    )
    .await??;
    let (status, text) = resp;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !text.contains("\"t0\""),
        "events at or before Last-Event-ID are not resent"
    );
    assert!(text.contains("t1") && text.contains("t2"));
    assert!(text.contains("event: end"));
    Ok(())
}

const OUTPUTS_ORIGIN: &str = "sp_test_api_outputs_origin";

systemprompt_agent_runs::register_run_origin!(OUTPUTS_ORIGIN);

const CLAIM_ORIGIN: &str = "sp_test_api_claim_origin";

systemprompt_agent_runs::register_run_origin!(CLAIM_ORIGIN);

async fn claim(
    state: &AgentRunsState,
    origin: &str,
    id: &systemprompt_identifiers::AgentRunId,
) -> Result<()> {
    while let Some(row) = state
        .runs()
        .claim_queued(origin, "covapi-worker", std::time::Duration::from_secs(60))
        .await?
    {
        if &row.id == id {
            return Ok(());
        }
    }
    anyhow::bail!("run {id} was never claimed")
}

#[tokio::test]
async fn outputs_are_listed_and_read_from_the_run_directory_only() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_admin_credential(&pool, "agent-runs-outputs").await?;
    let state = AgentRunsState::build(&ctx);
    let user = owner.user_id.as_str();
    let body = json!({
        "origin_kind": OUTPUTS_ORIGIN,
        "idempotency_key": format!("outputs-{user}"),
        "request": request("o")
    });
    let (status, created) = send(app(state.clone(), user), json_post("/", body)).await?;
    assert_eq!(status, StatusCode::ACCEPTED, "{created}");
    let id = created["id"].as_str().expect("id").to_owned();

    let (status, empty) = send(
        app(state.clone(), user),
        empty_get(&format!("/{id}/outputs")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["files"], json!([]));

    let run_id = systemprompt_identifiers::AgentRunId::new(&id);
    claim(&state, OUTPUTS_ORIGIN, &run_id).await?;
    let dir = tempfile::tempdir()?;
    let root = dir.path().to_path_buf();
    state
        .runs()
        .update_started(
            &run_id,
            systemprompt_agent_runs::repository::RunStarted {
                run_dir: &root,
                pid: None,
                gateway_session_id: None,
                api_key_id: None,
            },
        )
        .await?;
    tokio::fs::create_dir_all(root.join("outputs")).await?;
    tokio::fs::write(root.join("outputs/report.txt"), b"report body").await?;
    let manifest = json!([{ "path": "report.txt", "size": 11, "sha256": "ab" }]);
    tokio::fs::write(root.join("outputs.json"), manifest.to_string()).await?;

    let (status, listed) = send(
        app(state.clone(), user),
        empty_get(&format!("/{id}/outputs")),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["files"], manifest);

    let resp = app(state.clone(), user)
        .oneshot(empty_get(&format!("/{id}/outputs/report.txt")))
        .await?;
    assert_eq!(
        resp.headers()[http::header::CONTENT_TYPE],
        "application/octet-stream"
    );
    let (status, text) = body_to_string(resp).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(text, "report body");

    let (status, _) = send(
        app(state.clone(), user),
        empty_get(&format!("/{id}/outputs/missing.txt")),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = send(
        app(state.clone(), user),
        empty_get(&format!("/{id}/outputs/..%2Foutputs.json")),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn cancelling_a_claimed_run_records_the_request() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = seed_admin_credential(&pool, "agent-runs-cancel-claimed").await?;
    let state = AgentRunsState::build(&ctx);
    let user = owner.user_id.as_str();
    let origin = CLAIM_ORIGIN;
    let body = json!({
        "origin_kind": origin,
        "idempotency_key": format!("claimed-{user}"),
        "request": request("c")
    });
    let (_, created) = send(app(state.clone(), user), json_post("/", body)).await?;
    let id = systemprompt_identifiers::AgentRunId::new(created["id"].as_str().expect("id"));

    claim(&state, origin, &id).await?;

    let (status, _) = send(
        app(state.clone(), user),
        json_post(&format!("/{id}/cancel"), json!({})),
    )
    .await?;
    assert_eq!(status, StatusCode::ACCEPTED);
    let row = state.runs().find_run(&id).await?.expect("row");
    assert!(row.cancel_requested_at.is_some(), "{row:?}");
    assert_eq!(row.status, "preparing");
    Ok(())
}
