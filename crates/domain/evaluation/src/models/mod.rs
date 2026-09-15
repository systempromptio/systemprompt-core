//! Data model for golden cases captured from the AI request trace.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod case;
mod status;

pub use case::{CanonicalPrompt, EvalCase, NewCaseParams};
pub use status::{AccountingStatus, ApprovalStatus, CampaignStatus};
