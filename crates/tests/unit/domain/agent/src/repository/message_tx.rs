// DB-backed tests for the dyn-transaction message persistence path
// (persist_message_with_tx / get_next_sequence_number_in_tx via
// MessageService::persist_message_in_tx): replace-on-repersist semantics,
// file/data part arms, clientMessageId extraction, and reference_task_ids.

use serde_json::json;
use systemprompt_agent::models::a2a::{
    DataPart, FileContent, FilePart, Message, MessageRole, Part, TextPart,
};
use systemprompt_agent::services::message::{MessageService, PersistMessageInTxParams};
use systemprompt_database::DatabaseProvider;
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool};

fn rich_message(ctx: &ContextId, tid: &TaskId, message_id: &MessageId) -> Message {
    let serde_json::Value::Object(data) = json!({"k": "v"}) else {
        unreachable!()
    };
    Message {
        role: MessageRole::Agent,
        parts: vec![
            Part::Text(TextPart {
                text: "tx-text".to_owned(),
            }),
            Part::File(FilePart {
                file: FileContent {
                    name: Some("f.bin".to_owned()),
                    mime_type: Some("application/octet-stream".to_owned()),
                    bytes: Some("Zm9v".to_owned()),
                    url: None,
                },
            }),
            Part::Data(DataPart { data }),
        ],
        message_id: message_id.clone(),
        task_id: Some(tid.clone()),
        context_id: ctx.clone(),
        metadata: Some(json!({"clientMessageId": "client-42", "origin": "test"})),
        extensions: None,
        reference_task_ids: Some(vec![TaskId::new("ref-task-1"), TaskId::new("ref-task-2")]),
    }
}

async fn persist_in_tx(
    svc: &MessageService,
    pool: &systemprompt_database::DbPool,
    message: &Message,
    task_id: &TaskId,
    context_id: &ContextId,
    user_id: &UserId,
    session_id: &SessionId,
    trace_id: &TraceId,
) -> i32 {
    let mut tx = pool
        .as_ref()
        .begin_transaction()
        .await
        .expect("begin transaction");
    let seq = svc
        .persist_message_in_tx(PersistMessageInTxParams {
            tx: &mut *tx,
            message,
            task_id,
            context_id,
            user_id: Some(user_id),
            session_id,
            trace_id,
        })
        .await
        .expect("persist in tx");
    tx.commit().await.expect("commit");
    seq
}

#[tokio::test]
async fn persist_in_tx_round_trips_all_part_kinds_and_metadata() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let svc = MessageService::new(&pool).expect("message service");
    let trace_id = TraceId::generate();
    let message_id = MessageId::generate();
    let message = rich_message(&context_id, &task_id, &message_id);

    let seq = persist_in_tx(
        &svc,
        &pool,
        &message,
        &task_id,
        &context_id,
        &user_id,
        &session_id,
        &trace_id,
    )
    .await;
    assert_eq!(seq, 0);

    assert!(r.tasks.message_exists(&message_id).await.expect("exists"));

    let parts = r.tasks.get_message_parts(&message_id).await.expect("parts");
    assert_eq!(parts.len(), 3);
    match &parts[0] {
        Part::Text(t) => assert_eq!(t.text, "tx-text"),
        other => panic!("expected text part, got {other:?}"),
    }
    match &parts[1] {
        Part::File(f) => {
            assert_eq!(f.file.name.as_deref(), Some("f.bin"));
            assert_eq!(f.file.bytes.as_deref(), Some("Zm9v"));
        },
        other => panic!("expected file part, got {other:?}"),
    }
    match &parts[2] {
        Part::Data(d) => assert_eq!(d.data.get("k"), Some(&json!("v"))),
        other => panic!("expected data part, got {other:?}"),
    }

    let messages = r
        .tasks
        .get_messages_by_task(&task_id)
        .await
        .expect("messages");
    let stored = messages
        .iter()
        .find(|m| m.message_id == message_id)
        .expect("stored message");
    assert_eq!(stored.role, MessageRole::Agent);
    let refs = stored.reference_task_ids.as_ref().expect("reference ids");
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0].as_str(), "ref-task-1");
    let meta = stored.metadata.as_ref().expect("metadata");
    assert_eq!(meta.get("clientMessageId"), Some(&json!("client-42")));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn repersisting_same_message_id_replaces_instead_of_duplicating() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let svc = MessageService::new(&pool).expect("message service");
    let trace_id = TraceId::generate();
    let message_id = MessageId::generate();

    let mut message = rich_message(&context_id, &task_id, &message_id);
    persist_in_tx(
        &svc,
        &pool,
        &message,
        &task_id,
        &context_id,
        &user_id,
        &session_id,
        &trace_id,
    )
    .await;

    message.parts = vec![Part::Text(TextPart {
        text: "rewritten".to_owned(),
    })];
    let seq2 = persist_in_tx(
        &svc,
        &pool,
        &message,
        &task_id,
        &context_id,
        &user_id,
        &session_id,
        &trace_id,
    )
    .await;
    assert_eq!(seq2, 1);

    let messages = r
        .tasks
        .get_messages_by_task(&task_id)
        .await
        .expect("messages");
    let stored: Vec<_> = messages
        .iter()
        .filter(|m| m.message_id == message_id)
        .collect();
    assert_eq!(stored.len(), 1);

    let parts = r.tasks.get_message_parts(&message_id).await.expect("parts");
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        Part::Text(t) => assert_eq!(t.text, "rewritten"),
        other => panic!("expected replaced text part, got {other:?}"),
    }

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn sequence_numbers_in_tx_increase_per_task() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let svc = MessageService::new(&pool).expect("message service");
    let trace_id = TraceId::generate();

    for expected in 0..=2 {
        let message = rich_message(&context_id, &task_id, &MessageId::generate());
        let seq = persist_in_tx(
            &svc,
            &pool,
            &message,
            &task_id,
            &context_id,
            &user_id,
            &session_id,
            &trace_id,
        )
        .await;
        assert_eq!(seq, expected);
    }

    let next = r
        .tasks
        .get_next_sequence_number(&task_id)
        .await
        .expect("next sequence");
    assert_eq!(next, 3);

    r.tasks.delete_task(&task_id).await.ok();
}
