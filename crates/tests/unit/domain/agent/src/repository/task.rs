use super::{make_task, repos, seed_context_and_task, seed_user_and_session, try_pool};
use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TaskState, TextPart};
use systemprompt_agent::repository::task::{
    RepoCreateTaskParams, UpdateTaskAndSaveMessagesParams, task_state_to_db_string,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId, TraceId, UserId};

#[test]
fn task_state_to_db_string_all_variants() {
    assert_eq!(
        task_state_to_db_string(TaskState::Pending),
        "TASK_STATE_PENDING"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::Submitted),
        "TASK_STATE_SUBMITTED"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::Working),
        "TASK_STATE_WORKING"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::InputRequired),
        "TASK_STATE_INPUT_REQUIRED"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::Completed),
        "TASK_STATE_COMPLETED"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::Canceled),
        "TASK_STATE_CANCELED"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::Failed),
        "TASK_STATE_FAILED"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::Rejected),
        "TASK_STATE_REJECTED"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::AuthRequired),
        "TASK_STATE_AUTH_REQUIRED"
    );
    assert_eq!(
        task_state_to_db_string(TaskState::Unknown),
        "TASK_STATE_UNKNOWN"
    );
}

fn make_message(role: MessageRole, context_id: &ContextId, task_id: &TaskId, text: &str) -> Message {
    Message {
        role,
        parts: vec![Part::Text(TextPart {
            text: text.to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

#[tokio::test]
async fn create_and_get_task_roundtrip() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let task = r
        .tasks
        .get_task(&task_id)
        .await
        .expect("get")
        .expect("present");
    assert_eq!(task.id, task_id);
    assert_eq!(task.context_id, context_id);
    assert_eq!(task.status.state, TaskState::Submitted);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_task_unknown_returns_none() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let result = r.tasks.get_task(&TaskId::generate()).await.expect("get");
    assert!(result.is_none());
}

#[tokio::test]
async fn list_tasks_by_context_and_by_user() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let by_ctx = r
        .tasks
        .list_tasks_by_context(&context_id)
        .await
        .expect("by ctx");
    assert!(by_ctx.iter().any(|t| t.id == task_id));

    let by_user = r
        .tasks
        .get_tasks_by_user_id(&user_id, Some(10), Some(0))
        .await
        .expect("by user");
    assert!(by_user.iter().any(|t| t.id == task_id));

    let by_user_default = r
        .tasks
        .get_tasks_by_user_id(&user_id, None, None)
        .await
        .expect("by user default");
    assert!(by_user_default.iter().any(|t| t.id == task_id));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn get_task_context_info() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    let info = r
        .tasks
        .get_task_context_info(&task_id)
        .await
        .expect("info")
        .expect("present");
    assert_eq!(info.context_id, context_id);
    assert_eq!(info.user_id, Some(user_id));

    let none = r
        .tasks
        .get_task_context_info(&TaskId::generate())
        .await
        .expect("info");
    assert!(none.is_none());

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn validate_task_ownership() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    r.tasks
        .validate_task_ownership(&task_id, &user_id)
        .await
        .expect("owned");

    let err = r
        .tasks
        .validate_task_ownership(&task_id, &UserId::new("intruder"))
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound(_)
    ));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn update_task_state_valid_transition() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let now = chrono::Utc::now();

    // Submitted -> Working -> Completed
    r.tasks
        .update_task_state(&task_id, TaskState::Working, &now)
        .await
        .expect("to working");
    let task = r.tasks.get_task(&task_id).await.expect("get").unwrap();
    assert_eq!(task.status.state, TaskState::Working);

    r.tasks
        .update_task_state(&task_id, TaskState::Completed, &now)
        .await
        .expect("to completed");
    let task = r.tasks.get_task(&task_id).await.expect("get").unwrap();
    assert_eq!(task.status.state, TaskState::Completed);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn update_task_state_idempotent_same_state() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let now = chrono::Utc::now();

    // Already Submitted; setting Submitted again is a no-op Ok.
    r.tasks
        .update_task_state(&task_id, TaskState::Submitted, &now)
        .await
        .expect("noop");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn update_task_state_invalid_transition_errors() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let now = chrono::Utc::now();

    r.tasks
        .update_task_state(&task_id, TaskState::Completed, &now)
        .await
        .expect("submitted -> completed ok");

    // Completed is terminal; any further transition is invalid.
    let err = r
        .tasks
        .update_task_state(&task_id, TaskState::Working, &now)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::ConstraintViolation(_)
    ));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn update_task_state_unknown_task_not_found() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let now = chrono::Utc::now();
    let err = r
        .tasks
        .update_task_state(&TaskId::generate(), TaskState::Working, &now)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound(_)
    ));
}

