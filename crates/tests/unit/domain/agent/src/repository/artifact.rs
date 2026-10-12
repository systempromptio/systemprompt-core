use super::{repos, seed_context_and_task, seed_user_and_session};
use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part};
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};
use systemprompt_test_fixtures::test_db_pool;

fn make_artifact(
    artifact_id: &ArtifactId,
    context_id: &ContextId,
    task_id: &TaskId,
    parts: Vec<Part>,
) -> Artifact {
    Artifact {
        id: artifact_id.clone(),
        title: Some("my-artifact".to_owned()),
        description: Some("a test artifact".to_owned()),
        parts,
        extensions: vec![],
        metadata: ArtifactMetadata::new("text".to_owned(), context_id.clone(), task_id.clone())
            .with_tool_name("the-tool".to_owned())
            .with_fingerprint("fp-123".to_owned()),
    }
}

#[tokio::test]
async fn create_and_get_artifact_by_id() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = r.artifacts.clone();

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![Part::text("hello")],
    );
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    let fetched = artifacts
        .find_artifact_by_id(&artifact_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.id, artifact_id);
    assert_eq!(fetched.title.as_deref(), Some("my-artifact"));
    assert_eq!(fetched.parts.len(), 1);
    assert_eq!(fetched.metadata.tool_name.as_deref(), Some("the-tool"));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_artifact_by_id_unknown_returns_none() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let artifacts = r.artifacts.clone();
    let result = artifacts
        .find_artifact_by_id(&ArtifactId::generate())
        .await
        .expect("get");
    assert!(result.is_none());
}

#[tokio::test]
async fn artifact_with_all_part_kinds_roundtrip() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = r.artifacts.clone();

    let mut map = serde_json::Map::new();
    map.insert("n".to_owned(), serde_json::json!(7));
    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![
            Part::text("t"),
            Part::raw("AAAA", "application/octet-stream").with_filename("a.bin"),
            Part::data(serde_json::Value::Object(map)),
        ],
    );
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    let fetched = artifacts
        .find_artifact_by_id(&artifact_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.parts.len(), 3);
    assert!(fetched.parts[0].as_text().is_some());
    assert!(fetched.parts[1].is_file());
    assert!(fetched.parts[2].as_data().is_some());

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn create_artifact_upserts_on_conflict() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = r.artifacts.clone();

    let artifact_id = ArtifactId::generate();
    let mut artifact = make_artifact(&artifact_id, &context_id, &task_id, vec![Part::text("v1")]);
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    artifact.title = Some("renamed".to_owned());
    artifact.parts = vec![Part::text("v2")];
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("upsert");

    let fetched = artifacts
        .find_artifact_by_id(&artifact_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.title.as_deref(), Some("renamed"));
    assert_eq!(fetched.parts.len(), 1);
    let part = &fetched.parts[0];
    assert_eq!(part.as_text(), Some("v2"), "{part:?}");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn list_artifacts_by_task_context_and_user() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = r.artifacts.clone();

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(&artifact_id, &context_id, &task_id, vec![Part::text("x")]);
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    let by_task = artifacts
        .get_artifacts_by_task(&task_id)
        .await
        .expect("by task");
    assert!(by_task.iter().any(|a| a.id == artifact_id));

    let by_ctx = artifacts
        .get_artifacts_by_context(&context_id)
        .await
        .expect("by ctx");
    assert!(by_ctx.iter().any(|a| a.id == artifact_id));

    let by_user = artifacts
        .get_artifacts_by_user_id(&user_id, Some(50))
        .await
        .expect("by user");
    assert!(by_user.iter().any(|a| a.id == artifact_id));

    let all = artifacts.get_all_artifacts(Some(500)).await.expect("all");
    assert!(all.iter().any(|a| a.id == artifact_id));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn validate_artifact_ownership() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = r.artifacts.clone();

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(&artifact_id, &context_id, &task_id, vec![Part::text("x")]);
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    artifacts
        .validate_artifact_ownership(&artifact_id, &user_id)
        .await
        .expect("owned");

    let err = artifacts
        .validate_artifact_ownership(&artifact_id, &UserId::new("intruder"))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound { .. }
    ));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn delete_artifact() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = r.artifacts.clone();

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(&artifact_id, &context_id, &task_id, vec![Part::text("x")]);
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    artifacts
        .delete_artifact(&artifact_id)
        .await
        .expect("delete");
    let fetched = artifacts
        .find_artifact_by_id(&artifact_id)
        .await
        .expect("get");
    assert!(fetched.is_none());

    r.tasks.delete_task(&task_id).await.ok();
}
