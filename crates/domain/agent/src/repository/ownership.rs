//! [`OwnerReassignment`](systemprompt_traits::OwnerReassignment) for the agent
//! domain: moves every context, task and message row from one user to another
//! inside a single transaction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

systemprompt_database::owner_reassignment! {
    pub struct AgentOwnerReassignment;
    domain = "agent";
    update "user_contexts" => "UPDATE user_contexts SET user_id = $2, updated_at = CURRENT_TIMESTAMP WHERE user_id = $1";
    update "agent_tasks" => "UPDATE agent_tasks SET user_id = $2, updated_at = CURRENT_TIMESTAMP WHERE user_id = $1";
    update "task_messages" => "UPDATE task_messages SET user_id = $2 WHERE user_id = $1";
}
