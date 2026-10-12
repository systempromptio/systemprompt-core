//! A2A streaming event wire types, re-exported from `systemprompt_models`
//! so clients and the agent server share one definition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_models::a2a::{
    SendMessageResult, StreamResponse, TaskArtifactUpdateEvent, TaskStatusUpdateEvent,
};
