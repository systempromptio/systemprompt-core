//! Persistence for AI request records and their conversation turns.
//!
//! [`AiRequestRepository`] owns the `ai_requests`, `ai_request_messages`, and
//! `ai_request_tool_calls` tables. Inserts and status updates live in the
//! mutations submodule, read paths in queries, and per-turn message/tool-call
//! writes in message operations, and the transactional terminal settlement
//! (completion or failure under an owner check) in the settlement submodule.
//! [`InsertToolCallParams`] and [`SettlementOutcome`] are the grouped
//! argument types for the wider write methods. The repository also implements
//! `systemprompt_traits::AiRequestTrace`, the read seam other domains use
//! instead of querying these tables.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod message_operations;
mod mutations;
mod orphans;
mod queries;
mod repository;
mod settlement;
mod trace;

pub use message_operations::InsertToolCallParams;
pub use orphans::{ORPHAN_AGE, ORPHANED_REASON, OrphanedRequest};
pub use repository::AiRequestRepository;
pub use settlement::{
    SettleCompletion, SettledFailure, SettledToolCall, SettlementOutcome, SettlementUsage,
};
