use anyhow::Result;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::repository::context::message::MessageRepository;
use systemprompt_agent::services::{MessageService, PersistMessagesParams};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::TaskState;
use uuid::Uuid;

use crate::common::Fixture;

fn make_msg(text: &str, ctx: &ContextId, task_id: &TaskId, role: MessageRole) -> Message {
    Message {
        role,
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: ctx.clone(),
        parts: vec![Part::Text(TextPart {
            text: text.to_string(),
        })],
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

#[tokio::test]
async fn message_repository_new_succeeds() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = MessageRepository::new(&fx.db)?;
    let dbg = format!("{:?}", repo);
    assert!(dbg.contains("MessageRepository"));
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_repository_get_by_task_returns_persisted() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let repo = MessageRepository::new(&fx.db)?;
    let task_id = fx.insert_task(TaskState::Working).await?;

    let msgs = vec![
        make_msg("hello", &fx.context_id, &task_id, MessageRole::User),
        make_msg("hi there", &fx.context_id, &task_id, MessageRole::Agent),
    ];
    svc.persist_messages(PersistMessagesParams {
        task_id: &task_id,
        context_id: &fx.context_id,
        messages: msgs,
        user_id: Some(&fx.user_id),
        session_id: &fx.session_id,
        trace_id: &fx.trace_id,
    })
    .await?;

    let fetched = repo.get_messages_by_task(&task_id).await?;
    assert_eq!(fetched.len(), 2);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_repository_get_by_context_aggregates_tasks() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let repo = MessageRepository::new(&fx.db)?;
    let t1 = fx.insert_task(TaskState::Working).await?;
    let t2 = fx.insert_task(TaskState::Working).await?;

    svc.persist_messages(PersistMessagesParams {
        task_id: &t1,
        context_id: &fx.context_id,
        messages: vec![make_msg("m1", &fx.context_id, &t1, MessageRole::User)],
        user_id: Some(&fx.user_id),
        session_id: &fx.session_id,
        trace_id: &fx.trace_id,
    })
    .await?;
    svc.persist_messages(PersistMessagesParams {
        task_id: &t2,
        context_id: &fx.context_id,
        messages: vec![make_msg("m2", &fx.context_id, &t2, MessageRole::Agent)],
        user_id: Some(&fx.user_id),
        session_id: &fx.session_id,
        trace_id: &fx.trace_id,
    })
    .await?;

    let by_ctx = repo.get_messages_by_context(&fx.context_id).await?;
    assert!(by_ctx.len() >= 2);

    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_repository_next_sequence_advances() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let repo = MessageRepository::new(&fx.db)?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let before = repo.get_next_sequence_number(&task_id).await?;
    svc.persist_messages(PersistMessagesParams {
        task_id: &task_id,
        context_id: &fx.context_id,
        messages: vec![make_msg("a", &fx.context_id, &task_id, MessageRole::User)],
        user_id: Some(&fx.user_id),
        session_id: &fx.session_id,
        trace_id: &fx.trace_id,
    })
    .await?;
    let after = repo.get_next_sequence_number(&task_id).await?;
    assert!(after > before);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_repository_get_by_unknown_task_empty() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = MessageRepository::new(&fx.db)?;
    let unknown = TaskId::new("__no_task_for_messages_zz");
    let msgs = repo.get_messages_by_task(&unknown).await?;
    assert!(msgs.is_empty());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn message_repository_get_by_unknown_context_empty() -> Result<()> {
    let fx = Fixture::new().await?;
    let repo = MessageRepository::new(&fx.db)?;
    let unknown = ContextId::new(Uuid::new_v4().to_string());
    let msgs = repo.get_messages_by_context(&unknown).await?;
    assert!(msgs.is_empty());
    fx.cleanup().await?;
    Ok(())
}
