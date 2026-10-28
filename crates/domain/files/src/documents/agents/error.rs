//! [`AgentRefusal`]: why an agent was stopped where the user would not be.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::cap::{AgentCap, AgentCapability};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AgentRefusal {
    #[error(
        "agent {agent} may not {action} here: {reason}. Ask the user to do it, or to raise the \
         agent cap if they own the document"
    )]
    CapExceeded {
        agent: String,
        cap: AgentCap,
        action: String,
        reason: String,
    },
    #[error(
        "agents may not {capability} documents, whatever their access; ask the user to do it \
         themselves"
    )]
    HardRule {
        agent: String,
        capability: AgentCapability,
    },
    #[error(
        "agents may only suggest on this document: {reason}. Write the change with mode: \
         propose and a person will review it"
    )]
    SuggestOnly { agent: String, reason: String },
    #[error(
        "agent edits for this user are limited to {limit} per hour; retry in {retry_after_secs}s"
    )]
    RateLimited { limit: u64, retry_after_secs: u64 },
    #[error(
        "an agent cap override only tightens: {requested} is looser than the {policy_cap} the \
         policy allows; clear the override instead"
    )]
    OverrideLooser {
        requested: AgentCap,
        policy_cap: AgentCap,
    },
}
