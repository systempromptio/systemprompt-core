//! Agent artifact retrieval routes through `artifacts_router`.
//!
//! Drives `list_artifacts_by_user`, `get_artifact`, and `get_artifact_ui`
//! against a real DB with an injected `RequestContext` (the full router injects
//! this via the jwt-context middleware; the bare router test layers it
//! directly, matching `routes_agent_tasks`). The list path runs the user-scoped
//! repository query; the single-artifact and UI paths exercise the
//! ownership-validation and not-found branches.

use axum::Extension;
use systemprompt_api::routes::artifacts_router;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, request_context, setup_ctx};

fn app_for(ctx: &AppContext, user: &str) -> axum::Router {
    artifacts_router()
        .with_state(ctx.clone())
        .layer(Extension(request_context(user)))
}

#[tokio::test]
async fn list_artifacts_by_user_returns_array() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str()).oneshot(empty_get("/")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn list_artifacts_by_user_accepts_limit() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str())
        .oneshot(empty_get("/?limit=10"))
        .await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_unknown_artifact_returns_4xx() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str())
        .oneshot(empty_get("/artifact_does_not_exist"))
        .await?;
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn get_unknown_artifact_ui_returns_4xx() -> anyhow::Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let user = UserId::new(format!("art-{}", Uuid::new_v4()));
    let resp = app_for(&ctx, user.as_str())
        .oneshot(empty_get("/artifact_does_not_exist/ui"))
        .await?;
    assert!(resp.status().as_u16() >= 400, "{}", resp.status());
    Ok(())
}

// `list_artifacts_by_context` / `list_artifacts_by_task` both validate
// ownership before reading, and the existing coverage never seeds a context or
// task — so every call stops at the guard and the listing bodies never run.
mod owned {
    use anyhow::Result;
    use axum::Extension;
    use axum::body::to_bytes;
    use axum::extract::{Path, State};
    use axum::response::IntoResponse;
    use systemprompt_api::routes::agent::artifacts::{
        list_artifacts_by_context, list_artifacts_by_task,
    };
    use systemprompt_database::DbPool;
    use systemprompt_identifiers::{
        Actor, AgentName, ContextId, SessionId, TaskId, TraceId, UserId,
    };
    use systemprompt_models::RequestContext;
    use systemprompt_test_fixtures::{seed_user_row, seed_user_session};
    use uuid::Uuid;

    use crate::common::setup_ctx;

    fn ctx_for(user: &UserId) -> RequestContext {
        RequestContext::new(
            SessionId::generate(),
            TraceId::generate(),
            ContextId::generate(),
            AgentName::new("artifact-test"),
        )
        .with_actor(Actor::user(user.clone()))
    }

    async fn seed_user(db: &DbPool) -> Result<UserId> {
        let user = UserId::new(format!("art-{}", Uuid::new_v4().simple()));
        seed_user_row(db, &user, &format!("{}@artifacts.invalid", user.as_str())).await?;
        Ok(user)
    }

    async fn seed_context(db: &DbPool, user: &UserId) -> Result<ContextId> {
        let context = ContextId::generate();
        let session = SessionId::generate();
        seed_user_session(db, user, &session).await?;
        let p = db.pool_arc()?;
        sqlx::query(
            "INSERT INTO user_contexts (context_id, user_id, session_id, name) \
             VALUES ($1, $2, $3, 'artifact-fixture')",
        )
        .bind(context.as_str())
        .bind(user.as_str())
        .bind(session.as_str())
        .execute(p.as_ref())
        .await?;
        Ok(context)
    }

    #[tokio::test]
    async fn an_owner_lists_the_artifacts_of_their_own_context() -> Result<()> {
        let (db, ctx) = setup_ctx().await?;
        let user = seed_user(&db).await?;
        let context = seed_context(&db, &user).await?;

        let response = list_artifacts_by_context(
            Extension(ctx_for(&user)),
            State((*ctx).clone()),
            Path(context.as_str().to_owned()),
        )
        .await
        .map_err(|e| anyhow::anyhow!("owner listing must succeed: {e:?}"))?
        .into_response();

        assert_eq!(response.status().as_u16(), 200);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
        let artifacts: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(
            artifacts.as_array().map(Vec::len),
            Some(0),
            "a fresh context has no artifacts, but the listing must still succeed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_stranger_cannot_list_another_users_context() -> Result<()> {
        let (db, ctx) = setup_ctx().await?;
        let owner = seed_user(&db).await?;
        let stranger = seed_user(&db).await?;
        let context = seed_context(&db, &owner).await?;

        let result = list_artifacts_by_context(
            Extension(ctx_for(&stranger)),
            State((*ctx).clone()),
            Path(context.as_str().to_owned()),
        )
        .await;

        assert!(
            result.is_err(),
            "context ownership is the only thing standing between users' artifacts"
        );
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_context_is_refused_rather_than_listed_empty() -> Result<()> {
        let (db, ctx) = setup_ctx().await?;
        let user = seed_user(&db).await?;

        let result = list_artifacts_by_context(
            Extension(ctx_for(&user)),
            State((*ctx).clone()),
            Path(ContextId::generate().as_str().to_owned()),
        )
        .await;

        assert!(
            result.is_err(),
            "an unknown context must not be reported as an empty one the caller owns"
        );
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_task_is_refused() -> Result<()> {
        let (db, ctx) = setup_ctx().await?;
        let user = seed_user(&db).await?;

        let result = list_artifacts_by_task(
            Extension(ctx_for(&user)),
            State((*ctx).clone()),
            Path(TaskId::generate().as_str().to_owned()),
        )
        .await;

        assert!(
            result.is_err(),
            "task ownership is validated before any read"
        );
        Ok(())
    }
}
