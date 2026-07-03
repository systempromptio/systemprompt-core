//! Admin CLI gateway — subprocess forward / timeout / exit-code / SSE stream.
//!
//! Points the gateway at a throwaway shell script (via the `test-api`
//! `cli_router_with_binary` seam) so the spawn, line-forward, exit-code, and
//! timeout branches of `create_cli_stream` run end-to-end without the deployed
//! `/app/bin/systemprompt` binary. All frames are collected by draining the SSE
//! body to completion — no sleeps stand in for synchronisation.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use axum::Extension;
use systemprompt_api::routes::admin::cli_test_api::cli_router_with_binary;
use tower::ServiceExt;

use super::common::{body_to_string, json_post, request_context, setup_ctx};

fn write_script(body: &str) -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("cli-gw-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("fixture.sh");
    let mut f = std::fs::File::create(&path)?;
    f.write_all(body.as_bytes())?;
    f.flush()?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    Ok(path)
}

#[tokio::test]
async fn subprocess_forwards_stdout_stderr_and_exit_code() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let script = write_script("#!/bin/sh\necho out-line\necho err-line 1>&2\nexit 3\n")?;
    let app = cli_router_with_binary(&script.to_string_lossy())
        .with_state((*ctx).clone())
        .layer(Extension(request_context("cli_sub")));

    let resp = app
        .oneshot(json_post(
            "/",
            serde_json::json!({ "args": ["status"], "timeout_secs": 30 }),
        ))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("started"), "started frame: {body}");
    assert!(body.contains("out-line"), "stdout frame: {body}");
    assert!(body.contains("err-line"), "stderr frame: {body}");
    assert!(body.contains("exit_code"), "exit frame: {body}");
    assert!(body.contains("\"code\":3"), "exit code 3: {body}");
    Ok(())
}

#[tokio::test]
async fn subprocess_zero_timeout_yields_timeout_error() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let script = write_script("#!/bin/sh\nsleep 30\n")?;
    let app = cli_router_with_binary(&script.to_string_lossy())
        .with_state((*ctx).clone())
        .layer(Extension(request_context("cli_sub")));

    let resp = app
        .oneshot(json_post(
            "/",
            serde_json::json!({ "args": ["status"], "timeout_secs": 0 }),
        ))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("Timeout after 0s"), "timeout message: {body}");
    assert!(body.contains("\"code\":-1"), "timeout exit code: {body}");
    Ok(())
}

#[tokio::test]
async fn subprocess_spawn_failure_yields_error_frame() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let missing = std::env::temp_dir().join(format!("no-such-bin-{}", uuid::Uuid::new_v4()));
    let app = cli_router_with_binary(&missing.to_string_lossy())
        .with_state((*ctx).clone())
        .layer(Extension(request_context("cli_sub")));

    let resp = app
        .oneshot(json_post(
            "/",
            serde_json::json!({ "args": ["status"], "timeout_secs": 30 }),
        ))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("error"), "error frame: {body}");
    assert!(
        body.contains("\"code\":1"),
        "spawn-failure exit code: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn subprocess_clean_exit_zero() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let script = write_script("#!/bin/sh\necho hi\n")?;
    let app = cli_router_with_binary(&script.to_string_lossy())
        .with_state((*ctx).clone())
        .layer(Extension(request_context("cli_sub")));

    let resp = app
        .oneshot(json_post(
            "/",
            serde_json::json!({ "args": ["status"], "timeout_secs": 30 }),
        ))
        .await?;
    let (status, body) = body_to_string(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body.contains("\"code\":0"), "clean exit code: {body}");
    Ok(())
}
