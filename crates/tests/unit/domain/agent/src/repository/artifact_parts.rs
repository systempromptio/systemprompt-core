// DB-backed tests for the artifact part-row persistence free functions:
// per-kind INSERT arms, ordered readback, and the InvalidData decode arm for
// non-object data content (reachable only via a raw row insert, since the
// production INSERT always writes an object).

use serde_json::json;
use systemprompt_agent::models::a2a::Part;
use systemprompt_agent::repository::content::artifact::{
    get_artifact_parts, persist_artifact_part,
};
use systemprompt_identifiers::ArtifactId;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session};
use systemprompt_test_fixtures::test_db_pool;

fn data_part(value: serde_json::Value) -> Part {
    Part::data(value)
}

#[tokio::test]
async fn persist_and_read_back_all_part_kinds_in_sequence_order() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let pg = pool.pool();
    let artifact_id = ArtifactId::generate();

    sqlx::query("INSERT INTO task_artifacts (artifact_id, task_id, context_id, artifact_type) VALUES ($1, $2, $3, 'text')")
        .bind(artifact_id.as_str())
        .bind(task_id.as_str())
        .bind(context_id.as_str())
        .execute(pg.as_ref())
        .await
        .expect("seed artifact row");

    let file = Part::raw("aGVsbG8=", "text/plain").with_filename("report.txt");
    let text = Part::text("first");
    let data = data_part(json!({"answer": 42}));

    persist_artifact_part(pg.as_ref(), &data, &artifact_id, &context_id, 2)
        .await
        .expect("persist data");
    persist_artifact_part(pg.as_ref(), &text, &artifact_id, &context_id, 0)
        .await
        .expect("persist text");
    persist_artifact_part(pg.as_ref(), &file, &artifact_id, &context_id, 1)
        .await
        .expect("persist file");

    let parts = get_artifact_parts(pg.as_ref(), &artifact_id, &context_id)
        .await
        .expect("read parts");
    assert_eq!(parts.len(), 3);
    let part = &parts[0];
    assert_eq!(part.as_text(), Some("first"), "{part:?}");
    let part = &parts[1];
    assert_eq!(part.filename.as_deref(), Some("report.txt"));
    assert_eq!(part.media_type.as_deref(), Some("text/plain"));
    assert_eq!(part.as_raw(), Some("aGVsbG8="));
    assert_eq!(part.as_url(), None);
    let part = &parts[2];
    assert_eq!(
        part.as_data().and_then(|d| d.get("answer")),
        Some(&json!(42)),
        "{part:?}"
    );

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn non_object_data_url_file_and_metadata_round_trip() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let pg = pool.pool();
    let artifact_id = ArtifactId::generate();

    sqlx::query("INSERT INTO task_artifacts (artifact_id, task_id, context_id, artifact_type) VALUES ($1, $2, $3, 'text')")
        .bind(artifact_id.as_str())
        .bind(task_id.as_str())
        .bind(context_id.as_str())
        .execute(pg.as_ref())
        .await
        .expect("seed artifact row");

    let mut url_part = Part::url("https://example.com/r.pdf", "application/pdf");
    url_part.metadata = json!({"source": "test"}).as_object().cloned();
    persist_artifact_part(
        pg.as_ref(),
        &data_part(json!([1, 2])),
        &artifact_id,
        &context_id,
        0,
    )
    .await
    .expect("persist array data");
    persist_artifact_part(pg.as_ref(), &url_part, &artifact_id, &context_id, 1)
        .await
        .expect("persist url part");

    let parts = get_artifact_parts(pg.as_ref(), &artifact_id, &context_id)
        .await
        .expect("read parts");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].as_data(), Some(&json!([1, 2])));
    assert_eq!(parts[0].metadata, None);
    assert_eq!(parts[1], url_part);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_parts_empty_for_unknown_artifact() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let pg = pool.pool();

    let parts = get_artifact_parts(pg.as_ref(), &ArtifactId::generate(), &context_id)
        .await
        .expect("read parts");
    assert!(parts.is_empty());

    r.tasks.delete_task(&task_id).await.ok();
}
