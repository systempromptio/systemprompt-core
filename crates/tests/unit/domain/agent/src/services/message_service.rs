// DB-backed tests for MessageService: transactional message persistence,
// synthetic tool-execution messages, and sequence-number allocation.

use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::message::{
    CreateToolExecutionMessageParams, MessageService, PersistMessagesParams,
};
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn text_msg(role: MessageRole, ctx: &ContextId, tid: &TaskId, text: &str) -> Message {
    Message {
        role,
        parts: vec![Part::Text(TextPart {
            text: text.to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(tid.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

fn request_context(ctx: &ContextId, session: &SessionId, user: &UserId) -> RequestContext {
    let mut rc = RequestContext::new(
        session.clone(),
        TraceId::generate(),
        ctx.clone(),
        AgentName::new("msg-agent"),
    );
    rc.auth.actor = Actor::user(user.clone());
    rc
}

#[tokio::test]
async fn persist_messages_empty_returns_empty() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = MessageService::new(&pool).expect("service");
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;
    let trace = TraceId::generate();

    let seqs = svc
        .persist_messages(PersistMessagesParams {
            task_id: &tid,
            context_id: &ctx,
            messages: vec![],
            user_id: Some(&user_id),
            session_id: &session_id,
            trace_id: &trace,
        })
        .await
        .expect("persist empty");
    assert!(seqs.is_empty());

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn persist_messages_assigns_increasing_sequence_numbers() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = MessageService::new(&pool).expect("service");
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;
    let trace = TraceId::generate();

    let messages = vec![
        text_msg(MessageRole::User, &ctx, &tid, "first"),
        text_msg(MessageRole::Agent, &ctx, &tid, "second"),
    ];
    let seqs = svc
        .persist_messages(PersistMessagesParams {
            task_id: &tid,
            context_id: &ctx,
            messages,
            user_id: Some(&user_id),
            session_id: &session_id,
            trace_id: &trace,
        })
        .await
        .expect("persist");
    assert_eq!(seqs.len(), 2);
    assert!(seqs[1] > seqs[0]);

    let read = r.tasks.get_messages_by_task(&tid).await.expect("messages");
    assert_eq!(read.len(), 2);

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn create_tool_execution_message_persists_synthetic_user_message() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = MessageService::new(&pool).expect("service");
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;
    let rc = request_context(&ctx, &session_id, &user_id);
    let args = serde_json::json!({"query": "select 1"});

    let (message_id, seq) = svc
        .create_tool_execution_message(CreateToolExecutionMessageParams {
            task_id: &tid,
            context_id: &ctx,
            tool_name: "sql-runner",
            tool_args: &args,
            request_context: &rc,
        })
        .await
        .expect("create synthetic");
    assert!(!message_id.is_empty());
    assert_eq!(seq, 0);

    let read = r.tasks.get_messages_by_task(&tid).await.expect("messages");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].role, MessageRole::User);
    let parts = &read[0].parts;
    match &parts[0] {
        Part::Text(t) => assert!(t.text.contains("sql-runner")),
        other => panic!("expected text part, got {other:?}"),
    }

    r.tasks.delete_task(&tid).await.ok();
}

#[tokio::test]
async fn persist_messages_then_next_sequence_continues() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let svc = MessageService::new(&pool).expect("service");
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let r = repos(&pool);
    let (ctx, tid) = seed_context_and_task(&r, &user_id, &session_id).await;
    let trace = TraceId::generate();

    svc.persist_messages(PersistMessagesParams {
        task_id: &tid,
        context_id: &ctx,
        messages: vec![text_msg(MessageRole::User, &ctx, &tid, "only")],
        user_id: Some(&user_id),
        session_id: &session_id,
        trace_id: &trace,
    })
    .await
    .expect("persist");

    let next = r
        .tasks
        .get_next_sequence_number(&tid)
        .await
        .expect("next seq");
    assert!(next >= 1);

    r.tasks.delete_task(&tid).await.ok();
}
