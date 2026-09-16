//! Request-processing pipeline for the A2A server.
//!
//! Turns an inbound A2A [`Message`](crate::models::a2a::Message) into a
//! persisted [`Task`](crate::models::a2a::Task): validation,
//! conversation-history assembly, LLM execution (via [`ExecutionStrategy`]),
//! artifact construction, response synthesis, and persistence. The entry point
//! is [`MessageProcessor`]; [`StreamProcessor`] drives the streaming variant
//! and emits [`StreamEvent`]s.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod ai_executor;
pub mod artifact;
pub mod message;
pub mod strategies;
pub mod task_builder;

pub use artifact::ArtifactBuilder;
pub use message::{MessageProcessor, StreamEvent, StreamProcessor};
pub use strategies::{
    ExecutionContext, ExecutionResult, ExecutionStrategy, ExecutionStrategySelector,
};
pub use task_builder::{TaskBuilder, build_canceled_task, build_completed_task};
