use systemprompt_agent::models::a2a::{Artifact, ArtifactMetadata, Part, TextPart};
use systemprompt_agent::services::ContextService;
use systemprompt_identifiers::ArtifactId;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

#[tokio::test]
async fn conversation_history_includes_durable_artifact_identity_and_bounded_description() {
    let pool = try_pool_or_skip()
        .await
        .expect("agent database fixture must be configured");
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (context, task) = seed_context_and_task(&repositories, &user, &session).await;
    let artifact = Artifact {
        id: ArtifactId::generate(),
        title: Some("Quarterly report".to_owned()),
        description: Some("x".repeat(350)),
        parts: vec![Part::Text(TextPart {
            text: "the complete report body must not be injected as history".to_owned(),
        })],
        extensions: Vec::new(),
        metadata: ArtifactMetadata::new("report".to_owned(), context.clone(), task.clone()),
    };
    repositories
        .artifacts
        .create_artifact(&task, &context, &artifact)
        .await
        .expect("artifact persisted");

    let messages = ContextService::new(repositories.tasks.clone())
        .load_conversation_history(&context)
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
    let pool = try_pool_or_skip()
        .await
        .expect("agent database fixture must be configured");
    let repositories = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (context, task) = seed_context_and_task(&repositories, &user, &session).await;
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
        .create_artifact(&task, &context, &artifact)
        .await
        .expect("artifact persisted");

    let messages = ContextService::new(repositories.tasks.clone())
        .load_conversation_history(&context)
        .await
        .expect("conversation history");
    assert!(messages.iter().any(|message| {
        message.content == format!("[Artifact: unnamed (type: binary, id: {})]", artifact.id)
            && message.parts.is_empty()
    }));
}
