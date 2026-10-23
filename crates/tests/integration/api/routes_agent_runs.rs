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
