//! Request-processing pipeline for the A2A server.
//!
//! Turns an inbound A2A [`Message`](crate::models::a2a::Message) into a
//! persisted [`Task`](crate::models::a2a::Task): validation,
//! conversation-history assembly, LLM execution (via [`ExecutionStrategy`]),
//! artifact construction, response synthesis, and persistence. The entry point
//! is [`MessageProcessor`]; [`StreamProcessor`] drives the streaming variant
//! and emits [`StreamEvent`]s.

pub mod ai_executor;
pub mod artifact;
pub mod conversation_service;
pub mod message;
pub mod message_validation;
pub mod persistence_service;
pub mod strategies;
pub mod task_builder;

pub use artifact::ArtifactBuilder;
pub use conversation_service::ConversationService;
pub use message::{MessageProcessor, StreamEvent, StreamProcessor};
pub use message_validation::{MessageValidationService, ValidatedMessageRequest};
pub use persistence_service::PersistenceService;
pub use strategies::{
    ExecutionContext, ExecutionResult, ExecutionStrategy, ExecutionStrategySelector,
};
pub use task_builder::{
    TaskBuilder, build_canceled_task, build_completed_task, build_mock_task, build_multiturn_task,
};
