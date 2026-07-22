// DB-backed tests for the artifact part-row persistence free functions:
// per-kind INSERT arms, ordered readback, and the InvalidData decode arm for
// non-object data content (reachable only via a raw row insert, since the
// production INSERT always writes an object).

use serde_json::json;
use systemprompt_agent::models::a2a::{DataPart, FileContent, FilePart, Part, TextPart};
use systemprompt_agent::repository::content::artifact::{
    get_artifact_parts, persist_artifact_part,
};
use systemprompt_identifiers::ArtifactId;
use systemprompt_traits::RepositoryError;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn data_part(value: serde_json::Value) -> Part {
    let serde_json::Value::Object(map) = value else {
        panic!("test data must be an object");
    };
    Part::Data(DataPart { data: map })
}

#[tokio::test]
async fn persist_and_read_back_all_part_kinds_in_sequence_order() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let pg = pool.pool_arc().expect("pg pool");
    let artifact_id = ArtifactId::generate();

    sqlx::query("INSERT INTO task_artifacts (artifact_id, task_id, context_id, artifact_type) VALUES ($1, $2, $3, 'text')")
        .bind(artifact_id.as_str())
        .bind(task_id.as_str())
        .bind(context_id.as_str())
        .execute(pg.as_ref())
        .await
        .expect("seed artifact row");

    let file = Part::File(FilePart {
        file: FileContent {
            name: Some("report.txt".to_owned()),
            mime_type: Some("text/plain".to_owned()),
            bytes: Some("aGVsbG8=".to_owned()),
            url: None,
        },
    });
    let text = Part::Text(TextPart {
        text: "first".to_owned(),
    });
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
    match &parts[0] {
        Part::Text(t) => assert_eq!(t.text, "first"),
        other => panic!("expected text part first, got {other:?}"),
    }
    match &parts[1] {
        Part::File(f) => {
            assert_eq!(f.file.name.as_deref(), Some("report.txt"));
            assert_eq!(f.file.mime_type.as_deref(), Some("text/plain"));
            assert_eq!(f.file.bytes.as_deref(), Some("aGVsbG8="));
            assert_eq!(f.file.url, None);
        },
        other => panic!("expected file part second, got {other:?}"),
    }
    match &parts[2] {
        Part::Data(d) => assert_eq!(d.data.get("answer"), Some(&json!(42))),
        other => panic!("expected data part third, got {other:?}"),
    }

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_parts_rejects_non_object_data_content() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let pg = pool.pool_arc().expect("pg pool");
    let artifact_id = ArtifactId::generate();

    sqlx::query("INSERT INTO task_artifacts (artifact_id, task_id, context_id, artifact_type) VALUES ($1, $2, $3, 'text')")
        .bind(artifact_id.as_str())
        .bind(task_id.as_str())
        .bind(context_id.as_str())
        .execute(pg.as_ref())
        .await
        .expect("seed artifact row");
    sqlx::query(
        "INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, \
         data_content) VALUES ($1, $2, 'data', 0, '[1, 2]'::jsonb)",
    )
    .bind(artifact_id.as_str())
    .bind(context_id.as_str())
    .execute(pg.as_ref())
    .await
    .expect("seed malformed data part");

    let err = get_artifact_parts(pg.as_ref(), &artifact_id, &context_id)
        .await
        .expect_err("non-object data must be rejected");
    match err {
        RepositoryError::InvalidData(msg) => assert!(msg.contains("JSON object")),
        other => panic!("expected InvalidData, got {other:?}"),
    }

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_parts_empty_for_unknown_artifact() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let pg = pool.pool_arc().expect("pg pool");

    let parts = get_artifact_parts(pg.as_ref(), &ArtifactId::generate(), &context_id)
        .await
        .expect("read parts");
    assert!(parts.is_empty());

    r.tasks.delete_task(&task_id).await.ok();
}
