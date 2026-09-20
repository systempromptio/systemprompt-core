use anyhow::Result;
use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::{Extension, Router};
use systemprompt_api::routes::{artifacts_router, contexts_router, tasks_router};
use systemprompt_identifiers::{ArtifactId, ContextId, SessionId, TaskId, UserId};
use systemprompt_test_fixtures::{
    DisposableDb, fixture_app_context, seed_user_row, seed_user_session,
};
use tower::ServiceExt;

use super::common::{empty_get, request_context};

struct Seeded {
    user: UserId,
    context: ContextId,
    task: TaskId,
    artifact: ArtifactId,
    message: String,
}

async fn seed(db: &systemprompt_database::DbPool) -> Result<Seeded> {
    let user = UserId::new(format!("outage-{}", uuid::Uuid::new_v4().simple()));
    let session = SessionId::generate();
    seed_user_row(db, &user, &format!("{user}@outage.invalid")).await?;
    seed_user_session(db, &user, &session).await?;
    let context = ContextId::generate();
    let task = TaskId::generate();
    let artifact = ArtifactId::generate();
    let message = systemprompt_identifiers::MessageId::generate().to_string();
    let trace = systemprompt_identifiers::TraceId::generate();
    let pool = db.pool_arc()?;
    sqlx::query("INSERT INTO user_contexts(context_id,user_id,session_id,name) VALUES($1,$2,$3,'outage context')")
        .bind(context.as_str()).bind(user.as_str()).bind(session.as_str()).execute(pool.as_ref()).await?;
    sqlx::query("INSERT INTO agent_tasks(task_id,context_id,status,status_timestamp,user_id,agent_name) VALUES($1,$2,'TASK_STATE_WORKING',now(),$3,'outage-agent')")
        .bind(task.as_str()).bind(context.as_str()).bind(user.as_str()).execute(pool.as_ref()).await?;
    sqlx::query("INSERT INTO task_messages(task_id,message_id,role,context_id,user_id,session_id,trace_id,sequence_number) VALUES($1,$2,'user',$3,$4,$5,$6,0)")
        .bind(task.as_str()).bind(&message).bind(context.as_str()).bind(user.as_str())
        .bind(session.as_str()).bind(trace.as_str()).execute(pool.as_ref()).await?;
    sqlx::query("INSERT INTO task_artifacts(task_id,context_id,artifact_id,name,artifact_type) VALUES($1,$2,$3,'outage artifact','table')")
        .bind(task.as_str()).bind(context.as_str()).bind(artifact.as_str()).execute(pool.as_ref()).await?;
    sqlx::query("INSERT INTO artifact_parts(artifact_id,context_id,part_kind,sequence_number,data_content) VALUES($1,$2,'data',0,$3::jsonb)")
        .bind(artifact.as_str()).bind(context.as_str()).bind(r#"{"columns":["name"],"rows":[["kept"]]}"#)
        .execute(pool.as_ref()).await?;
    Ok(Seeded {
        user,
        context,
        task,
        artifact,
        message,
    })
}

fn apps(ctx: &systemprompt_runtime::AppContext, user: &UserId) -> (Router, Router, Router) {
    let layer = || Extension(request_context(user.as_str()));
    (
        contexts_router().with_state(ctx.clone()).layer(layer()),
        tasks_router().with_state(ctx.clone()).layer(layer()),
        artifacts_router().with_state(ctx.clone()).layer(layer()),
    )
}

async fn json(app: Router, uri: &str) -> Result<(StatusCode, serde_json::Value)> {
    let response = app.oneshot(empty_get(uri)).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
    Ok((status, serde_json::from_slice(&bytes)?))
}

async fn assert_live(ctx: &systemprompt_runtime::AppContext, s: &Seeded) -> Result<()> {
    let (contexts, tasks, artifacts) = apps(ctx, &s.user);
    let (status, body) = json(contexts.clone(), "/").await?;
    assert_eq!(status, StatusCode::OK);
    let rows = body["data"].as_array().expect("context collection data");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["context_id"].as_str(), Some(s.context.as_str()));
    let (status, body) = json(contexts, &format!("/{}/tasks", s.context)).await?;
    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("context task list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"].as_str(), Some(s.task.as_str()));
    assert_eq!(rows[0]["contextId"].as_str(), Some(s.context.as_str()));
    let (status, body) = json(tasks.clone(), "/").await?;
    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("user task list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"].as_str(), Some(s.task.as_str()));
    let (status, body) = json(tasks.clone(), &format!("/{}", s.task)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"].as_str(), Some(s.task.as_str()));
    assert_eq!(body["contextId"].as_str(), Some(s.context.as_str()));
    let (status, body) = json(tasks.clone(), &format!("/{}/messages", s.task)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().map(Vec::len), Some(1));
    assert_eq!(body[0]["messageId"].as_str(), Some(s.message.as_str()));
    assert_eq!(body[0]["taskId"].as_str(), Some(s.task.as_str()));
    let (status, body) = json(tasks, &format!("/{}/artifacts", s.task)).await?;
    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("task artifact list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"].as_str(), Some(s.artifact.as_str()));
    let (status, body) = json(artifacts.clone(), "/").await?;
    assert_eq!(status, StatusCode::OK);
    let rows = body.as_array().expect("user artifact list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"].as_str(), Some(s.artifact.as_str()));
    let (status, body) = json(artifacts, &format!("/{}", s.artifact)).await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"].as_str(), Some(s.artifact.as_str()));
    Ok(())
}

async fn assert_outage(
    ctx: &systemprompt_runtime::AppContext,
    s: &Seeded,
    database_url: &str,
) -> Result<()> {
    let (contexts, tasks, artifacts) = apps(ctx, &s.user);
    for (app, uri) in [
        (contexts.clone(), "/".to_owned()),
        (contexts, format!("/{}/tasks", s.context)),
        (tasks.clone(), "/".to_owned()),
        (tasks.clone(), format!("/{}", s.task)),
        (tasks.clone(), format!("/{}/messages", s.task)),
        (tasks, format!("/{}/artifacts", s.task)),
        (artifacts.clone(), "/".to_owned()),
        (artifacts, format!("/{}", s.artifact)),
    ] {
        let (status, body) = json(app, &uri).await?;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{uri}: {body}");
        assert!(
            body["message"]
                .as_str()
                .is_some_and(|message| !message.is_empty())
        );
        let rendered = body.to_string();
        assert!(!rendered.contains(database_url));
        assert!(!rendered.contains("postgres://"));
        assert!(!rendered.contains("password"));
    }
    Ok(())
}

#[tokio::test]
async fn agent_read_routes_report_database_outage_and_recover_without_data_loss() -> Result<()> {
    let owned = DisposableDb::installed("agent_read_outage").await?;
    let db = owned.pool().await?;
    let seeded = seed(&db).await?;
    let ctx = fixture_app_context(&db, owned.url())?;
    assert_live(&ctx, &seeded).await?;
    let raw = db.pool_arc()?;
    raw.close().await;
    assert_outage(&ctx, &seeded, owned.url()).await?;
    drop(ctx);
    drop(raw);
    drop(db);
    let recovered = owned.pool().await?;
    let recovered_ctx = fixture_app_context(&recovered, owned.url())?;
    assert_live(&recovered_ctx, &seeded).await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_tasks WHERE task_id=$1")
        .bind(seeded.task.as_str())
        .fetch_one(recovered.pool_arc()?.as_ref())
        .await?;
    assert_eq!(count, 1);
    drop(recovered_ctx);
    drop(recovered);
    owned.drop_now().await;
    Ok(())
}
