//! A2A mappings for agent runs.
//!
//! A run started from an A2A task reports through the task: [`RunStatus`]
//! maps onto [`TaskState`] both ways ([`state`]), a [`RunOutcome`] becomes
//! the task's artifacts and an inbound [`Message`] the run's prompt
//! ([`content`]), and each persisted [`RunEvent`] becomes the
//! [`TaskUpdate`]s a streaming client receives ([`updates`]).
//!
//! [`RunStatus`]: crate::RunStatus
//! [`TaskState`]: systemprompt_models::a2a::TaskState
//! [`RunOutcome`]: crate::RunOutcome
//! [`Message`]: systemprompt_models::a2a::Message
//! [`RunEvent`]: crate::RunEvent
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod content;
pub mod state;
pub mod updates;

pub use content::{AGENT_RUN_ARTIFACT_SOURCE, AGENT_RUN_ARTIFACT_TYPE};
pub use updates::{TaskUpdate, task_events};
