use super::{repos, seed_context_and_task, seed_user_and_session, try_pool};
use systemprompt_agent::models::a2a::{
    Artifact, ArtifactMetadata, DataPart, FileContent, FilePart, Part, TextPart,
};
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_identifiers::{ArtifactId, ContextId, TaskId, UserId};

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
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = ArtifactRepository::new(r.db_pool()).expect("artifact repo");

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "hello".to_owned(),
        })],
    );
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    let fetched = artifacts
        .get_artifact_by_id(&artifact_id)
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
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let artifacts = ArtifactRepository::new(r.db_pool()).expect("artifact repo");
    let result = artifacts
        .get_artifact_by_id(&ArtifactId::generate())
        .await
        .expect("get");
    assert!(result.is_none());
}

#[tokio::test]
async fn artifact_with_all_part_kinds_roundtrip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = ArtifactRepository::new(r.db_pool()).expect("artifact repo");

    let mut map = serde_json::Map::new();
    map.insert("n".to_owned(), serde_json::json!(7));
    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![
            Part::Text(TextPart {
                text: "t".to_owned(),
            }),
            Part::File(FilePart {
                file: FileContent {
                    name: Some("a.bin".to_owned()),
                    mime_type: Some("application/octet-stream".to_owned()),
                    bytes: Some("AAAA".to_owned()),
                    url: None,
                },
            }),
            Part::Data(DataPart { data: map }),
        ],
    );
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    let fetched = artifacts
        .get_artifact_by_id(&artifact_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.parts.len(), 3);
    assert!(matches!(fetched.parts[0], Part::Text(_)));
    assert!(matches!(fetched.parts[1], Part::File(_)));
    assert!(matches!(fetched.parts[2], Part::Data(_)));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn create_artifact_upserts_on_conflict() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = ArtifactRepository::new(r.db_pool()).expect("artifact repo");

    let artifact_id = ArtifactId::generate();
    let mut artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "v1".to_owned(),
        })],
    );
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    artifact.title = Some("renamed".to_owned());
    artifact.parts = vec![Part::Text(TextPart {
        text: "v2".to_owned(),
    })];
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("upsert");

    let fetched = artifacts
        .get_artifact_by_id(&artifact_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(fetched.title.as_deref(), Some("renamed"));
    assert_eq!(fetched.parts.len(), 1);
    match &fetched.parts[0] {
        Part::Text(t) => assert_eq!(t.text, "v2"),
        other => panic!("expected text, got {other:?}"),
    }

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn list_artifacts_by_task_context_and_user() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = ArtifactRepository::new(r.db_pool()).expect("artifact repo");

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "x".to_owned(),
        })],
    );
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
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = ArtifactRepository::new(r.db_pool()).expect("artifact repo");

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "x".to_owned(),
        })],
    );
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
        systemprompt_traits::RepositoryError::NotFound(_)
    ));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn delete_artifact() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let artifacts = ArtifactRepository::new(r.db_pool()).expect("artifact repo");

    let artifact_id = ArtifactId::generate();
    let artifact = make_artifact(
        &artifact_id,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "x".to_owned(),
        })],
    );
    artifacts
        .create_artifact(&task_id, &context_id, &artifact)
        .await
        .expect("create");

    artifacts
        .delete_artifact(&artifact_id)
        .await
        .expect("delete");
    let fetched = artifacts
        .get_artifact_by_id(&artifact_id)
        .await
        .expect("get");
    assert!(fetched.is_none());

    r.tasks.delete_task(&task_id).await.ok();
}
