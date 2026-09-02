// Two replicas appending to one task at the same time must not collide on
// UNIQUE(task_id, sequence_number): the task row is locked inside the
// transaction, so the second writer waits and takes the next number.

use serde_json::json;
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::services::message::{MessageService, PersistMessageInTxParams};
use systemprompt_database::DatabaseProvider;
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn text_message(ctx: &ContextId, tid: &TaskId, text: &str) -> Message {
    Message {
        role: MessageRole::Agent,
        parts: vec![Part::Text(TextPart {
            text: text.to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(tid.clone()),
        context_id: ctx.clone(),
        metadata: Some(json!({})),
        extensions: None,
        reference_task_ids: None,
    }
}

async fn append_in_tx(
    pool: &systemprompt_database::DbPool,
    message: Message,
    task_id: TaskId,
    context_id: ContextId,
    user_id: UserId,
    session_id: SessionId,
) -> i32 {
    let svc = MessageService::new(
        TaskRepository::new(pool, crate::session_usage(pool)).expect("task repo"),
    );
    let mut tx = pool
        .as_ref()
        .begin_transaction()
        .await
        .expect("begin transaction");
    let seq = svc
        .persist_message_in_tx(PersistMessageInTxParams {
            tx: &mut *tx,
            message: &message,
            task_id: &task_id,
            context_id: &context_id,
            user_id: Some(&user_id),
            session_id: &session_id,
            trace_id: &TraceId::generate(),
        })
        .await
        .expect("persist in tx");
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    tx.commit().await.expect("commit");
    seq
}

#[tokio::test]
async fn concurrent_appends_take_distinct_sequence_numbers() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let mut handles = Vec::new();
    for i in 0..4 {
        let pool = pool.clone();
        let message = text_message(&context_id, &task_id, &format!("m{i}"));
        let (task_id, context_id, user_id, session_id) = (
            task_id.clone(),
            context_id.clone(),
            user_id.clone(),
            session_id.clone(),
        );
        handles.push(tokio::spawn(async move {
            append_in_tx(&pool, message, task_id, context_id, user_id, session_id).await
        }));
    }
    let mut seqs: Vec<i32> = Vec::new();
    for handle in handles {
        seqs.push(handle.await.expect("append task"));
    }
    seqs.sort_unstable();
    assert_eq!(
        seqs,
        vec![0, 1, 2, 3],
        "every append committed with its own number"
    );

    let stored = r
        .tasks
        .get_messages_by_task(&task_id)
        .await
        .expect("messages by task");
    assert_eq!(stored.len(), 4);
    r.tasks.delete_task(&task_id).await.ok();
}
