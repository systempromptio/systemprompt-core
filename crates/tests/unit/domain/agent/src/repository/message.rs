use super::{make_task, repos, seed_context_and_task, seed_user_and_session, try_pool};
use systemprompt_agent::models::a2a::{
    DataPart, FileContent, FilePart, Message, MessageRole, Part, TaskState, TextPart,
};
use systemprompt_agent::repository::task::UpdateTaskAndSaveMessagesParams;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, MessageId, TaskId, TraceId, UserId};

fn msg_with_parts(
    role: MessageRole,
    context_id: &ContextId,
    task_id: &TaskId,
    parts: Vec<Part>,
) -> Message {
    Message {
        role,
        parts,
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

// Persists a user/agent pair through the high-level repo path so the message
// parts hit the real parts persister + parser on read-back.
async fn save_pair(
    pool: &DbPool,
    user_id: &UserId,
    session_id: &systemprompt_identifiers::SessionId,
    context_id: &ContextId,
    task_id: &TaskId,
    user_msg: &Message,
    agent_msg: &Message,
) {
    let r = repos(pool);
    let mut task = make_task(task_id, context_id);
    task.status.state = TaskState::Completed;
    let trace_id = TraceId::generate();
    r.tasks
        .update_task_and_save_messages(UpdateTaskAndSaveMessagesParams {
            task: &task,
            user_message: user_msg,
            agent_message: agent_msg,
            user_id: Some(user_id),
            session_id,
            trace_id: &trace_id,
        })
        .await
        .expect("save pair");
}

#[tokio::test]
async fn text_part_roundtrip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let user_msg = msg_with_parts(
        MessageRole::User,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "question".to_owned(),
        })],
    );
    let agent_msg = msg_with_parts(
        MessageRole::Agent,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "answer".to_owned(),
        })],
    );
    save_pair(
        &pool, &user_id, &session_id, &context_id, &task_id, &user_msg, &agent_msg,
    )
    .await;

    let parts = r
        .tasks
        .get_message_parts(&user_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        Part::Text(t) => assert_eq!(t.text, "question"),
        other => panic!("expected text, got {other:?}"),
    }

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn file_part_roundtrip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let file_part = Part::File(FilePart {
        file: FileContent {
            name: Some("report.txt".to_owned()),
            mime_type: Some("text/plain".to_owned()),
            bytes: Some("aGVsbG8=".to_owned()),
            url: None,
        },
    });
    let user_msg = msg_with_parts(MessageRole::User, &context_id, &task_id, vec![file_part]);
    let agent_msg = msg_with_parts(
        MessageRole::Agent,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "ok".to_owned(),
        })],
    );
    save_pair(
        &pool, &user_id, &session_id, &context_id, &task_id, &user_msg, &agent_msg,
    )
    .await;

    let parts = r
        .tasks
        .get_message_parts(&user_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        Part::File(f) => {
            assert_eq!(f.file.name.as_deref(), Some("report.txt"));
            assert_eq!(f.file.mime_type.as_deref(), Some("text/plain"));
            assert_eq!(f.file.bytes.as_deref(), Some("aGVsbG8="));
        },
        other => panic!("expected file, got {other:?}"),
    }

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn data_part_roundtrip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let mut map = serde_json::Map::new();
    map.insert("answer".to_owned(), serde_json::json!(42));
    map.insert("ok".to_owned(), serde_json::json!(true));
    let data_part = Part::Data(DataPart { data: map });

    let user_msg = msg_with_parts(
        MessageRole::User,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "compute".to_owned(),
        })],
    );
    let agent_msg = msg_with_parts(MessageRole::Agent, &context_id, &task_id, vec![data_part]);
    save_pair(
        &pool, &user_id, &session_id, &context_id, &task_id, &user_msg, &agent_msg,
    )
    .await;

    let parts = r
        .tasks
        .get_message_parts(&agent_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(parts.len(), 1);
    match &parts[0] {
        Part::Data(d) => {
            assert_eq!(d.data.get("answer"), Some(&serde_json::json!(42)));
            assert_eq!(d.data.get("ok"), Some(&serde_json::json!(true)));
        },
        other => panic!("expected data, got {other:?}"),
    }

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn mixed_parts_preserve_order() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let mut map = serde_json::Map::new();
    map.insert("k".to_owned(), serde_json::json!("v"));
    let parts = vec![
        Part::Text(TextPart {
            text: "first".to_owned(),
        }),
        Part::File(FilePart {
            file: FileContent {
                name: Some("f".to_owned()),
                mime_type: Some("text/plain".to_owned()),
                bytes: Some("ZGF0YQ==".to_owned()),
                url: None,
            },
        }),
        Part::Data(DataPart { data: map }),
    ];
    let agent_msg = msg_with_parts(MessageRole::Agent, &context_id, &task_id, parts);
    let user_msg = msg_with_parts(
        MessageRole::User,
        &context_id,
        &task_id,
        vec![Part::Text(TextPart {
            text: "u".to_owned(),
        })],
    );
    save_pair(
        &pool, &user_id, &session_id, &context_id, &task_id, &user_msg, &agent_msg,
    )
    .await;

    let read = r
        .tasks
        .get_message_parts(&agent_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(read.len(), 3);
    assert!(matches!(read[0], Part::Text(_)));
    assert!(matches!(read[1], Part::File(_)));
    assert!(matches!(read[2], Part::Data(_)));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_messages_by_task_empty() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let msgs = r
        .tasks
        .get_messages_by_task(&TaskId::generate())
        .await
        .expect("empty");
    assert!(msgs.is_empty());
}

#[tokio::test]
async fn get_messages_by_context_empty() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let msgs = r
        .tasks
        .get_messages_by_context(&ContextId::generate())
        .await
        .expect("empty");
    assert!(msgs.is_empty());
}

#[tokio::test]
async fn get_message_parts_unknown_message_empty() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let parts = r
        .tasks
        .get_message_parts(&MessageId::generate())
        .await
        .expect("empty");
    assert!(parts.is_empty());
}
