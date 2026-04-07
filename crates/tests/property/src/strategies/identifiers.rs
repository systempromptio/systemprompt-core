use proptest::prelude::*;
use systemprompt_identifiers::{ArtifactId, ContextId, MessageId, TaskId};

pub fn arb_task_id() -> impl Strategy<Value = TaskId> {
    Just(()).prop_map(|_| TaskId::generate())
}

pub fn arb_context_id() -> impl Strategy<Value = ContextId> {
    Just(()).prop_map(|_| ContextId::generate())
}

pub fn arb_message_id() -> impl Strategy<Value = MessageId> {
    Just(()).prop_map(|_| MessageId::generate())
}

pub fn arb_artifact_id() -> impl Strategy<Value = ArtifactId> {
    Just(()).prop_map(|_| ArtifactId::generate())
}
