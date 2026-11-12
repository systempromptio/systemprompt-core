use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part};
use systemprompt_agent::services::ContextService;
use systemprompt_identifiers::ArtifactId;

use crate::repository::{repos, seed_context_and_task, seed_runtime_task, seed_user_and_session};
use systemprompt_test_fixtures::test_db_pool;

#[tokio::test]
async fn conversation_history_includes_durable_artifact_identity_and_bounded_description() {
    let pool = test_db_pool().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (context, runtime) = seed_runtime_task(&repositories, &user, &session).await;
    let task = runtime.task_id().clone();
    let artifact = Artifact {
        id: ArtifactId::generate(),
        title: Some("Quarterly report".to_owned()),
        description: Some("x".repeat(350)),
        parts: vec![Part::text(
            "the complete report body must not be injected as history".to_owned(),
        )],
        extensions: Vec::new(),
        metadata: ArtifactMetadata::new("report".to_owned(), context.clone(), task.clone()),
    };
    repositories
        .artifacts
        .create_artifact(&runtime, &artifact)
        .await
        .expect("artifact persisted");

    let messages = ContextService::new(repositories.tasks.clone(), repositories.contexts.clone())
        .load_conversation_history(&context, &user)
        .await
        .expect("conversation history");
    let artifact_message = messages
        .iter()
        .find(|message| message.content.starts_with("[Artifact:"))
        .expect("artifact context message");
    assert_eq!(
        artifact_message.role,
        systemprompt_models::MessageRole::Assistant
    );
    assert_eq!(
        artifact_message.content,
        format!(
            "[Artifact: Quarterly report (type: report, id: {})]\n{}...",
            artifact.id,
            "x".repeat(297)
        )
    );
    assert!(artifact_message.parts.is_empty());
    assert!(!artifact_message.content.contains("complete report body"));
}

#[tokio::test]
async fn unnamed_artifact_without_description_still_contributes_stable_context_identity() {
    let pool = test_db_pool().await;
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (context, runtime) = seed_runtime_task(&repositories, &user, &session).await;
    let task = runtime.task_id().clone();
    let artifact = Artifact {
        id: ArtifactId::generate(),
        title: None,
        description: Some(String::new()),
        parts: Vec::new(),
        extensions: Vec::new(),
        metadata: ArtifactMetadata::new("binary".to_owned(), context.clone(), task.clone()),
    };
    repositories
        .artifacts
        .create_artifact(&runtime, &artifact)
        .await
        .expect("artifact persisted");

    let messages = ContextService::new(repositories.tasks.clone(), repositories.contexts.clone())
        .load_conversation_history(&context, &user)
        .await
        .expect("conversation history");
    assert!(messages.iter().any(|message| {
        message.content == format!("[Artifact: unnamed (type: binary, id: {})]", artifact.id)
            && message.parts.is_empty()
    }));
}

#[tokio::test]
async fn conversation_history_is_refused_for_a_context_the_caller_does_not_own() {
    let pool = test_db_pool().await;
    let repositories = repos(&pool);
    let (owner, session) = seed_user_and_session(&pool).await;
    let (context, _task) = seed_context_and_task(&repositories, &owner, &session).await;
    let (stranger, _) = seed_user_and_session(&pool).await;

    let result = ContextService::new(repositories.tasks.clone(), repositories.contexts.clone())
        .load_conversation_history(&context, &stranger)
        .await;
    assert!(
        result.is_err(),
        "another user's context history must not load: {result:?}"
    );
}
