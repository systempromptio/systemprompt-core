use super::{make_task, repos, seed_context_and_task, seed_user_and_session};
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState};
use systemprompt_agent::repository::task::UpdateTaskAndSaveMessagesParams;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContextId, MessageId, TaskId, TraceId, UserId};
use systemprompt_test_fixtures::test_db_pool;

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
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let user_msg = msg_with_parts(
        MessageRole::User,
        &context_id,
        &task_id,
        vec![Part::text("question")],
    );
    let agent_msg = msg_with_parts(
        MessageRole::Agent,
        &context_id,
        &task_id,
        vec![Part::text("answer")],
    );
    save_pair(
        &pool,
        &user_id,
        &session_id,
        &context_id,
        &task_id,
        &user_msg,
        &agent_msg,
    )
    .await;

    let parts = r
        .tasks
        .get_message_parts(&user_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(parts.len(), 1);
    let part = &parts[0];
    assert_eq!(part.as_text(), Some("question"), "{part:?}");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn file_part_roundtrip() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let file_part = Part::raw("aGVsbG8=", "text/plain").with_filename("report.txt");
    let user_msg = msg_with_parts(MessageRole::User, &context_id, &task_id, vec![file_part]);
    let agent_msg = msg_with_parts(
        MessageRole::Agent,
        &context_id,
        &task_id,
        vec![Part::text("ok")],
    );
    save_pair(
        &pool,
        &user_id,
        &session_id,
        &context_id,
        &task_id,
        &user_msg,
        &agent_msg,
    )
    .await;

    let parts = r
        .tasks
        .get_message_parts(&user_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(parts.len(), 1);
    let part = &parts[0];
    assert_eq!(part.filename.as_deref(), Some("report.txt"));
    assert_eq!(part.media_type.as_deref(), Some("text/plain"));
    assert_eq!(part.as_raw(), Some("aGVsbG8="));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn data_part_roundtrip() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let mut map = serde_json::Map::new();
    map.insert("answer".to_owned(), serde_json::json!(42));
    map.insert("ok".to_owned(), serde_json::json!(true));
    let data_part = Part::data(serde_json::Value::Object(map));

    let user_msg = msg_with_parts(
        MessageRole::User,
        &context_id,
        &task_id,
        vec![Part::text("compute")],
    );
    let agent_msg = msg_with_parts(MessageRole::Agent, &context_id, &task_id, vec![data_part]);
    save_pair(
        &pool,
        &user_id,
        &session_id,
        &context_id,
        &task_id,
        &user_msg,
        &agent_msg,
    )
    .await;

    let parts = r
        .tasks
        .get_message_parts(&agent_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(parts.len(), 1);
    let part = &parts[0];
    let data = part.as_data().expect("data part");
    assert_eq!(data.get("answer"), Some(&serde_json::json!(42)));
    assert_eq!(data.get("ok"), Some(&serde_json::json!(true)));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn mixed_parts_preserve_order() {
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let mut map = serde_json::Map::new();
    map.insert("k".to_owned(), serde_json::json!("v"));
    let parts = vec![
        Part::text("first"),
        Part::raw("ZGF0YQ==", "text/plain").with_filename("f"),
        Part::data(serde_json::Value::Object(map)),
    ];
    let agent_msg = msg_with_parts(MessageRole::Agent, &context_id, &task_id, parts);
    let user_msg = msg_with_parts(
        MessageRole::User,
        &context_id,
        &task_id,
        vec![Part::text("u")],
    );
    save_pair(
        &pool,
        &user_id,
        &session_id,
        &context_id,
        &task_id,
        &user_msg,
        &agent_msg,
    )
    .await;

    let read = r
        .tasks
        .get_message_parts(&agent_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(read.len(), 3);
    assert!(read[0].as_text().is_some());
    assert!(read[1].is_file());
    assert!(read[2].as_data().is_some());

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_messages_by_task_empty() {
    let pool = test_db_pool().await;
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
    let pool = test_db_pool().await;
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
    let pool = test_db_pool().await;
    let r = repos(&pool);
    let parts = r
        .tasks
        .get_message_parts(&MessageId::generate())
        .await
        .expect("empty");
    assert!(parts.is_empty());
}
