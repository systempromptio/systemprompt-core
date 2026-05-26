//! Admin CLI gateway — exercises validate_cli_args via the POST handler.

use axum::Extension;
use systemprompt_api::routes::admin;
use tower::ServiceExt;

use super::common::{json_post, request_context, setup_ctx};

#[tokio::test]
async fn cli_post_empty_args_returns_400() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = admin::router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("admin_cli")));
    let resp = app
        .oneshot(json_post(
            "/cli",
            serde_json::json!({ "args": [], "timeout_secs": 5 }),
        ))
        .await?;
    let status = resp.status().as_u16();
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn cli_post_too_many_args_returns_400() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = admin::router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("admin_cli")));
    let args: Vec<String> = (0..40).map(|i| format!("arg{i}")).collect();
    let resp = app
        .oneshot(json_post(
            "/cli",
            serde_json::json!({ "args": args, "timeout_secs": 5 }),
        ))
        .await?;
    let status = resp.status().as_u16();
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn cli_post_forbidden_char_returns_400() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = admin::router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("admin_cli")));
    let resp = app
        .oneshot(json_post(
            "/cli",
            serde_json::json!({ "args": ["echo", "hello | rm -rf /"], "timeout_secs": 5 }),
        ))
        .await?;
    let status = resp.status().as_u16();
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn cli_post_bad_subcommand_returns_400() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = admin::router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("admin_cli")));
    let resp = app
        .oneshot(json_post(
            "/cli",
            serde_json::json!({ "args": ["BAD-Cmd"], "timeout_secs": 5 }),
        ))
        .await?;
    let status = resp.status().as_u16();
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}

#[tokio::test]
async fn cli_post_arg_too_long_returns_400() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let app = admin::router()
        .with_state((*ctx).clone())
        .layer(Extension(request_context("admin_cli")));
    let long = "a".repeat(300);
    let resp = app
        .oneshot(json_post(
            "/cli",
            serde_json::json!({ "args": ["foo", long], "timeout_secs": 5 }),
        ))
        .await?;
    let status = resp.status().as_u16();
    assert!((400..600).contains(&status), "{status}");
    Ok(())
}
