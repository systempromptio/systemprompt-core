//! Persistence for AI request records and their conversation turns.
//!
//! [`AiRequestRepository`] owns the `ai_requests`, `ai_request_messages`, and
//! `ai_request_tool_calls` tables. Inserts and status updates live in the
//! mutations submodule, read paths in queries, and per-turn message/tool-call
//! writes in message operations. [`UpdateCompletionParams`] and
//! [`InsertToolCallParams`] are the grouped argument structs for the wider
//! write methods.

mod message_operations;
mod mutations;
mod queries;
mod repository;

pub use message_operations::InsertToolCallParams;
pub use mutations::UpdateCompletionParams;
pub use repository::AiRequestRepository;
