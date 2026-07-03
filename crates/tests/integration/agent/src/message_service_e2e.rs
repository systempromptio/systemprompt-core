use anyhow::Result;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::{
    CreateToolExecutionMessageParams, MessageService, PersistMessagesParams,
};
use systemprompt_identifiers::{Actor, AgentName, ContextId, MessageId, TaskId};
use systemprompt_models::a2a::TaskState;
use systemprompt_models::execution::context::RequestContext;

use crate::common::Fixture;

fn make_message(text: &str, ctx: &ContextId, task_id: &TaskId) -> Message {
    Message {
        role: MessageRole::User,
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

fn request_context(fx: &Fixture) -> RequestContext {
    let mut ctx = RequestContext::new(
        fx.session_id.clone(),
        fx.trace_id.clone(),
        fx.context_id.clone(),
        AgentName::new("test-agent"),
    );
    ctx.auth.actor = Actor::user(fx.user_id.clone());
    ctx
}

#[tokio::test]
async fn message_service_new_succeeds() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let dbg = format!("{:?}", svc);
    assert!(dbg.contains("MessageService"));
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn persist_messages_empty_returns_empty_vec() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let seqs = svc
        .persist_messages(PersistMessagesParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            messages: vec![],
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;
    assert!(seqs.is_empty());
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn persist_messages_returns_sequential_numbers() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let msgs = vec![
        make_message("first", &fx.context_id, &task_id),
        make_message("second", &fx.context_id, &task_id),
        make_message("third", &fx.context_id, &task_id),
    ];
    let seqs = svc
        .persist_messages(PersistMessagesParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            messages: msgs,
            user_id: Some(&fx.user_id),
            session_id: &fx.session_id,
            trace_id: &fx.trace_id,
        })
        .await?;
    assert_eq!(seqs.len(), 3);
    assert!(seqs[0] < seqs[1]);
    assert!(seqs[1] < seqs[2]);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn create_tool_execution_message_returns_id_and_sequence() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let ctx = request_context(&fx);
    let args = serde_json::json!({"path": "/tmp/x", "n": 7});
    let (mid, seq) = svc
        .create_tool_execution_message(CreateToolExecutionMessageParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            tool_name: "write_file",
            tool_args: &args,
            request_context: &ctx,
        })
        .await?;
    assert!(!mid.is_empty());
    assert!(seq >= 0);
    fx.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn create_tool_execution_message_works_without_pretty_args() -> Result<()> {
    let fx = Fixture::new().await?;
    let svc = MessageService::new(&fx.db)?;
    let task_id = fx.insert_task(TaskState::Working).await?;
    let ctx = request_context(&fx);
    let args = serde_json::json!(null);
    let (mid, seq) = svc
        .create_tool_execution_message(CreateToolExecutionMessageParams {
            task_id: &task_id,
            context_id: &fx.context_id,
            tool_name: "noop",
            tool_args: &args,
            request_context: &ctx,
        })
        .await?;
    assert!(!mid.is_empty(), "null tool_args must still yield a message id");
    assert!(seq >= 0);
    fx.cleanup().await?;
    Ok(())
}
