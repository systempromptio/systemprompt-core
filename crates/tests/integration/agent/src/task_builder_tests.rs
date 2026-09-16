use systemprompt_agent::models::a2a::{Message, MessageRole, Part, TextPart};
use systemprompt_agent::services::a2a_server::processing::task_builder::{
    build_canceled_task, build_completed_task,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::TaskState;

fn user_msg(ctx: &ContextId, task: &TaskId, text: &str) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart { text: text.into() })],
        message_id: MessageId::generate(),
        task_id: Some(task.clone()),
        context_id: ctx.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

#[test]
fn build_completed_task_has_completed_state() {
    let ctx = ContextId::generate();
    let task = TaskId::new("t1");
    let msg = user_msg(&ctx, &task, "hi");
    let t = build_completed_task(task.clone(), ctx.clone(), "done".into(), msg, vec![]);
    assert!(matches!(t.status.state, TaskState::Completed));
    assert_eq!(t.id, task);
    assert_eq!(t.context_id, ctx);
}

#[test]
fn build_canceled_task_has_canceled_state() {
    let task = TaskId::new("t-cancel");
    let ctx = ContextId::generate();
    let t = build_canceled_task(task.clone(), ctx.clone());
    assert!(matches!(t.status.state, TaskState::Canceled));
}