#[tokio::test]
async fn apply_notification_status_parses_state() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let now = chrono::Utc::now();

    r.tasks
        .apply_notification_status(&task_id, "working", &now)
        .await
        .expect("notify working");
    let task = r.tasks.get_task(&task_id).await.expect("get").unwrap();
    assert_eq!(task.status.state, TaskState::Working);

    let err = r
        .tasks
        .apply_notification_status(&task_id, "not-a-state", &now)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::InvalidData(_)
    ));

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn update_task_failed_with_error() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let now = chrono::Utc::now();

    r.tasks
        .update_task_failed_with_error(&task_id, "boom", &now)
        .await
        .expect("fail");
    let task = r.tasks.get_task(&task_id).await.expect("get").unwrap();
    assert_eq!(task.status.state, TaskState::Failed);

    // Failing again is idempotent.
    r.tasks
        .update_task_failed_with_error(&task_id, "boom2", &now)
        .await
        .expect("fail idempotent");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn update_task_failed_unknown_is_not_found() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let now = chrono::Utc::now();
    let err = r
        .tasks
        .update_task_failed_with_error(&TaskId::generate(), "x", &now)
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        systemprompt_traits::RepositoryError::NotFound(_)
    ));
}

#[tokio::test]
async fn track_agent_in_context_is_idempotent() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;

    r.tasks
        .track_agent_in_context(&context_id, "agent-x")
        .await
        .expect("track");
    r.tasks
        .track_agent_in_context(&context_id, "agent-x")
        .await
        .expect("track again");

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn update_task_and_save_messages_persists_history() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let trace_id = TraceId::generate();

    let mut task = make_task(&task_id, &context_id);
    task.status.state = TaskState::Completed;

    let user_msg = make_message(MessageRole::User, &context_id, &task_id, "hi");
    let agent_msg = make_message(MessageRole::Agent, &context_id, &task_id, "hello");

    let updated = r
        .tasks
        .update_task_and_save_messages(UpdateTaskAndSaveMessagesParams {
            task: &task,
            user_message: &user_msg,
            agent_message: &agent_msg,
            user_id: Some(&user_id),
            session_id: &session_id,
            trace_id: &trace_id,
        })
        .await
        .expect("update + save");
    assert_eq!(updated.status.state, TaskState::Completed);

    let messages = r
        .tasks
        .get_messages_by_task(&task_id)
        .await
        .expect("messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[1].role, MessageRole::Agent);

    let by_ctx = r
        .tasks
        .get_messages_by_context(&context_id)
        .await
        .expect("by ctx");
    assert_eq!(by_ctx.len(), 2);

    assert!(r.tasks.message_exists(&user_msg.message_id).await.unwrap());
    let next_seq = r
        .tasks
        .get_next_sequence_number(&task_id)
        .await
        .expect("seq");
    assert!(next_seq >= 2);

    let parts = r
        .tasks
        .get_message_parts(&user_msg.message_id)
        .await
        .expect("parts");
    assert_eq!(parts.len(), 1);

    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn message_exists_false_for_unknown() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    assert!(
        !r.tasks
            .message_exists(&MessageId::generate())
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn next_sequence_number_starts_at_zero() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let (_context_id, task_id) = seed_context_and_task(&r, &user_id, &session_id).await;
    let seq = r
        .tasks
        .get_next_sequence_number(&task_id)
        .await
        .expect("seq");
    assert_eq!(seq, 0);
    r.tasks.delete_task(&task_id).await.ok();
}

#[tokio::test]
async fn create_task_returns_id_string() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let r = repos(&pool);
    let (user_id, session_id) = seed_user_and_session(&pool).await;
    let ctx_repo = systemprompt_agent::repository::ContextRepository::new(r.db_pool()).unwrap();
    let context_id = ctx_repo
        .create_context(&user_id, Some(&session_id), "c")
        .await
        .unwrap();
    let task_id = TaskId::generate();
    let trace_id = TraceId::generate();
    let task = make_task(&task_id, &context_id);
    let returned = r
        .tasks
        .create_task(RepoCreateTaskParams {
            task: &task,
            user_id: &user_id,
            session_id: &session_id,
            trace_id: &trace_id,
            agent_name: "a",
        })
        .await
        .expect("create");
    assert_eq!(returned, task_id.to_string());
    r.tasks.delete_task(&task_id).await.ok();
}
