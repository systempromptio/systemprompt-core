use anyhow::Result;
use systemprompt_agent::services::a2a_server::processing::conversation_service::ConversationService;
use systemprompt_identifiers::ContextId;
use uuid::Uuid;

use crate::common::Fixture;

#[tokio::test]
async fn conversation_service_load_empty_context_returns_empty() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = ConversationService::new(fx.db.clone());

    // Use a fresh context id not in DB
    let unknown = ContextId::new(Uuid::new_v4().to_string());
    let history = svc.load_conversation_history(&unknown).await?;
    assert!(history.is_empty());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn conversation_service_load_with_tasks_returns_history() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = ConversationService::new(fx.db.clone());

    use systemprompt_models::a2a::TaskState;
    let _task1 = fx.insert_task(TaskState::Completed).await?;
    let _task2 = fx.insert_task(TaskState::Completed).await?;

    let history = svc.load_conversation_history(&fx.context_id).await?;
    // No messages persisted (only task rows), so history is empty
    assert!(history.is_empty());

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn conversation_service_debug_impl() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = ConversationService::new(fx.db.clone());
    let dbg = format!("{:?}", svc);
    assert!(dbg.contains("ConversationService"));
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn conversation_service_with_persisted_messages_returns_history() -> Result<()> {
    use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
    use systemprompt_agent::services::{MessageService, PersistMessagesParams};
    use systemprompt_identifiers::{MessageId, TaskId};
    use systemprompt_models::a2a::TaskState;

    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let msg_svc = MessageService::new(&fx.db)?;
    let conv_svc = ConversationService::new(fx.db.clone());

    let msgs = vec![
        Message {
            role: MessageRole::User,
            message_id: MessageId::generate(),
            task_id: Some(task_id.clone()),
            context_id: fx.context_id.clone(),
            parts: vec![Part::Text(TextPart {
                text: "hello agent".to_string(),
            })],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        },
        Message {
            role: MessageRole::Agent,
            message_id: MessageId::generate(),
            task_id: Some(task_id.clone()),
            context_id: fx.context_id.clone(),
            parts: vec![Part::Text(TextPart {
                text: "hello user".to_string(),
            })],
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        },
    ];

    msg_svc
        .persist_messages(PersistMessagesParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            messages: msgs,
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;

    let history = conv_svc.load_conversation_history(&fx.context_id).await?;
    assert!(!history.is_empty());

    let _ = task_id;
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn conversation_service_with_artifacts_in_history() -> Result<()> {
    use systemprompt_agent::models::a2a::{Artifact, Part, TextPart};
    use systemprompt_agent::repository::content::ArtifactRepository;
    use systemprompt_identifiers::ArtifactId;
    use systemprompt_models::a2a::{ArtifactMetadata, TaskState};
    use systemprompt_test_fixtures::ensure_test_bootstrap;

    ensure_test_bootstrap();
    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Completed).await?;
    let artifact_repo = ArtifactRepository::new(&fx.db)?;

    let artifact = Artifact {
        id: ArtifactId::generate(),
        title: Some("ResultArt".to_string()),
        description: Some("test".to_string()),
        parts: vec![Part::Text(TextPart {
            text: "artifact body".to_string(),
        })],
        extensions: vec![],
        metadata: ArtifactMetadata::new(
            "text".to_string(),
            fx.context_id.clone(),
            task_id.clone(),
        ),
    };
    artifact_repo
        .create_artifact(&task_id, &fx.context_id, &artifact)
        .await?;

    let conv_svc = ConversationService::new(fx.db.clone());
    let history = conv_svc.load_conversation_history(&fx.context_id).await?;
    assert!(history.iter().any(|m| m.content.contains("ResultArt")));

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn conversation_service_with_file_parts_in_message() -> Result<()> {
    use systemprompt_agent::models::a2a::{FileContent, FilePart, Message, MessageRole, Part};
    use systemprompt_agent::services::{MessageService, PersistMessagesParams};
    use systemprompt_identifiers::MessageId;
    use systemprompt_models::a2a::TaskState;

    let fx = Fixture::new().await?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let msg_svc = MessageService::new(&fx.db)?;
    let conv_svc = ConversationService::new(fx.db.clone());

    // base64 of "hello world"
    let b64 = "aGVsbG8gd29ybGQ=".to_string();

    let msgs = vec![Message {
        role: MessageRole::User,
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: fx.context_id.clone(),
        parts: vec![Part::File(FilePart {
            file: FileContent {
                name: Some("note.txt".to_string()),
                mime_type: Some("text/plain".to_string()),
                bytes: Some(b64),
                url: None,
            },
        })],
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }];

    msg_svc
        .persist_messages(PersistMessagesParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            messages: msgs,
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;

    let history = conv_svc.load_conversation_history(&fx.context_id).await?;
    assert!(!history.is_empty());

    fx.cleanup().await?;
    Ok(())
}
